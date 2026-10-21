//! Native starter equipment colours (`Skills_Wizard` starter roll).
//!
//! A fresh wizard's hat and robe take a primary tint rolled from the owner's base colour with
//! three jitter draws, mixed toward its luminance. Reference:
//! `Website/frontend/src/game/core-kernels/native-starter-equipment.ts`.

use crate::float::fround;
use crate::rng::NativeRng;

pub mod equipment;

/// Who the starter colour belongs to: a wizard element or the College itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StarterColorOwner {
    Air,
    Earth,
    Ether,
    Fire,
    Water,
    College,
}

impl StarterColorOwner {
    fn base_color(self) -> [f64; 3] {
        match self {
            Self::Air => [0.1, 1.0, 1.0],
            Self::College => [0.25, 0.5, 0.25],
            Self::Earth => [0.0, 0.75, 0.0],
            Self::Ether => [1.0, 0.1, 1.0],
            Self::Fire => [1.0, 0.1, 0.1],
            Self::Water => [0.1, 0.5, 1.0],
        }
    }
}

/// Packed `0xRRGGBB` tints for the two equipment colour passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StarterEquipmentAppearance {
    pub primary_tint: u32,
    pub secondary_tint: u32,
}

/// The tutorial wizard's fixed equipment colours.
pub const NATIVE_TUTORIAL_EQUIPMENT_APPEARANCE: StarterEquipmentAppearance =
    StarterEquipmentAppearance { primary_tint: 0xc4_91_5e, secondary_tint: 0xff_ff_ff };

// The Website mixes these as `Math.fround` singles, so the constants carry f32 rounding;
// the channel mix is the single-precision remainder of the luminance mix.
const JITTER_MAXIMUM: f64 = 0.1_f32 as f64;
const LUMINANCE_MIX: f64 = 0.8_f32 as f64;
const CHANNEL_MIX: f64 = (1.0_f32 - 0.8_f32) as f64;
const LUMINANCE_WEIGHTS: [f64; 3] = [0.3086_f32 as f64, 0.6094_f32 as f64, 0.082_f32 as f64];

pub(crate) fn clamp_unit(value: f64) -> f64 {
    fround(value).clamp(0.0, 1.0)
}

pub(crate) fn rgb_tint(color: [f64; 3]) -> u32 {
    let channel = |value: f64| (clamp_unit(value) * 255.0).round() as u32;
    (channel(color[0]) << 16) | (channel(color[1]) << 8) | channel(color[2])
}

/// Rolls a starter appearance, consuming three float draws from `rng`.
#[must_use]
pub fn roll_native_starter_equipment_appearance(
    rng: &mut NativeRng,
    owner: StarterColorOwner,
) -> StarterEquipmentAppearance {
    let mut color = owner.base_color().map(fround);
    for channel in &mut color {
        let jitter = rng.draw_scaled_float(JITTER_MAXIMUM, false);
        *channel = clamp_unit(fround(*channel + jitter));
    }
    let luminance = fround(
        fround(color[0] * LUMINANCE_WEIGHTS[0])
            + fround(color[1] * LUMINANCE_WEIGHTS[1])
            + fround(color[2] * LUMINANCE_WEIGHTS[2]),
    );
    let primary = color.map(|channel| {
        clamp_unit(fround(fround(luminance * LUMINANCE_MIX) + fround(channel * CHANNEL_MIX)))
    });
    StarterEquipmentAppearance { primary_tint: rgb_tint(primary), secondary_tint: 0xff_ff_ff }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_roll_consumes_three_draws_and_stays_in_range() {
        let mut rng = NativeRng::new(1);
        let mut witness = NativeRng::new(1);
        let appearance =
            roll_native_starter_equipment_appearance(&mut rng, StarterColorOwner::Ether);
        for _ in 0..3 {
            witness.draw_scaled_float(JITTER_MAXIMUM, false);
        }
        assert_eq!(rng, witness, "exactly three float draws");
        assert_eq!(appearance.secondary_tint, 0xff_ff_ff);
        assert!(appearance.primary_tint <= 0xff_ff_ff);
    }
}
