//! The opening Solomon encounter, replayed independently of rendering and collision.

mod support;

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::SolomonDig;
use solomon_sim::boneyard::encounter::{SolomonEncounter, SolomonPlayer};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn snapshot(encounter: &SolomonEncounter) -> Value {
    json!({
        "acceleration": encounter.acceleration,
        "digBodyBobAmplitude": encounter.dig_body_bob_amplitude,
        "digBodyOffsetY": encounter.dig_body_offset_y,
        "digEventId": encounter.dig_event_id,
        "digEvents": encounter.dig_events.iter().map(|event| json!({ "cue": event.cue.id(), "id": event.id, "tick": event.tick })).collect::<Vec<_>>(),
        "digFrame": encounter.dig_frame,
        "digFrameProgram": encounter.dig_frame_program,
        "digPhase": encounter.dig_phase,
        "digShovelArmed": encounter.dig_shovel_armed,
        "digTicksPerFrame": encounter.dig_ticks_per_frame,
        "digThrowDirtArmed": encounter.dig_throw_dirt_armed,
        "dialogueMode": encounter.dialogue_mode.id(),
        "escapeCollisionSourceIds": encounter.escape_collision_source_ids,
        "escapeSpeed": encounter.escape_speed,
        "escapeTarget": encounter.escape_target.map(|position| json!({ "x": position.x, "y": position.y })),
        "headingDeg": encounter.heading_degrees,
        "lifetimeTicksRemaining": encounter.lifetime_ticks_remaining,
        "mouthPose": encounter.mouth_pose,
        "mouthPoseTicksRemaining": encounter.mouth_pose_ticks_remaining,
        "motion": encounter.motion,
        "phase": encounter.phase.id(),
        "phaseTicksRemaining": encounter.phase_ticks_remaining,
        "position": { "x": encounter.position.x, "y": encounter.position.y },
        "queuedGetHimBoys": encounter.queued_get_him_boys,
        "rngState": { "indexA": encounter.rng.index_a, "indexB": encounter.rng.index_b, "words": encounter.rng.words.as_slice() },
        "runEventId": encounter.run_event_id,
        "targetPlayerId": encounter.target_player_id,
        "tutorialDialogueTicks": encounter.tutorial_dialogue_ticks,
        "transitionOffsetY": encounter.transition_offset_y,
        "turnRate": encounter.turn_rate,
        "voiceEvents": encounter.voice_events.iter().map(|event| json!({ "cue": event.cue.id(), "id": event.id })).collect::<Vec<_>>(),
        "voiceTicksRemaining": encounter.voice_ticks_remaining,
        "walkCycle": encounter.walk_cycle,
    })
}

fn compare(actual: &Value, expected: &Value, path: &str) {
    support::assert_website_json(
        actual,
        expected,
        path,
        &["position.x", "position.y", "headingDeg"],
    );
}

#[test]
fn solomon_digs_speaks_releases_the_run_and_escapes_like_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/solomon-encounter-goldens.json"))
            .unwrap();
    for scenario in fixture["scenarios"].as_array().unwrap() {
        let input = &scenario["dig"];
        let dig = SolomonDig {
            grave_position: point(&input["gravePosition"]),
            lantern_position: point(&input["lanternPosition"]),
            position: point(&input["position"]),
            frame_program: serde_json::from_value(input["frameProgram"].clone()).unwrap(),
            ticks_per_frame: input["ticksPerFrame"].as_u64().unwrap(),
        };
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
        let records = scenario["records"].as_array().unwrap();
        compare(&snapshot(&encounter), &records[0]["state"], seed);
        let mut record_index = 1;
        for tick in 1..=scenario["totalTicks"].as_u64().unwrap() {
            let players = if scenario["contact"].as_bool().unwrap() {
                vec![SolomonPlayer {
                    id: "local",
                    position: Vec2::new(dig.position.x + 30.0, dig.position.y + 50.0),
                }]
            } else {
                Vec::new()
            };
            encounter.step(&players, tick);
            if records
                .get(record_index)
                .is_some_and(|record| record["tick"].as_u64().unwrap() == tick)
            {
                compare(
                    &snapshot(&encounter),
                    &records[record_index]["state"],
                    &format!("{seed} tick {tick}"),
                );
                record_index += 1;
            }
        }
        assert_eq!(record_index, records.len());
    }
}
