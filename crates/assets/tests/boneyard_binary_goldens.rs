//! Actual authored files and binary trust-boundary cases from the unchanged editor.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;
use solomon_assets::boneyard::{boneyard_scene_source_value, decode_boneyard};
use solomon_assets::json_value::stringify;

#[test]
fn authored_binary_scenes_and_ordered_geometry_match_the_source() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/boneyard-binary-goldens.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let bytes = STANDARD.decode(case["bytes"].as_str().unwrap()).unwrap();
        let result = decode_boneyard(&bytes);
        if let Some(error) = case["error"].as_str() {
            assert_eq!(result.unwrap_err(), error, "{}", case["name"]);
            continue;
        }
        let scene = result.unwrap();
        let actual = stringify(&boneyard_scene_source_value(&scene)).unwrap();
        let expected = case["encoded"].as_str().unwrap();
        assert_eq!(actual.len(), expected.len(), "{}.encoded.length", case["name"]);
        if let Some(index) =
            actual.bytes().zip(expected.bytes()).position(|(left, right)| left != right)
        {
            assert_eq!(
                actual.as_bytes()[index],
                expected.as_bytes()[index],
                "{}.encoded[{index}]",
                case["name"]
            );
        }
    }
}
