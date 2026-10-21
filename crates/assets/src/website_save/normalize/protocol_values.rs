//! Source protocol/codecs/values.ts validation shared by persisted protocol records.

use serde_json::{Map, Value};

pub(super) fn object<'a>(
    value: &'a Value,
    field: &str,
    keys: &[&str],
) -> Result<&'a Map<String, Value>, String> {
    let source = value.as_object().ok_or_else(|| format!("{field} must be an object"))?;
    for key in source.keys() {
        if !keys.contains(&key.as_str()) {
            return Err(format!("{field}.{key} is not allowed"));
        }
    }
    Ok(source)
}

pub(super) fn array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, String> {
    value.as_array().ok_or_else(|| format!("{field} must be an array"))
}

pub(super) fn limited_array<'a>(
    value: &'a Value,
    field: &str,
    maximum: usize,
) -> Result<&'a Vec<Value>, String> {
    let values = array(value, field)?;
    if values.len() > maximum {
        return Err(format!("{field} may contain at most {maximum} entries"));
    }
    Ok(values)
}

pub(super) fn member<'a>(
    value: &'a Value,
    field: &str,
    members: &[&str],
) -> Result<&'a str, String> {
    let value = string(value, field, 64)?;
    if !members.contains(&value) {
        return Err(format!("{field} is not supported"));
    }
    Ok(value)
}

pub(super) fn bounded_integer(
    value: &Value,
    field: &str,
    min: i64,
    max: i64,
) -> Result<i64, String> {
    let value = finite(value, field)?;
    if value.fract() != 0.0 {
        return Err(format!("{field} must be an integer"));
    }
    if value < min as f64 || value > max as f64 {
        return Err(format!("{field} is outside [{min},{max}]"));
    }
    Ok(value as i64)
}

pub(super) fn player_id<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    let id = string(value, field, 128)?;
    if matches!(
        id,
        "constructor"
            | "__defineGetter__"
            | "__defineSetter__"
            | "hasOwnProperty"
            | "__lookupGetter__"
            | "__lookupSetter__"
            | "isPrototypeOf"
            | "propertyIsEnumerable"
            | "toString"
            | "valueOf"
            | "__proto__"
            | "toLocaleString"
    ) {
        return Err(format!("{field} is reserved"));
    }
    Ok(id)
}

pub(super) fn registration(value: &Value, field: &str, expected_lane: &str) -> Result<u64, String> {
    object(value, field, &["managerLane", "registrationOrdinal"])?;
    let key = format!("{field}.managerLane");
    let lane = string(&value["managerLane"], &key, 16)?;
    if !matches!(lane, "actor" | "transient") {
        return Err(format!("{key} is not supported"));
    }
    if lane != expected_lane {
        return Err(format!("{key} must be {expected_lane}"));
    }
    integer(&value["registrationOrdinal"], &format!("{field}.registrationOrdinal"), false)
}

pub(super) fn finite(value: &Value, field: &str) -> Result<f64, String> {
    value
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("{field} must be finite"))
}

pub(super) fn positive(value: &Value, field: &str) -> Result<f64, String> {
    let number = finite(value, field)?;
    if number <= 0.0 {
        return Err(format!("{field} must be positive"));
    }
    Ok(number)
}

pub(super) fn unit(value: &Value, field: &str) -> Result<f64, String> {
    let number = finite(value, field)?;
    if !(0.0..=1.0).contains(&number) {
        return Err(format!("{field} must be between zero and one"));
    }
    Ok(number)
}

pub(super) fn integer(value: &Value, field: &str, positive: bool) -> Result<u64, String> {
    let number = finite(value, field)?;
    if number.fract() != 0.0 {
        return Err(format!("{field} must be an integer"));
    }
    if number < if positive { 1.0 } else { 0.0 } {
        return Err(format!(
            "{field} must be {}",
            if positive { "positive" } else { "nonnegative" }
        ));
    }
    Ok(number as u64)
}

pub(super) fn boolean(value: &Value, field: &str) -> Result<(), String> {
    value.as_bool().map(|_| ()).ok_or_else(|| format!("{field} must be boolean"))
}

pub(super) fn string<'a>(value: &'a Value, field: &str, maximum: usize) -> Result<&'a str, String> {
    value
        .as_str()
        .filter(|text| !text.is_empty() && text.encode_utf16().count() <= maximum)
        .ok_or_else(|| format!("{field} must be a nonempty string of at most {maximum} characters"))
}

pub(super) fn vector(value: &Value, field: &str) -> Result<(), String> {
    object(value, field, &["x", "y"])?;
    finite(&value["x"], &format!("{field}.x"))?;
    finite(&value["y"], &format!("{field}.y"))?;
    Ok(())
}
