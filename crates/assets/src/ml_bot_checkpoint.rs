//! `.sdml` checkpoint boundary from Website `ml-bot-policy/checkpoint.ts`.
//!
//! The deployed policy is data, not a training/runtime dependency. Metadata keeps its
//! Source property order and unknown fields so re-encoding follows JSON.stringify.

use std::collections::BTreeSet;

use serde::Serialize;
use solomon_sim::ml_bot::{
    ARCHITECTURE, MODEL_FORMAT, MlBotPolicyChoiceMode, MlBotPolicyRuntime, MlBotPolicyTensors,
    TENSOR_SPECS,
};
use solomon_sim::modding::LuaValue;

const MAGIC: [u8; 8] = [0x53, 0x44, 0x4d, 0x4c, 0x56, 0x37, 0, 1];
const PREFIX_BYTES: usize = 12;
const SPEC_JSON: &str = include_str!("../data/ml-bot-policy-v7-spec.json");

#[derive(Clone, Debug)]
pub struct MlBotPolicyCheckpoint {
    pub metadata: LuaValue,
    pub tensors: MlBotPolicyTensors,
}

impl MlBotPolicyCheckpoint {
    pub fn validate(&self) -> Result<(), String> {
        validate_metadata(&self.metadata)?;
        self.tensors.validate()
    }

    pub fn into_runtime(self) -> Result<MlBotPolicyRuntime, String> {
        validate_metadata(&self.metadata)?;
        let mode = if field(&self.metadata, "choicePolicyMode").as_str() == Some("learned") {
            MlBotPolicyChoiceMode::Learned
        } else {
            MlBotPolicyChoiceMode::Scripted
        };
        let temperature = field(&self.metadata, "choiceTemperature")
            .as_number()
            .expect("metadata validation admits a finite temperature");
        MlBotPolicyRuntime::new(self.tensors, mode, temperature)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TensorDescriptor<'a> {
    byte_length: usize,
    byte_offset: usize,
    name: &'a str,
    shape: &'a [usize],
}

#[derive(Serialize)]
struct EncodedHeader<'a> {
    metadata: &'a LuaValue,
    tensors: Vec<TensorDescriptor<'a>>,
}

struct DecodedDescriptor {
    byte_length: u64,
    byte_offset: u64,
    name: String,
    shape: Vec<u64>,
}

pub fn decode_ml_bot_policy_checkpoint(encoded: &[u8]) -> Result<MlBotPolicyCheckpoint, String> {
    if encoded.len() < PREFIX_BYTES || encoded[..MAGIC.len()] != MAGIC {
        return Err("ML bot policy checkpoint magic is invalid".into());
    }
    let length = u32::from_le_bytes(encoded[8..12].try_into().unwrap());
    let payload_start = u64::from(length) + PREFIX_BYTES as u64;
    if payload_start > encoded.len() as u64 {
        return Err("ML bot policy checkpoint header is truncated".into());
    }
    let payload_start = payload_start as usize;
    // Source TextDecoder replaces malformed UTF-8 and consumes an initial BOM.
    let header_text = String::from_utf8_lossy(&encoded[PREFIX_BYTES..payload_start]);
    let header_text = header_text.strip_prefix('\u{feff}').unwrap_or(&header_text);
    let header: LuaValue = serde_json::from_str(header_text)
        .map_err(|_| "ML bot policy checkpoint header is invalid JSON")?;
    if !is_object(&header) {
        return Err("ML bot policy checkpoint header must be an object".into());
    }
    let metadata = field(&header, "metadata");
    let descriptors = field(&header, "tensors");
    if !is_object(metadata) || descriptors.as_array().is_none() {
        return Err("ML bot policy checkpoint header is incomplete".into());
    }
    let descriptors = parse_descriptors(descriptors.as_array().unwrap())?;
    let mut tensors = MlBotPolicyTensors::default();
    let mut maximum_end = 0;
    for descriptor in &descriptors {
        let spec =
            TENSOR_SPECS.iter().find(|spec| spec.name == descriptor.name).ok_or_else(|| {
                format!("ML bot policy checkpoint tensor {} is unknown", descriptor.name)
            })?;
        if descriptor.shape.len() != spec.shape.len()
            || descriptor
                .shape
                .iter()
                .zip(spec.shape)
                .any(|(actual, expected)| *actual != *expected as u64)
        {
            return Err(format!("ML bot policy {} shape do not match schema v7", descriptor.name));
        }
        let expected_length: usize = spec.shape.iter().product();
        if descriptor.byte_length != (expected_length * 4) as u64 {
            return Err(format!(
                "ML bot policy checkpoint tensor {} byte length is invalid",
                descriptor.name
            ));
        }
        let start = payload_start as u64 + descriptor.byte_offset;
        let end = start + descriptor.byte_length;
        if end > encoded.len() as u64 {
            return Err(format!(
                "ML bot policy checkpoint tensor {} is truncated",
                descriptor.name
            ));
        }
        let tensor = tensors
            .get_mut(&descriptor.name)
            .expect("a validated descriptor has one typed tensor field");
        tensor.reserve(expected_length);
        for bytes in encoded[start as usize..end as usize].as_chunks::<4>().0 {
            tensor.push(f32::from_le_bytes(*bytes));
        }
        maximum_end = maximum_end.max(descriptor.byte_offset + descriptor.byte_length);
    }
    // The pinned decoder checks the maximum endpoint, not pairwise disjoint ranges.
    // Its accepted overlapping-slice case is retained in the oracle as well.
    if payload_start as u64 + maximum_end != encoded.len() as u64 {
        return Err("ML bot policy checkpoint contains trailing or overlapping payload data".into());
    }
    let checkpoint = MlBotPolicyCheckpoint { metadata: metadata.clone(), tensors };
    validate_metadata(&checkpoint.metadata)?;
    if descriptors.len() != TENSOR_SPECS.len() {
        return Err("ML bot policy tensor names do not match schema v7".into());
    }
    checkpoint.tensors.validate()?;
    Ok(checkpoint)
}

pub fn encode_ml_bot_policy_checkpoint(
    checkpoint: &MlBotPolicyCheckpoint,
) -> Result<Vec<u8>, String> {
    checkpoint.validate()?;
    let mut byte_offset = 0;
    let mut descriptors = Vec::with_capacity(TENSOR_SPECS.len());
    for spec in TENSOR_SPECS {
        let tensor = checkpoint.tensors.get(spec.name).unwrap();
        let byte_length = tensor.len() * 4;
        descriptors.push(TensorDescriptor {
            byte_length,
            byte_offset,
            name: spec.name,
            shape: spec.shape,
        });
        byte_offset += byte_length;
    }
    let header = EncodedHeader { metadata: &checkpoint.metadata, tensors: descriptors };
    let header = crate::json_value::stringify_serializable(&header)?;
    let mut encoded = Vec::with_capacity(PREFIX_BYTES + header.len() + byte_offset);
    encoded.extend_from_slice(&MAGIC);
    encoded.extend_from_slice(&(header.len() as u32).to_le_bytes());
    encoded.extend_from_slice(header.as_bytes());
    for spec in TENSOR_SPECS {
        for value in checkpoint.tensors.get(spec.name).unwrap() {
            encoded.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(encoded)
}

fn parse_descriptors(rows: &[LuaValue]) -> Result<Vec<DecodedDescriptor>, String> {
    let mut names = BTreeSet::new();
    let mut descriptors = Vec::with_capacity(rows.len());
    for row in rows {
        if !is_object(row) {
            return Err("ML bot policy tensor descriptor must be an object".into());
        }
        let name = row.get("name").map_or_else(|| "undefined".into(), js_string);
        if !names.insert(name.clone()) {
            return Err(format!("ML bot policy tensor {name} is duplicated"));
        }
        let byte_length = nonnegative_integer(field(row, "byteLength"));
        let byte_offset = nonnegative_integer(field(row, "byteOffset"));
        let shape = field(row, "shape").as_array();
        let dimensions_valid = shape.is_some_and(|shape| {
            shape.iter().all(|value| nonnegative_integer(value).is_some_and(|value| value >= 1))
        });
        if byte_length.is_none() || byte_offset.is_none() || !dimensions_valid {
            return Err(format!("ML bot policy tensor {name} descriptor is invalid"));
        }
        descriptors.push(DecodedDescriptor {
            byte_length: byte_length.unwrap(),
            byte_offset: byte_offset.unwrap(),
            name,
            shape: shape.unwrap().iter().map(|value| nonnegative_integer(value).unwrap()).collect(),
        });
    }
    Ok(descriptors)
}

fn validate_metadata(metadata: &LuaValue) -> Result<(), String> {
    if field(metadata, "modelFormat").as_str() != Some(MODEL_FORMAT) {
        return Err(format!("ML bot policy format must be {MODEL_FORMAT}"));
    }
    if field(metadata, "architecture").as_str() != Some(ARCHITECTURE) {
        return Err(format!("ML bot policy architecture must be {ARCHITECTURE}"));
    }
    for (label, key) in [
        ("model", "modelVersion"),
        ("observation", "observationVersion"),
        ("main trajectory", "mainTrajectoryVersion"),
        ("choice trajectory", "choiceTrajectoryVersion"),
    ] {
        if field(metadata, key).as_number() != Some(7.0) {
            return Err(format!("ML bot policy {label} version must be 7"));
        }
    }
    let spec: LuaValue =
        serde_json::from_str(SPEC_JSON).expect("the retained Source policy spec is valid JSON");
    require_names(
        field(metadata, "observationNames"),
        field(&spec, "observationNames"),
        "observation names",
    )?;
    require_names(
        field(metadata, "optionDescriptorNames"),
        field(&spec, "optionDescriptorNames"),
        "option descriptor names",
    )?;
    let hidden_sizes = field(metadata, "hiddenSizes").as_array();
    if hidden_sizes
        .is_none_or(|sizes| sizes.iter().map(js_string).collect::<Vec<_>>() != ["512", "256"])
    {
        return Err("ML bot policy hidden sizes do not match schema v7".into());
    }
    if crate::json_value::stringify(field(metadata, "primaryCurriculum"))?
        != crate::json_value::stringify(field(&spec, "primaryCurriculum"))?
    {
        return Err("ML bot policy primary curriculum does not match schema v7".into());
    }
    if field(metadata, "choiceHiddenSize").as_number() != Some(128.0) {
        return Err("ML bot policy choice hidden size must be 128".into());
    }
    for head in ["movement", "target", "ability", "aim"] {
        require_names(
            field(field(metadata, "actionHeads"), head),
            field(field(&spec, "actionHeads"), head),
            &format!("{head} actions"),
        )?;
    }
    if nonnegative_integer(field(metadata, "seed")).is_none_or(|seed| seed > u32::MAX as u64) {
        return Err("ML bot policy seed must be a uint32".into());
    }
    for (key, label) in [
        ("trainedEnvironmentSteps", "trained environment steps"),
        ("trainedUpdates", "trained updates"),
    ] {
        if nonnegative_integer(field(metadata, key)).is_none() {
            return Err(format!("ML bot policy {label} must be a nonnegative integer"));
        }
    }
    if field(metadata, "choiceTemperature")
        .as_number()
        .is_none_or(|value| !value.is_finite() || value <= 0.0)
    {
        return Err("ML bot policy choice temperature must be positive and finite".into());
    }
    if let Some(mode) = metadata.get("choicePolicyMode")
        && !matches!(mode.as_str(), Some("learned" | "scripted"))
    {
        return Err("ML bot policy choice mode must be learned or scripted".into());
    }
    let coverage = field(metadata, "choiceCoverage");
    if let Some(coverage) = coverage.as_object() {
        for (key, value) in coverage.iter() {
            if key.is_empty() {
                return Err("ML bot policy choice coverage keys must not be empty".into());
            }
            if nonnegative_integer(value).is_none() {
                return Err(format!(
                    "ML bot policy choice coverage {key} must be a nonnegative integer"
                ));
            }
        }
    } else if let Some(coverage) = coverage.as_array() {
        for (index, value) in coverage.iter().enumerate() {
            if nonnegative_integer(value).is_none() {
                return Err(format!(
                    "ML bot policy choice coverage {index} must be a nonnegative integer"
                ));
            }
        }
    } else if coverage == &LuaValue::Null {
        return Err("Cannot convert undefined or null to object".into());
    } else if let Some(text) = coverage.as_str()
        && !text.is_empty()
    {
        return Err("ML bot policy choice coverage 0 must be a nonnegative integer".into());
    }
    Ok(())
}

fn is_object(value: &LuaValue) -> bool {
    matches!(value, LuaValue::Object(_) | LuaValue::Array(_))
}

fn field<'a>(value: &'a LuaValue, key: &str) -> &'a LuaValue {
    value.get(key).unwrap_or(&LuaValue::Null)
}

fn require_names(actual: &LuaValue, expected: &LuaValue, label: &str) -> Result<(), String> {
    if actual.as_array().is_none() || actual != expected {
        return Err(format!("ML bot policy {label} do not match schema v7"));
    }
    Ok(())
}

fn nonnegative_integer(value: &LuaValue) -> Option<u64> {
    let value = value.as_number()?;
    if value.is_finite() && value.fract() == 0.0 && (0.0..=9_007_199_254_740_991.0).contains(&value)
    {
        Some(value as u64)
    } else {
        None
    }
}

fn js_string(value: &LuaValue) -> String {
    match value {
        LuaValue::Null => "null".into(),
        LuaValue::Boolean(value) => value.to_string(),
        LuaValue::Number(_) => {
            crate::json_value::stringify(value).expect("JSON header numbers are finite")
        }
        LuaValue::String(value) => value.clone(),
        LuaValue::Array(values) => values
            .iter()
            .map(|value| if value == &LuaValue::Null { String::new() } else { js_string(value) })
            .collect::<Vec<_>>()
            .join(","),
        LuaValue::Object(_) => "[object Object]".into(),
    }
}
