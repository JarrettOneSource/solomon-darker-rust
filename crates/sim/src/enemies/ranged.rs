//! Archer/Mage attack range and native Arrow volley construction.
//!
//! Reference: Website `core-kernels/native-enemy-targeting.ts`. Shared randomness owns
//! shot pitch and the random accuracy selector; the retained private action seed owns aim,
//! fan spread, speed and lifetime. Their draw order must remain independent.

use super::config::ArrowType;
use crate::float;
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::{NativeRng, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangedFamily {
    Archer,
    Mage,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangedAttackRange {
    pub range: f64,
    pub range_easy_pending: bool,
}

#[must_use]
pub fn construct_native_ranged_attack_range(
    family: RangedFamily,
    mode: u8,
    rng: &mut NativeRng,
) -> RangedAttackRange {
    let (base, maximum) = match family {
        RangedFamily::Archer => (280.0, 170.0),
        RangedFamily::Mage => (312.0, 150.0),
    };
    let native_base = fround(base + rng.draw_scaled_float(maximum, false));
    RangedAttackRange {
        range: apply_native_ranged_range_mode(native_base, mode),
        range_easy_pending: mode == 3,
    }
}

#[must_use]
pub fn apply_native_ranged_range_mode(range: f64, mode: u8) -> f64 {
    let range = fround(range);
    match mode {
        0 => range,
        1 | 3 => fround(range / 1.8),
        2 => fround(range * (1.5_f32 as f64)),
        _ => panic!("native ranged range mode must be within 0..3"),
    }
}

pub fn restore_native_range_easy_after_volley(range: &mut RangedAttackRange) {
    if range.range_easy_pending {
        range.range_easy_pending = false;
        range.range = fround(fround(range.range) * 1.8);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcherVolleyRequest {
    pub accuracy_mode: u8,
    pub arrow_type: ArrowType,
    pub extra_arrows: usize,
    pub multi_arrow_mode: u8,
    pub origin: Vec2,
    pub private_seed: u32,
    pub target_position: Vec2,
    pub target_velocity_per_tick: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcherArrowBirth {
    pub arrow_type: ArrowType,
    pub heading_deg: f64,
    pub lifetime_ticks: u64,
    pub position: Vec2,
    pub speed: f64,
    pub visual_heading_deg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArcherVolley {
    pub aim_offset_radius: f64,
    pub arrows: Vec<ArcherArrowBirth>,
    pub effective_accuracy_mode: u8,
    pub shot_pitch: f64,
}

/// `nativeArcherFanOffset`: centre, then alternating left/right rays ten degrees apart.
#[must_use]
pub fn native_archer_fan_offset(index: usize) -> f64 {
    if index == 0 {
        return 0.0;
    }
    let magnitude = index.div_ceil(2) as f64 * 10.0;
    if index % 2 == 1 { -magnitude } else { magnitude }
}

/// `buildNativeArcherVolley`, including leading/scatter/random aim and fan gate draws.
#[must_use]
pub fn build_native_archer_volley(
    request: ArcherVolleyRequest,
    shared_rng: &mut NativeRng,
) -> ArcherVolley {
    assert!(request.extra_arrows <= 8, "Archer extra-arrow count must be within 0..8");
    assert!(
        request.private_seed < 1_000_000,
        "Archer private seed must be within the native bound"
    );
    let pitch = shared_rng.draw_scaled_float(0.1_f32 as f64, true);
    let effective_accuracy_mode = if request.accuracy_mode == 3 {
        shared_rng.draw_integer(3) as u8
    } else {
        request.accuracy_mode
    };
    let mut private_rng = NativeRng::new(i64::from(request.private_seed));
    let dx = fround(request.origin.x - request.target_position.x);
    let dy = fround(request.origin.y - request.target_position.y);
    let source_target_distance = fround(hypot(dx, dy));
    let mut aim_point =
        Vec2::new(fround(request.target_position.x), fround(request.target_position.y));
    let mut aim_offset_radius = 0.0;
    if effective_accuracy_mode == 1 {
        let lead_ticks = fround(source_target_distance / 6.0);
        aim_point.x = fround(aim_point.x + fround(lead_ticks * request.target_velocity_per_tick.x));
        aim_point.y = fround(aim_point.y + fround(lead_ticks * request.target_velocity_per_tick.y));
    } else if effective_accuracy_mode == 2 {
        aim_offset_radius = private_rng.draw_scaled_float(75.0, false);
        let heading = private_rng.draw_scaled_float(360.0, false);
        let direction = heading_vector(heading);
        aim_point.x = fround(aim_point.x + fround(aim_offset_radius * direction.x));
        aim_point.y = fround(aim_point.y + fround(aim_offset_radius * direction.y));
    }
    let threshold = match request.multi_arrow_mode {
        0 => 0,
        1 => 15,
        2 => 50,
        3 => 100,
        _ => panic!("native multi-arrow mode must be within 0..3"),
    };
    let extra_arrow_count = if threshold > 0 && private_rng.draw_integer(100) <= threshold {
        request.extra_arrows
    } else {
        0
    };
    let base_heading = fround(heading_degrees_from_vector(
        fround(aim_point.x - request.origin.x),
        fround(aim_point.y - request.origin.y),
    ));
    let mut arrows = Vec::with_capacity(extra_arrow_count + 1);
    for index in 0..=extra_arrow_count {
        let fan_offset = native_archer_fan_offset(index);
        let jittered = if fan_offset == 0.0 {
            0.0
        } else {
            private_rng.draw_float_range(fround(fan_offset * 0.9), fround(fan_offset * 1.1))
        };
        let heading_deg = positive_degrees(fround(base_heading + jittered));
        let direction = heading_vector(heading_deg);
        let speed = fround(
            (5.7_f32 as f64) + private_rng.draw_scaled_float(0.599_999_904_632_568_4, false),
        );
        let lifetime_draw = private_rng.draw_scaled_float(100.0, false);
        let lifetime_ticks =
            ((source_target_distance + 100.0 + lifetime_draw) / speed).round_ties_even() as u64;
        arrows.push(ArcherArrowBirth {
            arrow_type: request.arrow_type,
            heading_deg,
            lifetime_ticks,
            position: Vec2::new(
                fround(request.origin.x + fround(direction.x * 30.0)),
                fround(request.origin.y + fround(direction.y * 30.0)),
            ),
            speed,
            visual_heading_deg: heading_deg,
        });
    }
    ArcherVolley {
        aim_offset_radius,
        arrows,
        effective_accuracy_mode,
        shot_pitch: fround(1.0 + pitch),
    }
}

#[must_use]
pub fn native_enemy_projectile_velocity(heading_deg: f64, speed: f64) -> Vec2 {
    let direction = heading_vector(heading_deg);
    Vec2::new(fround(direction.x * speed), fround(direction.y * speed))
}

fn heading_vector(heading: f64) -> Vec2 {
    let radians = heading * std::f64::consts::PI / 180.0;
    Vec2::new(fround(float::sin(radians)), fround(-float::cos(radians)))
}

fn positive_degrees(value: f64) -> f64 {
    fround(((value % 360.0) + 360.0) % 360.0)
}
