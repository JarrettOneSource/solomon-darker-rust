use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::input::joystick_vector;
use solomon_sim::mobile_ui::{
    MobileUiSize, mobile_ui_joystick_input_radius, mobile_ui_joystick_knob_offset,
};

#[test]
fn saved_rotation_changes_the_knob_art_without_rotating_gameplay_input() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/mobile-joystick-goldens.json"))
            .unwrap();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let bounds = &case["bounds"];
        let width = bounds["width"].as_f64().unwrap();
        let height = bounds["height"].as_f64().unwrap();
        let center = Vec2::new(
            bounds["left"].as_f64().unwrap() + width / 2.0,
            bounds["top"].as_f64().unwrap() + height / 2.0,
        );
        let point =
            |value: &Value| Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap());
        let rotation = if case["customized"].as_bool().unwrap() {
            case["rotation"].as_f64().unwrap()
        } else {
            0.0
        };
        let movement = joystick_vector(
            point(&case["pointer"]),
            center,
            mobile_ui_joystick_input_radius(MobileUiSize { width, height }, rotation),
        );
        let knob = mobile_ui_joystick_knob_offset(
            movement,
            MobileUiSize { width: 238.0, height: 238.0 },
            rotation,
        );
        // These two source calculations use platform trigonometry; gameplay direction
        // and its zero/held admission stay unchanged.
        for (name, actual, expected) in [
            ("movement", movement, point(&case["movement"])),
            ("knob", knob, point(&case["knobOffset"])),
        ] {
            assert!(
                (actual.x - expected.x).abs() <= 1e-9 && (actual.y - expected.y).abs() <= 1e-9,
                "{name} case {index}: {actual:?} != {expected:?}"
            );
        }
    }
}
