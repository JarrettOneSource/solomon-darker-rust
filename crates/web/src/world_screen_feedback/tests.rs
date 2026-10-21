//! Unchanged current Source mixed-family camera and shared-flash journeys.
use serde_json::{Value, json};
use solomon_sim::screen_flash::NativeScreenFlashWrite;
use solomon_sim::secondary::NativeSecondaryScreenFlashState;

use super::*;

#[path = "../../../sim/tests/support/website_rows.rs"]
mod website_rows;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn context(value: &Value) -> NativeSecondaryScreenFeedbackContext {
    NativeSecondaryScreenFeedbackContext {
        camera_center: point(&value["cameraCenter"]),
        local_player_alternate: value["localPlayerAlternate"].as_bool().unwrap(),
        visible_world_width: value["visibleWorldWidth"].as_f64().unwrap(),
    }
}
fn state(value: &Value) -> NativeScreenFlashState {
    NativeScreenFlashState {
        epoch: value["epoch"].as_u64().unwrap(),
        next_order: value["nextOrder"].as_u64().unwrap(),
        writes: value["writes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let flash = &row["flash"];
                NativeScreenFlashWrite {
                    order: row["order"].as_u64().unwrap(),
                    tick: row["tick"].as_u64().unwrap(),
                    world_key: row["worldKey"].as_str().unwrap().into(),
                    position: point(&row["position"]),
                    only_if_clear: row["onlyIfClear"].as_bool().unwrap(),
                    flash: NativeSecondaryScreenFlashState {
                        alpha: flash["alpha"].as_f64().unwrap(),
                        red: flash["red"].as_f64().unwrap(),
                        green: flash["green"].as_f64().unwrap(),
                        blue: flash["blue"].as_f64().unwrap(),
                        decay_per_tick: flash["decayPerTick"].as_f64().unwrap(),
                        point_attenuated: flash["pointAttenuated"].as_bool().unwrap(),
                    },
                }
            })
            .collect(),
    }
}

#[test]
fn current_feedback_replays_late_birth_ties_flash_admission_and_epoch_resets() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../sim/tests/fixtures/website/flame-client-goldens.json"
    ))
    .unwrap();
    for case in fixture["feedbackCases"].as_array().unwrap() {
        let mut lane =
            FeedbackLane::new(case["initialTick"].as_u64().unwrap(), "boneyard:flame".into());
        for step in case["steps"].as_array().unwrap() {
            match step["kind"].as_str().unwrap() {
                "flame" => {
                    let proposals: Vec<_> = step["proposals"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|proposal| NativeFlameCameraProposal {
                            id: proposal["id"].as_u64().unwrap(),
                            birth_tick: proposal["birthTick"].as_u64().unwrap(),
                            world_key: proposal["worldKey"].as_str().unwrap().into(),
                            displacement: point(&proposal["displacement"]),
                        })
                        .collect();
                    lane.consume_flame_camera_displacements(&proposals);
                }
                "current" => lane.consume_primary_camera_displacement(
                    step["eventId"].as_u64().unwrap(),
                    step["worldKey"].as_str().unwrap(),
                    step["tick"].as_u64().unwrap(),
                    point(&step["displacement"]),
                ),
                "synchronize" => lane.synchronize_world_timeline(
                    step["worldTick"].as_f64().unwrap(),
                    step["epoch"].as_u64(),
                    step["liveTick"].as_f64().unwrap(),
                ),
                "flashes" => {
                    lane.flashes.consume(&state(&step["state"]), context(&step["context"]))
                }
                "sample" => {
                    let at = step["tick"].as_f64().unwrap();
                    let overlay = lane
                        .flashes
                        .sample(at)
                        .map(|overlay| json!({"alpha":overlay.alpha,"color":overlay.color}))
                        .unwrap_or(Value::Null);
                    lane.advance_to(tick(at));
                    let displacement = lane.camera.sample();
                    website_rows::compare(
                        &overlay,
                        &step["overlay"],
                        case["name"].as_str().unwrap(),
                    );
                    assert_eq!(lane.camera_magnitude, step["magnitude"].as_f64().unwrap());
                    website_rows::compare(
                        &json!({"x":displacement.x,"y":displacement.y}),
                        &step["displacement"],
                        case["name"].as_str().unwrap(),
                    );
                }
                other => panic!("unexpected current feedback operation {other}"),
            }
        }
    }
}
