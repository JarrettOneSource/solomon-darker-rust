//! Flame Lash birth samples, fade alpha and the camera displacement a main lash causes.
//! Birth samples are authoritative data. Rendering never guesses missing legacy
//! samples or consumes the gameplay stream to reconstruct them.

use crate::float::{cos, fround, sin};
use crate::painter::PainterRegistration;
use crate::{Rng, Vec2};

/// Inclusive magnitude four reaches the stored cutoff on update eight. Birth
/// already owns update one, leaving seven presentation updates to retain.
pub const FLAME_CAMERA_MAX_PRESENTATION_TICKS: u64 = 7;

/// Whether a Flame Lash ribbon is the main lash or a chained one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "lowercase"))]
pub enum FlameChannelRole {
    Main,
    Chain,
}

/// A ribbon's birth sample: a width factor for a short lash, or a phase for a long one.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "kind", rename_all = "lowercase"))]
pub enum FlameRibbonBirth {
    Short { width_factor: f64 },
    Long { phase_degrees: f64 },
}

/// A lash path's birth samples: light intensity, per-point radius factors and painter
/// registration.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct FlamePathBirth {
    pub intensity: f64,
    pub radius_factors: Vec<f64>,
    pub creator_registration: PainterRegistration,
    pub misc_append_ordinal: usize,
}

/// Everything sampled when a Flame Lash ribbon is born: role, path points, width, ribbon
/// and path samples, and camera displacement.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct FlameChannelBirth {
    pub role: FlameChannelRole,
    pub camera_displacement: Option<Vec2>,
    pub start: Vec2,
    pub control: Vec2,
    pub end: Vec2,
    pub offset: Vec2,
    pub width: f64,
    pub ribbon: FlameRibbonBirth,
    pub path: FlamePathBirth,
}

/// A channel's Flame Lash visual: its birth samples, or suppressed for a legacy record
/// saved without them.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "kind"))]
pub enum FlameChannelVisual {
    #[default]
    #[cfg_attr(feature = "persistence", serde(rename = "legacy-suppressed"))]
    LegacySuppressed,
    #[cfg_attr(feature = "persistence", serde(rename = "native-birth-v1"))]
    Birth(FlameChannelBirth),
}

/// A Flame Lash fade's birth alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct FlameFadeBirth {
    pub birth_alpha: f64,
}

/// A Flame Lash fade's visual: its birth alpha, or suppressed for a legacy record saved
/// without it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(tag = "kind"))]
pub enum FlameFadeVisual {
    #[default]
    #[cfg_attr(feature = "persistence", serde(rename = "legacy-suppressed"))]
    LegacySuppressed,
    #[cfg_attr(feature = "persistence", serde(rename = "native-birth-v1"))]
    Birth(FlameFadeBirth),
}

/// A camera displacement that a recent main Flame Lash proposes for the display, with the
/// lash's ID, birth tick and world.
#[derive(Clone, Debug, PartialEq)]
pub struct FlameCameraProposal {
    pub id: u64,
    pub birth_tick: u64,
    pub world_key: String,
    pub displacement: Vec2,
}

/// Native children have already received update one in a birth-tick snapshot.
#[must_use]
pub fn flame_fade_alpha(birth_alpha: f64, alpha_step: f64, age_ticks: u64) -> f64 {
    let mut alpha = birth_alpha;
    for _ in 0..=age_ticks {
        alpha = fround(alpha - alpha_step);
    }
    alpha
}

#[must_use]
pub fn flame_fade_wrapper_intensity(birth_alpha: f64, age_ticks: u64) -> f64 {
    let mut intensity = birth_alpha;
    for _ in 0..=age_ticks {
        intensity = fround(intensity - 0.05_f32 as f64);
    }
    intensity
}

/// Normal-only Float(4), then unsigned UnitVector. A weak cast draws no words.
pub fn draw_flame_camera_displacement(rng: &mut Rng, underpowered: bool) -> Option<Vec2> {
    if underpowered {
        return None;
    }
    let magnitude = rng.draw_scaled_float(4.0, false);
    let angle = rng.draw_scaled_float(360.0, false);
    // Recovered Math+4, rather than double-precision PI or fround(PI).
    let radians = fround(angle * 3.141592502593994 / 180.0);
    Some(Vec2::new(
        fround(magnitude * fround(sin(radians))),
        fround(magnitude * -fround(cos(radians))),
    ))
}

#[must_use]
pub fn camera_magnitude_squared(value: Vec2) -> f64 {
    fround(value.x * value.x + value.y * value.y)
}

/// Same-pass native decay. Equality at the stored .25 cutoff survives.
#[must_use]
pub fn decay_region_camera_displacement(value: Vec2, updates: u64) -> Vec2 {
    let mut x = fround(value.x);
    let mut y = fround(value.y);
    for _ in 0..updates {
        x = fround(x * 0.75);
        y = fround(y * 0.75);
        if camera_magnitude_squared(Vec2::new(x, y)) < 0.25 {
            return Vec2::ZERO;
        }
    }
    Vec2::new(x, y)
}
