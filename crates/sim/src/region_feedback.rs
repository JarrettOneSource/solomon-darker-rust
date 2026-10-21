//! Camera attenuation from `core-kernels/native-region-point-gain.ts`.
//! The hit lane has different inner/outer distances and alternate-player rules.

use crate::Vec2;
use crate::float::{fround, hypot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeRegionCameraShakeAttenuation {
    Fixed,
    Point,
    Hit,
    HitSquared,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeRegionCameraShake {
    pub attenuation: NativeRegionCameraShakeAttenuation,
    pub displacement: Vec2,
}

#[must_use]
pub fn native_region_hit_point_gain(
    position: Vec2,
    camera_center: Vec2,
    visible_world_width: f64,
    local_player_alternate: bool,
) -> f64 {
    let width = if visible_world_width > 0.0 {
        fround(visible_world_width)
    } else {
        return 0.0;
    };
    let distance =
        fround(hypot(fround(position.x - camera_center.x), fround(position.y - camera_center.y)));
    let inner = fround(width * (0.1_f32 as f64));
    let outer = fround(width * 0.5);
    // The native early full-gain return deliberately bypasses alternate attenuation.
    if distance < inner {
        return 1.0;
    }
    if distance > outer {
        return 0.0;
    }
    let gain = fround(1.0 - fround(fround(distance - inner) / fround(outer - inner)));
    if local_player_alternate { fround(gain * (0.1_f32 as f64)) } else { gain }
}

#[must_use]
pub fn native_region_point_gain(
    position: Vec2,
    camera_center: Vec2,
    visible_world_width: f64,
    local_player_alternate: bool,
) -> f64 {
    let width = if visible_world_width > 0.0 {
        fround(visible_world_width)
    } else {
        return 0.0;
    };
    let distance =
        fround(hypot(fround(position.x - camera_center.x), fround(position.y - camera_center.y)));
    let full_gain_distance = fround(width * (0.25_f32 as f64));
    let zero_gain_distance = fround(width * (1.1_f32 as f64));
    let gain = if distance <= full_gain_distance {
        1.0
    } else if distance >= zero_gain_distance {
        0.0
    } else {
        fround(
            fround(zero_gain_distance - distance) / fround(zero_gain_distance - full_gain_distance),
        )
    };
    if local_player_alternate { fround(gain * (0.1_f32 as f64)) } else { gain }
}
