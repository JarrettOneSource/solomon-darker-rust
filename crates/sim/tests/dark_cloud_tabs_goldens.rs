use serde_json::Value;
use solomon_sim::menu::dark_cloud::*;

#[test]
fn five_cloud_routes_preserve_source_bands_and_scrollable_minimum_scale() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/dark-cloud-tabs-goldens.json"))
            .unwrap();
    assert_eq!(DARK_CLOUD_TAB_STRIP_WIDTH, fixture["strip"]["width"].as_f64().unwrap());
    assert_eq!(DARK_CLOUD_TAB_STRIP_HEIGHT, fixture["strip"]["height"].as_f64().unwrap());
    for (tab, expected) in DarkCloudTab::ALL.into_iter().zip(fixture["tabs"].as_array().unwrap()) {
        assert_eq!(tab.id(), expected["id"]);
        assert_eq!(tab.label(), expected["label"]);
        assert_eq!(DarkCloudTab::from_id(tab.id()), Some(tab));
    }
    assert_eq!(DarkCloudTab::from_id("unknown"), None);
    for case in fixture["frames"].as_array().unwrap() {
        let actual = native_dark_cloud_tab_strip(
            case["width"].as_f64().unwrap(),
            case["height"].as_f64().unwrap(),
        );
        let expected = &case["expected"];
        assert_eq!(actual.width, expected["width"].as_f64().unwrap(), "{case}");
        assert_eq!(actual.height, expected["height"].as_f64().unwrap(), "{case}");
        assert_eq!(actual.scale, expected["scale"].as_f64().unwrap(), "{case}");
        for (tab, expected) in
            DarkCloudTab::ALL.into_iter().zip(expected["tabs"].as_array().unwrap())
        {
            let bounds = actual.bounds(tab);
            assert_eq!(bounds.x, expected["bounds"]["left"].as_f64().unwrap(), "{case}");
            assert_eq!(bounds.y, expected["bounds"]["top"].as_f64().unwrap(), "{case}");
            assert_eq!(bounds.width, expected["bounds"]["width"].as_f64().unwrap(), "{case}");
            assert_eq!(bounds.height, expected["bounds"]["height"].as_f64().unwrap(), "{case}");
        }
    }
}
