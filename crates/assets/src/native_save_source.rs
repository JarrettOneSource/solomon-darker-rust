//! The original native save files attached to a portable profile or web save, stored as
//! base64 with SHA-256 digests.

use serde::{Deserialize, Serialize};

/// One retained native file in the attachment: its path, base64 bytes and SHA-256.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSaveFile {
    pub base64: String,
    pub path: String,
    pub sha256: String,
}

/// The attached native save: `darkdata.cfg` and the run's `gamestate.sav` as base64 with
/// digests, plus any other retained files.
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
