//! The continuous player angle survives body-bank quantization, cast holds and idle ticks.

use serde::Deserialize;
use solomon_sim::hub::plan_hub_wizard_tick;
use solomon_sim::{HubWizard, Vec2};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Goldens {
    source_revision: String,
    facings: Vec<Facing>,
    frames: Vec<Frame>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Facing {
    heading: f64,
    heading_degrees: f64,
    heading_index: usize,
}

#[derive(Deserialize)]
struct Frame {
    tick: u64,
    movement: Point,
    casting: bool,
    face: bool,
    expected: [f64; 4],
}

#[derive(Deserialize)]
struct Point {
    x: f64,
    y: f64,
}

#[test]
fn current_player_heading_stores_the_source_angle_before_selecting_the_body_bank() {
    let fixture = include_str!("fixtures/website/current-player-heading-goldens.json");
    let goldens: Goldens = serde_json::from_str(fixture).unwrap();
    assert_eq!(goldens.source_revision, "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    let mut wizard = HubWizard::at(Vec2::ZERO);
    for case in goldens.facings {
        wizard.set_facing(case.heading);
        assert_eq!(wizard.heading_degrees, case.heading_degrees, "input{} angle", case.heading);
        assert_eq!(wizard.heading_index, case.heading_index, "input{} bank", case.heading);
    }
    wizard = HubWizard::at(Vec2::ZERO);
    for frame in goldens.frames {
        wizard.primary_cast.action_tick = frame.casting.then_some(3.0);
        let mut plan = plan_hub_wizard_tick(
            wizard.velocity,
            Vec2::new(frame.movement.x, frame.movement.y),
            1.0,
        );
        plan.face = frame.face;
        wizard.commit(
            &plan,
            Vec2::new(wizard.position.x + plan.delta.x, wizard.position.y + plan.delta.y),
        );
        assert_eq!(
            [
                wizard.heading_degrees,
                wizard.heading_index as f64,
                wizard.position.x,
                wizard.position.y
            ],
            frame.expected,
            "tick{}",
            frame.tick
        );
    }
}
