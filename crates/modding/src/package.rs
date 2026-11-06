//! Mod packages provisioned for a session: validation, definition compilation and the
//! accepted content. The accepted bytes and definition sources are retained once; the
//! canonical Session borrows their prepared content rather than owning a VM.

mod definitions;
mod materialize;

pub use definitions::compile_web_session_content_definitions;

pub use materialize::{
    materialize_web_session_content, materialize_web_session_content_value, project_mod_boneyard,
};

use std::collections::BTreeMap;

use solomon_assets::boneyard::ModBoneyardEntry;
use solomon_sim::modding::assets::GameModAsset;
use solomon_sim::modding::definition::{CompiledWebLuaMod, WebLuaModIdentity};
use solomon_sim::modding::identity::{GameContentManifest, WebSessionContentSummary};

/// One mod's Lua sources: its entry script, its packaged files by path and its identity.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebLuaModSource {
    pub entry_script: String,
    pub files: BTreeMap<String, Vec<u8>>,
    pub identity: WebLuaModIdentity,
}

/// A session's validated mod content: its assets, Boneyards, compiled mods, manifest, Lua
/// sources and summary.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterializedWebSessionContent {
    pub assets: Vec<GameModAsset>,
    pub boneyards: Vec<ModBoneyardEntry>,
    pub compiled_mods: Vec<CompiledWebLuaMod>,
    pub manifest: GameContentManifest,
    pub mod_sources: Vec<WebLuaModSource>,
    pub summary: WebSessionContentSummary,
}
