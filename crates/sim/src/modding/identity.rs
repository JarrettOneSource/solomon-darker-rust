//! Content identity sent by the Website's control plane, and the mod content payloads its
//! provisioner delivers.

use super::assets::GameModAsset;

/// One mod's identity in a session: its ID, version and content SHA-256.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct GameContentIdentity {
    pub content_sha256: String,
    pub id: String,
    pub version: String,
}

/// The enabled mods' identities and the manifest's SHA-256.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct GameContentManifest {
    pub manifest_sha256: String,
    pub mods: Vec<GameContentIdentity>,
}

/// One enabled mod as the session summarizes it: identity, name, slug, digests and assets.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct WebSessionModSummary {
    pub assets: Vec<GameModAsset>,
    pub content_sha256: String,
    pub graph_sha256: Option<String>,
    pub id: String,
    pub name: String,
    pub slug: String,
    pub version: String,
}

/// A session's mod content summary: the manifest digest and each mod's summary.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct WebSessionContentSummary {
    pub manifest_sha256: String,
    pub mods: Vec<WebSessionModSummary>,
}

/// One Boneyard file in a Website provisioner payload. Validation and byte
/// materialization live at the native package boundary, not in the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase", deny_unknown_fields))]
pub struct WebModBoneyardPayload {
    pub bytes_base64: String,
    pub target: String,
}

/// One package file in a provisioner payload: path, kind, content type, size, SHA-256 and
/// base64 bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase", deny_unknown_fields))]
pub struct WebModPackageFilePayload {
    pub byte_length: u64,
    pub bytes_base64: String,
    pub content_type: String,
    pub kind: String,
    pub path: String,
    pub sha256: String,
}

/// One mod in a provisioner payload: identity, priority, entry script, files and Boneyards.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase", deny_unknown_fields))]
pub struct WebSessionModPayload {
    pub boneyards: Vec<WebModBoneyardPayload>,
    pub content_sha256: String,
    #[cfg_attr(feature = "persistence", serde(deserialize_with = "required_nullable_text"))]
    pub entry_script: Option<String>,
    pub files: Vec<WebModPackageFilePayload>,
    pub id: String,
    pub name: String,
    pub priority: i32,
    pub slug: String,
    pub version: String,
}

/// The Website provisioner's mod content payload: the manifest digest and each mod.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase", deny_unknown_fields))]
pub struct WebSessionContentPayload {
    pub manifest_sha256: String,
    pub mods: Vec<WebSessionModPayload>,
}

#[cfg(feature = "persistence")]
fn required_nullable_text<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // The provisioner's payload must still include this nullable property.
    // Without deserialize_with, serde treats a missing Option as null.
    <Option<String> as serde::Deserialize>::deserialize(deserializer)
}
