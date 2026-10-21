use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::boneyard::lighting::*;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn source(value: &Value) -> NativeBoneyardLightSource {
    NativeBoneyardLightSource::new(
        point(&value["position"]),
        value["radius"].as_f64().unwrap(),
        value["intensity"].as_f64().unwrap(),
        false,
    )
}
fn factors(actual: NativeBoneyardLightFactors, expected: &Value) {
    assert_eq!(actual.radial, expected["radial"].as_f64().unwrap());
    assert_eq!(actual.elevated, expected["elevated"].as_f64().unwrap());
}

#[test]
fn current_pc24_camera_relative_light_lanes_match_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-light-query-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let position = point(&case["position"]);
        let query = NativeBoneyardLightQuery {
            camera_origin: case["query"].get("cameraOrigin").map_or(Vec2::ZERO, point),
            ambient: case["query"]["ambient"].as_f64().unwrap(),
        };
        if !case["source"].is_null() {
            factors(
                native_boneyard_source_light_factors(position, source(&case["source"]), query),
                &case["factors"],
            );
        }
        let sources: Vec<_> = case["sources"].as_array().unwrap().iter().map(source).collect();
        let actual = native_boneyard_light_factors(position, &sources, query);
        factors(actual, &case["combined"]);
        assert_eq!(actual.radial, case["scalar"].as_f64().unwrap());
        assert_eq!(
            native_boneyard_surface_light_scalar_with_query(position, &sources, query),
            case["surface"].as_f64().unwrap()
        );
    }
    for case in fixture["tints"].as_array().unwrap() {
        assert_eq!(
            native_boneyard_light_tint(case["scalar"].as_f64().unwrap()),
            case["tint"].as_u64().unwrap() as u32
        );
    }
}
