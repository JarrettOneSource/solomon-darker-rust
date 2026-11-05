//! Per-player authored stat books from `core-kernels/player-progression.ts`.
//! The stock catalog initializes a new player. Saved and Lua-authored overrides
//! belong to that player's component, independently of learned skill ranks.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use super::{SKILL_CATALOG, SkillValue};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(untagged))]
pub enum SkillStatBookValue {
    Scalar(f64),
    Ranks(Vec<f64>),
}

impl SkillStatBookValue {
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
pub struct SkillStatBookEntry {
    pub cap_level: u32,
    pub description: String,
    pub id: usize,
    pub maximum_level: u32,
    pub numeric_properties: BTreeMap<String, SkillStatBookValue>,
    pub quick_description: String,
}

impl SkillStatBookEntry {
    #[must_use]
    pub fn stat(&self, property: &str, rank: u32) -> Option<f64> {
        self.numeric_properties.get(property)?.at_rank(rank)
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerStatBookComponent {
    /// Snapshots share definitions; authored mutation uses `Arc::make_mut`.
    pub entries: Arc<[SkillStatBookEntry]>,
}

impl Default for PlayerStatBookComponent {
    fn default() -> Self {
        static STOCK: OnceLock<Arc<[SkillStatBookEntry]>> = OnceLock::new();
        let entries = STOCK.get_or_init(|| {
            let mut entries = Vec::new();
            for definition in SKILL_CATALOG {
                let mut numeric_properties = BTreeMap::new();
                for property in definition.numeric_properties {
                    let value = match property.value {
                        SkillValue::Scalar(value) => SkillStatBookValue::Scalar(value),
                        SkillValue::Ranks(values) => SkillStatBookValue::Ranks(values.to_vec()),
                    };
                    numeric_properties.insert(property.name.to_owned(), value);
                }
                entries.push(SkillStatBookEntry {
                    cap_level: definition.cap_level,
                    description: definition.description.to_owned(),
                    id: definition.id,
                    maximum_level: definition.maximum_level,
                    numeric_properties,
                    quick_description: definition.quick_description.to_owned(),
                });
            }
            entries.into()
        });
        Self { entries: Arc::clone(entries) }
    }
}

impl PlayerStatBookComponent {
    #[must_use]
    pub fn stat(&self, skill_id: usize, property: &str, rank: u32) -> Option<f64> {
        self.entries.get(skill_id).filter(|entry| entry.id == skill_id)?.stat(property, rank)
    }
}
