//! Independent escape trajectories over all retained native scenes and the tutorial.
//!
//! The Website oracle records the entire trajectory. No source spatial state is injected
//! between transitions; this detects the measured template-nine accumulated path divergence.

#[path = "../../sim/tests/support/mod.rs"]
mod support;

use std::path::PathBuf;

use serde_json::{Value, json};
use solomon_assets::boneyard::BoneyardCatalog;
use solomon_assets::navigation::decode_navigation;
use solomon_sim::Vec2;
use solomon_sim::boneyard::collision::BoneyardCollisionWorld;
use solomon_sim::boneyard::encounter::{SolomonEncounter, SolomonPlayer};
use solomon_sim::boneyard::escape::{
    prepare_solomon_escape_navigation, resolve_solomon_escape_movement,
};
use solomon_sim::boneyard::gate::create_gate_leaves;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn snapshot(state: &SolomonEncounter) -> Value {
    json!({
        "position": { "x": state.position.x, "y": state.position.y },
        "headingDeg": state.heading_degrees,
        "escapeTarget": state.escape_target.map(|point| json!({ "x": point.x, "y": point.y })),
        "escapeCollisionSourceIds": state.escape_collision_source_ids,
        "phase": state.phase.id(), "lifetimeTicksRemaining": state.lifetime_ticks_remaining,
        "acceleration": state.acceleration, "escapeSpeed": state.escape_speed,
        "transitionOffsetY": state.transition_offset_y,
        "rngState": { "indexA": state.rng.index_a, "indexB": state.rng.index_b, "words": state.rng.words.as_slice() },
        "voiceTicksRemaining": state.voice_ticks_remaining,
        "voiceEvents": state.voice_events.iter().map(|event| json!({ "cue": event.cue.id(), "id": event.id })).collect::<Vec<_>>(),
    })
}

fn compare(state: &SolomonEncounter, expected: &Value, tick: u64) {
    support::assert_website_json(
        &snapshot(state),
        expected,
        &format!("escape tick {tick}"),
        &["position.x", "position.y", "headingDeg", "escapeTarget.x", "escapeTarget.y"],
    );
}

#[test]
fn escape_steering_uses_the_source_math_for_identical_route_goal_operands() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/solomon-escape-math-goldens.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let position = point(&case["position"]);
        let goal = point(&case["goal"]);
        let x = goal.x - position.x;
        let y = goal.y - position.y;
        let angle = solomon_sim::float::atan2(x, -y);
        let product = angle * 180.0;
        let degrees = product / std::f64::consts::PI;
        let shifted = degrees + 360.0;
        let heading = solomon_sim::heading::heading_degrees_from_vector(x, y);
        for (name, actual) in [
            ("x", x),
            ("y", y),
            ("angle", angle),
            ("product", product),
            ("degrees", degrees),
            ("shifted", shifted),
            ("heading", heading),
        ] {
            let expected = case[name].as_f64().unwrap();
            assert_eq!(actual.to_bits(), expected.to_bits(), "tick {} {name}", case["tick"]);
        }
    }
}

#[test]
fn solomon_escape_trajectories_match_without_replacing_their_spatial_state() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/solomon-escape-goldens.json"
    ))
    .unwrap();
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    // Exercise the measured material divergence first, then every other retained scene.
    for index in [9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12] {
        let case = &cases[index];
        let index = case["templateIndex"].as_u64().unwrap() as usize;
        let mut seed = [0; 16];
        seed[..4].copy_from_slice(&(index as u32).to_be_bytes());
        let mut loaded = if index == catalog.templates.len() {
            catalog.materialize_tutorial(seed)
        } else {
            catalog.materialize(seed)
        };
        loaded.seed = case["seed"].as_str().unwrap().into();
        let directory =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/assets/navigation");
        let json =
            std::fs::read_to_string(directory.join(format!("{}.json", loaded.source_sha256)))
                .unwrap();
        let navigation = decode_navigation(&json, &loaded).unwrap();
        let collision = BoneyardCollisionWorld::from_scene(&loaded.scene);
        let mut leaves = create_gate_leaves(&loaded.scene.fences, &loaded.seed);
        let dig = loaded.scene.solomon_dig.as_ref().unwrap();
        let dialogue_ticks = case["tutorialDialogueTicks"].as_u64().unwrap();
        let mut state = if dialogue_ticks > 0 {
            SolomonEncounter::new_tutorial(dig, &loaded.seed, dialogue_ticks)
        } else {
            SolomonEncounter::new(dig, &loaded.seed)
        };
        let players =
            [SolomonPlayer { id: "local", position: point(&case["players"]["local"]["position"]) }];
        let initial_tick = case["initialTick"].as_u64().unwrap();
        for tick in 1..=initial_tick {
            for leaf in &mut leaves {
                leaf.step();
            }
            state.step(&players, tick);
        }
        let records = case["records"].as_array().unwrap();
        compare(&state, &records[0]["state"], initial_tick);
        // Each result becomes the next input. A source snapshot is an expected output,
        // never an instruction to overwrite the independently evolving Rust trajectory.
        let mut maximum_position_delta: f64 = 0.0;
        let mut maximum_heading_delta: f64 = 0.0;
        for pair in records.windows(2) {
            let tick = pair[1]["tick"].as_u64().unwrap();
            for leaf in &mut leaves {
                leaf.step();
            }
            let mut dynamic_collision = collision.clone();
            dynamic_collision.set_gate_leaves(&leaves);
            let mut source = state.clone();
            prepare_solomon_escape_navigation(
                &mut source,
                loaded.scene.bounds,
                &dynamic_collision,
                &collision,
                &navigation,
            );
            state = source.clone();
            state.step(&players, tick);
            resolve_solomon_escape_movement(
                &source,
                &mut state,
                loaded.scene.bounds,
                &dynamic_collision,
            );
            let expected = &pair[1]["state"];
            let expected_position = point(&expected["position"]);
            maximum_position_delta = maximum_position_delta.max(solomon_sim::float::hypot(
                state.position.x - expected_position.x,
                state.position.y - expected_position.y,
            ));
            maximum_heading_delta = maximum_heading_delta
                .max((state.heading_degrees - expected["headingDeg"].as_f64().unwrap()).abs());
            compare(&state, &pair[1]["state"], tick);
        }
        eprintln!(
            "template {index}: maximum independent position delta={maximum_position_delta}, heading delta={maximum_heading_delta}"
        );
    }
}
