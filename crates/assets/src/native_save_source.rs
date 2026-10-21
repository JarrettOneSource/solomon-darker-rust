//! Retained native archive attachment from Website `save/portable-game-profile.ts`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSaveFile {
    pub base64: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSaveSource {
    pub darkdata_base64: String,
    pub darkdata_sha256: String,
    pub gamestate_base64: String,
    pub gamestate_sha256: String,
    pub retained_files: Vec<NativeSaveFile>,
    pub run_name: String,
}
