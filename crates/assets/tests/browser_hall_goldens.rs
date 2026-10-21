use serde_json::Value;
use solomon_assets::browser_hall;

fn text(value: &Value) -> Option<&str> {
    if value.is_null() { None } else { Some(value.as_str().unwrap()) }
}

#[test]
fn existing_hall_documents_merge_and_record_like_current_website() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/browser-hall-goldens.json"
    ))
    .unwrap();
    assert_eq!(browser_hall::STORAGE_KEY, fixture["storageKey"].as_str().unwrap());
    assert_eq!(browser_hall::LEGACY_STORAGE_KEY, fixture["legacyStorageKey"].as_str().unwrap());
    for case in fixture["reads"].as_array().unwrap() {
        let entries = browser_hall::read(text(&case["current"]), text(&case["legacy"]));
        let serialized = browser_hall::serialize(&entries).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&serialized).unwrap(),
            case["expected"],
            "read {}",
            case["name"]
        );
    }
    for case in fixture["writes"].as_array().unwrap() {
        let input = format!("[{}]", case["next"]);
        let mut next = browser_hall::read(Some(&input), None);
        assert_eq!(next.len(), 1, "fixture next row {}", case["name"]);
        let entries =
            browser_hall::record(next.remove(0), text(&case["current"]), text(&case["legacy"]));
        let serialized = browser_hall::serialize(&entries).unwrap();
        assert_eq!(serialized, case["serialized"].as_str().unwrap(), "write {}", case["name"]);
        assert_eq!(
            serde_json::from_str::<Value>(&serialized).unwrap(),
            case["expected"],
            "record {}",
            case["name"]
        );
        assert_eq!(case["legacy"], case["legacyAfter"], "legacy is read-only {}", case["name"]);
    }
}
