//! The consumable carrier from `renderer/mod-consumable-effect-presentation.ts`.
//! Its private four-word cosmetic stream is seeded by the accepted use ID and
//! never consumes authoritative randomness.

use crate::float::hypot;

use super::inventory::GameSimulationModEffect;

pub const MOD_CONSUMABLE_TEXTURE_SIZE: u32 = 128;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModConsumableEffectPlan {
    pub alpha: f64,
    pub flash_alphas: [f64; 4],
    pub flash_rotations: [f64; 4],
    pub flash_scales: [f64; 4],
    pub flash_visible: bool,
    pub radius: f64,
    pub tint: u32,
}

#[must_use]
pub fn mod_consumable_effect_plan(
    effect: &GameSimulationModEffect,
    tick: f64,
) -> ModConsumableEffectPlan {
    let channel = |value: f64| (value * 255.0).round() as u32;
    let tint = (channel(effect.color[0]) << 16)
        | (channel(effect.color[1]) << 8)
        | channel(effect.color[2]);
    let phase = (tick - effect.started_tick as f64) / 120.0 * std::f64::consts::PI * 2.0;
    let random = effect_random(effect.use_id);
    let source_scale = 1.0 + random[0] * 0.5;
    let angle = random[1] * std::f64::consts::PI * 2.0;
    let pulse = ((angle * 15.0).sin().abs() * 0.15 + 3.5) * source_scale;
    ModConsumableEffectPlan {
        alpha: effect.color[3] * 0.8,
        flash_alphas: [
            effect.color[3] * (random[3] * 0.25 + 0.2),
            effect.color[3] * 0.5,
            effect.color[3] * 0.5,
            effect.color[3] * 0.25,
        ],
        flash_rotations: [
            angle,
            angle + std::f64::consts::PI / 2.0,
            angle + std::f64::consts::PI,
            angle + std::f64::consts::PI * 1.5,
        ],
        flash_scales: [pulse, pulse * 0.75, pulse * 0.5, pulse * (random[2] * 0.2 + 0.2)],
        flash_visible: tick == effect.started_tick as f64,
        radius: 42.0 + 3.0 * phase.sin(),
        tint,
    }
}

#[must_use]
pub fn mod_consumable_ring_alpha(x: f64, y: f64) -> f64 {
    let center = f64::from(MOD_CONSUMABLE_TEXTURE_SIZE - 1) * 0.5;
    let distance = hypot(x - center, y - center);
    let outer = (1.0 - (distance - 47.0).abs() / 11.0).clamp(0.0, 1.0);
    let inner = (1.0 - distance / 54.0).clamp(0.0, 1.0) * 0.28;
    (outer + inner).clamp(0.0, 1.0)
}

/// Exact ImageData bytes from `mod-consumable-effect-view.ts::createRingTexture`.
#[must_use]
pub fn mod_consumable_ring_rgba() -> Vec<u8> {
    let mut pixels = Vec::with_capacity((MOD_CONSUMABLE_TEXTURE_SIZE.pow(2) * 4) as usize);
    for y in 0..MOD_CONSUMABLE_TEXTURE_SIZE {
        for x in 0..MOD_CONSUMABLE_TEXTURE_SIZE {
            let alpha = (mod_consumable_ring_alpha(f64::from(x), f64::from(y)) * 210.0).round();
            pixels.extend_from_slice(&[255, 255, 255, alpha as u8]);
        }
    }
    pixels
}

#[must_use]
pub fn mod_consumable_effect_id(effect: &GameSimulationModEffect) -> String {
    format!("mod-effect:{}:{}", effect.player_id, effect.use_id)
}

fn effect_random(use_id: u64) -> [f64; 4] {
    // This source stream deliberately permits zero, unlike the actor/wave RNG.
    let mut state = use_id as u32;
    std::array::from_fn(|_| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        f64::from(state) / 4_294_967_296.0
    })
}
