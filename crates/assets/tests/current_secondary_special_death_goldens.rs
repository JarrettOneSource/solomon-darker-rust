//! Actual secondary receiver admission and deferred newborn publication against Source.
#[path = "../../sim/tests/support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;

use serde_json::{Value, json};
use solomon_sim::enemies::config::EnemyArenaScalars;
use solomon_sim::enemies::model::EnemyLifeState;
use solomon_sim::enemies::path::EnemyNavigation;
use solomon_sim::enemies::world::*;
use solomon_sim::enemies::{EnemyFlag, EnemySpawnIntent, EnemyToken};
use solomon_sim::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::secondary::*;
use solomon_sim::session::{GameSession, GameSessionOptions};
use solomon_sim::spells::boneyard::{
    BoneyardPrimaryContactEpoch, resolve_boneyard_primary_contacts_for_players,
};
use solomon_sim::spells::model::{
    FireExplosionPresentation, PrimarySpellTickResult, PrimarySpellTransient,
};
use solomon_sim::spells::special_death::{
    NativeSpellSpecialDeathKind, NativeSpellSpecialDeathPayload,
};
use solomon_sim::spells::weld::{NativeWeldSteamVariant, NativeWeldWorldActor};
use solomon_sim::{NativeRng, Vec2};

struct OpenGeometry;
impl EnemyNavigation for OpenGeometry {
    fn direct_path_clear(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> bool {
        true
    }
    fn find_route(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> Option<Vec<Vec2>> {
        None
    }
}
impl EnemyGeometry for OpenGeometry {
    fn resolve_movement(&self, request: EnemyMovementRequest) -> Vec2 {
        request.requested_position
    }
    fn line_obstruction(&self, _: Vec2, _: Vec2, _: u32) -> Option<Vec2> {
        None
    }
    fn projectile_world_blocked(&self, _: EnemyProjectileWorldQuery) -> bool {
        false
    }
}
fn registration(value: PainterRegistration) -> Value {
    json!([
        match value.manager_lane {
            WorldManagerLane::Actor => "actor",
            WorldManagerLane::Transient => "transient",
        },
        value.registration_ordinal
    ])
}
fn transient(value: &PrimarySpellTransient) -> Value {
    let common = value.common();
    let (kind, detail) = match value {
        PrimarySpellTransient::FireExplosion(actor) => (
            "fire-explosion",
            json!({
            "origin":[actor.origin.x,actor.origin.y],"damage":actor.damage,
            "footprintDimension":actor.footprint_dimension,"visualScale":actor.visual_scale,
            "soundPitch":actor.sound_pitch,"presentation":match actor.presentation {
                FireExplosionPresentation::Fire=>"fire",FireExplosionPresentation::Steam=>"steam"}}),
        ),
        PrimarySpellTransient::Weld(NativeWeldWorldActor::Steam(actor)) => (
            "weld-steam",
            json!({
            "position":[actor.position.x,actor.position.y],"velocity":[actor.velocity.x,actor.velocity.y],
            "life":actor.life,"scale":actor.scale,"contactEnabled":actor.contact_enabled,
            "contactDue":actor.contact_due,"contactDamage":actor.contact_damage,"phase":actor.phase,
            "variant":match actor.variant {NativeWeldSteamVariant::Normal=>"normal",NativeWeldSteamVariant::Over=>"over"}}),
        ),
        other => panic!("unexpected secondary special-death birth {other:?}"),
    };
    let mut result = json!({"id":common.id,"kind":kind,"age":common.age_ticks,
        "ownerId":common.owner_id,"worldKey":common.world_key,
        "light":common.light_registration.map(registration),
        "painters":common.painter_registrations.iter().copied().map(registration).collect::<Vec<_>>()});
    result.as_object_mut().unwrap().extend(detail.as_object().unwrap().clone());
    result
}
fn contact(value: &Value) -> NativeSecondaryDamageContact {
    NativeSecondaryDamageContact {
        amount: value["amount"].as_f64().unwrap(),
        ether_drain: false,
        hit_strength: value["hitStrength"].as_f64(),
        kind: NativeSecondaryDamageKind::Fire,
        owner_id: value["ownerId"].as_str().unwrap().into(),
        source_actor_id: value["sourceActorId"].as_u64().unwrap(),
        special_death_payload: value["specialDeathPayload"].as_object().map(|payload| {
            NativeSpellSpecialDeathPayload {
                kind: NativeSpellSpecialDeathKind::Steam,
                ember_damage: payload["emberDamage"].as_f64().unwrap(),
                ember_fragments: payload["emberFragments"].as_u64().unwrap() as u32,
                explode_damage: payload["explodeDamage"].as_f64().unwrap(),
                explode_radius: payload["explodeRadius"].as_f64().unwrap(),
                private_seed: payload["privateSeed"].as_u64().unwrap() as u32,
            }
        }),
        suppress_hit_reaction: value["suppressHitReaction"] == true,
        suppress_hurt_sound: value["suppressHurtSound"] == true,
        target_id: value["targetId"].as_u64().unwrap() as usize,
    }
}

#[test]
fn secondary_terminal_dispatch_precedes_next_hit_and_publishes_after_primary_pass() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/current-secondary-special-death-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let mut session = GameSession::new(
            "hero".into(),
            NewGameSelection {
                name: "Secondary".into(),
                element: WizardElement::Fire,
                discipline: WizardDiscipline::Arcane,
            },
            GameSessionOptions::default(),
        );
        let mut loaded = solomon_assets::boneyard::load_tutorial([0; 16]).unwrap();
        loaded.run_id = "secondary-special".into();
        session.install_boneyard(loaded).unwrap();
        session.enhanced_effects = case["enhancedEffects"].as_bool().unwrap_or(true);
        session.painter_order =
            WorldPainterOrder { next_actor_ordinal: 100, next_transient_ordinal: 70 };
        session.combat_rng = NativeRng::new(17);
        session.spells.next_id = 100;
        let world = &mut session.boneyard.as_mut().unwrap().enemies;
        *world = EnemyWorld::new(case["seed"].as_str().unwrap(), 50);
        let mut intents = [
            EnemySpawnIntent::new(
                EnemyToken::from_id(case["token"].as_str().unwrap_or("SKELETON")).unwrap(),
                Vec2::new(150.0, 250.0),
                1,
                10,
                31,
            ),
            EnemySpawnIntent::new(EnemyToken::Skeleton, Vec2::new(160.0, 250.0), 2, 10, 31),
        ];
        if let Some(flags) = case["flags"].as_array() {
            intents[0].flags = flags
                .iter()
                .map(|flag| EnemyFlag::from_id(flag.as_str().unwrap()).unwrap())
                .collect();
        }
        world.spawn(
            &intents,
            EnemySpawnContext {
                players: &[],
                tick: 10,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: None,
            },
            &OpenGeometry,
            Some(&mut session.painter_order),
        );
        world.actors[0].current_health = case["health"].as_f64().unwrap();
        world.actors[1].current_health = case["neighborHealth"].as_f64().unwrap_or(10.0);
        if let Some(shield) = case["shield"].as_f64() {
            world.actors[0].shield_health = shield;
            world.actors[0].shield_maximum_health = shield;
        }
        let result = NativeSecondaryTickResult {
            damage: case["contacts"].as_array().unwrap().iter().map(contact).collect(),
            ..NativeSecondaryTickResult::default()
        };
        let deaths = session.apply_secondary_contacts(&result, 10);
        assert!(
            session.spells.transients.is_empty(),
            "{} newborns stay outside the old-actor epoch",
            case["name"]
        );
        let primary = resolve_boneyard_primary_contacts_for_players(
            session.boneyard.as_mut().unwrap(),
            &[],
            &mut session.secondary,
            &mut session.spells,
            BoneyardPrimaryContactEpoch {
                enhanced_effects: session.enhanced_effects,
                tick: 10,
                cast: &PrimarySpellTickResult::default(),
                secondary_deaths: deaths,
                additional_scenery_targets: &[],
                light_at: &|_| 0.0,
            },
            &mut session.combat_rng,
            &mut session.painter_order,
        );
        assert!(
            primary.hits.is_empty(),
            "{} newborn area must not replay in ordinary primary pass",
            case["name"]
        );
        let world = &session.boneyard.as_ref().unwrap().enemies;
        let actual = json!({"actors":world.actors.iter().map(|actor|json!([actor.id,actor.current_health,
            match actor.life_state {EnemyLifeState::Alive=>"alive",EnemyLifeState::Dying=>"dying"},
            actor.shield_health,{"strength":actor.hit_feedback.strength,"tick":actor.hit_feedback.tick,
                "timer":actor.hit_feedback.timer},actor.hit_reaction_timer])).collect::<Vec<_>>(),
            "rng":{"indexA":session.combat_rng.index_a,"indexB":session.combat_rng.index_b,"words":session.combat_rng.words.as_slice()},
            "enemyRng":world.rng_state,"nextSpellId":session.spells.next_id,
            "transients":session.spells.transients.iter().map(transient).collect::<Vec<_>>(),
            "painter":{"actor":session.painter_order.next_actor_ordinal,"transient":session.painter_order.next_transient_ordinal}});
        let expected = &case["expected"];
        let expected = json!({"actors":expected["actors"],"rng":expected["rng"],"enemyRng":expected["enemyRng"],
            "nextSpellId":expected["nextSpellId"],"transients":expected["transients"],"painter":expected["painter"]});
        native_bits::compare(&actual, &expected, case["name"].as_str().unwrap());
    }
}
