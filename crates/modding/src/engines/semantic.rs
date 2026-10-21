//! Scoped primitive values from `mod-semantic-state-engine.ts`.

use std::collections::HashSet;

use solomon_sim::modding::LuaValue;
use solomon_sim::modding::definition::WebLuaScopeKind;
use solomon_sim::modding::runtime::{ModSemanticStateCheckpoint, ModSemanticValue, ModStateScope};

use super::MAXIMUM_SAFE_INTEGER;
use crate::json::SourceCollator;

pub struct ModSemanticStateEngine {
    mod_ids: HashSet<String>,
    values: Vec<(String, ModSemanticValue)>,
    revision: u64,
    collator: SourceCollator,
}

impl ModSemanticStateEngine {
    pub fn new(mod_ids: impl IntoIterator<Item = String>) -> Self {
        Self {
            mod_ids: mod_ids.into_iter().collect(),
            values: Vec::new(),
            revision: 0,
            collator: SourceCollator::default(),
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn checkpoint(&self) -> ModSemanticStateCheckpoint {
        ModSemanticStateCheckpoint { revision: self.revision, values: self.project(None) }
    }

    pub fn project(&self, viewer_id: Option<&str>) -> Vec<ModSemanticValue> {
        let mut values: Vec<_> = self
            .values
            .iter()
            .filter(|(_, row)| viewer_id.is_none_or(|id| visible_to(&row.scope, id)))
            .collect();
        values.sort_by(|(left, _), (right, _)| self.collator.compare(left, right));
        values.into_iter().map(|(_, row)| row.clone()).collect()
    }

    pub fn restore(&mut self, checkpoint: ModSemanticStateCheckpoint) -> Result<(), String> {
        if checkpoint.revision > MAXIMUM_SAFE_INTEGER || checkpoint.values.len() > 4_096 {
            return Err("mod semantic state checkpoint is invalid".into());
        }
        let mut values = Vec::with_capacity(checkpoint.values.len());
        let mut ids = HashSet::new();
        for row in checkpoint.values {
            self.validate_identity(&row.mod_id, &row.scope, &row.key)?;
            let id = identity(&row.mod_id, &row.scope, &row.key);
            if !ids.insert(id.clone()) {
                return Err("mod semantic state checkpoint contains duplicates".into());
            }
            validate_value(&row.value, 0, &mut 0)?;
            values.push((id, row));
        }
        self.values = values;
        self.revision = checkpoint.revision;
        Ok(())
    }

    pub fn set(
        &mut self,
        mod_id: &str,
        scope: ModStateScope,
        key: &str,
        value: LuaValue,
    ) -> Result<(), String> {
        self.validate_identity(mod_id, &scope, key)?;
        let id = identity(mod_id, &scope, key);
        let index = self.values.iter().position(|(key, _)| *key == id);
        if index.is_none() && self.values.len() >= 4_096 {
            return Err("mod semantic state limit reached".into());
        }
        validate_value(&value, 0, &mut 0)?;
        let row = ModSemanticValue { key: key.into(), mod_id: mod_id.into(), scope, value };
        if let Some(index) = index {
            self.values[index].1 = row
        } else {
            self.values.push((id, row))
        }
        self.revision += 1;
        Ok(())
    }

    pub fn clear(&mut self, mod_id: &str, scope: &ModStateScope, key: &str) -> bool {
        let id = identity(mod_id, scope, key);
        let Some(index) = self.values.iter().position(|(key, _)| *key == id) else { return false };
        self.values.remove(index);
        self.revision += 1;
        true
    }

    fn validate_identity(
        &self,
        mod_id: &str,
        scope: &ModStateScope,
        key: &str,
    ) -> Result<(), String> {
        let bytes = key.as_bytes();
        let valid_key = !bytes.is_empty()
            && bytes.len() <= 128
            && bytes[0].is_ascii_lowercase()
            && bytes.iter().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            });
        if !self.mod_ids.contains(mod_id)
            || !valid_key
            || scope.id.is_empty()
            || scope.id.encode_utf16().count() > 256
        {
            return Err("mod semantic state identity is invalid".into());
        }
        Ok(())
    }
}

fn visible_to(scope: &ModStateScope, viewer_id: &str) -> bool {
    !matches!(scope.kind, WebLuaScopeKind::ParticipantProfile | WebLuaScopeKind::ParticipantRun)
        || scope.id == viewer_id
        || scope.id.starts_with(&format!("{viewer_id}:"))
}

fn identity(mod_id: &str, scope: &ModStateScope, key: &str) -> String {
    format!("{mod_id}\0{}\0{}\0{key}", scope.kind.as_str(), scope.id)
}

fn validate_value(value: &LuaValue, depth: usize, nodes: &mut usize) -> Result<(), String> {
    *nodes += 1;
    if depth > 16 || *nodes > 4_096 {
        return Err("mod semantic state value exceeds its graph budget".into());
    }
    match value {
        LuaValue::Null | LuaValue::Boolean(_) => {}
        LuaValue::Number(number) if !number.is_finite() => {
            return Err("mod semantic state number must be finite".into());
        }
        LuaValue::Number(_) => {}
        LuaValue::String(value) => {
            if value.encode_utf16().count() > 4_096 {
                return Err("mod semantic state text is too long".into());
            }
        }
        LuaValue::Array(values) => {
            for value in values {
                validate_value(value, depth + 1, nodes)?
            }
        }
        LuaValue::Object(fields) => {
            for (key, value) in fields.iter() {
                if key.is_empty()
                    || key.len() > 128
                    || !key.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
                    })
                {
                    return Err("mod semantic state field is invalid".into());
                }
                validate_value(value, depth + 1, nodes)?;
            }
        }
    }
    Ok(())
}
