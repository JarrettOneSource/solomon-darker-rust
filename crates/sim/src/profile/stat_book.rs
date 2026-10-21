//! Per-player authored stat books from `core-kernels/player-progression.ts`.
//! The stock catalog initializes a new player. Saved and Lua-authored overrides
//! belong to that player's component, independently of learned skill ranks.

use std::collections::BTreeMap;

use super::{NATIVE_SKILL_CATALOG, NativeSkillValue};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(untagged))]
pub enum NativeSkillStatBookValue {
    Scalar(f64),
    Ranks(Vec<f64>),
}

impl NativeSkillStatBookValue {
    #[must_use]
    pub fn at_rank(&self, rank: u32) -> Option<f64> {
        match self {
            Self::Scalar(value) => Some(*value),
            Self::Ranks(values) => {
                values.get((rank as usize).min(values.len().saturating_sub(1))).copied()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct NativeSkillStatBookEntry {
    pub cap_level: u32,
    pub description: String,
    pub id: usize,
    pub maximum_level: u32,
    pub numeric_properties: BTreeMap<String, NativeSkillStatBookValue>,
    pub quick_description: String,
}

impl NativeSkillStatBookEntry {
    #[must_use]
    pub fn stat(&self, property: &str, rank: u32) -> Option<f64> {
        self.numeric_properties.get(property)?.at_rank(rank)
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerStatBookComponent {
    pub entries: Vec<NativeSkillStatBookEntry>,
}

impl Default for PlayerStatBookComponent {
    fn default() -> Self {
        let mut entries = Vec::new();
        for definition in NATIVE_SKILL_CATALOG {
            let mut numeric_properties = BTreeMap::new();
            for property in definition.numeric_properties {
                let value = match property.value {
                    NativeSkillValue::Scalar(value) => NativeSkillStatBookValue::Scalar(value),
                    NativeSkillValue::Ranks(values) => {
                        NativeSkillStatBookValue::Ranks(values.to_vec())
                    }
                };
                numeric_properties.insert(property.name.to_owned(), value);
            }
            entries.push(NativeSkillStatBookEntry {
                cap_level: definition.cap_level,
                description: definition.description.to_owned(),
                id: definition.id,
                maximum_level: definition.maximum_level,
                numeric_properties,
                quick_description: definition.quick_description.to_owned(),
            });
        }
        Self { entries }
    }
}

impl PlayerStatBookComponent {
    #[must_use]
    pub fn stat(&self, skill_id: usize, property: &str, rank: u32) -> Option<f64> {
        self.entries.get(skill_id).filter(|entry| entry.id == skill_id)?.stat(property, rank)
    }
}
