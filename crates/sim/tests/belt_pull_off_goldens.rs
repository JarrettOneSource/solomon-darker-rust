//! Complete current Source belt pull-off threshold and cosmetic member plans.

use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::hud::belt_pull_off::{
    NativeBeltPullOffStep, native_belt_pull_off_burst_members, native_belt_pull_off_started,
};

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}

fn point(value: &Value) -> Vec2 {
    Vec2::new(number(&value[0]), number(&value[1]))
}

fn trigonometric(actual: f64, expected: &Value) {
    // Only endpoints use sin/cos and geometric travel. This cosmetic plan does
    // not enter gameplay; permit the same last-bit libm difference as other views.
    const TRIG_TOLERANCE: f64 = 1e-10;
    assert!((actual - number(expected)).abs() <= TRIG_TOLERANCE, "{actual} != {expected}");
}

#[test]
fn belt_pull_off_threshold_random_order_and_every_member_match_current_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/belt-pull-off-goldens.json")).unwrap();
    for case in fixture["threshold"].as_array().unwrap() {
        assert_eq!(
            native_belt_pull_off_started(point(&case["origin"]), point(&case["current"])),
            case["expected"].as_bool().unwrap(),
            "{case}",
        );
    }
    for case in fixture["bursts"].as_array().unwrap() {
        let values = case["random"].as_array().unwrap();
        let mut draws = 0;
        let mut random = || {
            let value = number(&values[draws % values.len()]);
            draws += 1;
            value
        };
        let step = match case["step"].as_u64().unwrap() {
            90 => NativeBeltPullOffStep::Degrees90,
            120 => NativeBeltPullOffStep::Degrees120,
            other => panic!("Source branch {other}"),
        };
        let members = native_belt_pull_off_burst_members(step, &mut random);
        assert_eq!(draws, case["draws"].as_u64().unwrap() as usize);
        let expected = case["members"].as_array().unwrap();
        assert_eq!(members.len(), expected.len());
        for (actual, expected) in members.iter().zip(expected) {
            assert_eq!(actual.record, expected["record"].as_u64().unwrap() as usize);
            assert_eq!(actual.brightness, number(&expected["brightness"]));
            assert_eq!(actual.duration_ms, number(&expected["durationMs"]));
            assert_eq!(actual.rotation_degrees, number(&expected["rotationDegrees"]));
            assert_eq!(actual.scale.x, number(&expected["scaleX"]));
            assert_eq!(actual.scale.y, number(&expected["scaleY"]));
            trigonometric(actual.start.x, &expected["startX"]);
            trigonometric(actual.start.y, &expected["startY"]);
            trigonometric(actual.end.x, &expected["endX"]);
            trigonometric(actual.end.y, &expected["endY"]);
        }
    }
}
