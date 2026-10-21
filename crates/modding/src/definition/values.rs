//! Actual Lua 5.4 values through the pinned Wasmoon table/clone boundary.
//! `wasmoon@1.16.0 TableTypeExtension` uses a nonempty table as an array only
//! when its lua_next keys are exactly 1..N in that order. Empty tables are objects.

use std::collections::HashSet;

use mlua::{Lua, Value};
use serde::de::DeserializeOwned;
use solomon_sim::modding::{LuaObject, LuaValue};

pub fn clone_lua_value(value: &Value, field: &str) -> Result<LuaValue, String> {
    let mut nodes = 0;
    let mut seen = HashSet::new();
    clone_value(value, field, field, 0, &mut nodes, &mut seen)
}

fn clone_value(
    value: &Value,
    field: &str,
    path: &str,
    depth: usize,
    nodes: &mut usize,
    seen: &mut HashSet<usize>,
) -> Result<LuaValue, String> {
    *nodes += 1;
    if *nodes > 65_536 {
        return Err(format!("{field} exceeds its node limit"));
    }
    if depth > 32 {
        return Err(format!("{path} exceeds its nesting limit"));
    }
    match value {
        Value::Nil => Ok(LuaValue::Null),
        Value::Boolean(value) => Ok(LuaValue::Boolean(*value)),
        Value::String(value) => Ok(LuaValue::String(value.to_string_lossy())),
        Value::Integer(value) => Ok(LuaValue::Number(*value as f64)),
        Value::Number(value) => {
            if !value.is_finite() {
                return Err(format!("{path} must be finite"));
            }
            Ok(LuaValue::Number(*value))
        }
        Value::Function(_) => Err(format!("{path} may not contain a function")),
        Value::Table(table) => {
            let pointer = table.to_pointer() as usize;
            if !seen.insert(pointer) {
                return Err(format!("{path} is cyclic"));
            }
            let result = (|| match lua_table_entries(table)? {
                LuaTableEntries::Array(entries) => {
                    let mut output = Vec::with_capacity(entries.len());
                    for (index, value) in entries.iter().enumerate() {
                        output.push(clone_value(
                            value,
                            field,
                            &format!("{path}[{index}]"),
                            depth + 1,
                            nodes,
                            seen,
                        )?);
                    }
                    Ok(LuaValue::Array(output))
                }
                LuaTableEntries::Object(entries) => {
                    let mut output = LuaObject::new();
                    for (key, value) in entries {
                        output.insert(
                            key.clone(),
                            clone_value(
                                &value,
                                field,
                                &format!("{path}.{key}"),
                                depth + 1,
                                nodes,
                                seen,
                            )?,
                        );
                    }
                    Ok(LuaValue::Object(output))
                }
            })();
            seen.remove(&pointer);
            result
        }
        _ => Err(format!("{path} contains unsupported data")),
    }
}

/// Wasmoon's one table classification is shared by definition and Console
/// normalization; each caller retains its own Source limits and error order.
pub(crate) enum LuaTableEntries {
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

pub(crate) fn lua_table_entries(table: &mlua::Table) -> Result<LuaTableEntries, String> {
    let mut entries = Vec::new();
    for entry in table.clone().pairs::<Value, Value>() {
        let (key, value) = entry.map_err(|error| error.to_string())?;
        entries.push((key.to_string().map_err(|error| error.to_string())?, value));
    }
    let is_array = !entries.is_empty()
        && entries.iter().enumerate().all(|(index, (key, _))| *key == (index + 1).to_string());
    if is_array {
        return Ok(LuaTableEntries::Array(entries.into_iter().map(|(_, value)| value).collect()));
    }
    // JavaScript enumerates numeric string keys before clone visits children.
    let mut raw = LuaObject::new();
    let mut values = Vec::new();
    for (key, value) in entries {
        let index = values.len();
        values.push(value);
        raw.insert(key, LuaValue::Number(index as f64));
    }
    let mut output = Vec::new();
    for (key, index) in raw.iter() {
        let index = index.as_number().expect("table projection retains an index") as usize;
        output.push((key.to_owned(), values[index].clone()));
    }
    Ok(LuaTableEntries::Object(output))
}

/// `webLuaPayload`: omit null object children at the host-to-Lua boundary.
pub fn lua_payload(lua: &Lua, value: &LuaValue) -> mlua::Result<Value> {
    match value {
        LuaValue::Null => Ok(Value::Nil),
        LuaValue::Boolean(value) => Ok(Value::Boolean(*value)),
        LuaValue::Number(value) => Ok(Value::Number(*value)),
        LuaValue::String(value) => Ok(Value::String(lua.create_string(value)?)),
        LuaValue::Array(values) => {
            let table = lua.create_table_with_capacity(values.len(), 0)?;
            for (index, value) in values.iter().enumerate() {
                table.raw_set(index + 1, lua_payload(lua, value)?)?;
            }
            Ok(Value::Table(table))
        }
        LuaValue::Object(fields) => {
            let table = lua.create_table_with_capacity(0, fields.len())?;
            for (key, value) in fields.iter() {
                if !matches!(value, LuaValue::Null) {
                    table.raw_set(key, lua_payload(lua, value)?)?;
                }
            }
            Ok(Value::Table(table))
        }
    }
}

pub fn typed_value<T: DeserializeOwned>(value: &LuaValue) -> Result<T, String> {
    // Preserve JS integer spelling at this boundary: serializing LuaValue's
    // f64 through ordinary serde would produce 4.0, rejected by integer DTOs.
    let text = crate::json::stringify(value)?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}
