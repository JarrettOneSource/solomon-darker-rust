//! Typed counterparts of `native-secondary-presentation-types.ts`.

use crate::Vec2;
use crate::secondary::SecondaryActorState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondaryAtlas {
    BadGuys,
    Clothes,
    DeadHawg,
    Demon,
    Golem,
}
impl SecondaryAtlas {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BadGuys => "BadGuys",
            Self::Clothes => "Clothes",
            Self::DeadHawg => "DeadHawg",
            Self::Demon => "Demon",
            Self::Golem => "Golem",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondaryBlend {
    Normal,
    Add,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondaryColorMode {
    AlphaMask,
    Texture,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SecondaryQueueFamily {
    OrdinaryDynamic,
    #[default]
    Zanim,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SecondarySpriteDraw {
    pub alpha: f64,
    pub atlas: SecondaryAtlas,
    pub blend: SecondaryBlend,
    pub color_mode: Option<SecondaryColorMode>,
    /// Field-local rectangle, independent of the image's offset and rotation.
    pub clip: Option<[f64; 4]>,
    pub entry: usize,
    pub offset: Vec2,
    pub role: String,
    pub rotation_radians: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub tint: u32,
}

impl SecondarySpriteDraw {
    #[must_use]
    pub fn new(actor: &SecondaryActorState, atlas: SecondaryAtlas, entry: usize) -> Self {
        Self {
            alpha: actor.alpha,
            atlas,
            blend: SecondaryBlend::Normal,
            color_mode: None,
            clip: None,
            entry,
            offset: Vec2::ZERO,
            role: format!("{}-{}-{entry}", actor.kind.as_str(), atlas.as_str()),
            rotation_radians: 0.0,
            scale_x: actor.scale,
            scale_y: actor.scale,
            tint: 0xffffff,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SecondaryQuadDraw {
    pub alpha: f64,
    pub atlas: Option<SecondaryAtlas>,
    pub blend: SecondaryBlend,
    pub entry: Option<usize>,
    pub role: String,
    /// Local XY pairs ordered top-left, top-right, bottom-left, bottom-right.
    pub vertices: [f64; 8],
    pub tint: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SecondaryMeshDraw {
    pub alpha: f64,
    pub blend: SecondaryBlend,
    pub indices: Vec<u32>,
    pub role: String,
    pub texture: SecondaryMeshTexture,
    pub tint: u32,
    pub uvs: Vec<f64>,
    pub vertex_colors: Vec<u32>,
    pub vertices: Vec<f64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondaryMeshTexture {
    EtherPlane,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeviathanCompositeClear {
    pub color: u32,
    pub height: f64,
    pub width: f64,
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeviathanCompositeMask {
    pub clip_size: f64,
    pub clip_top: f64,
    pub entry: usize,
    pub scale: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeviathanCompositeOutput {
    pub alpha: f64,
    pub blend: SecondaryBlend,
}
#[derive(Clone, Debug, PartialEq)]
pub struct LeviathanCompositePlan {
    /// Both clear and mask use the source multiply operation.
    pub clear: LeviathanCompositeClear,
    pub mask: LeviathanCompositeMask,
    pub outputs: [LeviathanCompositeOutput; 2],
}
#[derive(Clone, Debug, PartialEq)]
pub struct SecondaryGradientDraw {
    pub bottom_alpha: f64,
    pub bottom_color: u32,
    pub height: f64,
    pub role: String,
    pub top_alpha: f64,
    pub top_color: u32,
    pub top_left: Vec2,
    pub width: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct StormWeatherComposite {
    pub draws: Vec<SecondarySpriteDraw>,
    pub offset: Vec2,
    pub scale: f64,
}
/// The independent ground, burst and snow children from `freezeWaveVisualDraws`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FreezeWaveVisualDraws {
    pub background_draws: Vec<SecondarySpriteDraw>,
    pub draws: Vec<SecondarySpriteDraw>,
    pub underlay_draws: Vec<SecondarySpriteDraw>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SecondaryPresentationPlan {
    pub background_draws: Vec<SecondarySpriteDraw>,
    pub draws: Vec<SecondarySpriteDraw>,
    pub gradients: Vec<SecondaryGradientDraw>,
    pub meshes: Vec<SecondaryMeshDraw>,
    pub quads: Vec<SecondaryQuadDraw>,
    pub queue_family: SecondaryQueueFamily,
    pub root: Vec2,
    pub sort_bias: f64,
    pub storm_composite: Option<StormWeatherComposite>,
    pub underlay_draws: Vec<SecondarySpriteDraw>,
    pub world_y: f64,
}
impl SecondaryPresentationPlan {
    #[must_use]
    pub fn new(root: Vec2) -> Self {
        Self {
            background_draws: Vec::new(),
            draws: Vec::new(),
            gradients: Vec::new(),
            meshes: Vec::new(),
            quads: Vec::new(),
            queue_family: SecondaryQueueFamily::Zanim,
            root,
            sort_bias: 0.0,
            storm_composite: None,
            underlay_draws: Vec::new(),
            world_y: root.y,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerMagicShieldPlan {
    pub alpha: f64,
    pub scale: f64,
    pub tint: u32,
    pub visible: bool,
}

pub const LEVIATHAN_RENDER_TARGET_SIZE: f64 = 256.0;
pub const PLAYER_MAGIC_SHIELD_ENTRY: usize = 2;
pub const PLAYER_MAGIC_SHIELD_OFFSET_Y: f64 = -35.0;
pub const PLAYER_MAGIC_SHIELD_SCALE: f64 = 2.15_f32 as f64;
