//! Accepted mod package asset descriptors: images with their sprite frames, audio and
//! documents.
//! The compiler validates bytes and metadata; views borrow these prepared values.

use super::definition::WebLuaAssetKind;

/// One asset file in a session's mod content: its mod, path, kind, content type, size and
/// SHA-256 digest.
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

/// One sprite frame in an image asset: its atlas rectangle, content and logical sizes, and
/// center offset.
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

/// The identity every prepared asset shares: its ID, key, mod, path and SHA-256 digest.
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

/// A prepared image asset: its size, sprite frames, named animations and heading count.
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

/// A prepared audio asset: its bus, content type, looping and volume.
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

/// A prepared document asset and its content type.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct PreparedModDocumentAsset {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub identity: PreparedModAssetIdentity,
    pub asset_kind: WebLuaAssetKind,
    pub content_type: String,
}

/// A prepared mod asset: an image, audio or a document.
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
