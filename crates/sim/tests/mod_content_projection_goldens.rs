//! Public Source codec records, including actual view identity and icon relationships.
#![cfg(feature = "persistence")]

use serde_json::Value;
use solomon_sim::modding::projection::ModContentProjection;
use solomon_sim::modding::{LuaObject, LuaValue};

#[test]
fn mod_content_projection_matches_source_codec() {
    let golden: Value =
        serde_json::from_str(include_str!("fixtures/website/mod-content-projection-goldens.json"))
            .unwrap();
    for case in golden["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let input = lua(&case["input"]);
        let decoded = ModContentProjection::read(input.as_object().unwrap());
        if let Some(error) = case["error"].as_str() {
            assert_eq!(decoded.unwrap_err(), error, "{name}");
        } else {
            let decoded = decoded.unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_source_values(&serde_json::to_value(decoded).unwrap(), &case["output"], name);
        }
    }
}

fn lua(value: &Value) -> LuaValue {
    match value {
        Value::Null => LuaValue::Null,
        Value::Bool(value) => LuaValue::Boolean(*value),
        Value::Number(value) => LuaValue::Number(value.as_f64().unwrap()),
        Value::String(value) => LuaValue::String(value.clone()),
        Value::Array(values) => LuaValue::Array(values.iter().map(lua).collect()),
        Value::Object(values) => LuaValue::Object(
            values.iter().map(|(key, value)| (key.clone(), lua(value))).collect::<LuaObject>(),
        ),
    }
}

fn assert_source_values(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(actual.as_f64(), expected.as_f64(), "{path}");
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_source_values(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (key, expected) in expected {
                assert_source_values(actual.get(key).unwrap(), expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}
