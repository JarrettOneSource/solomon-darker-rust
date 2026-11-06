//! Mod content metadata the host projects to clients: content, boasts, powerups and statuses.
//! This is the host's accepted presentation projection, separate from its mutable
//! runtime rows and from the prepared catalogue that owns gameplay definitions.

mod read;

use super::content::{ModBoastSelection, PreparedModBoastIcon};
use super::definition::WebLuaContentKind;
use crate::hub::interaction::BoastFailureProducer;

/// One art slot of a content entry and its image path.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub struct ModContentArt {
    pub path: String,
    pub slot: String,
}

/// One content entry as clients see it: identity, kind, name, description, art and
/// presentation.
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

/// A mod boast as clients see it: its identity, text, icon and rules.
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

/// A spawned mod powerup: its content, ID, spawn tick and position.
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

/// An active mod status: its content, instance, target and the ticks it starts and expires.
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

/// The host's accepted mod content projection: content and boast entries, live powerups and
/// statuses, the manifest digest and a revision.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModContentProjection {
    pub boasts: Vec<ModBoastProjectionEntry>,
    pub content: Vec<ModContentProjectionEntry>,
    pub manifest_sha256: String,
    pub powerups: Vec<ModPowerupProjectionEntry>,
    // Integer checks impose no safe-integer or u64 bound, so these view clocks
    // and instance IDs stay `f64` after validation.
    pub revision: f64,
    pub statuses: Vec<ModStatusProjectionEntry>,
}

impl ModContentProjection {
    /// The boast for a namespaced selection, as `hub_boast_instruction` and the selector
    /// resolve it.
    #[must_use]
    pub fn boast(&self, selection: &ModBoastSelection) -> Option<&ModBoastProjectionEntry> {
        self.boasts.iter().find(|boast| {
            boast.content_id == selection.content_id && boast.mod_id == selection.mod_id
        })
    }
}
