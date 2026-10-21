//! Typed counterparts of `native-secondary-presentation-types.ts`.

use crate::Vec2;
use crate::secondary::NativeSecondaryActorState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSecondaryAtlas {
    BadGuys,
    Clothes,
    DeadHawg,
    Golem,
}
impl NativeSecondaryAtlas {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BadGuys => "BadGuys",
            Self::Clothes => "Clothes",
            Self::DeadHawg => "DeadHawg",
            Self::Golem => "Golem",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSecondaryBlend {
    Normal,
    Add,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSecondaryColorMode {
    AlphaMask,
    Texture,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeSecondaryQueueFamily {
    OrdinaryDynamic,
    #[default]
    Zanim,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeSecondarySpriteDraw {
    pub alpha: f64,
    pub atlas: NativeSecondaryAtlas,
    pub blend: NativeSecondaryBlend,
    pub color_mode: Option<NativeSecondaryColorMode>,
    pub entry: usize,
    pub offset: Vec2,
    pub role: String,
    pub rotation_radians: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub tint: u32,
}

impl NativeSecondarySpriteDraw {
    #[must_use]
    pub fn new(
        actor: &NativeSecondaryActorState,
        atlas: NativeSecondaryAtlas,
        entry: usize,
    ) -> Self {
        Self {
            alpha: actor.alpha,
            atlas,
            blend: NativeSecondaryBlend::Normal,
            color_mode: None,
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
pub struct NativeSecondaryQuadDraw {
    pub alpha: f64,
    pub atlas: Option<NativeSecondaryAtlas>,
    pub blend: NativeSecondaryBlend,
    pub entry: Option<usize>,
    pub role: String,
    /// Local XY pairs ordered top-left, top-right, bottom-left, bottom-right.
    pub vertices: [f64; 8],
    pub tint: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct NativeSecondaryMeshDraw {
    pub alpha: f64,
    pub blend: NativeSecondaryBlend,
    pub indices: Vec<u32>,
    pub role: String,
    pub texture: NativeSecondaryMeshTexture,
    pub tint: u32,
    pub uvs: Vec<f64>,
    pub vertex_colors: Vec<u32>,
    pub vertices: Vec<f64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSecondaryMeshTexture {
    EtherPlane,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeLeviathanCompositeClear {
    pub color: u32,
    pub height: f64,
    pub width: f64,
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeLeviathanCompositeMask {
    pub clip_size: f64,
    pub clip_top: f64,
    pub entry: usize,
    pub scale: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeLeviathanCompositeOutput {
    pub alpha: f64,
    pub blend: NativeSecondaryBlend,
}
#[derive(Clone, Debug, PartialEq)]
pub struct NativeLeviathanCompositePlan {
    /// Both clear and mask use the source multiply operation.
    pub clear: NativeLeviathanCompositeClear,
    pub mask: NativeLeviathanCompositeMask,
    pub outputs: [NativeLeviathanCompositeOutput; 2],
}
#[derive(Clone, Debug, PartialEq)]
pub struct NativeSecondaryGradientDraw {
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
pub struct NativeStormWeatherComposite {
    pub draws: Vec<NativeSecondarySpriteDraw>,
    pub offset: Vec2,
    pub scale: f64,
}
/// The independent ground, burst and snow children from `freezeWaveVisualDraws`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeFreezeWaveVisualDraws {
    pub background_draws: Vec<NativeSecondarySpriteDraw>,
    pub draws: Vec<NativeSecondarySpriteDraw>,
    pub underlay_draws: Vec<NativeSecondarySpriteDraw>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct NativeSecondaryPresentationPlan {
    pub background_draws: Vec<NativeSecondarySpriteDraw>,
    pub draws: Vec<NativeSecondarySpriteDraw>,
    pub gradients: Vec<NativeSecondaryGradientDraw>,
    pub meshes: Vec<NativeSecondaryMeshDraw>,
    pub quads: Vec<NativeSecondaryQuadDraw>,
    pub queue_family: NativeSecondaryQueueFamily,
    pub root: Vec2,
    pub sort_bias: f64,
    pub storm_composite: Option<NativeStormWeatherComposite>,
    pub underlay_draws: Vec<NativeSecondarySpriteDraw>,
    pub world_y: f64,
}
impl NativeSecondaryPresentationPlan {
    #[must_use]
    pub fn new(root: Vec2) -> Self {
        Self {
            background_draws: Vec::new(),
            draws: Vec::new(),
            gradients: Vec::new(),
            meshes: Vec::new(),
            quads: Vec::new(),
            queue_family: NativeSecondaryQueueFamily::Zanim,
            root,
            sort_bias: 0.0,
            storm_composite: None,
            underlay_draws: Vec::new(),
            world_y: root.y,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativePlayerMagicShieldPlan {
    pub alpha: f64,
    pub scale: f64,
    pub tint: u32,
    pub visible: bool,
}

pub const NATIVE_LEVIATHAN_RENDER_TARGET_SIZE: f64 = 256.0;
pub const NATIVE_PLAYER_MAGIC_SHIELD_ENTRY: usize = 2;
pub const NATIVE_PLAYER_MAGIC_SHIELD_OFFSET_Y: f64 = -35.0;
pub const NATIVE_PLAYER_MAGIC_SHIELD_SCALE: f64 = 2.15_f32 as f64;
