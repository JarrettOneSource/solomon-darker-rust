//! Authored Create reveal/hand curves, title grave rows, pause fixed-tick fades.
use serde_json::Value;
use solomon_sim::input::{self, MenuDirection, MenuGamepadButtons};
use solomon_sim::menu::{CreateHandPose, CreateMotion};
use solomon_sim::profile::{WizardDiscipline, WizardElement};
use solomon_sim::settings::{GameControlAction, GameControlBindings};
use solomon_sim::{Vec2, menu};

fn number(value: &Value) -> f64 {
    value.as_f64().expect("fixture number")
}

fn direction(value: &Value) -> Option<MenuDirection> {
    match value.as_str() {
        Some("up") => Some(MenuDirection::Up),
        Some("down") => Some(MenuDirection::Down),
        Some("left") => Some(MenuDirection::Left),
        Some("right") => Some(MenuDirection::Right),
        None => None,
        name => panic!("unknown fixture menu direction {name:?}"),
    }
}

#[test]
fn menu_gamepad_directions_repeat_and_spatial_focus_match_the_website() {
    let golden: Value = serde_json::from_str(include_str!("fixtures/website/menu-goldens.json"))
        .expect("menu fixture");
    for case in golden["navPads"].as_array().expect("navigation pads") {
        let pressed =
            |index| case["buttons"].as_array().expect("buttons").contains(&Value::from(index));
        let actual = input::read_menu_gamepad(
            Vec2::new(number(&case["x"]), number(&case["y"])),
            MenuGamepadButtons {
                back: pressed(1),
                confirm: pressed(0),
                next: pressed(5),
                previous: pressed(4),
                up: pressed(12),
                down: pressed(13),
                left: pressed(14),
                right: pressed(15),
            },
        );
        let expected = &case["state"];
        assert_eq!(actual.direction, direction(&expected["direction"]));
        for (actual, name) in [
            (actual.back, "back"),
            (actual.confirm, "confirm"),
            (actual.next, "next"),
            (actual.previous, "previous"),
        ] {
            assert_eq!(actual, expected[name].as_bool().expect("navigation button"));
        }
    }
    for case in golden["repeats"].as_array().expect("navigation repeat") {
        let args = &case["args"];
        let actual = input::advance_menu_direction_repeat(
            direction(&args[0]),
            direction(&args[1]),
            number(&args[2]),
            number(&args[3]),
        );
        assert_eq!(actual.0, case["value"]["move"].as_bool().expect("repeat move"));
        assert_eq!(actual.1, number(&case["value"]["nextRepeatAt"]));
    }
    let centers: Vec<_> = golden["centers"]
        .as_array()
        .expect("focus centers")
        .iter()
        .map(|point| Vec2::new(number(&point[0]), number(&point[1])))
        .collect();
    for case in golden["spatial"].as_array().expect("spatial cases") {
        let actual = input::choose_spatial_menu_target(
            0,
            &centers,
            direction(&case["direction"]).expect("focus direction"),
        );
        assert_eq!(actual.map(|index| index as u64), case["selected"].as_u64());
    }
    for case in golden["bindings"].as_array().expect("binding cases") {
        let mut controls = GameControlBindings::default();
        controls.rebind(
            GameControlAction::ALL[case["action"].as_u64().expect("binding action") as usize],
            case["code"].as_str().expect("binding code"),
        );
        let actual = GameControlAction::ALL.map(|action| controls.code(action));
        let expected: Vec<_> = case["values"]
            .as_array()
            .expect("binding values")
            .iter()
            .map(|value| value.as_str().expect("key code"))
            .collect();
        assert_eq!(actual.as_slice(), expected);
    }
}
fn vector(value: &Value) -> Vec2 {
    Vec2::new(number(&value["x"]), number(&value["y"]))
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() <= 1e-9, "{actual} != {expected}");
}
fn pose(value: &Value) -> CreateHandPose {
    match value.as_str().expect("hand pose") {
        "fist" => CreateHandPose::Fist,
        "cupped" => CreateHandPose::Cupped,
        "raised" => CreateHandPose::Raised,
        name => panic!("unknown hand pose {name}"),
    }
}
fn motion(actual: CreateMotion, expected: &Value) {
    assert_eq!(actual.left_pose, pose(&expected["leftPose"]));
    assert_eq!(actual.right_pose, pose(&expected["rightPose"]));
    assert_eq!(
        actual.elements_visible,
        expected["elementsVisible"].as_bool().expect("elements visible")
    );
    assert_eq!(
        actual.disciplines_visible,
        expected["disciplinesVisible"].as_bool().expect("disciplines visible")
    );
    assert_eq!(actual.settled, expected["settled"].as_bool().expect("settled"));
    // Impulses contain trigonometry; the rest of the fixed-point recurrence is exact.
    for (actual, name) in
        [(actual.left_impulse, "leftImpulse"), (actual.right_impulse, "rightImpulse")]
    {
        let expected = vector(&expected[name]);
        close(actual.x, expected.x);
        close(actual.y, expected.y);
    }
    assert_eq!(actual.left_offset, vector(&expected["leftOffset"]));
    assert_eq!(actual.right_offset, vector(&expected["rightOffset"]));
}

#[test]
fn create_reveal_curves_title_rows_and_pause_fades_match_the_website() {
    let golden: Value = serde_json::from_str(include_str!("fixtures/website/menu-goldens.json"))
        .expect("menu fixture");
    for case in golden["createCases"].as_array().expect("Create cases") {
        let elapsed = number(&case["elapsed"]);
        motion(menu::create_entry_motion_at(elapsed), &case["entry"]);
        motion(menu::create_selection_motion_at(elapsed), &case["selection"]);
        let idle = menu::create_hand_idle_offset_at(elapsed);
        close(idle.x, number(&case["idle"]["x"]));
        close(idle.y, number(&case["idle"]["y"]));
        for case in case["elements"].as_array().expect("elements") {
            let element = WizardElement::ALL
                .into_iter()
                .find(|element| element.as_str() == case["element"].as_str().expect("element"))
                .expect("known element");
            let actual = menu::create_element_reveal_motion_at(element, elapsed);
            assert_eq!(actual.opacity, number(&case["reveal"]["opacity"]));
            assert_eq!(actual.position, vector(&case["reveal"]["position"]));
            let actual = menu::create_selected_element_motion_at(element, elapsed);
            assert_eq!(actual.position, vector(&case["selected"]["position"]));
            assert_eq!(actual.scale, number(&case["selected"]["scale"]));
        }
        for case in case["disciplines"].as_array().expect("disciplines") {
            let discipline = WizardDiscipline::ALL
                .into_iter()
                .find(|discipline| {
                    discipline.as_str() == case["discipline"].as_str().expect("discipline")
                })
                .expect("known discipline");
            assert_eq!(
                menu::create_discipline_reveal_motion_at(discipline, elapsed),
                vector(&case["position"])
            );
        }
    }
    let widths: [f64; 9] = std::array::from_fn(|index| number(&golden["titleWidths"][index]));
    let mut title = menu::TitleState::new(widths);
    for case in golden["titleCases"].as_array().expect("title cases") {
        while title.ticks < case["ticks"].as_u64().expect("title ticks") {
            title.step();
        }
        for (actual, expected) in
            title.rows.iter().zip(case["rows"].as_array().expect("grave rows"))
        {
            assert_eq!(actual.baseline, number(&expected["baseline"]));
            assert_eq!(actual.gray, number(&expected["gray"]));
            assert_eq!(actual.scale, number(&expected["scale"]));
            assert_eq!(
                actual.next_image_index as u64,
                expected["nextImageIndex"].as_u64().expect("next grave")
            );
            let graves = expected["graves"].as_array().expect("graves");
            assert_eq!(actual.graves.len(), graves.len());
            for (actual, expected) in actual.graves.iter().zip(graves) {
                assert_eq!(
                    actual.image_index as u64,
                    expected["imageIndex"].as_u64().expect("grave image")
                );
                assert_eq!(actual.rotation, number(&expected["rotation"]));
                assert_eq!(actual.x, number(&expected["x"]));
            }
        }
        let offsets = menu::title_backdrop_offsets_at(title.ticks as f64 / 60.0);
        assert_eq!(
            offsets,
            ["cloudDetail", "cloudShadow", "grass", "horizon"]
                .map(|key| number(&case["offsets"][key]))
        );
    }
    for case in golden["pauseCases"].as_array().expect("pause cases") {
        assert_eq!(
            menu::native_pause_menu_reveal(true, number(&case["elapsed"])),
            number(&case["opening"])
        );
        assert_eq!(
            menu::native_pause_menu_reveal(false, number(&case["elapsed"])),
            number(&case["closing"])
        );
    }
}
