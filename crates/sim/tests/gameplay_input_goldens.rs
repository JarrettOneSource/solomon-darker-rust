use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::hud::HudRect;
use solomon_sim::input::{
    BeltInputSource, GameplayAction, HeldBeltInputs, StandardGamepadSnapshot,
    StandardGameplayState, project_native_stick_aim, project_native_world_pointer,
};

#[test]
fn held_sources_release_independently_and_gamepad_sampling_does_not_steal_priority() {
    let mut belt = HeldBeltInputs::default();
    assert!(belt.hold(BeltInputSource::Keyboard(2), 2));
    belt.sync_gamepad(Some(0));
    assert!(belt.hold(BeltInputSource::Touch(4), 4));
    belt.sync_gamepad(Some(0));
    assert_eq!(belt.current(), Some(4));
    assert!(!belt.hold(BeltInputSource::Touch(4), 4));
    belt.release(BeltInputSource::Touch(4), 4);
    assert_eq!(belt.current(), Some(0));
    belt.sync_gamepad(None);
    assert_eq!(belt.current(), Some(2));
    belt.release(BeltInputSource::Keyboard(2), 2);
    assert_eq!(belt.current(), None);
}

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn projected(point: Option<Vec2>) -> Value {
    point.map_or(Value::Null, |point| json!({"x":point.x,"y":point.y}))
}

// JSON distinguishes integer and floating-point syntax; the browser's Number does not.
fn numeric_values(value: Value) -> Value {
    match value {
        Value::Number(number) => json!(number.as_f64().unwrap()),
        Value::Array(values) => Value::Array(values.into_iter().map(numeric_values).collect()),
        Value::Object(values) => Value::Object(
            values.into_iter().map(|(key, value)| (key, numeric_values(value))).collect(),
        ),
        other => other,
    }
}

#[test]
fn standard_pad_selection_edges_held_belt_and_modal_neutrality_match_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/gameplay-input-goldens.json")).unwrap();
    let mut state = StandardGameplayState::default();
    for (index, step) in fixture["trace"].as_array().unwrap().iter().enumerate() {
        if step["clear"] == true {
            state.clear(true);
            continue;
        }
        let mut pads = Vec::new();
        for source in step["gamepads"].as_array().unwrap() {
            let axes = source["axes"].as_array().unwrap();
            let mut buttons = [false; 16];
            for index in source["buttons"].as_array().unwrap() {
                buttons[index.as_u64().unwrap() as usize] = true;
            }
            pads.push(StandardGamepadSnapshot {
                index: source["index"].as_u64().unwrap() as u32,
                connected: true,
                standard: true,
                left: Vec2::new(axes[0].as_f64().unwrap(), axes[1].as_f64().unwrap()),
                right: Vec2::new(axes[2].as_f64().unwrap(), axes[3].as_f64().unwrap()),
                buttons,
            });
        }
        let sample = state.sample(&pads);
        let actions: Vec<_> = sample
            .actions
            .iter()
            .map(|action| match action {
                GameplayAction::Interact => "interact",
                GameplayAction::Inventory => "inventory",
                GameplayAction::Skills => "skills",
                GameplayAction::Pause => "pause",
                GameplayAction::Chat => "chat",
            })
            .collect();
        let actual = json!({"actions":actions,"aimActive":sample.aim_active,"aimDirection":projected(sample.aim_direction),"gamepadIndex":sample.gamepad_index,
            "primary":sample.primary,"quickbar":sample.quickbar,"quickbarSelectionChanged":sample.quickbar_selection_changed,"selectedQuickbarSlot":sample.selected_quickbar_slot});
        assert_eq!(
            numeric_values(actual),
            numeric_values(step["expected"].clone()),
            "trace step {index}"
        );
    }
}

#[test]
fn native_pointer_and_stick_aim_use_the_actual_viewport_and_player_anchor() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/gameplay-input-goldens.json")).unwrap();
    for case in fixture["pointers"].as_array().unwrap() {
        let a = &case["args"];
        let surface = HudRect::new(
            a[1]["left"].as_f64().unwrap(),
            a[1]["top"].as_f64().unwrap(),
            a[1]["width"].as_f64().unwrap(),
            a[1]["height"].as_f64().unwrap(),
        );
        let viewport = Vec2::new(a[2]["width"].as_f64().unwrap(), a[2]["height"].as_f64().unwrap());
        assert_eq!(
            numeric_values(projected(project_native_world_pointer(
                point(&a[0]),
                surface,
                viewport,
                point(&a[3]),
                a[4].as_f64().unwrap()
            ))),
            numeric_values(case["expected"].clone())
        );
    }
    for case in fixture["sticks"].as_array().unwrap() {
        let a = &case["args"];
        let viewport = Vec2::new(a[2]["width"].as_f64().unwrap(), a[2]["height"].as_f64().unwrap());
        assert_eq!(
            numeric_values(projected(project_native_stick_aim(
                point(&a[0]),
                point(&a[1]),
                viewport,
                a[3].as_f64().unwrap()
            ))),
            numeric_values(case["expected"].clone())
        );
    }
}
