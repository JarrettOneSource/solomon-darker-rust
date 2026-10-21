//! Current shared warp state and per-world camera proposals, including terminal waves.

#[path = "support/secondary.rs"]
#[allow(dead_code)] // The shared Source adapter also serves complete ability-family fixtures.
mod support;

use std::collections::BTreeMap;

use serde_json::{Value, json};
use solomon_sim::belt::PlayerBeltComponent;
use solomon_sim::secondary::motion::create_native_stoneskin_warp;
use solomon_sim::secondary::*;
use solomon_sim::{HubWizard, NativeRng, Vec2};
use support::*;

fn rng_values(rng: &NativeRng) -> Vec<u32> {
    let mut values = vec![rng.index_a as u32, rng.index_b as u32];
    values.extend(rng.words);
    values
}

fn cameras(state: &NativeSecondarySimulationState) -> Value {
    Value::Array(state.camera_displacements.iter().map(|row| json!({
        "displacement":point_snapshot(row.displacement),"tick":row.tick,"worldKey":row.world_key,
    })).collect())
}

#[test]
fn current_secondary_warps_and_wave_camera_proposals_preserve_the_source_rng_and_expiry_order() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/current-secondary-motion-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["gridCases"].as_array().unwrap() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        for grid in case["grids"].as_array().unwrap() {
            assert_snapshot(
                &json!(create_native_stoneskin_warp(&mut rng)),
                &grid["positions"],
                "warp",
            );
            assert_snapshot(&json!(rng_values(&rng)), &grid["rng"], "warp RNG");
        }
    }
    let mut state = NativeSecondarySimulationState::new(17);
    assert!(state.camera_displacements.is_empty());
    assert!(state.stoneskin_warp.is_none());
    for (proposal, expected) in fixture["proposals"]
        .as_array()
        .unwrap()
        .iter()
        .zip(fixture["proposalFrames"].as_array().unwrap())
    {
        state.record_camera_displacement(
            proposal[0].as_str().unwrap(),
            Vec2::new(proposal[1].as_f64().unwrap(), proposal[2].as_f64().unwrap()),
            proposal[3].as_u64().unwrap(),
        );
        assert_snapshot(&cameras(&state), expected, "camera admission order");
    }

    let declared = &fixture["player"];
    let book = read_book(&declared["skillBook"]);
    let belt = PlayerBeltComponent::new(&book);
    let wizard = HubWizard::at(Vec2::ZERO);
    let mut authority = authority_from_fixture(declared, &book, &belt, &wizard);
    authority.input.quickbar = None;
    let players = BTreeMap::from([("player".to_owned(), authority)]);
    let empty_world = json!({"scenery":[]});
    let mut world = World { case: &empty_world, targets: Vec::new() };
    for case in fixture["waveCases"].as_array().unwrap() {
        let kind = NativeSecondaryActorKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == case["kind"].as_str().unwrap())
            .unwrap();
        let mut state = NativeSecondarySimulationState::new(case["seed"].as_i64().unwrap());
        let mut wave = NativeSecondaryActorState::new(
            0,
            kind,
            "player".into(),
            declared["worldKey"].as_str().unwrap().into(),
            match kind {
                NativeSecondaryActorKind::Shockwave => Some(21),
                NativeSecondaryActorKind::FreezeWave => Some(35),
                _ => None,
            },
        );
        wave.position = Vec2::new(21.0, 34.0);
        wave.velocity = Vec2::new(13.0, -7.0);
        wave.phase = 0.03_f32 as f64;
        wave.slow_factor = 0.0375_f32 as f64;
        wave.quantity = 6.0;
        wave.alpha = 2.0;
        wave.radius = 175.0;
        wave.lifetime_ticks = 100;
        state.spawn_actor(wave);
        for frame in case["frames"].as_array().unwrap() {
            let tick = frame["tick"].as_u64().unwrap();
            if case["gameOver"].as_bool().unwrap() {
                state.step_mindblast_presentation(tick);
            } else {
                state.step(
                    &NativeSecondaryTickContext { enhanced_effects: None, players: &players, tick },
                    &mut world,
                    None,
                );
            }
            let context = format!("{} gameOver{} tick{tick}", case["kind"], case["gameOver"]);
            let actors: Vec<_> = state
                .actors
                .iter()
                .map(|actor| {
                    [
                        actor.age_ticks as f64,
                        actor.alpha,
                        actor.phase,
                        actor.radius,
                        actor.scale,
                        actor.position.x,
                        actor.position.y,
                        actor.velocity.x,
                        actor.velocity.y,
                    ]
                })
                .collect();
            assert_snapshot(&json!(actors), &frame["actors"], &context);
            assert_snapshot(&cameras(&state), &frame["camera"], &format!("{context} camera"));
            assert_snapshot(
                &json!(rng_values(&state.rng)),
                &frame["rng"],
                &format!("{context} RNG"),
            );
        }
    }
}
