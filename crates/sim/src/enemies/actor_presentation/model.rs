//! Stock enemy draw values from `renderer/native-enemy-presentation-model.ts`.

use crate::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyArtAtlas {
    BadGuys,
    DeadHawg,
    Demon,
    Faculty,
    Heartmonger,
    Unholy,
}

impl EnemyArtAtlas {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::BadGuys => "BadGuys",
            Self::DeadHawg => "DeadHawg",
            Self::Demon => "Demon",
            Self::Faculty => "Faculty",
            Self::Heartmonger => "Heartmonger",
            Self::Unholy => "Unholy",
        }
    }
}

/// Authored sockets come from the same registered stock records as the GPU atlas.
pub trait EnemyArtPoints {
    fn points(&self, atlas: EnemyArtAtlas, entry: usize) -> &[Vec2];
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyArtRecord {
    pub anchor_x: f64,
    pub anchor_y: f64,
    pub width: f64,
    pub height: f64,
}

/// Registered frame geometry is shared by actor sprites and their native underlays.
pub trait EnemyArtRecords: EnemyArtPoints {
    fn record(&self, atlas: EnemyArtAtlas, entry: usize) -> EnemyArtRecord;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyArtBlend {
    Normal,
    Add,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyArtStretch {
    pub start: Vec2,
    pub end: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnemySpriteLayer {
    pub alpha: f64,
    pub atlas: EnemyArtAtlas,
    pub blend: EnemyArtBlend,
    pub entry: usize,
    pub offset: Vec2,
    pub role: String,
    pub rotation_radians: f64,
    pub scale: f64,
    pub scale_x: Option<f64>,
    pub scale_y: Option<f64>,
    pub stretch: Option<EnemyArtStretch>,
    pub tint: u32,
    pub apply_vertical_offset: bool,
    pub texture_diffuse: bool,
}

impl EnemySpriteLayer {
    #[must_use]
    pub fn new(atlas: EnemyArtAtlas, entry: usize, role: impl Into<String>) -> Self {
        Self {
            alpha: 1.0,
            atlas,
            blend: EnemyArtBlend::Normal,
            entry,
            offset: Vec2::ZERO,
            role: role.into(),
            rotation_radians: 0.0,
            scale: 1.0,
            scale_x: None,
            scale_y: None,
            stretch: None,
            tint: 0xffffff,
            apply_vertical_offset: true,
            texture_diffuse: false,
        }
    }
    pub(super) fn into_actor_space(mut self, scale: f64) -> Self {
        self.offset.x /= scale;
        self.offset.y /= scale;
        self.scale /= scale;
        self.scale_x = self.scale_x.map(|value| value / scale);
        self.scale_y = self.scale_y.map(|value| value / scale);
        self
    }
}

impl From<crate::enemies::presentation::NativeEnemyProjectileLayer> for EnemySpriteLayer {
    fn from(layer: crate::enemies::presentation::NativeEnemyProjectileLayer) -> Self {
        use crate::spells::presentation::{SpellAtlas, SpellBlend};
        Self {
            alpha: layer.alpha,
            atlas: match layer.atlas {
                SpellAtlas::BadGuys => EnemyArtAtlas::BadGuys,
                SpellAtlas::DeadHawg => EnemyArtAtlas::DeadHawg,
            },
            blend: match layer.blend {
                SpellBlend::Normal => EnemyArtBlend::Normal,
                SpellBlend::Add => EnemyArtBlend::Add,
            },
            entry: layer.entry,
            offset: layer.offset,
            role: layer.role,
            rotation_radians: layer.rotation_radians,
            scale: layer.scale,
            scale_x: None,
            scale_y: layer.scale_y,
            stretch: None,
            tint: layer.tint,
            apply_vertical_offset: true,
            texture_diffuse: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnemySegmentLayer {
    pub alpha: f64,
    pub before_role: String,
    pub blend: EnemyArtBlend,
    pub start: Vec2,
    pub end: Vec2,
    pub role: String,
    pub tint: u32,
    pub width: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EnemyFamilyPresentation {
    pub before: Vec<EnemySpriteLayer>,
    pub body: Vec<EnemySpriteLayer>,
    pub after: Vec<EnemySpriteLayer>,
    pub hit_body: Vec<EnemySpriteLayer>,
    pub segments: Vec<EnemySegmentLayer>,
}

impl EnemyFamilyPresentation {
    pub(super) fn body(body: Vec<EnemySpriteLayer>) -> Self {
        Self { hit_body: body.clone(), body, ..Self::default() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemySpiderOutlineTarget {
    pub position: Vec2,
    pub primary_tint: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyActorAppearanceContext {
    pub tick: u64,
    pub application_tick: u64,
    pub complex_lighting: bool,
    pub spider_exposure: f64,
    pub spider_target: Option<EnemySpiderOutlineTarget>,
    pub pike_target_position: Option<Vec2>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnemyActorPresentationPlan {
    pub attack_effect_origin: Option<Vec2>,
    pub facing: usize,
    pub root_scale: f64,
    pub layers: Vec<EnemySpriteLayer>,
    pub segments: Vec<EnemySegmentLayer>,
    pub spawn_age_ticks: u64,
}
