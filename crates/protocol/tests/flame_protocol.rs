//! Current Source admission of complete retained Flame recipes at snapshot scope.
use serde_json::Value;
use solomon_protocol::flame_presentation::FlamePresentationFrame;

#[test]
fn current_flame_snapshot_codec_preserves_native_recipes_and_rejects_invalid_births() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/flame-protocol-goldens.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let context = &case["context"];
        let result = serde_json::from_value::<FlamePresentationFrame>(case["input"].clone())
            .map_err(|error| error.to_string())
            .and_then(|frame| {
                frame.validate_snapshot(
                    context["worldKey"].as_str().unwrap(),
                    context["worldTick"].as_u64().unwrap(),
                    context["nextPrimaryId"].as_u64().unwrap(),
                )
            });
        assert_eq!(
            result.is_ok(),
            case["accepted"].as_bool().unwrap(),
            "{}: Source={}, Rust={result:?}",
            case["name"],
            case["error"]
        );
    }
}
