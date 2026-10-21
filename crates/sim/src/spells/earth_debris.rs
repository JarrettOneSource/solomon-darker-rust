//! The native Boulder contact child and lit-bit physics from
//! `core-kernels/native-weld-boulder-debris.ts`. Ordinary Earth uses this same factory.

use crate::float;
use crate::float::fround;
use crate::{NativeRng, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldMeteorDebrisSeed {
    pub alpha: f64,
    pub color_green: f64,
    pub height: f64,
    pub index: usize,
    pub position: Vec2,
    pub record: usize,
    pub rotation_degrees: f64,
    pub rotation_step_degrees: f64,
    pub scale: f64,
    pub velocity: Vec2,
    pub vertical_velocity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldBoulderDebrisParticleState {
    pub common: NativeWeldMeteorDebrisSeed,
    pub bounce_velocity: f64,
    pub enhanced_shadow: bool,
}

/// EBoulder's terminal vslot, after its separately owned Ether fade.
pub fn create_native_weld_ethereal_boulder_breakup_debris_program(
    scale: f64,
    rng: &mut NativeRng,
) -> Vec<NativeWeldMeteorDebrisSeed> {
    create_ethereal_boulder_debris_program(scale, None, rng)
}

/// Weak held EBoulder at `0x0054572C..0x00545B0C` sheds its extra pieces.
pub fn create_native_weld_ethereal_boulder_weak_debris_program(
    direction: Vec2,
    scale: f64,
    rng: &mut NativeRng,
) -> Vec<NativeWeldMeteorDebrisSeed> {
    create_ethereal_boulder_debris_program(scale, Some(direction), rng)
}

// These two source factories have the same ordered constructor draws. Their
// spawn offsets, count rounding and final size are the three native differences.
fn create_ethereal_boulder_debris_program(
    input_scale: f64,
    weak_direction: Option<Vec2>,
    rng: &mut NativeRng,
) -> Vec<NativeWeldMeteorDebrisSeed> {
    let scale = input_scale.min(1.0);
    let count_scalar = fround(input_scale * 30.0).max(8.0);
    let count = if weak_direction.is_some() {
        count_scalar.round_ties_even() as usize
    } else {
        count_scalar.trunc() as usize
    };
    let angular_step = fround(360.0 / count_scalar);
    let mut angle = rng.draw_scaled_float(360.0, false);
    let mut debris = Vec::with_capacity(count);
    for index in 0..count {
        let bounce = rng.draw_scaled_float(3.0, false);
        rng.draw_scaled_float(20.0, false);
        let rotation = rng.draw_scaled_float(360.0, false);
        let rotation_step = rng.draw_scaled_float(10.0, false);
        let color = rng.draw_scaled_float(0.5, false);
        let record = rng.draw_integer(3) as usize;
        let vertical_scale = rng.draw_scaled_float(fround(scale * 1.5), false);
        let height = rng.draw_scaled_float(fround(scale * 50.0), false);
        let spawn_radius = if weak_direction.is_some() { 40.0 } else { fround(input_scale * 45.0) };
        let spawn_distance = rng.draw_scaled_float(spawn_radius, false);
        let scale_probe = rng.draw_scaled_float(0.75, false);
        let scale_candidate = fround(fround(scale_probe + 0.5) * input_scale);
        let mut native_scale = 0.45_f32 as f64;
        if scale_candidate >= native_scale {
            // Retail's MAX macro evaluates the selected random argument twice.
            let selected = rng.draw_scaled_float(0.75, false);
            native_scale = fround(fround(selected + 0.5) * input_scale);
        }
        native_scale = native_scale.min(0.75_f32 as f64);
        let motion_scale = rng.draw_scaled_float(fround(input_scale * 1.5), false);
        let angle_jitter = rng.draw_scaled_float(fround(angular_step / 3.0), true);
        let radians = angle * std::f64::consts::PI / 180.0;
        let radial = Vec2::new(fround(float::sin(radians)), fround(-float::cos(radians)));
        let movement = fround(motion_scale + 1.5);
        let (position, velocity) = if let Some(direction) = weak_direction {
            (
                Vec2::new(
                    fround(direction.x * 20.0 + radial.x * spawn_distance),
                    fround(direction.y * 20.0 + radial.y * 0.8_f32 as f64 * spawn_distance),
                ),
                Vec2::new(
                    fround(radial.x * movement),
                    fround(radial.y * 0.8_f32 as f64 * movement),
                ),
            )
        } else {
            let direction = Vec2::new(radial.x, fround(radial.y * 0.8_f32 as f64));
            (
                Vec2::new(
                    fround(direction.x * spawn_distance),
                    fround(direction.y * spawn_distance),
                ),
                Vec2::new(fround(direction.x * movement), fround(direction.y * movement)),
            )
        };
        debris.push(NativeWeldMeteorDebrisSeed {
            alpha: 2.0,
            color_green: color,
            height: fround(-height),
            index,
            position,
            record: 2008 + record,
            rotation_degrees: rotation,
            rotation_step_degrees: fround(rotation_step + 1.0),
            scale: if weak_direction.is_some() {
                fround(native_scale * 0.75)
            } else {
                native_scale
            },
            velocity,
            vertical_velocity: fround(fround(-(bounce + 2.0)) * fround(vertical_scale + 0.75)),
        });
        angle = fround(angle + angular_step + angle_jitter);
    }
    debris
}

#[must_use]
pub fn create_native_weld_boulder_contact_debris_program(
    scale: f64,
    rng: &mut NativeRng,
) -> NativeWeldMeteorDebrisSeed {
    let angle = rng.draw_scaled_float(360.0, false);
    let bounce = rng.draw_scaled_float(3.0, false);
    let height = rng.draw_scaled_float(20.0, false);
    let rotation = rng.draw_scaled_float(360.0, false);
    let rotation_step = rng.draw_scaled_float(10.0, false);
    let color = rng.draw_scaled_float(0.5, false);
    let record = rng.draw_integer(3) as usize;
    // This constructor field is overwritten, but its draw remains in the shared stream.
    rng.draw_scaled_float(2.0, false);
    let distance = rng.draw_scaled_float(10.0, false);
    let first_scale_probe = rng.draw_scaled_float(0.75, false);
    let first_scale_candidate = fround(scale * fround(first_scale_probe + 0.5));
    let mut native_scale = 0.45_f32 as f64;
    if first_scale_candidate >= native_scale {
        // Retail's MAX macro evaluates the random argument again after comparison.
        let selected_scale_probe = rng.draw_scaled_float(0.75, false);
        native_scale = fround(scale * fround(selected_scale_probe + 0.5));
    }
    let radians = angle * std::f64::consts::PI / 180.0;
    let radial = Vec2::new(fround(float::sin(radians)), fround(-float::cos(radians)));
    let velocity = Vec2::new(fround(radial.x * 1.5), radial.y);
    let spawn_distance = fround(distance + 10.0);
    NativeWeldMeteorDebrisSeed {
        alpha: 2.0,
        color_green: color,
        height: fround(-height),
        index: 0,
        position: Vec2::new(
            fround(velocity.x * spawn_distance),
            fround(velocity.y * spawn_distance),
        ),
        record: 2008 + record,
        rotation_degrees: rotation,
        rotation_step_degrees: fround(rotation_step + 1.0),
        scale: fround(native_scale * 0.5).min(0.75_f32 as f64),
        velocity: Vec2::new(fround(velocity.x * scale * 0.75), fround(velocity.y * scale * 0.75)),
        vertical_velocity: fround(-(bounce + 2.0)),
    }
}

#[must_use]
pub fn create_native_weld_boulder_debris_particle(
    mut seed: NativeWeldMeteorDebrisSeed,
    enhanced_shadow: bool,
) -> NativeWeldBoulderDebrisParticleState {
    seed.alpha = if enhanced_shadow { 10.0 } else { 2.0 };
    NativeWeldBoulderDebrisParticleState {
        common: seed,
        bounce_velocity: seed.vertical_velocity,
        enhanced_shadow,
    }
}

/// Shared `Anim_BoulderBit` motion from `native-weld-boulder-debris.ts`.
/// Coffin's art and primary Boulder metadata remain with their owning actors.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeBoulderDebrisMotion {
    pub alpha: f64,
    pub bounce_velocity: f64,
    pub height: f64,
    pub position: Vec2,
    pub rotation_degrees: f64,
    pub rotation_step_degrees: f64,
    pub velocity: Vec2,
    pub vertical_velocity: f64,
}

#[must_use]
pub fn step_native_weld_boulder_debris_particle(
    mut particle: NativeWeldBoulderDebrisParticleState,
    global_tick: u64,
    rng: &mut NativeRng,
) -> Option<NativeWeldBoulderDebrisParticleState> {
    let motion = NativeBoulderDebrisMotion {
        alpha: particle.common.alpha,
        bounce_velocity: particle.bounce_velocity,
        height: particle.common.height,
        position: particle.common.position,
        rotation_degrees: particle.common.rotation_degrees,
        rotation_step_degrees: particle.common.rotation_step_degrees,
        velocity: particle.common.velocity,
        vertical_velocity: particle.common.vertical_velocity,
    };
    let motion = step_native_boulder_debris_motion(motion, global_tick, rng)?;
    particle.common.alpha = motion.alpha;
    particle.bounce_velocity = motion.bounce_velocity;
    particle.common.height = motion.height;
    particle.common.position = motion.position;
    particle.common.rotation_degrees = motion.rotation_degrees;
    particle.common.rotation_step_degrees = motion.rotation_step_degrees;
    particle.common.velocity = motion.velocity;
    particle.common.vertical_velocity = motion.vertical_velocity;
    Some(particle)
}

#[must_use]
pub fn step_native_boulder_debris_motion(
    mut particle: NativeBoulderDebrisMotion,
    global_tick: u64,
    rng: &mut NativeRng,
) -> Option<NativeBoulderDebrisMotion> {
    // On every third global tick an airborne bit only fades; its gravity waits.
    if particle.height != 0.0 && global_tick.is_multiple_of(3) {
        particle.alpha = fround(particle.alpha - 0.025_f32 as f64);
        return (particle.alpha > 0.0).then_some(particle);
    }
    if particle.height != 0.0 {
        particle.position = Vec2::new(
            fround(particle.position.x + particle.velocity.x),
            fround(particle.position.y + particle.velocity.y),
        );
        particle.height = fround(particle.height + particle.vertical_velocity);
        particle.vertical_velocity = fround(particle.vertical_velocity + 0.4_f32 as f64);
        particle.rotation_degrees =
            fround(particle.rotation_degrees + particle.rotation_step_degrees);
        if particle.height > 0.0 {
            let spin = rng.draw_scaled_float(10.0, false);
            let damping = rng.draw_integer(2);
            particle.rotation_step_degrees = fround(spin + 1.0);
            particle.bounce_velocity = fround(particle.bounce_velocity * 0.3_f32 as f64);
            particle.vertical_velocity = particle.bounce_velocity;
            if damping == 1 {
                particle.velocity = Vec2::new(
                    fround(particle.velocity.x * 0.65_f32 as f64),
                    fround(particle.velocity.y * 0.65_f32 as f64),
                );
            }
            if particle.vertical_velocity > -(0.75_f32 as f64) {
                particle.velocity = Vec2::ZERO;
                particle.vertical_velocity = 0.0;
                particle.bounce_velocity = 0.0;
                particle.rotation_step_degrees = 0.0;
            }
            particle.height = particle.vertical_velocity;
        }
    }
    particle.alpha = fround(particle.alpha - 0.015_f32 as f64);
    particle.alpha = fround(particle.alpha - 0.025_f32 as f64);
    (particle.alpha > 0.0).then_some(particle)
}
