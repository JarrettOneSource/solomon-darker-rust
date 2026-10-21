//! Stock draw operations shared by the primary spell view. Positions are relative
//! to the owning plan; rotation is in radians and scale preserves both native axes.

use crate::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellAtlas {
    BadGuys,
    DeadHawg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellBlend {
    Normal,
    Add,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellDrawQueue {
    World,
    PreWorld,
    PostWorld,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSpellSprite {
    pub alpha: f64,
    pub atlas: SpellAtlas,
    pub blend: SpellBlend,
    pub offset: Vec2,
    pub queue: SpellDrawQueue,
    pub record: usize,
    pub rotation: f64,
    pub scale: Vec2,
    pub tint: u32,
}

impl NativeSpellSprite {
    #[must_use]
    pub const fn new(record: usize, scale: f64) -> Self {
        Self {
            alpha: 1.0,
            atlas: SpellAtlas::BadGuys,
            blend: SpellBlend::Normal,
            offset: Vec2::ZERO,
            queue: SpellDrawQueue::World,
            record,
            rotation: 0.0,
            scale: Vec2::new(scale, scale),
            tint: 0xff_ffff,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeSpellSpritePlan {
    pub position: Vec2,
    pub world_y: f64,
    pub sort_bias: f64,
    pub region_light_point: Option<Vec2>,
    pub draws: Vec<NativeSpellSprite>,
}

impl NativeSpellSpritePlan {
    pub(super) fn at(position: Vec2) -> Self {
        Self {
            position,
            world_y: position.y,
            sort_bias: 0.0,
            region_light_point: None,
            draws: Vec::new(),
        }
    }
}

#[must_use]
pub fn pack_spell_rgb(red: f64, green: f64, blue: f64) -> u32 {
    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
    (channel(red) << 16) | (channel(green) << 8) | channel(blue)
}
