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
        &current_document_expectation(&case["reencoded"]),
        name,
        &trig_paths.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let retired: Value = serde_json::from_str(&website_save::retire(&document).unwrap()).unwrap();
    support::assert_website_json(
        &retired,
        &current_document_expectation(&case["retired"]),
        &format!("{name} retired"),
        &[],
    );
}

fn current_document_expectation(recorded: &Value) -> Value {
    let mut expected = recorded.clone();
    assert_eq!(expected["schemaVersion"], 51);
    expected["schemaVersion"] = serde_json::json!(website_save::SCHEMA_VERSION);
    let Some(actors) = expected
        .pointer_mut("/continuation/simulation/primarySpells/transients")
        .and_then(Value::as_array_mut)
    else {
        return expected;
    };
    for actor in actors {
        if actor["kind"] != "weld-persistent" || actor["buildId"] != 1006 {
            continue;
        }
        // These four frozen actors predate schema 52. Held rocks receive the
        // native release speed; existing flight retains its recorded velocity
        // magnitude. Their directions are south and south plus ten degrees.
        // No saved motion, identity, damage, RNG or other field is discarded.
        let (speed, heading, reacquires) = match (actor["phase"].as_str(), actor["id"].as_u64()) {
            (Some("held"), Some(1)) => (2.25, 180.0, true),
            (Some("flight"), Some(1)) => (3.3000001907348633, 180.0, false),
            (Some("flight"), Some(2)) => (2.999999931662434, 190.0, false),
            _ => panic!("unqualified recorded Boulder"),
        };
        let fields = actor.as_object_mut().unwrap();
        let clock = fields.remove("lifetimeTicksRemaining").unwrap();
        let turn = fields.remove("speedFactor").unwrap();
        for (key, value) in [
            ("flightSpeed", serde_json::json!(speed)),
            ("headingDegrees", serde_json::json!(heading)),
            ("reacquiresTarget", serde_json::json!(reacquires)),
            ("targetId", Value::Null),
            ("trackingTicksRemaining", clock),
            ("turnAccumulator", serde_json::json!(0.01_f32 as f64)),
            ("turnInput", turn),
        ] {
            assert!(fields.insert(key.into(), value).is_none());
        }
    }
    expected
}
