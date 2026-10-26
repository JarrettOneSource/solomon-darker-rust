//! Boulder growth, contact pool, orientation and called rocks from `native-earth-boulder.ts`,
//! `primary-spell-earth-orientation.ts` and the Earth helpers in `primary-spells.ts`.

use super::model::PrimarySpellTransientBase;
use super::model::{
    EarthBoulderOrientation, EarthProjectileProgram, PrimarySpellEarthCalledRock,
    PrimarySpellProjectilePhase, ProjectileRef,
};
use super::visual::visual_unit_random;
use crate::float;
use crate::painter::PainterRegistration;

pub use super::earth_fragments::{
    EarthFragmentState, EarthImpactSeed, earth_impact_fragment_count,
    earth_impact_fragments_at_age, earth_impact_lifetime_ticks,
};
use crate::Vec2;
use crate::float::{fround, hypot, hypot3};

pub const EARTH_BOULDER_IDENTITY_ORIENTATION: EarthBoulderOrientation =
    [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
pub const NATIVE_EARTH_BOULDER_TRAVERSAL_POOL_THRESHOLD: f64 = 0.001;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeEarthBoulderContactInput {
    pub release_base_damage: f64,
    pub release_charge: f64,
    pub remaining_pool: f64,
    pub target_health: f64,
    pub toughness: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeEarthBoulderContactResult {
    pub charge: f64,
    pub continue_traversal: bool,
    pub damage: f64,
    pub depleted: bool,
    pub remaining_pool: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarthBoulderVector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[must_use]
pub fn advance_native_earth_boulder_charge(
    charge: f64,
    growth_factor: f64,
    maximum_charge: f64,
    cast_speed: f64,
) -> f64 {
    for value in [charge, growth_factor, maximum_charge, cast_speed] {
        require_nonnegative(value);
    }
    let growth = fround(fround(0.5 * cast_speed) * growth_factor);
    let increment = fround(growth * 0.0025);
    fround(charge + increment).min(maximum_charge)
}

#[must_use]
pub fn native_earth_boulder_released_damage(
    base_damage: f64,
    charge: f64,
    release_factor: f64,
) -> f64 {
    require_nonnegative(base_damage);
    require_nonnegative(charge);
    assert!(
        release_factor == 0.5 || release_factor == 1.0,
        "Boulder release factor is half or full"
    );
    let release_base = base_damage * release_factor;
    let base_charge = fround(release_base * charge);
    let quadratic = fround(base_charge * charge);
    quadratic.min(release_base * 1.25).max(0.25)
}

#[must_use]
pub fn consume_native_earth_boulder_contact(
    input: NativeEarthBoulderContactInput,
) -> NativeEarthBoulderContactResult {
    for value in
        [input.release_base_damage, input.release_charge, input.remaining_pool, input.target_health]
    {
        require_nonnegative(value);
    }
    assert!(
        input.toughness.is_finite() && input.toughness > 0.0,
        "Boulder toughness is finite and positive"
    );
    let damage = fround(input.target_health.min(input.remaining_pool));
    let consumed = fround(if input.remaining_pool < input.target_health {
        damage
    } else {
        damage / (2.0 * input.toughness)
    });
    let remaining_pool = fround(input.remaining_pool - consumed).max(0.0);
    let diminished_charge =
        input.release_charge * (1.0 - (1.0 - remaining_pool / input.release_base_damage) * 0.35);
    // Math.min propagates the source's 0/0 at an empty release pool; Rust min ignores NaN.
    let charge = if diminished_charge.is_nan() {
        f64::NAN
    } else {
        fround(input.release_charge.min(diminished_charge)).min(input.release_charge)
    };
    NativeEarthBoulderContactResult {
        charge,
        continue_traversal: remaining_pool > NATIVE_EARTH_BOULDER_TRAVERSAL_POOL_THRESHOLD,
        damage,
        depleted: remaining_pool <= 0.0,
        remaining_pool,
    }
}

#[must_use]
pub fn native_percent_roll_succeeds(chance_percent: f64, draw_percent: f64) -> bool {
    assert!(
        (0.0..=100.0).contains(&chance_percent) && (0.0..=100.0).contains(&draw_percent),
        "native percentages lie in zero through one hundred"
    );
    draw_percent < chance_percent
}

#[must_use]
pub fn native_earth_boulder_rock_hit_pitch(charge: f64) -> f64 {
    require_nonnegative(charge);
    assert!(charge > 0.0, "Boulder terminal charge is positive");
    fround(1.0 + 0.05 / charge)
}

#[must_use]
pub fn native_earth_boulder_stone_break_pitch(charge: f64) -> f64 {
    require_nonnegative(charge);
    fround(1.0 - charge * 0.5)
}

#[must_use]
pub fn earth_boulder_held_orientation_step(
    orientation: EarthBoulderOrientation,
    direction: Vec2,
) -> EarthBoulderOrientation {
    rotate_orientation(orientation, orientation_axis(direction), 0.75)
}

#[must_use]
pub fn earth_boulder_flight_orientation_step(
    orientation: EarthBoulderOrientation,
    direction: Vec2,
    stored_delta: Vec2,
    charge: f64,
) -> EarthBoulderOrientation {
    let degrees = fround(hypot(stored_delta.x, stored_delta.y) / charge);
    rotate_orientation(orientation, orientation_axis(direction), degrees)
}

#[must_use]
pub fn earth_boulder_transform_point(
    point: EarthBoulderVector3,
    orientation: EarthBoulderOrientation,
) -> EarthBoulderVector3 {
    EarthBoulderVector3 {
        x: point.x * orientation[0] + point.y * orientation[3] + point.z * orientation[6],
        y: point.x * orientation[1] + point.y * orientation[4] + point.z * orientation[7],
        z: point.x * orientation[2] + point.y * orientation[5] + point.z * orientation[8],
    }
}

fn orientation_axis(direction: Vec2) -> EarthBoulderVector3 {
    let normalizer = 1.64_f64.sqrt();
    let x = -direction.y;
    let y = direction.x / normalizer;
    let z = 0.8 * direction.x / normalizer;
    let length = hypot3(x, y, z);
    EarthBoulderVector3 { x: fround(x / length), y: fround(y / length), z: fround(z / length) }
}

fn rotate_orientation(
    orientation: EarthBoulderOrientation,
    axis: EarthBoulderVector3,
    degrees: f64,
) -> EarthBoulderOrientation {
    let radians = degrees * std::f64::consts::PI / 180.0;
    let cosine = float::cos(radians);
    let sine = float::sin(radians);
    let complement = 1.0 - cosine;
    let EarthBoulderVector3 { x, y, z } = axis;
    let rotation = [
        fround(cosine + x * x * complement),
        fround(x * y * complement + z * sine),
        fround(x * z * complement - y * sine),
        fround(y * x * complement - z * sine),
        fround(cosine + y * y * complement),
        fround(y * z * complement + x * sine),
        fround(z * x * complement + y * sine),
        fround(z * y * complement - x * sine),
        fround(cosine + z * z * complement),
    ];
    let mut result = [0.0; 9];
    for row in 0..3 {
        let offset = row * 3;
        for column in 0..3 {
            result[offset + column] = fround(
                orientation[offset] * rotation[column]
                    + orientation[offset + 1] * rotation[3 + column]
                    + orientation[offset + 2] * rotation[6 + column],
            );
        }
    }
    result
}

#[must_use]
pub fn earth_called_rock_emits(
    boulder: ProjectileRef<'_, EarthProjectileProgram>,
    tick: u64,
) -> bool {
    boulder.common.charge < 0.25
        || earth_visual_random_int(boulder.common.id, 0x3000_u32.wrapping_add(tick as u32), 3) == 1
}

#[must_use]
pub fn create_earth_called_rock(
    id: u64,
    boulder: ProjectileRef<'_, EarthProjectileProgram>,
    registration: PainterRegistration,
) -> PrimarySpellEarthCalledRock {
    let angle = visual_unit_random(id, 0x4000) * std::f64::consts::PI * 2.0;
    let spawn_radius =
        visual_unit_random(id, 0x5000) * (50.0 * boulder.common.charge).clamp(5.0, 120.0);
    PrimarySpellEarthCalledRock {
        common: PrimarySpellTransientBase {
            age_ticks: 0,
            id,
            light_registration: None,
            owner_id: boulder.common.owner_id.clone(),
            painter_registrations: vec![registration],
            world_key: boulder.common.world_key.clone(),
        },
        falling: false,
        fall_velocity: 0.0,
        height: -2.0,
        lateral_magnitude: fround(visual_unit_random(id, 0x6000) * 4.0),
        parent_id: boulder.common.id,
        position: Vec2::new(
            fround(boulder.common.position.x + float::cos(angle) * spawn_radius),
            fround(boulder.common.position.y + float::sin(angle) * spawn_radius),
        ),
        rotation: fround(visual_unit_random(id, 0x7000) * 360.0),
        rotation_step: fround((visual_unit_random(id, 0x7100) * 2.0 - 1.0) * 30.0),
        scale: fround(0.75 * boulder.common.charge.min(0.75)),
        speed: 0.1_f32 as f64,
        target_height: fround(
            -40.0 - 30.0 * boulder.common.charge + visual_unit_random(id, 0x7200) * 5.0,
        ),
        variant: earth_visual_random_int(id, 0x7300, 3),
    }
}

pub fn advance_earth_called_rock(
    mut rock: PrimarySpellEarthCalledRock,
    parent: Option<ProjectileRef<'_, EarthProjectileProgram>>,
) -> Option<PrimarySpellEarthCalledRock> {
    if rock.falling {
        let height = fround(rock.height + rock.fall_velocity);
        let accelerated = fround(rock.fall_velocity + 1.0);
        if height > 10.0 {
            return None;
        }
        rock.common.age_ticks += 1;
        rock.fall_velocity = if height > 0.0 { 0.25_f32 as f64 } else { accelerated };
        rock.height = height;
        return Some(rock);
    }
    rock.speed = fround(rock.speed * 1.1_f32 as f64).min(5.0);
    rock.falling =
        parent.is_none_or(|parent| parent.common.phase != PrimarySpellProjectilePhase::Held);
    if !rock.falling
        && let Some(parent) = parent
    {
        let dx = parent.common.position.x - rock.position.x;
        let dy = parent.common.position.y - rock.position.y;
        let distance = hypot(dx, dy);
        if distance < 5.0 {
            return None;
        }
        let toward =
            if distance > 0.0 { Vec2::new(dx / distance, dy / distance) } else { Vec2::ZERO };
        rock.position = Vec2::new(
            fround(rock.position.x + toward.x * rock.speed),
            fround(rock.position.y + toward.y * rock.speed),
        );
        let next_dx = parent.common.position.x - rock.position.x;
        let next_dy = parent.common.position.y - rock.position.y;
        let next_distance = hypot(next_dx, next_dy);
        if next_distance > 0.0 {
            rock.position = Vec2::new(
                fround(rock.position.x - next_dy / next_distance * rock.lateral_magnitude),
                fround(rock.position.y + next_dx / next_distance * rock.lateral_magnitude),
            );
        }
    }
    let difference = rock.target_height - rock.height;
    if difference != 0.0 {
        rock.height = fround(rock.height + difference.signum() * 1.5);
    }
    rock.common.age_ticks += 1;
    rock.rotation = fround(rock.rotation + rock.rotation_step);
    Some(rock)
}

#[must_use]
pub fn earth_visual_random_int(id: u64, salt: u32, exclusive_maximum: u32) -> u32 {
    (visual_unit_random(id, salt) * f64::from(exclusive_maximum)).floor() as u32
}

fn require_nonnegative(value: f64) {
    assert!(value.is_finite() && value >= 0.0, "Boulder values must be finite and nonnegative");
}
