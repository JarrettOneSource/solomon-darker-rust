use serde_json::Value;
use solomon_sim::hud::quickbar::*;

#[test]
fn quickbar_global_cooldowns_and_square_sectors_match_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/skill-quickbar-goldens.json")).unwrap();
    for case in fixture["cooldowns"].as_array().unwrap() {
        let actual = native_skill_quickbar_cooldown_presentation(
            case["row"].as_f64().unwrap(),
            case["capacity"].as_f64().unwrap(),
            case["global"].as_f64().unwrap(),
        );
        assert_eq!(
            actual.capacity,
            case["expected"]["capacity"].as_f64().unwrap(),
            "capacity {case}"
        );
        assert_eq!(
            actual.remaining,
            case["expected"]["remaining"].as_f64().unwrap(),
            "remaining {case}"
        );
    }
    for case in fixture["paths"].as_array().unwrap() {
        assert_eq!(
            native_cooldown_sector_path(
                case["remaining"].as_f64().unwrap(),
                case["capacity"].as_f64().unwrap()
            ),
            case["expected"].as_str().unwrap(),
            "sector {case}"
        );
    }
    for case in fixture["alpha"].as_array().unwrap() {
        assert_eq!(
            native_skill_quickbar_icon_alpha(
                case["cooldown"].as_bool().unwrap(),
                case["unavailable"].as_bool().unwrap()
            ),
            case["expected"].as_f64().unwrap(),
            "alpha {case}"
        );
    }
    for case in fixture["bindings"].as_array().unwrap() {
        let actual = layout_native_quickbar_binding(case["text"].as_str().unwrap());
        assert_eq!(actual.advance, case["advance"].as_f64().unwrap());
        assert_eq!(actual.backing_left, case["backingLeft"].as_f64().unwrap());
        assert_eq!(actual.backing_width, case["backingWidth"].as_f64().unwrap());
    }
}
