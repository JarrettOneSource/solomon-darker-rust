//! Offline Boneyard scene data retained from the stock generator.
//!
//! Reference: `Website/frontend/src/game/core-kernels/boneyard.ts`. Scene geometry is data;
//! `solomon_assets::boneyard` decodes the pinned bank for both browser and native hosts.

pub mod arena;
pub mod collision;
pub mod compact_masks;
pub mod encounter;
pub mod escape;
pub mod feedback;
pub mod gate;
pub mod ground_glyph;
pub mod lighting;
pub mod loot;
pub mod navigation;
pub mod presentation;
pub mod projectile_world;
pub mod scenery;
pub mod shadows;
pub mod solomon_presentation;
pub mod spawn;
pub mod spectator;
pub mod surface;
pub mod terrain_surface;
pub mod tutorial;
pub mod waves;
pub mod weather;
pub mod world;
pub mod world_geometry;
pub mod world_light;

pub use world::{
    BoneyardCheckpoint, BoneyardEncounterStep, BoneyardMovementContact, BoneyardMovementInput,
    BoneyardMovementStep, BoneyardPlayerCombatStatus, BoneyardPlayerStatus, BoneyardTutorialPlayer,
    BoneyardWorld, BoneyardWorldTickContext, BoneyardWorldTickResult, boneyard_camera_origin,
};

use crate::Vec2;

/// The stock browser Arena projection (`boneyard-render-contract.ts`).
pub const BONEYARD_CAMERA_SCALE: f64 = 1.35;

/// A Boneyard's world rectangle (`BoneyardBounds`).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl BoneyardBounds {
    #[must_use]
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }
}

/// The authored player placement and facing at the entrance.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardSpawn {
    pub position: Vec2,
    pub facing_degrees: f64,
}

/// A world actor such as a tree, monument, gravestone or building.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardObject {
    pub eid: String,
    pub type_id: u32,
    pub position: Vec2,
    pub variant: Option<u32>,
    pub rotation_degrees: Option<f64>,
    pub scale: Option<f64>,
    pub sort_bias: Option<f64>,
    pub atlas_entry: Option<usize>,
    pub secondary_atlas_entry: Option<usize>,
    pub secondary_variant: Option<u32>,
    pub secondary_visible: Option<bool>,
    pub overlay_atlas_entry: Option<usize>,
    pub overlay_variant: Option<u32>,
    pub atlas_entries: Option<Vec<usize>>,
}

/// A ground sprite; s0/s1/s2 and flags retain the native authored fields.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardSprite {
    pub eid: String,
    pub atlas_entry: usize,
    // Authored u32 atlas indices gain 114, which must also fit on wasm32.
    pub dead_hawg_entry: Option<u64>,
    pub position: Vec2,
    pub s0: f64,
    pub s1: f64,
    pub s2: f64,
    pub flags: u32,
}

/// A linked textured road segment, with optional exact authored quad.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardRoad {
    pub eid: String,
    pub link_mask: u8,
    pub type_id: u32,
    pub points: Vec<Vec2>,
    pub style: Option<u32>,
    pub start_width_scale: Option<f64>,
    pub end_width_scale: Option<f64>,
    pub quad: Option<Vec<Vec2>>,
}

/// A fence section; segment code 2 owns swinging gate leaves.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardFence {
    pub eid: String,
    pub type_id: u32,
    pub points: Vec<Vec2>,
    pub style: Option<u32>,
    pub segment_code: Option<u32>,
    pub start_post_variant: Option<u32>,
    pub end_post_variant: Option<u32>,
}

impl BoneyardFence {
    #[must_use]
    pub fn code(&self) -> u32 {
        self.segment_code.or(self.style).unwrap_or(0)
    }
}

/// Shared endpoint order in `core-kernels/boneyard-fence-posts.ts`.
/// Main-layer registrations and primary Puppet targets use this same prefix.
#[must_use]
pub fn native_boneyard_fence_post_positions(fences: &[BoneyardFence]) -> Vec<Vec2> {
    scenery::native_boneyard_fence_posts(fences).into_iter().map(|post| post.position).collect()
}

/// An authored terrain patch.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardTerrain {
    pub eid: String,
    pub position: Vec2,
    pub points: Option<Vec<Vec2>>,
    pub style: Option<u32>,
    pub entry: Option<usize>,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub uid: Option<f64>,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub profile_samples: Option<Vec<f64>>,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub side_sign: Option<f64>,
}

/// The opening Solomon set piece and its retained digging frame program.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SolomonDig {
    pub grave_position: Vec2,
    pub lantern_position: Vec2,
    pub position: Vec2,
    pub frame_program: Vec<usize>,
    pub ticks_per_frame: u64,
}

/// All authored geometry consumed by the scene renderer and collision world.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardScene {
    pub name: String,
    pub environment_mode: u32,
    pub bounds: BoneyardBounds,
    pub spawn: BoneyardSpawn,
    pub objects: Vec<BoneyardObject>,
    pub sprites: Vec<BoneyardSprite>,
    pub roads: Vec<BoneyardRoad>,
    pub fences: Vec<BoneyardFence>,
    pub terrain: Vec<BoneyardTerrain>,
    pub solomon_dig: Option<SolomonDig>,
}

/// A retained stock generator output, before a run receives its seed and identity.
#[derive(Clone, Debug, PartialEq)]
pub struct BoneyardTemplate {
    pub source_sha256: String,
    pub geometry_sha256: String,
    pub scene: BoneyardScene,
}

/// The source `BoneyardChoice.source` decides ownership of the retail encounter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "kebab-case"))]
pub enum BoneyardSource {
    #[default]
    Default,
    Mod,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct BoneyardChoice {
    pub id: String,
    pub name: String,
    pub source: BoneyardSource,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub mod_id: Option<String>,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub mod_name: Option<String>,
}

/// Geometry and identity of one admitted run (`LoadedBoneyard`).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct LoadedBoneyard {
    pub choice_id: String,
    pub choice_source: BoneyardSource,
    pub mod_id: Option<String>,
    pub mod_name: Option<String>,
    pub name: String,
    pub run_id: String,
    pub seed: String,
    pub source_sha256: String,
    pub geometry_sha256: String,
    /// Admitted geometry is immutable and shared by authority and display snapshots.
    pub scene: std::sync::Arc<BoneyardScene>,
}
