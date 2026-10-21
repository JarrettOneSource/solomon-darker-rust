//! Complete feedback state and output replay, including late and rejected events.

use serde_json::{Value, json};
use solomon_sim::region_feedback::*;
use solomon_sim::screen_feedback::*;
use solomon_sim::secondary::{
    NativeSecondaryActorKind, NativeSecondaryActorState, NativeSecondaryEventKind,
    NativeSecondaryEventState, NativeSecondaryScreenFlashState,
};
use solomon_sim::spells::model::{PrimarySpellEtherBlast, PrimarySpellTransientBase};
use solomon_sim::{NativeRng, Vec2};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn point_snapshot(value: Vec2) -> Value {
    json!({ "x": value.x, "y": value.y })
}
fn overlay_snapshot(value: Option<NativeSecondaryScreenOverlay>) -> Value {
    json!(value.map(|overlay| json!({ "alpha": overlay.alpha, "color": overlay.color })))
}
fn read_overlay(value: &Value) -> Option<NativeSecondaryScreenOverlay> {
    value.as_object().map(|_| NativeSecondaryScreenOverlay {
        alpha: value["alpha"].as_f64().unwrap(),
        color: value["color"].as_u64().unwrap() as u32,
    })
}
fn flash(value: &Value) -> Option<NativeSecondaryScreenFlashState> {
    value.as_object().map(|_| NativeSecondaryScreenFlashState {
        alpha: value["alpha"].as_f64().unwrap(),
        blue: value["blue"].as_f64().unwrap(),
        decay_per_tick: value["decayPerTick"].as_f64().unwrap(),
        green: value["green"].as_f64().unwrap(),
        point_attenuated: value["pointAttenuated"].as_bool().unwrap(),
        red: value["red"].as_f64().unwrap(),
    })
}
fn feedback_snapshot(state: &NativeSecondaryScreenFeedbackPresentation) -> Value {
    json!({
        "alpha": state.alpha,
        "blue": state.blue,
        "cameraDisplacementX": state.camera_displacement_x,
        "cameraDisplacementY": state.camera_displacement_y,
        "cameraMagnitude": state.camera_magnitude,
        "green": state.green,
        "lastEnemyEventId": state.last_enemy_event_id,
        "lastEventId": state.last_event_id,
        "lastPrimaryImpactId": state.last_primary_impact_id,
        "lastPrimaryFeedbackId": state.last_primary_feedback_id,
        "lastPrimaryMagnitudeId": state.last_primary_magnitude_id,
        "lastTick": state.last_tick,
        "red": state.red,
        "decayPerTick": state.decay_per_tick,
        "worldKey": state.world_key,
    })
}
fn assert_snapshot(actual: &Value, expected: &Value, label: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64(), "{label}"),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{label} length");
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                assert_snapshot(a, b, &format!("{label}[{index}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{label} fields");
            for (key, b) in b {
                assert_snapshot(&a[key], b, &format!("{label}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{label}"),
    }
}
fn apply_action(
    state: &mut NativeSecondaryScreenFeedbackPresentation,
    action: &Value,
    context: NativeSecondaryScreenFeedbackContext,
) -> Value {
    match action["kind"].as_str().unwrap() {
        "secondary" => {
            let value = &action["event"];
            let mut event = NativeSecondaryEventState::new(
                NativeSecondaryEventKind::Impact,
                "player".into(),
                value["worldKey"].as_str().unwrap().into(),
                value["tick"].as_u64().unwrap(),
            );
            event.event_id = value["eventId"].as_u64().unwrap();
            event.camera_displacement = value["cameraDisplacement"]
                .as_object()
                .map(|_| point(&value["cameraDisplacement"]));
            event.camera_magnitude = value["cameraMagnitude"].as_f64().unwrap();
            event.position = point(&value["position"]);
            event.screen_flash = flash(&value["screenFlash"]);
            state.consume(&event, context);
        }
        "enemy" => {
            let value = &action["event"];
            let camera_shake = value["cameraShake"].as_object().map(|_| NativeRegionCameraShake {
                attenuation: match value["cameraShake"]["attenuation"].as_str().unwrap() {
                    "fixed" => NativeRegionCameraShakeAttenuation::Fixed,
                    "point" => NativeRegionCameraShakeAttenuation::Point,
                    "hit" => NativeRegionCameraShakeAttenuation::Hit,
                    "hit-squared" => NativeRegionCameraShakeAttenuation::HitSquared,
                    other => panic!("unknown attenuation {other}"),
                },
                displacement: point(&value["cameraShake"]["displacement"]),
            });
            state.consume_enemy(
                NativeEnemyScreenFeedbackInput {
                    camera_shake,
                    event_id: value["eventId"].as_u64().unwrap(),
                    run_id: value["runId"].as_str().unwrap(),
                    screen_flash: flash(&value["screenFlash"]),
                    screen_flash_only_if_clear: value["screenFlashOnlyIfClear"]
                        .as_bool()
                        .unwrap_or(false),
                    source_position: value["sourcePosition"]
                        .as_object()
                        .map(|_| point(&value["sourcePosition"])),
                    tick: value["tick"].as_u64().unwrap(),
                },
                context,
            );
        }
        "displacement" => {
            let value = &action["input"];
            state.consume_primary_camera_displacement(NativePrimaryCameraDisplacementInput {
                displacement: point(&value["displacement"]),
                event_id: value["eventId"].as_u64().unwrap(),
                tick: value["tick"].as_u64().unwrap(),
                world_key: value["worldKey"].as_str().unwrap(),
            });
        }
        "magnitude" => {
            let value = &action["input"];
            state.consume_primary_camera_magnitude(NativePrimaryCameraMagnitudeInput {
                event_id: value["eventId"].as_u64().unwrap(),
                magnitude: value["magnitude"].as_f64().unwrap(),
                tick: value["tick"].as_u64().unwrap(),
                world_key: value["worldKey"].as_str().unwrap(),
            });
        }
        "ether" => {
            let value = &action["effect"];
            state.consume_primary_ether_blast(
                &PrimarySpellEtherBlast {
                    common: PrimarySpellTransientBase {
                        age_ticks: 0,
                        id: value["id"].as_u64().unwrap(),
                        light_registration: None,
                        owner_id: "player".into(),
                        painter_registrations: Vec::new(),
                        world_key: value["worldKey"].as_str().unwrap().into(),
                    },
                    birth_tick: value["birthTick"].as_u64().unwrap(),
                    charges: value["charges"].as_u64().unwrap() as u32,
                    origin: point(&value["origin"]),
                    presentation_rng: NativeRng::new(0),
                },
                context,
            );
        }
        "sample" => return overlay_snapshot(state.sample(action["tick"].as_u64().unwrap())),
        "camera" => {
            return point_snapshot(
                state.sample_camera_displacement(action["tick"].as_u64().unwrap()),
            );
        }
        "magnitude-sample" => {
            return json!(state.sample_camera_magnitude(action["tick"].as_u64().unwrap()));
        }
        other => panic!("unknown feedback action {other}"),
    }
    Value::Null
}

#[test]
fn screen_overwrites_late_events_camera_arbitration_and_attenuation_replay_the_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/screen-feedback-goldens.json"))
            .unwrap();
    for case in fixture["journeys"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut state = NativeSecondaryScreenFeedbackPresentation::new(
            case["initialTick"].as_u64().unwrap(),
            "boneyard:test".into(),
        );
        let context = NativeSecondaryScreenFeedbackContext {
            camera_center: point(&case["context"]["cameraCenter"]),
            local_player_alternate: case["context"]["localPlayerAlternate"].as_bool().unwrap(),
            visible_world_width: case["context"]["visibleWorldWidth"].as_f64().unwrap(),
        };
        for (index, sample) in case["samples"].as_array().unwrap().iter().enumerate() {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                apply_action(&mut state, &sample["action"], context)
            }));
            if let Some(expected_error) = sample["error"].as_str() {
                let error = result.expect_err("source contract rejection must also reject in Rust");
                let message = error
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| error.downcast_ref::<&str>().copied())
                    .unwrap();
                assert!(message.contains(expected_error), "{name} {index}: {message}");
            } else {
                assert_snapshot(
                    &result.unwrap(),
                    &sample["result"],
                    &format!("{name} {index} output"),
                );
            }
            assert_snapshot(
                &feedback_snapshot(&state),
                &sample["state"],
                &format!("{name} {index} state"),
            );
        }
    }
    for case in fixture["gains"].as_array().unwrap() {
        let position = point(&case["position"]);
        let center = point(&case["center"]);
        let width = case["width"].as_f64().unwrap();
        let alternate = case["alternate"].as_bool().unwrap();
        assert_eq!(
            native_region_point_gain(position, center, width, alternate),
            case["point"].as_f64().unwrap()
        );
        assert_eq!(
            native_region_hit_point_gain(position, center, width, alternate),
            case["hit"].as_f64().unwrap()
        );
    }
    for case in fixture["overlays"].as_array().unwrap() {
        let actual = present_native_secondary_screen_overlay(
            read_overlay(&case["overlay"]),
            case["reduced"].as_bool().unwrap(),
        );
        assert_snapshot(&overlay_snapshot(actual), &case["result"], "reduced flash");
    }
    for case in fixture["shakes"].as_array().unwrap() {
        let actors = case["actors"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let mut actor = NativeSecondaryActorState::new(
                    index as u64,
                    if value["kind"] == "earthquake" {
                        NativeSecondaryActorKind::Earthquake
                    } else {
                        NativeSecondaryActorKind::Golem
                    },
                    "player".into(),
                    value["worldKey"].as_str().unwrap().into(),
                    None,
                );
                actor.velocity = point(&value["velocity"]);
                actor
            })
            .collect::<Vec<_>>();
        let actual =
            native_secondary_world_shake(&actors, "boneyard:test", point(&case["current"]));
        assert_snapshot(&point_snapshot(actual), &case["result"], "earthquake arbitration");
    }
}
