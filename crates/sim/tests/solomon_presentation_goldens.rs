//! The actual encounter and retained Solomon view are sampled together by the oracle.

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::SolomonDig;
use solomon_sim::boneyard::encounter::{
    SolomonDigCue, SolomonDigEvent, SolomonEncounter, SolomonPlayer,
};
use solomon_sim::boneyard::solomon_presentation::*;
use solomon_sim::painter::{WorldManagerLane, WorldPainterOrder};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn clip_json(clip: Option<SolomonClipRect>) -> Value {
    clip.map_or(
        Value::Null,
        |clip| json!({ "x": clip.x, "y": clip.y, "width": clip.width, "height": clip.height }),
    )
}

fn visual_json(visual: SolomonVisualState) -> Value {
    json!({
        "bodyBank": visual.body_bank.id(), "bodyPose": visual.body_pose,
        "clipRectWorld": clip_json(visual.clip_rect_world), "direction": visual.direction,
        "mouthPose": visual.mouth_pose, "nativeBodyRecord": visual.native_body_record,
        "nativeMouthRecord": visual.native_mouth_record, "offsetY": visual.offset_y,
        "graveMarkVisible": visual.grave_mark_visible, "visible": visual.visible,
    })
}

fn dirt_json(state: NativeSolomonDirtState) -> Value {
    json!({ "ageTicks": state.age_ticks, "alpha": state.alpha,
        "headingDegrees": state.heading_degrees,
        "position": { "x": state.position.x, "y": state.position.y }, "speed": state.speed })
}

fn compare(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                compare(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} keys");
            for (key, value) in expected {
                compare(&actual[key], value, &format!("{path}.{key}"));
            }
        }
        (Value::Number(actual), Value::Number(expected)) => {
            let actual = actual.as_f64().unwrap();
            let expected = expected.as_f64().unwrap();
            // The encounter's escape heading uses platform trigonometry. All frame,
            // tint and fround dirt samples remain exact; only its world mask may drift.
            if path.ends_with(".x") || path.ends_with(".y") || path.ends_with(".worldY") {
                assert!((actual - expected).abs() <= 1e-9, "{path}: {actual} != {expected}");
            } else {
                assert_eq!(actual, expected, "{path}");
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

fn event(value: &Value) -> SolomonDigEvent {
    let cue = match value["cue"].as_str().unwrap() {
        "shovel-1" => SolomonDigCue::Shovel1,
        "shovel-2" => SolomonDigCue::Shovel2,
        "throw-dirt-1" => SolomonDigCue::ThrowDirt1,
        "throw-dirt-2" => SolomonDigCue::ThrowDirt2,
        cue => panic!("unknown Solomon cue {cue}"),
    };
    SolomonDigEvent {
        cue,
        id: value["id"].as_u64().unwrap(),
        tick: value["tick"].as_u64().unwrap(),
    }
}

#[test]
fn solomon_body_masks_painters_and_retained_dirt_match_the_website_view() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/solomon-presentation-goldens.json"))
            .unwrap();
    let input = &fixture["dig"];
    let dig = SolomonDig {
        position: point(&input["position"]),
        grave_position: point(&input["gravePosition"]),
        lantern_position: point(&input["lanternPosition"]),
        frame_program: serde_json::from_value(input["frameProgram"].clone()).unwrap(),
        ticks_per_frame: input["ticksPerFrame"].as_u64().unwrap(),
    };
    for record in fixture["directions"].as_array().unwrap() {
        assert_eq!(
            native_solomon_direction(record["heading"].as_f64().unwrap()),
            record["direction"].as_u64().unwrap() as usize
        );
    }
    for record in fixture["dirtStates"].as_array().unwrap() {
        let state = native_solomon_dirt_state_at(
            point(&record["position"]),
            record["age"].as_u64().unwrap(),
        );
        // These motions narrow every trigonometric component before storing it.
        if record["expected"].is_null() {
            assert!(state.is_none());
        } else {
            let expected = &record["expected"];
            assert_eq!(
                state.unwrap(),
                NativeSolomonDirtState {
                    age_ticks: expected["ageTicks"].as_u64().unwrap(),
                    alpha: expected["alpha"].as_f64().unwrap(),
                    heading_degrees: expected["headingDegrees"].as_f64().unwrap(),
                    position: point(&expected["position"]),
                    speed: expected["speed"].as_f64().unwrap(),
                }
            );
        }
    }
    for record in fixture["eventDeltas"].as_array().unwrap() {
        let events: Vec<_> = record["events"].as_array().unwrap().iter().map(event).collect();
        let (id, delta) = native_solomon_dirt_event_delta(record["last"].as_u64(), &events);
        assert_eq!(id, record["expected"]["eventId"].as_u64().unwrap());
        let expected: Vec<_> =
            record["expected"]["events"].as_array().unwrap().iter().map(event).collect();
        assert_eq!(delta, expected);
    }
    for record in fixture["fallback"].as_array().unwrap() {
        let state = boneyard_solomon_visual_state(None, &dig, record["tick"].as_u64().unwrap());
        assert_eq!(state.native_body_record as u64, record["frame"].as_u64().unwrap());
        assert_eq!(state.offset_y, record["offsetY"].as_f64().unwrap());
        compare(&clip_json(state.clip_rect_world), &record["clip"], "fallback clip");
    }
    let lighting =
        native_solomon_set_piece_lighting(dig.position, dig.lantern_position, &|position| {
            if position == dig.lantern_position { 0x9f9f9f } else { 0x3f3f3f }
        });
    assert_eq!(
        json!({ "bodyTint": lighting.body_tint, "dirtTint": lighting.dirt_tint,
        "lanternTint": lighting.lantern_tint }),
        fixture["lighting"]
    );

    let mut order = WorldPainterOrder { next_actor_ordinal: 40, next_transient_ordinal: 70 };
    let lantern_registration = order.register(WorldManagerLane::Actor);
    let actor_registration = order.register(WorldManagerLane::Actor);
    for scenario in fixture["scenarios"].as_array().unwrap() {
        let seed = scenario["seed"].as_str().unwrap();
        let mut encounter = if scenario["mode"] == "tutorial" {
            SolomonEncounter::new_tutorial(
                &dig,
                seed,
                scenario["tutorialDialogueTicks"].as_u64().unwrap(),
            )
        } else {
            SolomonEncounter::new(&dig, seed)
        };
        let mut paint = SolomonDirtPaintState::default();
        let records = scenario["records"].as_array().unwrap();
        let mut record_index = 0;
        let initial_tick = scenario["initialPaintTick"].as_u64().unwrap();
        let paint_every = scenario["paintEvery"].as_u64().unwrap();
        for tick in 0..=scenario["totalTicks"].as_u64().unwrap() {
            if tick > 0 {
                let players = if scenario["contact"].as_bool().unwrap() {
                    vec![SolomonPlayer {
                        id: "local",
                        position: Vec2::new(dig.position.x + 30.0, dig.position.y + 50.0),
                    }]
                } else {
                    Vec::new()
                };
                encounter.step(&players, tick);
            }
            if tick < initial_tick || (tick - initial_tick) % paint_every != 0 {
                continue;
            }
            paint.prepare(Some(&encounter), tick);
            let visual = boneyard_solomon_visual_state(Some(&encounter), &dig, tick);
            let lantern = if tick % 150 == 0 { None } else { Some(dig.lantern_position) };
            let painters = boneyard_solomon_painters(
                &dig,
                Some(&encounter),
                lantern_registration,
                actor_registration,
                lantern,
            );
            if !records
                .get(record_index)
                .is_some_and(|record| record["tick"].as_u64() == Some(tick))
            {
                continue;
            }
            let record = &records[record_index];
            let path = format!("{seed} tick {tick}");
            compare(&visual_json(visual), &record["visual"], &path);
            let painters: Vec<_> = painters.iter().map(|painter| json!({
                "id": match painter.kind { SolomonPainterKind::Lantern => "lantern", SolomonPainterKind::Actor => "solomon-actor" },
                "queueFamily": "ordinary-dynamic", "sortBias": 0,
                "registration": { "managerLane": "actor", "registrationOrdinal": painter.registration.registration_ordinal },
                "worldY": painter.world_y,
            })).collect();
            compare(&json!(painters), &record["painters"], &format!("{path}.painters"));
            let dirt = paint.particles().last().map_or(Value::Null, |particle| {
                json!({
                    "eventId": particle.event_id, "state": dirt_json(particle.state),
                })
            });
            compare(&dirt, &record["dirt"], &format!("{path}.dirt"));
            assert_eq!(
                paint.particles().len() as u64,
                record["dirtCount"].as_u64().unwrap(),
                "{path}"
            );
            assert_eq!(
                (paint.particles().len() * SOLOMON_DIRT_DRAW_PASSES) as u64,
                record["dirtPassCount"].as_u64().unwrap(),
                "{path}"
            );
            assert_eq!(
                visual.visible && visual.grave_mark_visible,
                record["graveMarkPassCount"] == 1,
                "{path}"
            );
            record_index += 1;
        }
        assert_eq!(record_index, records.len());
        paint.prepare(None, 0);
        assert!(paint.particles().is_empty());
    }
}
