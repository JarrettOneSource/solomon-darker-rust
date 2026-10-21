//! Content identity sent by the Website's control plane.
//! Source: `protocol/game-protocol-contract.ts` and `host/web-mod-content.ts`.

use super::assets::GameModAsset;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct GameContentIdentity {
    pub content_sha256: String,
    pub id: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct GameContentManifest {
    pub manifest_sha256: String,
    pub mods: Vec<GameContentIdentity>,
}

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

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct WebSessionContentSummary {
    pub manifest_sha256: String,
    pub mods: Vec<WebSessionModSummary>,
}

/// Website provisioner payloads from `host/web-mod-content.ts`. Validation and
/// byte materialization live at the native package boundary, not in the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase", deny_unknown_fields))]
pub struct WebModBoneyardPayload {
    pub bytes_base64: String,
    pub target: String,
}

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
    // A nullable property is still required by the provisioner's exactKeys.
    // Without deserialize_with, serde treats a missing Option as null.
    <Option<String> as serde::Deserialize>::deserialize(deserializer)
}
