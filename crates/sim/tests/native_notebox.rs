#[cfg(feature = "persistence")]
#[test]
fn source_native_notebox_layout_and_lifetime() {
    use solomon_sim::ui_notebox::{
        NoteboxKind, NoteboxLayout, notebox_duration_ms, notebox_layout, notebox_opacity,
    };
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/website/native-notebox-goldens.json")).unwrap();
    for case in fixture["layouts"].as_array().unwrap() {
        let expected: NoteboxLayout = serde_json::from_value(case["expected"].clone()).unwrap();
        assert_eq!(notebox_layout(case["text"].as_str().unwrap()), expected);
    }
    for case in fixture["frames"].as_array().unwrap() {
        let kind =
            if case["kind"] == "failure" { NoteboxKind::Failure } else { NoteboxKind::Instruction };
        assert_eq!(notebox_duration_ms(kind), case["duration"].as_f64().unwrap());
        assert_eq!(
            notebox_opacity(kind, case["elapsed"].as_f64().unwrap(), case["dismissed"].as_f64()),
            case["opacity"].as_f64().unwrap()
        );
    }
}
