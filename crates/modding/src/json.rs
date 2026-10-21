//! Source graph ordering and JSON numeric spelling.
//!
//! `web-lua-definition-compiler.ts` uses en-US `localeCompare` for graph keys,
//! then `JSON.stringify`. Byte ordering and Rust's `1.0` spelling change its hash.

use std::cmp::Ordering;

use icu_collator::{Collator, CollatorBorrowed, options::CollatorOptions};
use icu_locale::locale;
use serde::Serialize;
use solomon_sim::modding::{LuaObject, LuaValue};

pub struct SourceCollator {
    collator: CollatorBorrowed<'static>,
}

impl Default for SourceCollator {
    fn default() -> Self {
        Self {
            // This is the pinned Node oracle's resolved locale, independent of
            // which locale the Rust host machine happens to use.
            collator: Collator::try_new(locale!("en-US").into(), CollatorOptions::default())
                .expect("compiled ICU data includes en-US collation"),
        }
    }
}

impl SourceCollator {
    pub fn compare(&self, left: &str, right: &str) -> Ordering {
        self.collator.compare(left, right)
    }
}

/// Convert an existing source-shaped value without sorting its object fields.
pub fn lua_value<T: Serialize>(value: &T) -> Result<LuaValue, String> {
    let encoded = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    serde_json::from_slice(&encoded).map_err(|error| error.to_string())
}

/// `canonicalGraph`: source locations and callbacks are excluded recursively,
/// including a user field named `source`. That source behavior is digest policy.
pub fn canonical_graph(value: &LuaValue, collator: &SourceCollator) -> LuaValue {
    match value {
        LuaValue::Array(values) => {
            LuaValue::Array(values.iter().map(|value| canonical_graph(value, collator)).collect())
        }
        LuaValue::Object(values) => {
            let mut fields: Vec<_> = values.iter().collect();
            fields.retain(|(key, _)| !matches!(*key, "source" | "callback"));
            fields.sort_by(|(left, _), (right, _)| collator.compare(left, right));
            let mut object = LuaObject::new();
            for (key, value) in fields {
                object.insert(key.to_owned(), canonical_graph(value, collator));
            }
            LuaValue::Object(object)
        }
        _ => value.clone(),
    }
}

pub use solomon_assets::json_value::stringify;
