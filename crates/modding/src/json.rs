//! Key ordering and JSON number spelling for mod content graphs.
//!
//! Graph keys sort by en-US collation and serialize with ECMAScript number spelling. Both are
//! part of the package graph digest contract in `docs/DECISIONS.md`: byte ordering or Rust's
//! `1.0` spelling would change a package's digest.

use std::cmp::Ordering;

use icu_collator::{Collator, CollatorBorrowed, options::CollatorOptions};
use icu_locale::locale;
use serde::Serialize;
use solomon_sim::modding::{LuaObject, LuaValue};

/// En-US collation for ordering mod content keys and names, whatever the host's locale.
pub struct SourceCollator {
    collator: CollatorBorrowed<'static>,
}

impl Default for SourceCollator {
    fn default() -> Self {
        Self {
            // Always en-US, independent of which locale the Rust host machine
            // happens to use, so every host orders keys the same way.
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

/// Convert a serializable value to a `LuaValue` without sorting its object fields.
pub fn lua_value<T: Serialize>(value: &T) -> Result<LuaValue, String> {
    let encoded = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    serde_json::from_slice(&encoded).map_err(|error| error.to_string())
}

/// The digest form of a content graph: object keys sorted by `collator`, with every `source`
/// (definition location) and `callback` field removed at any depth, including a user field
/// named `source`. That exclusion is part of the digest contract.
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
