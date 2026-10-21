use serde_json::Value;
use solomon_sim::belt::NATIVE_BELT_SLOT_COUNT;
use solomon_sim::input::{BeltInputSource, HeldBeltInputs};

#[test]
fn physical_bindings_keep_their_press_time_bank_until_release() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/hotbar-input-goldens.json")).unwrap();
    for trace in fixture["traces"].as_array().unwrap() {
        let mut bank = trace["initialBank"].as_u64().unwrap() as usize;
        let mut belt = HeldBeltInputs::default();
        let mut gamepad_pressed = false;
        for step in trace["steps"].as_array().unwrap() {
            let kind = step["type"].as_str().unwrap();
            if kind == "bank" {
                bank = step["bank"].as_u64().unwrap() as usize;
            } else {
                let slot = step["slot"].as_u64().unwrap_or(0) as usize;
                let source = match kind {
                    "key" => BeltInputSource::Keyboard(slot),
                    "mouse" => BeltInputSource::Mouse(0),
                    "touch" => BeltInputSource::Touch(slot),
                    "gamepad" => BeltInputSource::Gamepad,
                    other => panic!("unknown physical source {other}"),
                };
                let pressed = step["pressed"].as_bool().unwrap();
                if kind == "gamepad" {
                    if gamepad_pressed != pressed {
                        belt.sync_gamepad(pressed.then_some(bank * NATIVE_BELT_SLOT_COUNT));
                    }
                    gamepad_pressed = pressed;
                } else if !pressed {
                    belt.release_source(source);
                } else if belt.held_slot(source).is_none() {
                    belt.hold(source, bank * NATIVE_BELT_SLOT_COUNT + slot);
                }
            }
            let expected = step["expected"].as_u64().map(|slot| slot as usize);
            assert_eq!(belt.current(), expected, "bank {bank}, step {step}");
        }
    }
}
