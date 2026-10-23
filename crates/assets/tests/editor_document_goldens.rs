//! Complete lossless editor documents, including preserve-only records and raw bytes.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;
use solomon_assets::boneyard::parse_editor_boneyard;

fn same_value(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(e)) => {
            assert_eq!(a.as_f64().unwrap().to_bits(), e.as_f64().unwrap().to_bits(), "{path}")
        }
        (Value::Array(a), Value::Array(e)) => {
            assert_eq!(a.len(), e.len(), "{path}.length");
            for (index, (a, e)) in a.iter().zip(e).enumerate() {
                same_value(a, e, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(a), Value::Object(e)) => {
            assert_eq!(a.keys().collect::<Vec<_>>(), e.keys().collect::<Vec<_>>(), "{path}.keys");
            for (key, a) in a {
                same_value(a, &e[key], &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}
#[test]
fn native_import_keeps_the_complete_editor_document_and_opaque_native_sections() {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!(
        "../../sim/tests/fixtures/website/editor-document-goldens.json.zlib"
    ))
    .unwrap();
    let fixture: Value = serde_json::from_slice(&bytes).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let input = STANDARD.decode(case["bytes"].as_str().unwrap()).unwrap();
        let result = parse_editor_boneyard(&input);
        if let Some(error) = case["error"].as_str() {
            assert_eq!(result.unwrap_err(), error, "{}", case["name"]);
        } else {
            same_value(&result.unwrap(), &case["document"], case["name"].as_str().unwrap());
        }
    }
}
