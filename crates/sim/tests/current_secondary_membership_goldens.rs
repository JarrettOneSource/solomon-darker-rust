//! Retained modifiers and real rescue waves follow current Source identity and cell order.
#![cfg(feature = "persistence")]
#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;
#[path = "support/secondary.rs"]
#[allow(dead_code)]
mod support;

use std::collections::BTreeMap;

use serde_json::{Value, json};
use solomon_sim::belt::PlayerBeltComponent;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use solomon_sim::secondary::*;
use solomon_sim::{HubWizard, NativeRng, Vec2};

struct World {
    retained: Value,
    targets: Vec<NativeSecondaryTarget>,
}
impl NativeSecondaryWorld for World {
    fn target_exists(&self, key: &str, id: usize) -> bool {
        if self.retained["kind"] != "boneyard"
            || key != format!("boneyard:{}", self.retained["runId"].as_str().unwrap())
        {
            return false;
        }
        ["actors", "maggots"].iter().any(|lane| {
            self.retained["enemies"][lane]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["id"].as_u64() == Some(id as u64))
        })
    }
    fn dampen_candidates(&self, _: &str, _: Vec2) -> NativeSecondaryDampenCandidates {
        NativeSecondaryDampenCandidates::default()
    }
    fn phasing_destination(&self, _: &str, _: Vec2, _: Vec2) -> Option<Vec2> {
        None
    }
    fn golem_movement(&self, _: &str, _: &str, _: Vec2, requested: Vec2, _: f64) -> Vec2 {
        requested
    }
    fn golem_placement(&self, _: &str, _: &str, requested: Vec2, _: &mut NativeRng) -> Vec2 {
        requested
    }
    fn teleport_destination(&self, _: &str, _: &mut NativeRng) -> Vec2 {
        Vec2::ZERO
    }
    fn target(&self, _: &str, id: usize) -> Option<NativeSecondaryTarget> {
        self.targets.iter().find(|target| target.id == id).cloned()
    }
    fn targets(&self, _: &str, center: Vec2, radius: f64) -> Vec<NativeSecondaryTarget> {
        self.targets
            .iter()
            .filter(|target| {
                solomon_sim::float::hypot(
                    target.position.x - center.x,
                    target.position.y - center.y,
                ) <= radius + target.radius
            })
            .cloned()
            .collect()
    }
}
fn snapshot(state: &NativeSecondarySimulationState) -> Value {
    json!({"effects":state.target_effects.iter().map(support::effect_snapshot).collect::<Vec<_>>(),
        "rng":support::rng_snapshot(&state.rng)})
}
fn fixture() -> Value {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/current-secondary-membership-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    fixture
}
#[test]
fn modifiers_end_at_actor_destruction_and_world_identity_changes() {
    for case in fixture()["membership"].as_array().unwrap() {
        let mut state = NativeSecondarySimulationState::new(77);
        for identity in case["identities"].as_array().unwrap() {
            apply_native_secondary_target_effect(
                &mut state.target_effects,
                identity["worldKey"].as_str().unwrap(),
                identity["targetId"].as_u64().unwrap() as usize,
                NativeSecondaryTargetEffectPatch {
                    dazzle_ticks: Some(20),
                    cold_slow_factor: Some(0.75),
                    cold_slow_material: Some(true),
                    cold_slow_ticks: Some(10),
                    ..NativeSecondaryTargetEffectPatch::default()
                },
            );
        }
        for frame in case["frames"].as_array().unwrap() {
            let tick = frame["tick"].as_u64().unwrap();
            state.step(
                &NativeSecondaryTickContext {
                    enhanced_effects: None,
                    players: &BTreeMap::new(),
                    tick,
                },
                &mut World { retained: frame["retained"].clone(), targets: Vec::new() },
                None,
            );
            native_bits::compare(
                &native_bits::source_value(&snapshot(&state)),
                &native_bits::source_value(&frame["expected"]),
                &format!("{} tick{tick}", case["kind"]),
            );
        }
    }
}
#[test]
fn rescue_wave_contacts_follow_native_cells_and_registration_order() {
    for case in fixture()["waves"].as_array().unwrap() {
        let declared = &case["player"];
        let book = support::read_book(&declared["skillBook"]);
        let belt = PlayerBeltComponent::new(&book);
        let wizard = HubWizard::at(Vec2::ZERO);
        let targets: Vec<_> =
            case["targets"].as_array().unwrap().iter().map(support::target).collect();
        let mut world = World {
            retained: json!({"kind":"boneyard","runId":"oracle",
            "enemies":{"actors":case["targets"],"maggots":[]}}),
            targets,
        };
        let mut state = NativeSecondarySimulationState::new(91);
        let mut order = WorldPainterOrder::default();
        state.spawn_player_rescue_shockwave(
            NativePlayerRescueInput {
                light_registration: PainterRegistration {
                    manager_lane: WorldManagerLane::Actor,
                    registration_ordinal: 100,
                },
                owner_id: "player",
                position: Vec2::ZERO,
                world_key: "boneyard:oracle",
            },
            &mut order,
        );
        let mut frame_index = 0;
        let frames = case["frames"].as_array().unwrap();
        for tick in 1..=30 {
            let mut authority = support::authority_from_fixture(declared, &book, &belt, &wizard);
            // This replay begins with an accepted rescue, not a belt input. The
            // historical helper defaults to a first-slot press for cast fixtures.
            authority.input.quickbar = None;
            let players = BTreeMap::from([("player".into(), authority)]);
            let result = state.step(
                &NativeSecondaryTickContext { enhanced_effects: None, players: &players, tick },
                &mut world,
                Some(&mut order),
            );
            if tick % 10 != 0 {
                continue;
            }
            let mut actual = snapshot(&state);
            actual["actorKinds"] =
                json!(state.actors.iter().map(|actor| actor.kind.as_str()).collect::<Vec<_>>());
            actual["hitTargetIds"] = json!(state.actors[0].hit_target_ids);
            actual["damagedTargetIds"] =
                json!(result.damage.iter().map(|contact| contact.target_id).collect::<Vec<_>>());
            native_bits::compare(
                &native_bits::source_value(&actual),
                &native_bits::source_value(&frames[frame_index]["expected"]),
                &format!("{} tick{tick}", case["orderMode"]),
            );
            frame_index += 1;
        }
        assert_eq!(frame_index, frames.len());
    }
}
