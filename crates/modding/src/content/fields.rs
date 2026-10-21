//! The catalog's source value checks, kept together for every authored family.

use std::collections::BTreeSet;

use serde::de::DeserializeOwned;
use solomon_sim::modding::definition::{
    ResolvedWebLuaContentReference, WebLuaContentKind, WebLuaRuleDefinition,
};
use solomon_sim::modding::{LuaObject, LuaValue};

use crate::json::stringify;

pub(super) fn object<'a>(
    value: Option<&'a LuaValue>,
    field: &str,
) -> Result<&'a LuaObject, String> {
    value.and_then(LuaValue::as_object).ok_or_else(|| format!("{field} must be an object"))
}

pub(super) fn optional_object(value: Option<&LuaValue>, field: &str) -> Result<LuaObject, String> {
    match value {
        None | Some(LuaValue::Null) => Ok(LuaObject::new()),
        _ => Ok(object(value, field)?.clone()),
    }
}

pub(super) fn exact_keys(source: &LuaObject, allowed: &[&str], field: &str) -> Result<(), String> {
    let unknown: Vec<_> =
        source.iter().map(|(key, _)| key).filter(|key| !allowed.contains(key)).collect();
    if !unknown.is_empty() {
        return Err(format!("{field} contains unknown fields: {}", unknown.join(", ")));
    }
    Ok(())
}

pub(super) fn number(
    value: Option<&LuaValue>,
    minimum: f64,
    maximum: f64,
    field: &str,
) -> Result<f64, String> {
    if let Some(value) = value.and_then(LuaValue::as_number)
        && value.is_finite()
        && value >= minimum
        && value <= maximum
    {
        return Ok(value);
    }
    Err(format!("{field} must be finite within {}..{}", number_text(minimum), number_text(maximum)))
}

pub(super) fn integer(
    value: Option<&LuaValue>,
    minimum: u64,
    maximum: u64,
    field: &str,
) -> Result<u64, String> {
    if let Some(value) = value.and_then(LuaValue::as_number)
        && value.is_finite()
        && value.fract() == 0.0
        && value >= minimum as f64
        && value <= maximum as f64
    {
        return Ok(value as u64);
    }
    Err(format!("{field} must be an integer within {minimum}..{maximum}"))
}

pub(super) fn boolean(value: Option<&LuaValue>, field: &str) -> Result<bool, String> {
    value.and_then(LuaValue::as_bool).ok_or_else(|| format!("{field} must be boolean"))
}

pub(super) fn optional_text(
    value: Option<&LuaValue>,
    fallback: &str,
    maximum: usize,
    field: &str,
) -> Result<String, String> {
    let Some(value) = value else { return Ok(fallback.to_owned()) };
    if let Some(value) = value.as_str()
        && !value.contains('\0')
        && value.len() <= maximum
    {
        return Ok(value.to_owned());
    }
    Err(format!("{field} must contain at most {maximum} bytes of text"))
}

pub(super) fn required_text(
    value: Option<&LuaValue>,
    maximum: usize,
    field: &str,
) -> Result<String, String> {
    let value = optional_text(value, "", maximum, field)?;
    if value.is_empty() {
        return Err(format!("{field} must be nonempty text"));
    }
    Ok(value)
}

pub(super) fn text_array(value: Option<&LuaValue>, field: &str) -> Result<Vec<String>, String> {
    let values = value
        .and_then(LuaValue::as_array)
        .filter(|values| values.len() <= 64)
        .ok_or_else(|| format!("{field} must be an array of text"))?;
    let mut result = Vec::new();
    for (index, value) in values.iter().enumerate() {
        let text = optional_text(Some(value), "", 64, &format!("{field}[{index}]"))?;
        result.push(text);
    }
    // Source validates each value before checking uniqueness, so a malformed
    // later element wins over an earlier duplicate or empty string.
    let mut seen = BTreeSet::new();
    for text in &result {
        if text.is_empty() || !seen.insert(text) {
            return Err(format!("{field} contains invalid or duplicate text"));
        }
    }
    Ok(result)
}

pub(super) fn number_or(
    value: Option<&LuaValue>,
    fallback: f64,
    minimum: f64,
    maximum: f64,
    field: &str,
) -> Result<f64, String> {
    match value {
        None => Ok(fallback),
        Some(value) => number(Some(value), minimum, maximum, field),
    }
}

pub(super) fn integer_or(
    value: Option<&LuaValue>,
    fallback: u64,
    minimum: u64,
    maximum: u64,
    field: &str,
) -> Result<u64, String> {
    match value {
        None => Ok(fallback),
        Some(value) => integer(Some(value), minimum, maximum, field),
    }
}

/// Several source fields default only when absent; explicit null is rejected.
pub(super) fn object_or_empty(value: Option<&LuaValue>, field: &str) -> Result<LuaObject, String> {
    match value {
        None => Ok(LuaObject::new()),
        Some(value) => Ok(object(Some(value), field)?.clone()),
    }
}

pub(super) fn duration_ms(value: Option<&LuaValue>, field: &str) -> Result<f64, String> {
    const MAXIMUM: f64 = 24.0 * 60.0 * 60.0 * 1_000.0;
    if let Some(value) = value.and_then(LuaValue::as_number)
        && value.fract() == 0.0
        && (0.0..=MAXIMUM).contains(&value)
    {
        return Ok(value);
    }
    let invalid = || format!("{field} is not a duration");
    let text = value.and_then(LuaValue::as_str).ok_or_else(invalid)?;
    let result = crate::duration::parse_duration_text(text).ok_or_else(invalid)?;
    if !result.is_finite() || !(0.0..=MAXIMUM).contains(&result) {
        return Err(format!("{field} exceeds its duration limit"));
    }
    Ok(result)
}

pub(super) fn nullish(value: Option<&LuaValue>) -> Option<&LuaValue> {
    value.filter(|value| **value != LuaValue::Null)
}

pub(super) fn is_token(value: &LuaValue, kind: &str) -> bool {
    value.get("kind").and_then(LuaValue::as_str) == Some(kind)
}

pub(super) fn decode<T: DeserializeOwned>(value: &LuaValue) -> Result<T, String> {
    serde_json::from_str(&stringify(value)?).map_err(|error| error.to_string())
}

pub(super) fn rule(
    value: Option<&LuaValue>,
    error: String,
) -> Result<WebLuaRuleDefinition, String> {
    let value = value.filter(|value| is_token(value, "rule-definition")).ok_or(error)?;
    decode(value)
}

pub(super) fn reference(
    value: Option<&LuaValue>,
    field: &str,
) -> Result<ResolvedWebLuaContentReference, String> {
    let value = value
        .filter(|value| is_token(value, "resolved-content-reference"))
        .ok_or_else(|| format!("{field} must be a resolved content reference"))?;
    decode(value)
}

pub(super) fn optional_reference(
    value: Option<&LuaValue>,
    kind: WebLuaContentKind,
    field: &str,
) -> Result<Option<ResolvedWebLuaContentReference>, String> {
    let Some(value) = nullish(value) else { return Ok(None) };
    if !is_token(value, "resolved-content-reference")
        || value.get("targetKind").and_then(LuaValue::as_str) != Some(kind.as_str())
    {
        return Err(format!("{field} must reference {}", kind.as_str()));
    }
    Ok(Some(decode(value)?))
}

pub(super) fn reference_array(
    value: Option<&LuaValue>,
    kind: WebLuaContentKind,
    field: &str,
) -> Result<Vec<ResolvedWebLuaContentReference>, String> {
    let values = value
        .and_then(LuaValue::as_array)
        .filter(|values| values.len() <= 64)
        .ok_or_else(|| format!("{field} must be an array of references"))?;
    let mut result = Vec::new();
    for (index, value) in values.iter().enumerate() {
        let field = format!("{field}[{index}]");
        let reference = optional_reference(Some(value), kind, &field)?
            .ok_or_else(|| format!("{field} requires {}", kind.as_str()))?;
        result.push(reference);
    }
    Ok(result)
}

pub(super) fn validate_modifier_map(source: &LuaObject, field: &str) -> Result<(), String> {
    for (lane, value) in source.iter() {
        if value.as_number().is_some_and(f64::is_finite) {
            continue;
        }
        let modifier = object(Some(value), &format!("{field}.{lane}"))?;
        exact_keys(modifier, &["add", "multiply", "set"], &format!("{field}.{lane}"))?;
        if modifier.len() != 1 {
            return Err(format!("{field}.{lane} requires one modifier operation"));
        }
        if !modifier.iter().next().unwrap().1.as_number().is_some_and(f64::is_finite) {
            return Err(format!("{field}.{lane} modifier must be finite"));
        }
    }
    Ok(())
}

pub(super) fn point(value: Option<&LuaValue>, field: &str) -> Result<(), String> {
    let point = object(value, field)?;
    exact_keys(point, &["x", "y"], field)?;
    number(point.get("x"), -100_000.0, 100_000.0, &format!("{field}.x"))?;
    number(point.get("y"), -100_000.0, 100_000.0, &format!("{field}.y"))?;
    Ok(())
}

fn number_text(value: f64) -> String {
    stringify(&LuaValue::Number(value)).expect("the catalog's numeric bounds are finite")
}
