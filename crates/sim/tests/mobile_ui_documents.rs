#![cfg(feature = "persistence")]

use std::collections::BTreeMap;

use serde_json::Value;
use solomon_sim::mobile_ui::*;

fn layout(value: &Value) -> MobileUiLayout {
    let map: BTreeMap<MobileUiElementId, MobileUiElementTransform> =
        serde_json::from_value(value.clone()).unwrap();
    MobileUiLayout { elements: MobileUiElementId::ALL.map(|id| map[&id]) }
}

#[test]
fn current_shared_and_previous_local_documents_match_source_admission() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/mobile-ui-document-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let document =
            serde_json::from_value::<MobileUiLayoutDocument>(case["document"].clone()).ok();
        let strict = document.as_ref().and_then(mobile_ui_layout_from_document);
        let expected = (!case["expected"].is_null()).then(|| layout(&case["expected"]));
        assert_eq!(strict, expected, "shared {name}");
        let local = strict.or_else(|| document.as_ref().and_then(legacy_mobile_ui_layout));
        let actual = local.map_or_else(MobileUiLayoutState::default, |layout| {
            MobileUiLayoutState { customized: true, layout }
        });
        assert_eq!(
            actual.customized,
            case["local"]["customized"].as_bool().unwrap(),
            "local {name}"
        );
        assert_eq!(actual.layout, layout(&case["local"]["layout"]), "local {name}");
    }
    for case in fixture["geometries"].as_array().unwrap() {
        let actual = default_mobile_ui_geometry(
            case["width"].as_f64().unwrap(),
            case["height"].as_f64().unwrap(),
            case["scale"].as_f64().unwrap(),
        );
        assert_eq!(actual.layout, layout(&case["expected"]["layout"]), "geometry {case}");
        for id in MobileUiElementId::ALL {
            let expected = &case["expected"]["sizes"][id.as_str()];
            assert_eq!(
                actual.sizes[id as usize],
                MobileUiSize {
                    width: expected["width"].as_f64().unwrap(),
                    height: expected["height"].as_f64().unwrap()
                },
                "size {} {case}",
                id.as_str()
            );
        }
    }
    for case in fixture["normalized"].as_array().unwrap() {
        let input = &case["transform"];
        let number = |name: &str| input[name].as_f64().unwrap_or(f64::NAN);
        let actual = normalize_mobile_ui_transform(MobileUiElementTransform {
            rotation: number("rotation"),
            scale: number("scale"),
            x: number("x"),
            y: number("y"),
        });
        let expected: MobileUiElementTransform =
            serde_json::from_value(case["expected"].clone()).unwrap();
        assert_eq!(actual, expected, "normalize {case}");
        assert_eq!(actual.x.to_bits(), expected.x.to_bits(), "normalized x sign {case}");
        assert_eq!(actual.y.to_bits(), expected.y.to_bits(), "normalized y sign {case}");
    }
}
