//! Client content metadata from `protocol/game-mod-contract.ts`.
//! This is the host's accepted presentation projection, separate from its mutable
//! runtime rows and from the prepared catalogue that owns gameplay definitions.

mod read;

use super::content::{ModBoastSelection, PreparedModBoastIcon};
use super::definition::WebLuaContentKind;
use crate::hub::interaction::BoastFailureProducer;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub struct ModContentArt {
    pub path: String,
    pub slot: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModContentProjectionEntry {
    pub art: Vec<ModContentArt>,
    pub content_id: String,
    pub content_kind: WebLuaContentKind,
    pub description: String,
    pub key: String,
    pub mod_id: String,
    pub name: String,
    pub presentation: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModBoastProjectionEntry {
    pub content_id: String,
    pub failure_producers: Vec<BoastFailureProducer>,
    pub icon: PreparedModBoastIcon,
    pub instruction: String,
    pub mod_id: String,
    pub name: String,
    pub random_skill_choices: bool,
    pub response: String,
    pub score_multiplier: f64,
    pub statement: String,
    pub success_wave: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModPowerupProjectionEntry {
    pub content_id: String,
    pub id: f64,
    pub spawned_tick: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModStatusProjectionEntry {
    pub content_id: String,
    pub expires_tick: f64,
    pub instance_id: f64,
    pub started_tick: f64,
    pub target_id: String,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModContentProjection {
    pub boasts: Vec<ModBoastProjectionEntry>,
    pub content: Vec<ModContentProjectionEntry>,
    pub manifest_sha256: String,
    pub powerups: Vec<ModPowerupProjectionEntry>,
    // Source integer checks do not impose a safe-integer/u64 bound. These view
    // clocks and instance IDs retain JavaScript Number values after validation.
    pub revision: f64,
    pub statuses: Vec<ModStatusProjectionEntry>,
}

impl ModContentProjection {
    /// `hubBoastInstruction` and the selector resolve the namespaced selection.
    #[must_use]
    pub fn boast(&self, selection: &ModBoastSelection) -> Option<&ModBoastProjectionEntry> {
        self.boasts.iter().find(|boast| {
            boast.content_id == selection.content_id && boast.mod_id == selection.mod_id
        })
    }
}
