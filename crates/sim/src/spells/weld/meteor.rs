//! Marker, falling Meteor and impact programs from `native-weld-meteor.ts` and
//! the Meteor branch of `native-weld-primary-runtime.ts`.

use super::model::{NativeWeldActorBase, NativeWeldActorBirth};
use crate::float;
use crate::float::fround;
use crate::heading::heading_degrees_from_vector;
use crate::painter::WorldPainterOrder;
use crate::spells::earth_debris::NativeWeldMeteorDebrisSeed;
use crate::{NativeRng, Vec2};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldMeteorMarkerState {
    pub common: NativeWeldActorBase,
    pub alpha: f64,
    pub color_green: f64,
    pub growth_factor: f64,
    pub rotation_degrees: f64,
    pub scale: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeWeldMeteorSpawnProgram {
    pub body_scale: f64,
    pub fall_heading_degrees: f64,
    pub fall_height: f64,
    pub fall_step: f64,
    pub impact_ticks: u64,
    pub position: Vec2,
    pub private_seed: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeWeldMeteorImpactProgram {
    pub camera_displacement: Vec2,
    pub debris: Vec<NativeWeldMeteorDebrisSeed>,
    pub impact_radius_scalar: f64,
    pub impact_rotation_degrees: f64,
    pub impact_sound_pitch: f64,
    pub impact_throw_fire_pitch: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeWeldMeteorPhase {
    Fall,
    Impact,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldMeteorActorState {
    pub common: NativeWeldActorBase,
    pub landing_position: Vec2,
    pub body_scale: f64,
    pub camera_displacement: Option<Vec2>,
    pub damage: f64,
    pub debris: Vec<NativeWeldMeteorDebrisSeed>,
    pub fall_heading_degrees: f64,
    pub fall_height: f64,
    pub fall_step: f64,
    pub impact_due: bool,
    pub impact_age_ticks: u64,
    pub impact_radius_scalar: f64,
    pub impact_rotation_degrees: f64,
    pub impact_sound_pitch: Option<f64>,
    pub impact_throw_fire_pitch: Option<f64>,
    pub impact_ticks_remaining: u64,
    pub phase: NativeWeldMeteorPhase,
    pub position: Vec2,
    pub private_seed: u32,
    pub pulse_due: bool,
    pub pulse_sequence: u64,
    pub pulse_ticks_remaining: u64,
    pub underpowered: bool,
}

#[must_use]
pub fn spawn_native_weld_meteor_marker(
    birth: NativeWeldActorBirth<'_>,
    rng: &mut NativeRng,
) -> NativeWeldMeteorMarkerState {
    let color = rng.draw_scaled_float(0.5, false);
    let rotation = rng.draw_scaled_float(360.0, false);
    let growth = rng.draw_integer(2);
    let alpha = rng.draw_scaled_float(0.5, false);
    NativeWeldMeteorMarkerState {
        common: birth.actor(),
        alpha: fround(fround(alpha + 0.5) * 0.5),
        color_green: color,
        growth_factor: if growth == 1 { 0.99_f32 as f64 } else { 1.015_f32 as f64 },
        rotation_degrees: rotation,
        scale: 3.5,
    }
}

/// Returns false exactly on the source retirement tick.
pub fn step_native_weld_meteor_marker(marker: &mut NativeWeldMeteorMarkerState) -> bool {
    marker.alpha = fround(marker.alpha - 0.025_f32 as f64);
    if marker.alpha <= 0.0 {
        return false;
    }
    marker.common.owner.age_ticks += 1;
    marker.scale = fround(marker.scale * marker.growth_factor);
    true
}

#[must_use]
pub fn native_weld_meteor_target_point(player_position: Vec2, aim_direction: Vec2) -> Vec2 {
    Vec2::new(
        fround(player_position.x + aim_direction.x * 160.0),
        fround(player_position.y + aim_direction.y * 160.0),
    )
}

#[must_use]
pub fn native_weld_meteor_cadence_ticks(cast_progress_factor: f64, underpowered: bool) -> u64 {
    let rounded = cast_progress_factor.round();
    assert!(rounded >= 1.0, "Meteor cast progress factor must round above zero");
    ((if underpowered { 35.0 } else { 25.0 }) / rounded).trunc().max(5.0) as u64
}

pub fn create_native_weld_meteor_spawn_program(
    aim_direction: Vec2,
    center: Vec2,
    resolve_position: &dyn Fn(Vec2) -> Vec2,
    underpowered: bool,
    vector: &[f64],
    rng: &mut NativeRng,
) -> NativeWeldMeteorSpawnProgram {
    let body_scale = rng.draw_scaled_float(0.25, false);
    let radius = rng.draw_scaled_float(150.0, false);
    let radial_direction = draw_unit_vector(rng);
    let candidate = Vec2::new(
        fround(center.x + radial_direction.x * radius),
        fround(center.y + radial_direction.y * radius * 0.8_f32 as f64),
    );
    // Constructor +0x13C is overwritten by size, but its draw still advances RNG.
    rng.draw_scaled_float(0.25, false);
    let fall_heading = rng.draw_scaled_float(40.0, false);
    let size = rng.draw_scaled_float(0.25, false);
    let private_seed = if underpowered { 0 } else { rng.draw_integer(10_000_000) as u32 };
    let aim_heading = heading_degrees_from_vector(aim_direction.x, aim_direction.y);
    let growth = if underpowered { 2.0 } else { fround(vector[3] * 2.0) };
    let toughness_ticks = if underpowered { 0.0 } else { fround(vector[4] * 50.0) };
    NativeWeldMeteorSpawnProgram {
        body_scale: fround(1.0 - body_scale),
        fall_heading_degrees: fround(
            (if aim_heading > 180.0 { -1.0 } else { 1.0 }) * fround(10.0 + fall_heading),
        ),
        fall_height: fround(fround(size + 1.0) * 2.5 * 2.0),
        fall_step: fround(0.02_f32 as f64 * growth),
        impact_ticks: (200.0 + toughness_ticks).round() as u64,
        position: resolve_position(candidate),
        private_seed,
    }
}

pub fn create_native_weld_meteor_impact_program(
    body_scale: f64,
    underpowered: bool,
    rng: &mut NativeRng,
) -> NativeWeldMeteorImpactProgram {
    let camera_direction = draw_unit_vector(rng);
    let rotation = rng.draw_scaled_float(360.0, false);
    let radius = rng.draw_scaled_float(0.5, false);
    let mut angle = rng.draw_scaled_float(360.0, false);
    let mut debris = Vec::with_capacity(5);
    let scale_input = body_scale.min(1.0);
    for index in 0..5 {
        let bounce = rng.draw_scaled_float(3.0, false);
        rng.draw_scaled_float(20.0, false);
        let rotation_draw = rng.draw_scaled_float(360.0, false);
        let rotation_step = rng.draw_scaled_float(10.0, false);
        let color = rng.draw_scaled_float(0.5, false);
        let record = rng.draw_integer(3) as usize;
        let vertical_scale = rng.draw_scaled_float(1.5, false);
        let height = rng.draw_scaled_float(fround(scale_input * 50.0), false);
        let spawn_distance = rng.draw_scaled_float(15.0, false);
        let scale_probe = rng.draw_scaled_float(0.75, false);
        let motion_scale = rng.draw_scaled_float(fround(scale_input * 1.5), true);
        let angle_step = rng.draw_scaled_float(24.0, false);
        let direction = heading_vector(angle);
        let movement = fround(fround(motion_scale + 1.5) * 0.5);
        let native_scale_probe = fround(fround(scale_probe + 0.5) * scale_input * 0.25);
        debris.push(NativeWeldMeteorDebrisSeed {
            alpha: 2.0,
            color_green: color,
            height: fround(-height),
            index,
            position: Vec2::new(
                fround(direction.x * spawn_distance),
                fround(direction.y * 0.8_f32 as f64 * spawn_distance),
            ),
            record: 2008 + record,
            rotation_degrees: rotation_draw,
            rotation_step_degrees: fround(rotation_step + 1.0),
            scale: native_scale_probe.clamp(0.45_f32 as f64, 0.75),
            velocity: Vec2::new(
                fround(direction.x * movement),
                fround(direction.y * 0.8_f32 as f64 * movement),
            ),
            vertical_velocity: fround(
                fround(-(bounce + 2.0)) * fround(fround(vertical_scale * scale_input) + 0.75),
            ),
        });
        angle = fround(angle + 72.0 + angle_step);
    }
    let pitch_range = if underpowered { 0.2_f32 as f64 } else { 0.1_f32 as f64 };
    let pitch = rng.draw_scaled_float(pitch_range, true);
    NativeWeldMeteorImpactProgram {
        camera_displacement: Vec2::new(
            fround(camera_direction.x * 10.0),
            fround(camera_direction.y * 10.0),
        ),
        debris,
        impact_radius_scalar: fround(body_scale + radius),
        impact_rotation_degrees: rotation,
        impact_sound_pitch: fround(1.0 + pitch),
        impact_throw_fire_pitch: (!underpowered).then_some(0.8_f32 as f64),
    }
}

#[must_use]
pub fn create_native_weld_meteor(
    birth: NativeWeldActorBirth<'_>,
    spawn: NativeWeldMeteorSpawnProgram,
    damage: f64,
    underpowered: bool,
    painters: Option<&mut WorldPainterOrder>,
) -> NativeWeldMeteorActorState {
    NativeWeldMeteorActorState {
        common: birth.actor_with_light(painters),
        landing_position: spawn.position,
        body_scale: spawn.body_scale,
        camera_displacement: None,
        damage,
        debris: Vec::new(),
        fall_heading_degrees: spawn.fall_heading_degrees,
        fall_height: spawn.fall_height,
        fall_step: spawn.fall_step,
        impact_due: false,
        impact_age_ticks: 0,
        impact_radius_scalar: 1.0,
        impact_rotation_degrees: 0.0,
        impact_sound_pitch: None,
        impact_throw_fire_pitch: None,
        impact_ticks_remaining: spawn.impact_ticks,
        phase: NativeWeldMeteorPhase::Fall,
        position: Vec2::ZERO,
        private_seed: spawn.private_seed,
        pulse_due: false,
        pulse_sequence: 0,
        pulse_ticks_remaining: 10,
        underpowered,
    }
}

/// The impact program returns debris separately; world ownership enrolls those
/// children after the parent step, preserving constructor and painter order.
pub fn step_native_weld_meteor(
    meteor: &mut NativeWeldMeteorActorState,
    rng: &mut NativeRng,
) -> (bool, Vec<NativeWeldMeteorDebrisSeed>) {
    if meteor.phase == NativeWeldMeteorPhase::Fall {
        meteor.position = native_weld_meteor_root_position(
            meteor.landing_position,
            meteor.fall_heading_degrees,
            meteor.fall_height,
        );
        meteor.fall_height = fround(meteor.fall_height - meteor.fall_step);
        meteor.common.owner.age_ticks += 1;
        if meteor.fall_height > 0.0 {
            return (true, Vec::new());
        }
        let impact =
            create_native_weld_meteor_impact_program(meteor.body_scale, meteor.underpowered, rng);
        meteor.camera_displacement = Some(impact.camera_displacement);
        meteor.debris.clear();
        meteor.impact_due = true;
        meteor.impact_radius_scalar = impact.impact_radius_scalar;
        meteor.impact_rotation_degrees = impact.impact_rotation_degrees;
        meteor.impact_sound_pitch = Some(impact.impact_sound_pitch);
        meteor.impact_throw_fire_pitch = impact.impact_throw_fire_pitch;
        meteor.phase = NativeWeldMeteorPhase::Impact;
        return (true, impact.debris);
    }
    meteor.impact_ticks_remaining -= 1;
    if meteor.impact_ticks_remaining == 0 {
        return (false, Vec::new());
    }
    meteor.common.owner.age_ticks += 1;
    meteor.impact_age_ticks += 1;
    meteor.impact_due = false;
    meteor.pulse_ticks_remaining -= 1;
    meteor.pulse_due = meteor.pulse_ticks_remaining == 0;
    if meteor.pulse_due {
        meteor.pulse_sequence += 1;
        meteor.pulse_ticks_remaining = 10;
    }
    (true, Vec::new())
}

#[must_use]
pub fn native_weld_meteor_root_position(landing: Vec2, heading: f64, height: f64) -> Vec2 {
    let radians = heading * std::f64::consts::PI / 180.0;
    let velocity_x = fround(fround(float::sin(radians)) * 300.0);
    let velocity_y = fround(fround(fround(-float::cos(radians)) * 300.0) * 0.25);
    Vec2::new(
        fround(landing.x + fround(height * velocity_x)),
        fround(landing.y + fround(height * velocity_y)),
    )
}

#[must_use]
pub fn native_weld_meteor_direct_radius() -> f64 {
    45.0
}

#[must_use]
pub fn native_weld_meteor_pulse_radius(impact_radius_scalar: f64) -> f64 {
    fround(impact_radius_scalar * 45.0)
}

pub(super) fn draw_unit_vector(rng: &mut NativeRng) -> Vec2 {
    let heading = rng.draw_integer(100_001);
    let degrees = fround(fround(f64::from(heading) / 100_000.0) * 360.0);
    heading_vector(degrees)
}

fn heading_vector(degrees: f64) -> Vec2 {
    let radians = degrees * std::f64::consts::PI / 180.0;
    Vec2::new(fround(float::sin(radians)), fround(-float::cos(radians)))
}
