//! Current Account import/export cases recorded from the unmodified Website APIs.

use serde_json::{Value, json};
use solomon_assets::native_save::{
    create_web_game_save_from_portable_profile, export_web_game_save_to_native_archive,
    parse_portable_game_profile,
};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/current-save-tools-goldens.json"
    ))
    .expect("the recorded Account save fixture is valid JSON")
}

fn export_summary(case: &Value) -> Value {
    let mode = case["mode"].as_str().expect("an export case has a mode");
    let document = case["input"].as_str().expect("an export case has a document");
    let exported = export_web_game_save_to_native_archive(document)
        .unwrap_or_else(|error| panic!("{mode}: {error}"));
    assert!(exported.archive.starts_with(b"PK"), "{mode}: a real ZIP archive");
    json!({
        "warnings": exported.warnings,
        "learnedOrder": exported.portable.wizard.learned_order,
        "skillQuickbar": exported.portable.wizard.skill_quickbar,
        "disciplineRoot": exported.portable.wizard.discipline_root,
    })
}

#[test]
fn current_portable_import_keeps_all_eight_discipline_roots() {
    let fixture = fixture();
    for case in fixture["imports"].as_array().expect("recorded imports") {
        let root = &case["root"];
        let input = case["input"].as_str().expect("a portable input document");
        let portable = parse_portable_game_profile(input)
            .unwrap_or_else(|error| panic!("discipline root {root}: {error}"));
        let imported = create_web_game_save_from_portable_profile(&portable)
            .unwrap_or_else(|error| panic!("discipline root {root}: {error}"));
        let character = json!({
            "discipline": imported.character.discipline.as_str(),
            "displayName": imported.character.name,
            "element": imported.character.element.as_str(),
        });
        assert_eq!(character, case["expected"], "discipline root {root}");
    }
}

#[test]
fn current_native_export_warns_when_later_hotbars_are_populated() {
    let fixture = fixture();
    for case in fixture["exports"].as_array().expect("recorded exports") {
        let mode = case["mode"].as_str().expect("an export case has a mode");
        if mode == "equipment-only-visible-row" {
            continue;
        }
        assert_eq!(export_summary(case), case["expected"], "{mode}");
    }
}

#[test]
fn current_native_export_keeps_only_permanent_learned_rows() {
    let fixture = fixture();
    let case = fixture["exports"]
        .as_array()
        .expect("recorded exports")
        .iter()
        .find(|case| case["mode"] == "equipment-only-visible-row")
        .expect("the source equipment-only row witness");
    assert_eq!(export_summary(case), case["expected"]);
}
