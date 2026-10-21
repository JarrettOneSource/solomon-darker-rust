//! Native Harden coating from `native-harden.ts`.

use crate::float::fround;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeHardenState {
    pub armor: f64,
    pub coating: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHardenChannel {
    Water,
    WeakWater,
    Other,
    Idle,
}

impl NativeHardenState {
    pub fn step(&mut self, channel: NativeHardenChannel, armor_per_tick: f64, maximum_armor: f64) {
        if matches!(channel, NativeHardenChannel::WeakWater | NativeHardenChannel::Idle) {
            if self.coating > 0.0 {
                *self = Self::default();
            }
            return;
        }
        if channel != NativeHardenChannel::Water || armor_per_tick <= 0.0 {
            return;
        }
        self.armor = fround(maximum_armor.min(self.armor + armor_per_tick));
        self.coating = fround((self.coating + (0.005_f32 as f64)).min(1.0));
    }
}

#[must_use]
pub fn native_harden_break_angle_step(coating: f64) -> Option<f64> {
    if coating <= 0.1_f32 as f64 {
        None
    } else if coating < 0.2_f32 as f64 {
        Some(90.0)
    } else if coating < 0.3_f32 as f64 {
        Some(60.0)
    } else if coating < 0.4_f32 as f64 {
        Some(45.0)
    } else if coating < 0.5 {
        Some(35.0)
    } else {
        Some(20.0)
    }
}

#[must_use]
pub fn interpolate_native_harden_coating(
    older: f64,
    newer: f64,
    same_cast: bool,
    blend: f64,
) -> f64 {
    if same_cast && newer >= older {
        older + (newer - older) * blend
    } else if blend < 1.0 {
        older
    } else {
        newer
    }
}
