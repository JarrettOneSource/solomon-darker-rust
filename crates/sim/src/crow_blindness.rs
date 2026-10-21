//! Crow contact duration and Arena_Render opacity from `native-crow-blindness.ts`.

use crate::float;
use crate::float::fround;
use crate::{NativeRng, Vec2};

/// The native float duration is measured in ticks but is deliberately not
/// integral. Player lighting subtracts one each tick and retains the fraction.
pub fn native_crow_blindness(
    current_health: f64,
    remaining_shield: f64,
    chance_percent: f64,
    rng: &mut NativeRng,
) -> f64 {
    if current_health <= 10.0 || remaining_shield > 0.0 {
        return 0.0;
    }
    if f64::from(rng.draw_integer(100)) >= chance_percent {
        return 0.0;
    }
    let duration = rng.draw_float_range(1.0, 2.5);
    fround(fround(duration + 1.0) * 100.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeCrowBlindnessOpacity {
    pub cover: f64,
    pub stamp: f64,
}

#[must_use]
pub fn native_crow_blindness_opacity(ticks_remaining: f64) -> NativeCrowBlindnessOpacity {
    let cover = fround(ticks_remaining / 100.0).clamp(0.0, 1.0);
    NativeCrowBlindnessOpacity { cover, stamp: fround(cover.powf(4.0)) }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeCrowBlindnessFrame {
    pub cover_alpha: f64,
    pub stamp_alpha: f64,
    pub stamp_position: Vec2,
}

/// `NativeCrowBlindnessView.update` uses a private frame-seeded presentation RNG,
/// independent of the stream that decided the contact duration.
#[must_use]
pub fn native_crow_blindness_frame(
    ticks_remaining: f64,
    application_time_ms: f64,
    viewport: Vec2,
) -> Option<NativeCrowBlindnessFrame> {
    let opacity = native_crow_blindness_opacity(ticks_remaining);
    if opacity.cover <= 0.0 {
        return None;
    }
    let mut rng = NativeRng::new(application_time_ms.floor() as i64);
    let flicker = rng.draw_scaled_float(0.19999998807907104, false);
    let radius = rng.draw_scaled_float(2.0, false);
    let radians = rng.draw_scaled_float(360.0, false) * std::f64::consts::PI / 180.0;
    Some(NativeCrowBlindnessFrame {
        cover_alpha: opacity.cover,
        stamp_alpha: opacity.stamp * (0.800000011920929 + flicker),
        stamp_position: Vec2::new(
            viewport.x / 2.0 + float::sin(radians) * radius,
            viewport.y / 2.0 - float::cos(radians) * radius,
        ),
    })
}
