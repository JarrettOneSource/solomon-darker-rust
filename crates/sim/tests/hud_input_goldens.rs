//! Website HUD geometry, meter layering and movement-device contracts.

use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::hud::{self, HudHealthLayerKind, HudRect};
use solomon_sim::input::{self, MovementInputDevice, MovementKeys};

fn number(value: &Value) -> f64 {
    value.as_f64().expect("oracle number")
}

fn point(value: &Value) -> Vec2 {
    Vec2::new(number(&value[0]), number(&value[1]))
}

fn rectangle(value: &Value) -> HudRect {
    HudRect::new(
        number(&value["x"]),
        number(&value["y"]),
        number(&value["width"]),
        number(&value["height"]),
    )
}

fn keys(mask: u64) -> MovementKeys {
    MovementKeys {
        left: mask & 1 != 0,
        right: mask & 2 != 0,
        up: mask & 4 != 0,
        down: mask & 8 != 0,
    }
}

fn device(value: &Value) -> MovementInputDevice {
    match value.as_str().expect("oracle device") {
        "none" => MovementInputDevice::None,
        "keyboard" => MovementInputDevice::Keyboard,
        "gamepad" => MovementInputDevice::Gamepad,
        "touch" => MovementInputDevice::Touch,
        other => panic!("unknown oracle device {other}"),
    }
}

#[test]
fn hud_rectangles_meters_and_layers_replay_the_website_exactly() {
    let golden: Value = serde_json::from_str(include_str!("fixtures/website/hud-goldens.json"))
        .expect("HUD fixture");
    for case in golden["layouts"].as_array().expect("layouts") {
        let actual = hud::native_hud_control_layout(
            number(&case["width"]),
            number(&case["height"]),
            case["progress"].as_f64(),
        );
        assert_eq!(actual.backpack, rectangle(&case["layout"]["backpack"]));
        assert_eq!(actual.tome, rectangle(&case["layout"]["tome"]));
        for (index, rect) in actual.belt.iter().enumerate() {
            assert_eq!(*rect, rectangle(&case["layout"]["belt"][index]), "belt slot {index}");
        }
    }
    for case in golden["health"].as_array().expect("health") {
        let args = &case["args"];
        let actual = hud::native_health_hud_presentation(
            number(&args[0]),
            number(&args[1]),
            number(&args[2]),
            number(&args[3]),
        );
        let expected = &case["presentation"];
        assert_eq!(
            [
                actual.core_width,
                actual.fill_progress,
                actual.fill_width,
                actual.shield_progress,
                actual.shield_width,
                actual.track_width
            ],
            [
                "coreWidth",
                "fillProgress",
                "fillWidth",
                "shieldProgress",
                "shieldWidth",
                "trackWidth"
            ]
            .map(|field| number(&expected[field]))
        );
        for layer_case in case["layers"].as_array().expect("layers") {
            let layers = hud::native_health_hud_layers(
                actual.fill_progress,
                actual.shield_progress,
                layer_case["poisoned"].as_bool().expect("poisoned"),
            );
            let expected = layer_case["layers"].as_array().expect("expected layers");
            assert_eq!(layers.len(), expected.len());
            for (actual, expected) in layers.iter().zip(expected) {
                let kind = match actual.kind {
                    HudHealthLayerKind::Health => "health",
                    HudHealthLayerKind::Shield => "shield",
                };
                assert_eq!(kind, expected["kind"].as_str().expect("layer kind"));
                assert_eq!(actual.progress, number(&expected["progress"]));
                assert_eq!(actual.record as u64, expected["record"].as_u64().expect("record"));
            }
        }
    }
    for case in golden["mana"].as_array().expect("mana") {
        let args = &case["args"];
        let actual =
            hud::native_mana_hud_presentation(number(&args[0]), number(&args[1]), number(&args[2]));
        let expected = &case["presentation"];
        assert_eq!(
            [
                actual.core_width,
                actual.fill_progress,
                actual.fill_width,
                actual.reserve_progress,
                actual.reserve_width,
                actual.track_width
            ],
            [
                "coreWidth",
                "fillProgress",
                "fillWidth",
                "reserveProgress",
                "reserveWidth",
                "trackWidth"
            ]
            .map(|field| number(&expected[field]))
        );
    }
}

#[test]
fn movement_dead_zones_keys_and_device_priority_replay_the_website_exactly() {
    let golden: Value =
        serde_json::from_str(include_str!("fixtures/website/movement-input-goldens.json"))
            .expect("movement fixture");
    for case in golden["sticks"].as_array().expect("sticks") {
        let position = point(&case["input"]);
        assert_eq!(
            input::radial_dead_zone(position.x, position.y, 0.2, 0.95),
            point(&case["movement"])
        );
    }
    for case in golden["keyboard"].as_array().expect("keyboard") {
        let keyboard = keys(case["mask"].as_u64().expect("key mask")).movement();
        let sample = input::sample_movement(Vec2::ZERO, Vec2::ZERO, keyboard);
        assert_eq!(sample.movement, point(&case["movement"]));
        assert_eq!(sample.device, device(&case["device"]));
    }
    for case in golden["gamepads"].as_array().expect("gamepads") {
        let pad = &case["gamepad"];
        let pressed = |index| pad["buttons"][index]["pressed"].as_bool().unwrap_or(false);
        let dpad = MovementKeys {
            left: pressed(14),
            right: pressed(15),
            up: pressed(12),
            down: pressed(13),
        };
        let actual = input::movement_from_gamepad(
            point(&pad["axes"]),
            dpad,
            pad["connected"].as_bool().expect("connected"),
            pad["mapping"] == "standard",
        );
        assert_eq!(actual, point(&case["movement"]));
    }
    for case in golden["touch"].as_array().expect("touch") {
        assert_eq!(input::normalize_movement(point(&case["input"])), point(&case["movement"]));
    }
    for case in golden["priorities"].as_array().expect("priorities") {
        let sample = input::sample_movement(
            point(&case["touch"]),
            Vec2::new(0.0, 1.0),
            Vec2::new(-1.0, 0.0),
        );
        assert_eq!(sample.movement, point(&case["movement"]));
        assert_eq!(sample.device, device(&case["device"]));
    }
}
