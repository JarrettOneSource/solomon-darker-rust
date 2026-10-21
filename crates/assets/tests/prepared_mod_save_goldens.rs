//! Source save rows retain per-mod partitioning, shared counters and exact JSON order.

use serde::Deserialize;
use serde_json::Value;
use solomon_assets::json_value;
use solomon_assets::prepared_mod_save::{
    decode_prepared_mod_save_state, encode_prepared_mod_save_state,
};
use solomon_sim::modding::definition::CompiledWebLuaMod;
use solomon_sim::modding::runtime::PreparedModHostCheckpoint;
use solomon_sim::modding::{LuaObject, LuaValue};

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    name: String,
    operation: String,
    value: LuaValue,
    mods: Vec<CompiledWebLuaMod>,
    output: Option<Value>,
    output_json: Option<String>,
    reencoded_json: Option<String>,
    error: Option<String>,
}

fn assert_value(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(left), Value::Number(right)) => {
            assert_eq!(left.as_f64(), right.as_f64(), "{path}");
        }
        (Value::Array(left), Value::Array(right)) => {
            assert_eq!(left.len(), right.len(), "{path}");
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                assert_value(left, right, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(left), Value::Object(right)) => {
            assert_eq!(left.len(), right.len(), "{path}");
            for (key, expected) in right {
                let path = format!("{path}.{key}");
                assert_value(
                    left.get(key).unwrap_or_else(|| panic!("missing {path}")),
                    expected,
                    &path,
                );
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn prepared_mod_saves_match_all_ten_source_domains_ordering_and_validation() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/prepared-mod-save-goldens.json"
    ))
    .unwrap();
    for case in fixture.cases {
        let result: Result<(Value, String), String> = match case.operation.as_str() {
            "encode" => {
                let checkpoint: PreparedModHostCheckpoint =
                    serde_json::from_str(&json_value::stringify(&case.value).unwrap()).unwrap();
                encode_prepared_mod_save_state(&case.mods, &checkpoint).map(|state| {
                    let text = json_value::stringify(&LuaValue::Object(state)).unwrap();
                    (serde_json::from_str(&text).unwrap(), text)
                })
            }
            "decode" => {
                let state: &LuaObject = case.value.as_object().unwrap();
                decode_prepared_mod_save_state(&case.mods, state).map(|checkpoint| {
                    let state = encode_prepared_mod_save_state(&case.mods, &checkpoint).unwrap();
                    let text = json_value::stringify(&LuaValue::Object(state)).unwrap();
                    (serde_json::to_value(checkpoint).unwrap(), text)
                })
            }
            operation => panic!("unknown Source operation {operation}"),
        };
        match result {
            Ok((value, text)) => {
                assert!(case.error.is_none(), "{}: Source rejected this input", case.name);
                assert_value(&value, case.output.as_ref().unwrap(), &case.name);
                // Object insertion order is observable at this save boundary.
                // Compare complete strings, including all nested LuaObject values.
                assert_eq!(
                    text,
                    case.output_json.or(case.reencoded_json).unwrap(),
                    "{}",
                    case.name
                );
            }
            Err(error) => assert_eq!(Some(error), case.error, "{}", case.name),
        }
    }
}
