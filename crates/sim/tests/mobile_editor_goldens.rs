use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::belt::{NativeBeltEntry, PlayerBeltComponent};
use solomon_sim::mobile_ui::*;

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(f64::NAN)
}
fn point(value: &Value) -> Vec2 {
    Vec2::new(number(&value["x"]), number(&value["y"]))
}
fn size(value: &Value) -> MobileUiSize {
    MobileUiSize { width: number(&value["width"]), height: number(&value["height"]) }
}
fn transform(value: &Value) -> MobileUiElementTransform {
    MobileUiElementTransform {
        x: number(&value["x"]),
        y: number(&value["y"]),
        scale: number(&value["scale"]),
        rotation: number(&value["rotation"]),
    }
}
fn siblings(value: &Value) -> Vec<MobileUiSnapRect> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|value| MobileUiSnapRect {
            bottom: number(&value["bottom"]),
            center_x: number(&value["centerX"]),
            center_y: number(&value["centerY"]),
            left: number(&value["left"]),
            right: number(&value["right"]),
            top: number(&value["top"]),
        })
        .collect()
}
fn assert_number(actual: f64, expected: f64, tolerance: f64, context: &str) {
    if tolerance == 0.0 {
        assert_eq!(actual, expected, "{context}");
    } else {
        assert!((actual - expected).abs() <= tolerance, "{context}: {actual} != {expected}");
    }
}
fn assert_transform(
    actual: MobileUiElementTransform,
    expected: &Value,
    tolerance: f64,
    context: &str,
) {
    let expected = transform(expected);
    for (name, actual, expected) in [
        ("x", actual.x, expected.x),
        ("y", actual.y, expected.y),
        ("scale", actual.scale, expected.scale),
        ("rotation", actual.rotation, expected.rotation),
    ] {
        assert_number(actual, expected, tolerance, &format!("{context} {name}"));
    }
}
fn assert_snap(actual: MobileUiSnapResult, expected: &Value, tolerance: f64, context: &str) {
    assert_transform(actual.transform, &expected["transform"], tolerance, context);
    let guides = expected["guides"].as_array().unwrap();
    assert_eq!(actual.guides.len(), guides.len(), "{context} guide count");
    for (actual, expected) in actual.guides.iter().zip(guides) {
        assert_eq!(actual.axis.as_str(), expected["axis"].as_str().unwrap(), "{context} axis");
        assert_eq!(
            actual.kind.as_str(),
            expected["kind"].as_str().unwrap(),
            "{context} guide priority"
        );
        assert_number(actual.position, number(&expected["position"]), tolerance, context);
    }
}

#[test]
fn editor_edges_snap_priorities_and_all_eight_resize_anchors_match_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/mobile-editor-goldens.json")).unwrap();
    for (index, case) in fixture["frames"].as_array().unwrap().iter().enumerate() {
        let context = format!("frame {index}");
        let input = transform(&case["transform"]);
        let natural = size(&case["naturalSize"]);
        let page = size(&case["page"]);
        let siblings = siblings(&case["siblings"]);
        let threshold = number(&case["threshold"]);
        // Only rotated geometry uses a platform-trigonometry tolerance. Grid priorities,
        // guide ordering, unrotated arithmetic, normalization and admission remain exact.
        let tolerance = if input.rotation == 0.0 { 0.0 } else { 1e-9 };
        assert_transform(
            constrain_mobile_ui_transform(input, natural, page),
            &case["constrained"],
            tolerance,
            &context,
        );
        let frame = mobile_ui_element_snap_rect(input, natural, page);
        for (name, actual) in [
            ("bottom", frame.bottom),
            ("centerX", frame.center_x),
            ("centerY", frame.center_y),
            ("left", frame.left),
            ("right", frame.right),
            ("top", frame.top),
        ] {
            assert_number(
                actual,
                number(&case["frame"][name]),
                tolerance,
                &format!("{context} {name}"),
            );
        }
        assert_snap(
            snap_mobile_ui_move(input, natural, page, &siblings, threshold),
            &case["move"],
            tolerance,
            &context,
        );
        assert_snap(
            snap_mobile_ui_scale(input, natural, page, &siblings, threshold),
            &case["scale"],
            tolerance,
            &context,
        );
    }
    for (index, case) in fixture["resizes"].as_array().unwrap().iter().enumerate() {
        let context = format!("resize {index}");
        let initial = transform(&case["transform"]);
        let natural_size = size(&case["naturalSize"]);
        let page = size(&case["page"]);
        let handle = MobileUiResizeHandle::from_id(case["handle"].as_str().unwrap()).unwrap();
        let initial_pointer = point(&case["initialPointer"]);
        let current_pointer = point(&case["currentPointer"]);
        let tolerance = if initial.rotation == 0.0 { 0.0 } else { 1e-9 };
        assert_transform(
            mobile_ui_resize_transform(
                initial,
                natural_size,
                page,
                handle,
                initial_pointer,
                current_pointer,
            ),
            &case["resized"],
            tolerance,
            &context,
        );
        assert_snap(
            snap_mobile_ui_resize(
                MobileUiResizeGesture {
                    initial,
                    natural_size,
                    page,
                    handle,
                    initial_pointer,
                    current_pointer,
                },
                &siblings(&case["siblings"]),
                number(&case["threshold"]),
            ),
            &case["snapped"],
            tolerance,
            &context,
        );
    }
    for case in fixture["pinches"].as_array().unwrap() {
        let initial = number(&case["initial"]);
        let from = number(&case["from"]);
        let to = number(&case["to"]);
        assert_eq!(
            mobile_ui_element_pinch_scale(initial, from, to),
            number(&case["scale"]),
            "pinch {case}"
        );
        assert_eq!(
            mobile_ui_page_pinch_zoom(initial, from, to),
            number(&case["zoom"]),
            "zoom {case}"
        );
    }
    for case in fixture["rotations"].as_array().unwrap() {
        assert_eq!(
            mobile_ui_element_rotation(
                number(&case["initial"]),
                number(&case["from"]),
                number(&case["to"]),
                case["snap"].as_bool().unwrap()
            ),
            number(&case["expected"]),
            "rotation {case}"
        );
    }
    for case in fixture["pageSizes"].as_array().unwrap() {
        assert_eq!(
            mobile_ui_editor_page_size(
                number(&case["width"]),
                number(&case["height"]),
                case["coarse"].as_bool().unwrap()
            ),
            size(&case["expected"]),
            "editor page {case}"
        );
    }
    for case in fixture["belts"].as_array().unwrap() {
        let mut belt = PlayerBeltComponent::from_entries(&[None; 8]).expect("legacy editor belt");
        for (index, entry) in case["entries"].as_array().unwrap().iter().enumerate() {
            belt.entries[index] = match entry["kind"].as_str() {
                Some("health-potion") => Some(NativeBeltEntry::HealthPotion),
                Some("mana-potion") => Some(NativeBeltEntry::ManaPotion),
                Some("skill") => Some(NativeBeltEntry::Skill {
                    skill_id: entry["skillId"].as_u64().unwrap() as usize,
                }),
                None => None,
                _ => panic!("unknown fixture belt entry {entry}"),
            };
        }
        for slot in 0..8 {
            assert_eq!(
                mobile_ui_belt_element_id(&belt, slot).as_str(),
                case["expected"][slot].as_str().unwrap(),
                "belt {slot} {case}"
            );
        }
    }
}
