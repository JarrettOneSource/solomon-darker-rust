//! The actual packaged policy, complete Source head outputs and its checkpoint boundary.

#[path = "../../sim/tests/support/mod.rs"]
mod support;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use solomon_assets::ml_bot_checkpoint::{
    decode_ml_bot_policy_checkpoint, encode_ml_bot_policy_checkpoint,
};
use solomon_sim::ml_bot::{
    MlBotPolicyActionMaskPlan, MlBotPolicyActionMasks, MlBotPolicyHeadResult,
    MlBotPolicySelectionMode, MlBotPolicySelectionOptions,
};
use solomon_sim::modding::LuaValue;

const CHECKPOINT: &[u8] =
    include_bytes!("../../../web/assets/native/ml-bot-policy-v7-selected.sdml");
const FIXTURE: &str =
    include_str!("../../sim/tests/fixtures/website/ml-bot-inference-goldens.json");

fn sha256(bytes: &[u8]) -> String {
    let mut result = String::new();
    for byte in Sha256::digest(bytes) {
        use std::fmt::Write;
        write!(result, "{byte:02x}").unwrap();
    }
    result
}

fn with_header(header: &LuaValue) -> Vec<u8> {
    let old_header_length = u32::from_le_bytes(CHECKPOINT[8..12].try_into().unwrap()) as usize;
    let header = solomon_assets::json_value::stringify(header).unwrap();
    let mut bytes = CHECKPOINT[..8].to_vec();
    bytes.extend_from_slice(&(header.len() as u32).to_le_bytes());
    bytes.extend_from_slice(header.as_bytes());
    bytes.extend_from_slice(&CHECKPOINT[12 + old_header_length..]);
    bytes
}

#[test]
fn actual_deployed_checkpoint_and_source_rejections_match() {
    let fixture: LuaValue = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(sha256(CHECKPOINT), fixture.get("modelSha256").unwrap().as_str().unwrap());
    let checkpoint = decode_ml_bot_policy_checkpoint(CHECKPOINT).unwrap();
    assert_eq!(
        sha256(&encode_ml_bot_policy_checkpoint(&checkpoint).unwrap()),
        fixture.get("sourceEncodedSha256").unwrap().as_str().unwrap(),
    );
    for case in fixture.get("errors").unwrap().as_array().unwrap() {
        let name = case.get("name").unwrap().as_str().unwrap();
        let result = decode_ml_bot_policy_checkpoint(&with_header(case.get("header").unwrap()));
        if let Some(error) = case.get("error").and_then(LuaValue::as_str) {
            assert_eq!(result.unwrap_err(), error, "{name}");
        } else {
            let encoded = encode_ml_bot_policy_checkpoint(&result.unwrap()).unwrap();
            assert_eq!(
                sha256(&encoded),
                case.get("encodedSha256").unwrap().as_str().unwrap(),
                "{name}"
            );
        }
    }
    for case in fixture.get("malformed").unwrap().as_array().unwrap() {
        let name = case.get("name").unwrap().as_str().unwrap();
        let mut bytes = CHECKPOINT.to_vec();
        match name {
            "magic" => bytes[0] = 0,
            "prefix" => bytes.truncate(11),
            "header truncated" => {
                let length = bytes.len() as u32;
                bytes[8..12].copy_from_slice(&length.to_le_bytes());
            }
            "header JSON" => bytes[12] = 0xff,
            "tensor NaN" => {
                let header_length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
                bytes[12 + header_length..16 + header_length]
                    .copy_from_slice(&0x7fc0_0000_u32.to_le_bytes());
            }
            "trailing payload" => bytes.push(0),
            _ => panic!("unknown malformed Source case {name}"),
        }
        assert_eq!(
            decode_ml_bot_policy_checkpoint(&bytes).unwrap_err(),
            case.get("error").unwrap().as_str().unwrap(),
            "{name}"
        );
    }
}

fn floats(value: &Value) -> Vec<f32> {
    value.as_array().unwrap().iter().map(|value| value.as_f64().unwrap() as f32).collect()
}

fn mask(value: &Value) -> Vec<u8> {
    value.as_array().unwrap().iter().map(|value| value.as_u64().unwrap() as u8).collect()
}

fn masks(value: &Value) -> MlBotPolicyActionMasks {
    MlBotPolicyActionMasks {
        ability: mask(&value["ability"]),
        aim: mask(&value["aim"]),
        movement: mask(&value["movement"]),
        target: mask(&value["target"]),
    }
}

fn options(value: &Value) -> MlBotPolicySelectionOptions {
    MlBotPolicySelectionOptions {
        mode: if value["mode"] == "sample" {
            MlBotPolicySelectionMode::Sample
        } else {
            MlBotPolicySelectionMode::Argmax
        },
        temperature: value.get("temperature").and_then(Value::as_f64),
    }
}

fn result(value: &MlBotPolicyHeadResult) -> Value {
    json!({
        "actions": { "ability": value.actions.ability, "aim": value.actions.aim, "movement": value.actions.movement, "target": value.actions.target },
        "latent": value.latent,
        "logProbability": value.log_probability,
        "probabilities": {"ability": value.probabilities.ability, "aim": value.probabilities.aim, "movement": value.probabilities.movement, "target": value.probabilities.target},
        "value": value.value,
    })
}

#[test]
fn actual_checkpoint_heads_and_autoregressive_masks_match_source_float32_outputs() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let runtime = decode_ml_bot_policy_checkpoint(CHECKPOINT).unwrap().into_runtime().unwrap();
    for (index, case) in fixture["inferences"].as_array().unwrap().iter().enumerate() {
        let observation = floats(&case["observation"]);
        let draws: Vec<_> =
            case["draws"].as_array().unwrap().iter().map(|value| value.as_f64().unwrap()).collect();
        let mut draw_index = 0;
        let mut random = || {
            let value = draws[draw_index];
            draw_index += 1;
            value
        };
        let actual = if case["kind"] == "infer" {
            result(
                &runtime
                    .infer(
                        &observation,
                        &masks(&case["masks"]),
                        options(&case["options"]),
                        Some(&mut random),
                    )
                    .unwrap(),
            )
        } else {
            let plan = &case["plan"];
            let plan = MlBotPolicyActionMaskPlan {
                ability_by_target: plan["abilityByTarget"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(mask)
                    .collect(),
                aim_by_ability: plan["aimByAbility"].as_array().unwrap().iter().map(mask).collect(),
                movement: mask(&plan["movement"]),
                target: mask(&plan["target"]),
            };
            let value = runtime
                .infer_autoregressive(
                    &observation,
                    &plan,
                    options(&case["options"]),
                    Some(&mut random),
                )
                .unwrap();
            let mut output = result(&value.head);
            output["masks"] = json!({ "ability": value.masks.ability, "aim": value.masks.aim, "movement": value.masks.movement, "target": value.masks.target });
            output
        };
        support::assert_website_json(&actual, &case["output"], &format!("inference[{index}]"), &[]);
    }
}

#[test]
fn actual_checkpoint_choice_descriptors_temperature_and_sampling_match() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let runtime = decode_ml_bot_policy_checkpoint(CHECKPOINT).unwrap().into_runtime().unwrap();
    for (index, case) in fixture["choices"].as_array().unwrap().iter().enumerate() {
        let mut random = || case["draw"].as_f64().unwrap();
        let value = runtime
            .choose(
                &floats(&case["observation"]),
                &floats(&case["descriptors"]),
                &mask(&case["mask"]),
                options(&case["options"]),
                Some(&mut random),
            )
            .unwrap();
        let actual = json!({ "latent": value.latent, "logProbability": value.log_probability, "probabilities": value.probabilities, "selectedOption": value.selected_option, "value": value.value });
        support::assert_website_json(&actual, &case["output"], &format!("choice[{index}]"), &[]);
    }
}
