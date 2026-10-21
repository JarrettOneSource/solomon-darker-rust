//! Accepted package asset descriptors from `modding/assets/mod-asset-compiler.ts`.
//! The compiler validates bytes and metadata; views borrow these prepared values.

use super::definition::WebLuaAssetKind;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct GameModAsset {
    pub byte_length: u64,
    pub content_type: String,
    pub kind: String,
    pub mod_id: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct ModSpriteFrame {
    pub center_offset_x: f64,
    pub center_offset_y: f64,
    pub content_height: f64,
    pub content_width: f64,
    pub height: f64,
    pub logical_height: f64,
    pub logical_width: f64,
    pub width: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModAssetIdentity {
    pub id: String,
    pub key: String,
    pub mod_id: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModSpriteAsset {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub identity: PreparedModAssetIdentity,
    pub animations: std::collections::BTreeMap<String, Vec<usize>>,
    pub asset_kind: WebLuaAssetKind,
    pub frames: Vec<ModSpriteFrame>,
    pub height: u32,
    pub heading_count: Option<u32>,
    pub width: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModAudioAsset {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub identity: PreparedModAssetIdentity,
    pub asset_kind: WebLuaAssetKind,
    pub bus: String,
    pub content_type: String,
    #[cfg_attr(feature = "persistence", serde(rename = "loop"))]
    pub loop_audio: bool,
    pub volume: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModDocumentAsset {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub identity: PreparedModAssetIdentity,
    pub asset_kind: WebLuaAssetKind,
    pub content_type: String,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "kind", rename_all = "kebab-case"))]
pub enum PreparedModAsset {
    Image(PreparedModSpriteAsset),
    Audio(PreparedModAudioAsset),
    Document(PreparedModDocumentAsset),
}

impl PreparedModAsset {
    #[must_use]
    pub fn identity(&self) -> &PreparedModAssetIdentity {
        match self {
            Self::Image(value) => &value.identity,
            Self::Audio(value) => &value.identity,
            Self::Document(value) => &value.identity,
        }
    }

    #[must_use]
    pub const fn asset_kind(&self) -> WebLuaAssetKind {
        match self {
            Self::Image(value) => value.asset_kind,
            Self::Audio(value) => value.asset_kind,
            Self::Document(value) => value.asset_kind,
        }
    }
}
