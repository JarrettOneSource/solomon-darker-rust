//! Source coating transitions enroll real effects in the shared spell/painter/event queues.
#[path = "support/player_combat.rs"]
// Shared player projections are consumed in different subsets by each host fixture.
#[allow(dead_code)]
mod player;
#[path = "support/secondary_state.rs"]
// Shared secondary projections are consumed in different subsets by each host fixture.
#[allow(dead_code)]
mod secondary;

use serde_json::{Value, json};
use solomon_sim::harden_effects::*;
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::secondary::NativeSecondarySimulationState;
use solomon_sim::session::harden::*;
use solomon_sim::spells::model::{PrimarySpellTransient, SpellState};
use solomon_sim::{NativeRng, Vec2};

fn read_rng(value: &Value) -> NativeRng {
    let mut rng = NativeRng::new(0);
    rng.index_a = value["indexA"].as_u64().unwrap() as usize;
    rng.index_b = value["indexB"].as_u64().unwrap() as usize;
    for (word, value) in rng.words.iter_mut().zip(value["words"].as_array().unwrap()) {
        *word = value.as_u64().unwrap() as u32;
    }
    rng
}
fn read_shard(value: &Value) -> NativeHardenShardBody {
    NativeHardenShardBody {
        bounce_velocity: value["bounceVelocity"].as_f64().unwrap(),
        enhanced_shadow: true,
        height: value["height"].as_f64().unwrap(),
        life: value["life"].as_f64().unwrap(),
        position: secondary::point(&value["position"]),
        record: value["record"].as_u64().unwrap() as u32,
        rotation_degrees: value["rotationDegrees"].as_f64().unwrap(),
        rotation_step_degrees: value["rotationStepDegrees"].as_f64().unwrap(),
        velocity: secondary::point(&value["velocity"]),
        vertical_velocity: value["verticalVelocity"].as_f64().unwrap(),
    }
}
fn effect_snapshot(effect: &NativeHardenEffect) -> Value {
    let (owner, mut value) = match effect {
        NativeHardenEffect::Shard { owner, body } => (
            owner,
            json!({
                "kind": "harden-shard", "bounceVelocity": body.bounce_velocity, "height": body.height,
                "life": body.life, "position": secondary::point_snapshot(body.position), "record": body.record,
                "rotationDegrees": body.rotation_degrees, "rotationStepDegrees": body.rotation_step_degrees,
                "velocity": secondary::point_snapshot(body.velocity), "verticalVelocity": body.vertical_velocity,
            }),
        ),
        NativeHardenEffect::Burst { owner, alpha, position } => (
            owner,
            json!({
                "kind": "harden-burst", "alpha": alpha, "position": secondary::point_snapshot(*position),
            }),
        ),
    };
    let common = &owner.common;
    assert!(common.light_registration.is_none(), "Harden has no point-light provider");
    value["ageTicks"] = json!(common.age_ticks);
    value["birthTick"] = json!(owner.birth_tick);
    value["id"] = json!(common.id);
    value["ownerId"] = json!(common.owner_id);
    value["worldKey"] = json!(common.world_key);
    value["painterRegistrations"] = json!(
        common
            .painter_registrations
            .iter()
            .copied()
            .map(secondary::registration_snapshot)
            .collect::<Vec<_>>()
    );
    value
}

#[test]
fn harden_chips_and_coating_transitions_enroll_in_source_order_with_exact_rng() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/player-harden-host-goldens.json"))
            .unwrap();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let runtime = |coating: &Value| {
            player::configured_player(&json!({
                "runtime": { "harden": { "armor": 2, "coating": coating } },
            }))
            .0
            .skill_runtime
        };
        let before = case["before"].as_f64().map(|_| runtime(&case["before"]));
        let after = runtime(&case["after"]);
        let chips = case["chips"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pending| PendingPlayerHardenChip {
                chip: NativeHardenChip {
                    pitch: pending["chip"]["pitch"].as_f64().unwrap(),
                    shard: read_shard(&pending["chip"]["shard"]),
                },
                owner_id: pending["ownerId"].as_str().unwrap().into(),
                position: secondary::point(&pending["position"]),
                world_key: pending["worldKey"].as_str().unwrap().into(),
            })
            .collect::<Vec<_>>();
        let mut spells = SpellState { next_id: 81, ..SpellState::default() };
        let mut secondary = NativeSecondarySimulationState::new(55);
        let mut rng = read_rng(&case["beforeRng"]);
        let mut order = WorldPainterOrder { next_actor_ordinal: 300, next_transient_ordinal: 500 };
        synchronize_player_harden_effects(
            PlayerHardenContext {
                enhanced_effects: true,
                player_id: "local",
                before: before.as_ref(),
                after: &after,
                position: Vec2::new(321.25, 245.5),
                world_key: "boneyard:oracle",
                spells: &mut spells,
                secondary: &mut secondary,
                tick: 202,
                rng: &mut rng,
                painter_order: &mut order,
            },
            &chips,
        );
        let effects = spells
            .transients
            .iter()
            .map(|effect| {
                let PrimarySpellTransient::Harden(effect) = effect else {
                    panic!("Harden enrolls actual Harden actors")
                };
                effect_snapshot(effect)
            })
            .collect::<Vec<_>>();
        let actual = json!({
            "spells": { "nextId": spells.next_id, "projectiles": [], "transients": effects },
            "secondary": secondary::state_snapshot(&secondary), "rng": player::rng_snapshot(&rng),
            "order": { "nextRegistrationOrdinal": { "actor": order.next_actor_ordinal, "transient": order.next_transient_ordinal } },
        });
        player::assert_snapshot(&actual, &case["expected"], &format!("Harden:{index}"));
    }
}
