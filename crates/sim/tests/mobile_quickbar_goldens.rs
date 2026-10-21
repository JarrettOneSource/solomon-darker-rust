//! Every source bank fit/yield/floor branch and all eight mirrored placements.
use serde_json::Value;
use solomon_sim::hud::mobile::{
    MobileQuickbarBank, mobile_quickbar_bank_layout, mobile_quickbar_slot_placement,
};

#[test]
fn touch_banks_and_all_eight_slot_positions_match_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/mobile-quickbar-goldens.json"))
            .expect("mobile quickbar fixture");
    let number = |value: &Value| value.as_f64().expect("fixture number");
    for case in fixture["cases"].as_array().expect("bank cases") {
        let bank = mobile_quickbar_bank_layout(number(&case["width"]), number(&case["scale"]));
        assert_eq!(bank.inset, number(&case["bank"]["inset"]));
        assert_eq!(bank.size, number(&case["bank"]["size"]));
        for (slot, expected) in case["slots"].as_array().expect("eight slots").iter().enumerate() {
            let actual = mobile_quickbar_slot_placement(slot, bank);
            assert_eq!(actual.inset, number(&expected["inset"]));
            assert_eq!(actual.bottom, number(&expected["bottom"]));
            assert_eq!(actual.size, number(&expected["size"]));
            assert_eq!(
                actual.bank,
                if expected["bank"] == "left" {
                    MobileQuickbarBank::Left
                } else {
                    MobileQuickbarBank::Right
                }
            );
        }
    }
}
