use serde_json::{Map, Value};
use solomon_assets::browser_settings;
use solomon_sim::settings::GameSettings;

#[test]
fn browser_settings_read_write_and_historical_documents_match_current_website() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/browser-settings-goldens.json"
    ))
    .unwrap();
    assert_eq!(browser_settings::STORAGE_KEY, fixture["storageKey"].as_str().unwrap());
    for case in fixture["reads"].as_array().unwrap() {
        let actual = browser_settings::parse(case["serialized"].as_str().unwrap());
        let serialized = browser_settings::serialize(&actual).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&serialized).unwrap(),
            case["expected"],
            "read {}",
            case["name"]
        );
    }
    for case in fixture["writes"].as_array().unwrap() {
        let input = native_input(&case["input"]);
        let actual = browser_settings::serialize(&input).unwrap();
        assert_eq!(actual, case["serialized"].as_str().unwrap(), "write {}", case["name"]);
        assert_eq!(
            serde_json::from_str::<Value>(&actual).unwrap(),
            case["normalized"],
            "normalized {}",
            case["name"]
        );
    }
}

// The fixture retains Source's input spelling. This mechanical DTO bridge lets
// write tests supply out-of-range integers and invalid strings to the real Rust
// model without running the read admission being tested in the loop above.
fn native_input(source: &Value) -> GameSettings {
    let mut record = Map::new();
    for (key, value) in source.as_object().unwrap() {
        if key == "controls" {
            let mut controls = Map::new();
            let mut belt = Vec::new();
            for slot in 1..=8 {
                belt.push(value[format!("belt{slot}")].clone());
            }
            controls.insert("belt".into(), Value::Array(belt));
            for (key, value) in value.as_object().unwrap() {
                if !key.starts_with("belt") {
                    controls.insert(snake_case(key), value.clone());
                }
            }
            record.insert("controls".into(), Value::Object(controls));
        } else {
            record.insert(snake_case(key), value.clone());
        }
    }
    serde_json::from_value(Value::Object(record)).unwrap()
}

fn snake_case(source: &str) -> String {
    let mut result = String::new();
    for character in source.chars() {
        if character.is_ascii_uppercase() {
            result.push('_');
        }
        result.push(character.to_ascii_lowercase());
    }
    result
}
