//! Native Hurricane charge, eight-lane presentation, contact cadence and orbit
//! force from `core-kernels/native-hurricane.ts`.

use crate::float::fround;
use crate::{NativeRng, Vec2};

pub const NATIVE_HURRICANE_CONTACT_RADIUS: f64 = 280.0;
pub const NATIVE_HURRICANE_FULL_FORCE_RADIUS: f64 = 100.0;
pub const NATIVE_HURRICANE_FALLOFF_WIDTH: f64 = 180.0;
pub const NATIVE_HURRICANE_FORCE_PER_TICK: f64 = 1.5;
pub const NATIVE_HURRICANE_DEFAULT_MOVEMENT_STEP: u64 = 10;
pub const NATIVE_HURRICANE_CONTACT_COOLDOWN: u64 = 100;
pub const NATIVE_HURRICANE_LOW_CHARGE_THRESHOLD: f64 = 0.5;
pub const NATIVE_HURRICANE_LANE_COUNT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHurricaneChargeTick {
    pub contact_charge: f64,
    pub next_charge: f64,
    pub refreshed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeHurricaneLane {
    pub angle_degrees: f64,
    pub angular_velocity_degrees: f64,
    pub radius: f64,
    pub vertical_offset: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHurricanePresentationProgram {
    pub lanes: [NativeHurricaneLane; NATIVE_HURRICANE_LANE_COUNT],
    pub phase_degrees: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHurricaneDamageResult {
    pub damage: f64,
    pub suppress_hit_sound: bool,
}

/// Early PlayerWizard decay precedes contacts; the later Lightning channel
/// refresh supplies the next charge. Profile runtime calls this same function.
#[must_use]
pub fn native_hurricane_charge_tick(
    charge: f64,
    was_refreshed: bool,
    enabled: bool,
    lightning_active: bool,
) -> NativeHurricaneChargeTick {
    assert!(
        charge.is_finite() && (0.0..=1.0).contains(&charge),
        "Hurricane charge is inside [0,1]"
    );
    if !enabled {
        return NativeHurricaneChargeTick {
            contact_charge: 0.0,
            next_charge: 0.0,
            refreshed: false,
        };
    }
    let contact_charge =
        if was_refreshed { fround(charge) } else { fround(charge - (0.03_f32 as f64)).max(0.0) };
    NativeHurricaneChargeTick {
        contact_charge,
        next_charge: if lightning_active {
            fround(contact_charge + (0.0015_f32 as f64)).min(1.0)
        } else {
            contact_charge
        },
        refreshed: lightning_active,
    }
}

/// Exactly two draws per lane, including lanes whose current radius is small.
pub fn create_native_hurricane_presentation(
    rng: &mut NativeRng,
) -> NativeHurricanePresentationProgram {
    let mut velocity = 10.0;
    let mut radius = 1.5;
    let lanes = std::array::from_fn(|_| {
        let angle = rng.draw_scaled_float(360.0, false);
        let vertical = rng.draw_scaled_float(15.0, false);
        let lane = NativeHurricaneLane {
            angle_degrees: angle,
            angular_velocity_degrees: velocity,
            radius,
            vertical_offset: vertical,
        };
        velocity = fround(velocity * 0.75);
        radius = fround(radius * (1.2_f32 as f64));
        lane
    });
    NativeHurricanePresentationProgram { lanes, phase_degrees: 0.0 }
}

pub fn step_native_hurricane_presentation(
    program: &mut NativeHurricanePresentationProgram,
    charge: f64,
    rng: &mut NativeRng,
) {
    require_active_charge(charge);
    let phase_step = rng.draw_float_range(2.0, 3.0);
    for lane in &mut program.lanes {
        lane.angle_degrees =
            fround(lane.angle_degrees + lane.angular_velocity_degrees * charge * 0.75);
    }
    program.phase_degrees = fround(program.phase_degrees + phase_step * charge);
}

#[must_use]
pub fn native_hurricane_movement_due(serial: u64, tick: u64, movement_step: u64) -> bool {
    require_movement_step(movement_step);
    assert!(
        serial <= 9_007_199_254_740_991 && tick <= 9_007_199_254_740_991,
        "Hurricane serial and tick are non-negative safe integers"
    );
    serial % movement_step == tick % movement_step
}

#[must_use]
pub fn native_hurricane_orbit_force(
    source: Vec2,
    target: Vec2,
    charge: f64,
    movement_step: u64,
) -> Option<Vec2> {
    require_active_charge(charge);
    require_movement_step(movement_step);
    let dx = fround(source.x - target.x);
    let dy = fround(source.y - target.y);
    let squared = fround(dx * dx + dy * dy);
    if squared >= NATIVE_HURRICANE_CONTACT_RADIUS * NATIVE_HURRICANE_CONTACT_RADIUS {
        return None;
    }
    if squared == 0.0 {
        return Some(Vec2::ZERO);
    }
    let distance = native_hurricane_fast_distance(squared);
    let falloff = fround(
        1.0 - (fround(distance - NATIVE_HURRICANE_FULL_FORCE_RADIUS).max(0.0)
            / NATIVE_HURRICANE_FALLOFF_WIDTH)
            .min(1.0),
    );
    let tangent = squared.sqrt();
    let magnitude = movement_step as f64 * charge * NATIVE_HURRICANE_FORCE_PER_TICK * falloff;
    Some(Vec2::new(fround(dy / tangent * magnitude), fround(-dx / tangent * magnitude)))
}

pub fn draw_native_hurricane_damage(
    rng: &mut NativeRng,
    charge: f64,
    minimum: f64,
    maximum: f64,
) -> NativeHurricaneDamageResult {
    require_active_charge(charge);
    let damage = rng.draw_float_range(minimum, maximum);
    NativeHurricaneDamageResult {
        damage: fround(charge * charge * charge * damage),
        suppress_hit_sound: charge < NATIVE_HURRICANE_LOW_CHARGE_THRESHOLD,
    }
}

/// Shared native reciprocal-square-root approximation (`nativeHurricaneFastDistance`).
#[must_use]
pub fn native_hurricane_fast_distance(squared: f64) -> f64 {
    let half = fround(squared * 0.5);
    let bits = 0x5f3759df_u32.wrapping_sub((squared as f32).to_bits() >> 1);
    let mut estimate = f64::from(f32::from_bits(bits));
    estimate = fround((1.5 - estimate * estimate * half) * estimate);
    fround(1.0 / estimate)
}

fn require_active_charge(charge: f64) {
    assert!(
        charge.is_finite() && charge > 0.0 && charge <= 1.0,
        "active Hurricane charge is inside (0,1]"
    );
}
fn require_movement_step(step: u64) {
    assert!(
        step > 0 && step <= 9_007_199_254_740_991,
        "Hurricane movement step is a positive safe integer"
    );
}
