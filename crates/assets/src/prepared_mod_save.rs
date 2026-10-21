//! Per-mod save rows from Website `host/prepared-mod-save.ts`.
//!
//! These functions project the canonical ten-domain host checkpoint. Decoding
//! follows this boundary's validation; engine restoration subsequently validates
//! content/rank/scene semantics and is deliberately a separate transaction.

mod decode;
mod encode;

use solomon_sim::modding::definition::CompiledWebLuaMod;
use solomon_sim::modding::{LuaObject, LuaValue};

pub use decode::decode_prepared_mod_save_state;
pub use encode::encode_prepared_mod_save_state;

fn content_owner<'a>(mods: &'a [CompiledWebLuaMod], content_id: &str) -> Option<&'a str> {
    // Source constructs a Map in mod/content order. A later duplicate identity
    // owns the entry at this codec boundary, independently of catalog admission.
    for module in mods.iter().rev() {
        if module.content.iter().any(|content| content.content_id == content_id) {
            return Some(&module.identity.id);
        }
    }
    None
}

fn object<const N: usize>(fields: [(&str, LuaValue); N]) -> LuaValue {
    LuaValue::Object(fields.into_iter().map(|(key, value)| (key.to_owned(), value)).collect())
}

fn string(value: &str) -> LuaValue {
    LuaValue::String(value.to_owned())
}

fn integer(value: u64) -> LuaValue {
    LuaValue::Number(value as f64)
}

fn optional_integer(value: Option<u64>) -> LuaValue {
    value.map_or(LuaValue::Null, integer)
}

fn optional_string(value: Option<&str>) -> LuaValue {
    value.map_or(LuaValue::Null, string)
}

fn string_array(values: &[String]) -> LuaValue {
    LuaValue::Array(values.iter().map(|value| string(value)).collect())
}

fn integer_array(values: &[u64]) -> LuaValue {
    LuaValue::Array(values.iter().map(|value| integer(*value)).collect())
}

fn object_value(value: &LuaObject) -> LuaValue {
    LuaValue::Object(value.clone())
}
