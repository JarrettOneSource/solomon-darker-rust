//! Current public save interchange, including genuine previously supported documents.

use serde_json::Value;
use solomon_assets::website_save::{self, CreateSaveOptions};

#[path = "../../sim/tests/support/mod.rs"]
mod support;

#[test]
fn current_and_historical_continuations_reencode_without_losing_owned_state() {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!(
        "../../sim/tests/fixtures/website/current-save-continuation-goldens.json.zlib"
    ))
    .unwrap();
    let manifest: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/current-save-continuation-goldens.manifest.json"
    ))
    .unwrap();
    use sha2::{Digest, Sha256};
    assert_eq!(bytes.len() as u64, manifest["bytes"].as_u64().unwrap());
    let digest =
        Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    assert_eq!(digest, manifest["sha256"].as_str().unwrap());
    let fixture: Value = serde_json::from_slice(&bytes).unwrap();
    let mut failures = Vec::new();
    for case in fixture["cases"].as_array().unwrap() {
        if std::panic::catch_unwind(|| check_case(case)).is_err() {
            failures.push(case["name"].as_str().unwrap());
        }
    }
    assert!(failures.is_empty(), "continuation cases failed: {failures:?}");
}

fn check_case(case: &Value) {
    let name = case["name"].as_str().unwrap();
    let document = case["document"].to_string();
    let result = website_save::restore(&document);
    if case["accepted"] == false {
        assert_eq!(result.unwrap_err(), case["error"].as_str().unwrap(), "{name}");
        return;
    }
    let restored = result.unwrap_or_else(|error| panic!("{name}: {error}"));
    let reencoded = website_save::create(CreateSaveOptions {
        state: &restored.session,
        player_id: &restored.player_id,
        loaded_boneyard: restored.loaded_boneyard.as_ref(),
        metadata: &restored.metadata,
        party_rejoin_token: None,
    })
    .unwrap_or_else(|error| panic!("{name} encode: {error}"));
    let student_count =
        case["reencoded"]["continuation"]["simulation"]["world"]["studentPopulation"]["students"]
            .as_array()
            .map_or(0, Vec::len);
    let trig_paths: Vec<String> = (0..student_count)
        .flat_map(|index| {
            [
                "wander.x",
                "wander.y",
                "position.x",
                "position.y",
                "framePhase",
                "gaitDegrees",
                "profile.pushResistance",
            ]
            .map(|field| format!(".world.studentPopulation.students[{index}].{field}"))
        })
        .collect();
    support::assert_website_json(
        &serde_json::from_str(&reencoded).unwrap(),
        &case["reencoded"],
        name,
        &trig_paths.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let retired: Value = serde_json::from_str(&website_save::retire(&document).unwrap()).unwrap();
    support::assert_website_json(&retired, &case["retired"], &format!("{name} retired"), &[]);
}
