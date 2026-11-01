//! Current Website five-field HandCast1 continuation and legacy absence.
use serde_json::{Value, json};
use solomon_assets::website_save::{self, CreateSaveOptions, SaveIntegrity, SaveMetadata};
use solomon_sim::modding::LuaObject;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions, GameSessionTickContext};
use solomon_sim::spells::hand_cast::NativeHandCast1;
use std::collections::BTreeMap;

fn document() -> Value {
    let mut session = GameSession::new(
        "local".into(),
        NewGameSelection {
            name: "Bare continuation".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    let hand = NativeHandCast1::new(0.095_f32 as f64, 0, &mut session.combat_rng);
    {
        let participant = session.participants.get_mut("local").unwrap();
        participant.player.economy.equipment.weapon = None;
        participant.wizard.primary_cast.action_tick = Some(0.0);
        participant.wizard.primary_cast.selected_primary_id = Some(8);
        participant.wizard.primary_cast.hand_cast = Some(hand);
    }
    let metadata = SaveMetadata {
        integrity: SaveIntegrity::LocalOnly,
        mods: Vec::new(),
        mod_state: LuaObject::default(),
        native_source: None,
    };
    serde_json::from_str(
        &website_save::create(CreateSaveOptions {
            state: &session,
            player_id: "local",
            loaded_boneyard: None,
            metadata: &metadata,
            party_rejoin_token: None,
        })
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn website_hand_cast1_shape_restores_reexports_and_defaults_without_sampling() {
    let mut document = document();
    let cast = &mut document["continuation"]["simulation"]["playerEntities"]["primaryCasts"][0];
    assert!(cast.get("handCast1").is_some());
    assert!(cast.get("handCast").is_none());
    let hand = json!({"firstPose":3,"progress":0.5,"rate":0.095_f32 as f64,
        "pose":3,"done":false});
    cast["handCast1"] = hand.clone();
    let restored = website_save::restore(&document.to_string()).unwrap();
    let state = &restored.session.wizard(&restored.player_id).unwrap().primary_cast;
    assert_eq!(state.hand_cast.as_ref().unwrap().pose(), 3);
    assert_eq!(state.hand_cast.as_ref().unwrap().progress(), 0.5);
    let rng = restored.session.combat_rng.clone();
    let round_trip: Value = serde_json::from_str(
        &website_save::create(CreateSaveOptions {
            state: &restored.session,
            player_id: &restored.player_id,
            loaded_boneyard: restored.loaded_boneyard.as_ref(),
            metadata: &restored.metadata,
            party_rejoin_token: None,
        })
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        round_trip["continuation"]["simulation"]["playerEntities"]["primaryCasts"][0]["handCast1"],
        hand
    );
    assert_eq!(restored.session.combat_rng, rng);
    let mut resumed = website_save::restore(&round_trip.to_string()).unwrap().session;
    let saved_checkpoint =
        serde_json::from_value(serde_json::to_value(restored.session.snapshot()).unwrap()).unwrap();
    let mut checkpoint = GameSession::restore(saved_checkpoint, None).unwrap();
    for _ in 0..80 {
        resumed.step(&BTreeMap::new(), GameSessionTickContext::default());
        checkpoint.step(&BTreeMap::new(), GameSessionTickContext::default());
        assert_eq!(resumed.combat_rng, checkpoint.combat_rng);
        assert_eq!(
            resumed.wizard("local").unwrap().primary_cast,
            checkpoint.wizard("local").unwrap().primary_cast
        );
    }
    assert!(resumed.wizard("local").unwrap().primary_cast.hand_cast.is_none());
    assert_eq!(resumed.wizard("local").unwrap().primary_cast.emission_sequence, 1);
    document["continuation"]["simulation"]["playerEntities"]["primaryCasts"][0]
        .as_object_mut()
        .unwrap()
        .remove("handCast1");
    let old = website_save::restore(&document.to_string()).unwrap();
    assert!(old.session.wizard(&old.player_id).unwrap().primary_cast.hand_cast.is_none());
    assert_eq!(old.session.combat_rng, rng);
}

#[test]
fn website_hand_cast1_rejects_invalid_native_state() {
    for (field, value) in
        [("firstPose", json!(1)), ("rate", json!(-0.1)), ("pose", json!(4)), ("done", json!(true))]
    {
        let mut document = document();
        document["continuation"]["simulation"]["playerEntities"]["primaryCasts"][0]["handCast1"]
            [field] = value;
        assert!(website_save::restore(&document.to_string()).is_err(), "invalid {field}");
    }
}
