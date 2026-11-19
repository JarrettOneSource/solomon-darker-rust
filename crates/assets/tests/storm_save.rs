use serde_json::{Value, json};
use solomon_assets::boneyard::BoneyardCatalog;
use solomon_assets::website_save::{self, CreateSaveOptions, SaveIntegrity, SaveMetadata};
use solomon_sim::Vec2;
use solomon_sim::modding::LuaObject;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::secondary::storm::StormBurst;
use solomon_sim::secondary::{
    SecondaryActorKind, SecondaryActorState, enroll_secondary_light_owners,
    enroll_secondary_painter_owners,
};
use solomon_sim::session::{GameSession, GameSessionOptions};

#[test]
fn pending_storm_roundtrips_and_pre_53_migration_preserves_gameplay_rng() {
    let loaded = BoneyardCatalog::load_embedded().unwrap().materialize([45; 16]);
    let mut session = GameSession::new(
        "local".into(),
        NewGameSelection {
            name: "Storm continuation".into(),
            element: WizardElement::Air,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    session.install_boneyard(loaded.clone()).unwrap();
    let mut cloud = SecondaryActorState::new(
        0,
        SecondaryActorKind::StormCloud,
        "local".into(),
        session.world_key("local").unwrap(),
        Some(27),
    );
    cloud.storm_burst = Some(StormBurst {
        remaining_ticks: 3,
        source: Vec2::new(100.0, -175.0),
        midpoint: Vec2::new(150.0, -90.0),
        endpoint: Vec2::new(200.0, -15.0),
        target_id: 999,
    });
    let mut strike = cloud.clone();
    strike.kind = SecondaryActorKind::StormStrike;
    strike.storm_burst = None;
    strike.lifetime_ticks = 6;
    session.secondary.spawn_actor(cloud);
    session.secondary.spawn_actor(strike);
    enroll_secondary_painter_owners(&mut session.secondary.actors, &mut session.painter_order);
    enroll_secondary_light_owners(
        &mut session.secondary.actors,
        Some(&mut session.painter_order),
        |_, _| None,
    );
    let metadata = SaveMetadata {
        integrity: SaveIntegrity::LocalOnly,
        mods: Vec::new(),
        mod_state: LuaObject::default(),
        native_source: None,
    };
    let document = website_save::create(CreateSaveOptions {
        state: &session,
        player_id: "local",
        loaded_boneyard: Some(&loaded),
        metadata: &metadata,
        party_rejoin_token: None,
    })
    .unwrap();
    let restored = website_save::restore(&document).unwrap();
    assert_eq!(restored.session.secondary.actors, session.secondary.actors);
    assert_eq!(restored.session.secondary.rng, session.secondary.rng);
    let value: Value = serde_json::from_str(&document).unwrap();
    assert_eq!(value["schemaVersion"], 53);
    for ticks in [0, 5] {
        let mut malformed = value.clone();
        malformed["continuation"]["simulation"]["secondaryAbilities"]["actors"][0]["stormBurst"]
            ["remainingTicks"] = json!(ticks);
        let error =
            website_save::restore(&malformed.to_string()).expect_err("invalid pending countdown");
        assert!(error.contains("Storm burst"), "{error}");
    }
    let mut legacy = value;
    legacy["schemaVersion"] = json!(52);
    let actors = legacy["continuation"]["simulation"]["secondaryAbilities"]["actors"]
        .as_array_mut()
        .unwrap();
    actors[0].as_object_mut().unwrap().remove("stormBurst");
    actors[1]["lifetimeTicks"] = json!(1);
    actors[1]["lightRegistration"] = Value::Null;
    actors[1]["painterRegistrations"] = json!([{"managerLane":"actor","registrationOrdinal":90}]);
    let migrated = website_save::restore(&legacy.to_string()).unwrap();
    assert_eq!(migrated.session.secondary.actors.len(), 1);
    assert_eq!(migrated.session.secondary.actors[0].kind, SecondaryActorKind::StormCloud);
    assert!(migrated.session.secondary.actors[0].storm_burst.is_none());
    assert_eq!(migrated.session.secondary.rng, session.secondary.rng);
}
