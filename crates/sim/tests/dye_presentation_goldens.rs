use serde_json::Value;
use solomon_sim::player_panels::dye::{
    native_dye_modal_opacity, native_dye_selected_pulse, native_dye_swatch_rect,
};

#[test]
fn fabric_dye_palette_and_open_close_pulse_frames_match_current_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/dye-presentation-goldens.json"))
            .unwrap();
    for case in fixture["swatches"].as_array().unwrap() {
        let rect = native_dye_swatch_rect(case["index"].as_u64().unwrap() as usize);
        for (actual, expected) in [rect.x, rect.y, rect.width, rect.height]
            .into_iter()
            .zip(case["expected"].as_array().unwrap())
        {
            assert_eq!(actual, expected.as_f64().unwrap());
        }
    }
    for case in fixture["opacity"].as_array().unwrap() {
        let actual = native_dye_modal_opacity(
            case["openedAt"].as_f64().unwrap(),
            case["closingAt"].as_f64(),
            case["now"].as_f64().unwrap(),
        );
        assert_eq!(actual, case["expected"].as_f64().unwrap(), "{case}");
    }
    for case in fixture["pulse"].as_array().unwrap() {
        let actual =
            native_dye_selected_pulse(case["selectedAt"].as_f64(), case["now"].as_f64().unwrap());
        assert_eq!(actual, case["expected"].as_f64().unwrap(), "{case}");
    }
}
