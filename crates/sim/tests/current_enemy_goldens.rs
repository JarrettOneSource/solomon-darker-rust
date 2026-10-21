//! Current Website enemy behavior, independently recorded after the original pinned port.

use serde::Deserialize;
use solomon_sim::enemies::model::WraithBrain;
use solomon_sim::enemies::wraith::{create_native_wraith_flight_state, step_wraith_flight_clock};

#[path = "current_enemy_goldens/stationary.rs"]
mod stationary;
#[path = "current_enemy_goldens/worlds.rs"]
mod worlds;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurrentEnemyGoldens {
    source_revision: String,
    wraith_clocks: Vec<WraithClock>,
    wraith_worlds: Vec<worlds::WraithWorld>,
    stationary_worlds: Vec<stationary::StationaryWorld>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WraithClock {
    chase_speed: f64,
    resting_sample: f64,
    initial_sample: f64,
    flyby_offset: u64,
    initial: [f64; 7],
    frames: Vec<WraithClockFrame>,
}

#[derive(Deserialize)]
struct WraithClockFrame {
    admitted: bool,
    expected: [f64; 7],
}

fn flight_values(brain: &WraithBrain) -> [f64; 7] {
    [
        brain.base_flyby_speed,
        brain.contact_cooldown_ticks as f64,
        brain.current_speed,
        brain.current_turn_gain,
        brain.flyby_ticks_remaining as f64,
        brain.resting_speed,
        brain.target_turn_gain,
    ]
}

#[test]
fn current_wraith_cruise_and_visibility_clocks_match_the_independent_source() {
    let fixture = include_str!("fixtures/website/current-enemy-goldens.json");
    let goldens: CurrentEnemyGoldens = serde_json::from_str(fixture).unwrap();
    assert_eq!(goldens.source_revision, "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for (case_index, case) in goldens.wraith_clocks.iter().enumerate() {
        let mut brain = create_native_wraith_flight_state(
            case.chase_speed,
            case.resting_sample,
            case.initial_sample,
            case.flyby_offset,
        );
        assert_eq!(flight_values(&brain), case.initial, "Wraith {case_index} constructor");
        for (tick, frame) in case.frames.iter().enumerate() {
            step_wraith_flight_clock(&mut brain, frame.admitted);
            assert_eq!(flight_values(&brain), frame.expected, "Wraith {case_index} tick {tick}");
        }
    }
}
