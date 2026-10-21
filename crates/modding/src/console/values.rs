//! Console-specific normalization and error ordering from `web-lua-values.ts`.

use crate::definition::values::{LuaTableEntries, lua_table_entries};
use mlua::Value;
use solomon_sim::modding::{LuaObject, LuaValue};
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct ValueBudget {
    nodes: usize,
    seen: HashSet<usize>,
}
impl ValueBudget {
    pub fn normalize(
        &mut self,
        value: &Value,
        field: &str,
        depth: usize,
    ) -> Result<LuaValue, String> {
        self.nodes += 1;
        if self.nodes > 2_048 {
            return Err(format!("{field} exceeds the node limit"));
        }
        if depth > 16 {
            return Err(format!("{field} exceeds the depth limit"));
        }
        match value {
            Value::Nil => Ok(LuaValue::Null),
            Value::Boolean(value) => Ok(LuaValue::Boolean(*value)),
            Value::Integer(value) => Ok(LuaValue::Number(*value as f64)),
            Value::Number(value) if value.is_finite() => Ok(LuaValue::Number(*value)),
            Value::Number(_) => Err(format!("{field} must be finite")),
            Value::String(value) => {
                let value = value.to_string_lossy();
                if value.len() > 16_384 {
                    return Err(format!("{field} exceeds the string limit"));
                }
                Ok(LuaValue::String(value))
            }
            Value::Table(table) => {
                let pointer = table.to_pointer() as usize;
                if !self.seen.insert(pointer) {
                    return Err(format!("{field} is cyclic"));
                }
                let result = (|| match lua_table_entries(table)? {
                    LuaTableEntries::Array(entries) => {
                        if entries.len() > 128 {
                            return Err(format!("{field} exceeds the array limit"));
                        }
                        let mut output = Vec::new();
                        for (index, value) in entries.iter().enumerate() {
                            output.push(self.normalize(
                                value,
                                &format!("{field}[{index}]"),
                                depth + 1,
                            )?);
                        }
                        Ok(LuaValue::Array(output))
                    }
                    LuaTableEntries::Object(entries) => {
                        if entries.len() > 128 {
                            return Err(format!("{field} exceeds the field limit"));
                        }
                        let mut output = LuaObject::new();
                        for (key, value) in entries {
                            if key.is_empty() || key.len() > 128 {
                                return Err(format!("{field} has invalid key"));
                            }
                            output.insert(
                                key.clone(),
                                self.normalize(&value, &format!("{field}.{key}"), depth + 1)?,
                            );
                        }
                        Ok(LuaValue::Object(output))
                    }
                })();
                self.seen.remove(&pointer);
                result
            }
            _ => Err(format!("{field} has unsupported Lua type")),
        }
    }
}
pub(super) fn normalize(value: &Value, field: &str) -> Result<LuaValue, String> {
    let output = ValueBudget::default().normalize(value, field, 0)?;
    if crate::json::stringify(&output)?.len() > 32 * 1_024 {
        return Err(format!("{field} exceeds 32 KiB"));
    }
    Ok(output)
}
/// Save restoration enters as JSON values, before the Wasmoon table boundary.
/// Preserve empty arrays here; turning them into Lua tables would erase their
/// JSON shape and change both the retained state and its byte admission.
pub(super) fn normalize_saved(value: &LuaValue, field: &str) -> Result<LuaValue, String> {
    fn visit(value: &LuaValue, field: &str, depth: usize, nodes: &mut usize) -> Result<(), String> {
        *nodes += 1;
        if *nodes > 2_048 {
            return Err(format!("{field} exceeds the node limit"));
        }
        if depth > 16 {
            return Err(format!("{field} exceeds the depth limit"));
        }
        match value {
            LuaValue::Number(value) if !value.is_finite() => {
                return Err(format!("{field} must be finite"));
            }
            LuaValue::String(value) if value.len() > 16_384 => {
                return Err(format!("{field} exceeds the string limit"));
            }
            LuaValue::Array(entries) => {
                if entries.len() > 128 {
                    return Err(format!("{field} exceeds the array limit"));
                }
                for (index, value) in entries.iter().enumerate() {
                    visit(value, &format!("{field}[{index}]"), depth + 1, nodes)?;
                }
            }
            LuaValue::Object(entries) => {
                if entries.len() > 128 {
                    return Err(format!("{field} exceeds the field limit"));
                }
                for (key, value) in entries.iter() {
                    if key.is_empty() || key.len() > 128 {
                        return Err(format!("{field} has invalid key"));
                    }
                    visit(value, &format!("{field}.{key}"), depth + 1, nodes)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    visit(value, field, 0, &mut 0)?;
    if crate::json::stringify(value)?.len() > 32 * 1_024 {
        return Err(format!("{field} exceeds 32 KiB"));
    }
    Ok(value.clone())
}
pub(super) fn require_text(value: &Value, field: &str, maximum: usize) -> Result<String, String> {
    if let Value::String(value) = value {
        let text = value.to_string_lossy();
        if !text.is_empty() && text.len() <= maximum {
            return Ok(text);
        }
    }
    Err(format!("{field} must be nonempty text of at most {maximum} bytes"))
}
pub(super) fn finite(value: &Value, field: &str) -> Result<f64, String> {
    let number = match value {
        Value::Integer(value) => *value as f64,
        Value::Number(value) => *value,
        _ => f64::NAN,
    };
    if !number.is_finite() {
        return Err(format!("{field} must be finite"));
    }
    Ok(number)
}
pub(super) fn finite_within(
    value: &Value,
    field: &str,
    minimum: f64,
    maximum: f64,
) -> Result<f64, String> {
    let number = finite(value, field)?;
    if number < minimum || number > maximum {
        return Err(format!("{field} must be within {minimum}..{maximum}"));
    }
    Ok(number)
}
pub(super) fn nonnegative_integer(value: &Value, field: &str) -> Result<u64, String> {
    let number = finite(value, field)?;
    if number < 0.0 {
        return Err(format!("{field} must be non-negative"));
    }
    if number.fract() != 0.0 || number > 9_007_199_254_740_991.0 {
        return Err(format!("{field} must be a safe integer"));
    }
    Ok(number as u64)
}
pub(super) fn integer_within(
    value: &Value,
    field: &str,
    minimum: u64,
    maximum: u64,
) -> Result<u64, String> {
    let number = nonnegative_integer(value, field)?;
    if number < minimum || number > maximum {
        return Err(format!("{field} must be within {minimum}..{maximum}"));
    }
    Ok(number)
}
pub(super) fn bounded_error(error: &mlua::Error) -> String {
    let message = match error {
        mlua::Error::CallbackError { cause, .. } => return bounded_error(cause),
        mlua::Error::RuntimeError(message) | mlua::Error::MemoryError(message) => {
            message.split("\nstack traceback:").next().unwrap_or(message).to_owned()
        }
        mlua::Error::SyntaxError { message, .. } => message.clone(),
        _ => error.to_string(),
    };
    truncate_error(message)
}
pub(super) fn truncate_error(mut text: String) -> String {
    if text.len() > 4096 {
        let mut end = 4096;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}
pub(super) fn object(fields: impl IntoIterator<Item = (impl Into<String>, LuaValue)>) -> LuaValue {
    LuaValue::Object(fields.into_iter().map(|(key, value)| (key.into(), value)).collect())
}
pub(super) fn json_value(value: impl serde::Serialize) -> Result<LuaValue, String> {
    serde_json::from_str(&serde_json::to_string(&value).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}
pub(super) fn js_string(value: &Value) -> String {
    match value {
        Value::Nil => "null".into(),
        Value::Boolean(value) => value.to_string(),
        Value::String(value) => value.to_string_lossy(),
        Value::Integer(value) => value.to_string(),
        Value::Number(value) if value.is_nan() => "NaN".into(),
        Value::Number(value) if *value == f64::INFINITY => "Infinity".into(),
        Value::Number(value) if *value == f64::NEG_INFINITY => "-Infinity".into(),
        Value::Number(value) => {
            crate::json::stringify(&LuaValue::Number(*value)).unwrap_or_default()
        }
        Value::Table(_) => "[object Object]".into(),
        _ => "function".into(),
    }
}
