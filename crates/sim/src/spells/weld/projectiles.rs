//! One-shot births, native tracking and private GroundSpark turns from
//! `core-kernels/native-weld-primary-runtime.ts` and `native-weld-ground-spark.ts`.

use crate::float::fround;
use crate::heading::heading_degrees_from_vector;
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::profile::primary::NativeWeldPrimarySkillProfile;
use crate::profile::weld::NativeWeldCastKind;
use crate::spells::model::{PrimarySpellProjectileBase, PrimarySpellProjectilePhase};
use crate::spells::targeting::{
    ETHER_PRIMARY_INITIAL_TURN, EtherPrimaryTrackingInput, PrimarySpellTarget,
    advance_ether_primary_tracking, direction_from_heading, native_missile_fan_heading,
    native_missile_fan_turn_scale, select_ether_primary_target,
};
use crate::{NativeRng, Vec2};

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeWeldFrostPresentationLane {
    pub aspect: f64,
    pub rotation_degrees: f64,
    pub scale: f64,
}

#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct NativeWeldProjectileState {
    pub common: PrimarySpellProjectileBase,
    pub ball_lightning_acceleration: Option<f64>,
    pub base_presentation_phase_degrees: Option<f64>,
    pub build_id: u32,
    pub cast_playback_rate: f64,
    pub cast_sound_variant: Option<u32>,
    pub contacts_remaining: u32,
    pub heading_degrees: f64,
    pub hit_target_ids: Vec<String>,
    pub frost_pulse_aspect: Option<f64>,
    pub frost_presentation_lanes: Option<[NativeWeldFrostPresentationLane; 2]>,
    pub frost_turn_degrees: Option<f64>,
    pub ground_spark_native_age_ticks: Option<u64>,
    pub ground_spark_turn_ticks_remaining: Option<i64>,
    pub presentation_seed: Option<u32>,
    pub projectile_index: usize,
    pub reacquires_target: bool,
    pub secondary_presentation_phase_degrees: Option<f64>,
    pub speed: f64,
    pub target_id: Option<String>,
    pub turn_accumulator: f64,
    pub turn_input: f64,
    pub underpowered: bool,
    pub vector: Vec<f64>,
}

pub struct SpawnNativeWeldOneShotInput<'a> {
    pub aim_direction: Vec2,
    pub first_id: u64,
    pub origin: Vec2,
    pub owner_id: &'a str,
    pub primary_skill: &'a NativeWeldPrimarySkillProfile,
    pub targets: &'a [PrimarySpellTarget],
    pub underpowered: bool,
    pub world_key: &'a str,
}

/// Weld damage differs from normal Air/Water damage: an equal range draws no word.
pub fn draw_native_weld_damage(rng: &mut NativeRng, minimum: f64, maximum: f64) -> f64 {
    assert!(
        minimum.is_finite() && maximum.is_finite() && maximum >= minimum,
        "weld damage endpoints must be finite and ordered"
    );
    if minimum == maximum {
        return minimum;
    }
    let draw = rng.draw_scaled_float(fround(maximum - minimum), false);
    fround(fround(minimum) + draw)
}

pub fn spawn_native_weld_one_shot(
    input: SpawnNativeWeldOneShotInput<'_>,
    rng: &mut NativeRng,
    mut painters: Option<&mut WorldPainterOrder>,
) -> Vec<NativeWeldProjectileState> {
    let profile = input.primary_skill;
    let build_id = profile.build_id;
    assert!(
        profile.cast_kind == NativeWeldCastKind::OneShot && matches!(build_id, 1000..=1002 | 1009),
        "weld build is not a one-shot actor"
    );
    let damage =
        draw_native_weld_damage(rng, profile.common.damage_minimum, profile.common.damage_maximum);
    let aim_heading = heading_degrees_from_vector(input.aim_direction.x, input.aim_direction.y);
    let quantity = if input.underpowered {
        1
    } else if build_id == 1009 {
        3
    } else {
        profile.vector.values[3].round() as usize
    };
    let mut playback = if input.underpowered { 0.75_f32 as f64 } else { 1.0 };
    let mut ball_turn_scale = 1.0;
    match build_id {
        1001 => {
            let draw = rng.draw_scaled_float(0.1_f32 as f64, false);
            if !input.underpowered {
                playback = fround(1.0 + draw);
            }
        }
        1002 => {
            let draw = rng.draw_scaled_float(0.25_f32 as f64, false);
            ball_turn_scale = if input.underpowered { 0.75_f32 as f64 } else { fround(1.0 + draw) };
            playback = ball_turn_scale;
        }
        1009 => {
            let draw = rng.draw_scaled_float(0.05_f32 as f64, true);
            playback = fround(1.0 + draw);
            if input.underpowered {
                playback = fround(playback * 0.8_f32 as f64);
            }
        }
        _ => {}
    }
    let cast_sound_variant = match build_id {
        1002 => Some(rng.draw_integer(2) as u32),
        1009 => Some(rng.draw_integer(3) as u32),
        _ => None,
    };
    let mut projectiles = Vec::with_capacity(quantity);
    for index in 0..quantity {
        let heading = if build_id == 1009 {
            normalize_degrees(
                aim_heading
                    + match index {
                        0 => 0.0,
                        1 => -30.0,
                        _ => 30.0,
                    },
            )
        } else {
            native_missile_fan_heading(aim_heading, quantity, index)
        };
        let direction = direction_from_heading(heading);
        let position = Vec2::new(
            input.origin.x,
            fround(input.origin.y + if build_id == 1009 { 15.0 } else { 10.0 }),
        );
        let speed_factor = if build_id == 1009 {
            if index == 0 { 4.0 } else { 3.0 }
        } else if input.underpowered {
            0.8_f32 as f64
        } else if build_id == 1002 {
            fround(profile.vector.values[4] * 0.8500000238418579)
        } else {
            profile.vector.values[4]
        };
        let speed = if build_id == 1009 { speed_factor } else { fround(3.0 * speed_factor) };
        let target = if build_id == 1009 {
            None
        } else {
            select_ether_primary_target(position, direction, input.targets)
        };
        let base_phase =
            if build_id == 1009 { None } else { Some(rng.draw_scaled_float(360.0, false)) };
        let mut secondary_phase = None;
        let mut frost_aspect = None;
        if build_id == 1001 {
            secondary_phase = Some(rng.draw_scaled_float(360.0, false));
            frost_aspect = Some(fround(0.5 + rng.draw_scaled_float(0.25_f32 as f64, false)));
        }
        let mut private_seed = None;
        let mut native_age = None;
        if build_id == 1000 || build_id == 1009 {
            private_seed =
                Some(rng.draw_integer(if build_id == 1000 { 100_000 } else { 1_000_000 }) as u32);
            if build_id == 1009 {
                native_age = Some(rng.draw_integer(360) as u64);
            }
        }
        let vector = underpowered_vector(build_id, &profile.vector.values, input.underpowered);
        let acceleration = (build_id == 1002).then_some(2.0);
        let movement_speed = acceleration
            .map_or(speed, |acceleration| ball_lightning_movement_speed(speed, acceleration));
        let id = input.first_id + index as u64;
        let registration = if let Some(order) = painters.as_deref_mut() {
            order.register(WorldManagerLane::Actor)
        } else {
            PainterRegistration {
                manager_lane: WorldManagerLane::Actor,
                registration_ordinal: id as usize,
            }
        };
        projectiles.push(NativeWeldProjectileState {
            common: PrimarySpellProjectileBase {
                age_ticks: 0,
                charge: 1.0,
                damage: fround(damage * if input.underpowered { 0.5 } else { 1.0 }),
                direction,
                flight_ticks: 0,
                id,
                registration,
                owner_id: input.owner_id.into(),
                phase: PrimarySpellProjectilePhase::Flight,
                position,
                velocity: Vec2::new(
                    fround(direction.x * movement_speed),
                    fround(direction.y * movement_speed),
                ),
                world_key: input.world_key.into(),
            },
            ball_lightning_acceleration: acceleration,
            base_presentation_phase_degrees: base_phase,
            build_id,
            cast_playback_rate: playback,
            cast_sound_variant,
            contacts_remaining: if build_id == 1009 { vector[4].round() as u32 + 1 } else { 1 },
            heading_degrees: heading,
            hit_target_ids: Vec::new(),
            frost_pulse_aspect: frost_aspect,
            frost_presentation_lanes: if build_id == 1001 {
                Some([
                    NativeWeldFrostPresentationLane {
                        aspect: 0.0,
                        rotation_degrees: 0.0,
                        scale: 0.0,
                    },
                    NativeWeldFrostPresentationLane {
                        aspect: frost_aspect.unwrap(),
                        rotation_degrees: secondary_phase.unwrap(),
                        scale: 0.0,
                    },
                ])
            } else {
                None
            },
            frost_turn_degrees: (build_id == 1001).then_some(0.0),
            ground_spark_native_age_ticks: native_age,
            ground_spark_turn_ticks_remaining: (build_id == 1009).then_some(0),
            presentation_seed: private_seed,
            projectile_index: index,
            reacquires_target: missile_reacquires(build_id, speed_factor, input.underpowered),
            secondary_presentation_phase_degrees: secondary_phase,
            speed,
            target_id: target.map(|target| target.id.clone()),
            turn_accumulator: ETHER_PRIMARY_INITIAL_TURN,
            turn_input: if build_id == 1009 {
                0.0
            } else {
                fround(
                    2.0 * speed_factor
                        * if build_id == 1002 { ball_turn_scale } else { 1.0 }
                        * native_missile_fan_turn_scale(index),
                )
            },
            underpowered: input.underpowered,
            vector,
        });
    }
    projectiles
}

pub fn step_native_weld_projectile_presentation(
    projectile: &mut NativeWeldProjectileState,
    rng: &mut NativeRng,
) {
    if projectile.build_id != 1001 {
        return;
    }
    for lane in projectile.frost_presentation_lanes.as_mut().expect("Frost Missile owns two lanes")
    {
        lane.scale = fround(lane.scale - 0.01_f32 as f64);
        if lane.scale < 0.1_f32 as f64 {
            let aspect = rng.draw_scaled_float(0.25_f32 as f64, false);
            let scale = rng.draw_scaled_float(0.75_f32 as f64, false);
            let rotation = rng.draw_scaled_float(45.0, false);
            lane.aspect = fround(aspect + 0.5);
            lane.scale = fround(scale + 0.5);
            lane.rotation_degrees = rotation;
        }
    }
}

pub fn step_native_weld_projectile(
    projectile: &mut NativeWeldProjectileState,
    targets: &[PrimarySpellTarget],
) {
    if projectile.build_id == 1009 {
        projectile.common.position = Vec2::new(
            fround(projectile.common.position.x + projectile.common.velocity.x),
            fround(projectile.common.position.y + projectile.common.velocity.y),
        );
        let mut ticks =
            projectile.ground_spark_turn_ticks_remaining.expect("GroundSpark owns turn clock") - 1;
        if ticks < 1 {
            let magnitude_word =
                native_ground_spark_private_word(projectile.presentation_seed.unwrap());
            let sign_word = native_ground_spark_private_word(magnitude_word);
            let speed_word = native_ground_spark_private_word(sign_word);
            let magnitude =
                fround(fround(f64::from(magnitude_word % 100_000) / 100_000.0) * 20.0 + 17.0);
            let signed = if sign_word.is_multiple_of(2) { -magnitude } else { magnitude };
            // GroundSpark turns around its original heading, not its previous direction.
            projectile.common.direction =
                direction_from_heading(projectile.heading_degrees + signed);
            projectile.presentation_seed = Some(speed_word);
            projectile.speed = f64::from(speed_word % 4 + 1);
            ticks = 20;
        }
        projectile.common.age_ticks += 1;
        projectile.common.flight_ticks += 1;
        *projectile.ground_spark_native_age_ticks.as_mut().unwrap() += 1;
        projectile.ground_spark_turn_ticks_remaining = Some(ticks);
        projectile.common.velocity = Vec2::new(
            fround(projectile.common.direction.x * projectile.speed),
            fround(projectile.common.direction.y * projectile.speed),
        );
        return;
    }
    let speed = projectile.ball_lightning_acceleration.map_or(projectile.speed, |acceleration| {
        ball_lightning_movement_speed(projectile.speed, acceleration)
    });
    let advanced = advance_ether_primary_tracking(EtherPrimaryTrackingInput {
        heading_degrees: projectile.heading_degrees,
        movement_scalar: 1.0,
        position: projectile.common.position,
        reacquires_target: projectile.reacquires_target,
        speed,
        target_id: projectile.target_id.as_deref(),
        targets,
        turn_accumulator: projectile.turn_accumulator,
        turn_input: projectile.turn_input,
    });
    if let Some(acceleration) = &mut projectile.ball_lightning_acceleration {
        *acceleration = fround(*acceleration * 0.8999999761581421);
    }
    let next_speed =
        projectile.ball_lightning_acceleration.map_or(projectile.speed, |acceleration| {
            ball_lightning_movement_speed(projectile.speed, acceleration)
        });
    if let Some(turn) = &mut projectile.frost_turn_degrees {
        *turn = fround(*turn * 0.949999988079071);
        if advanced.movement.heading_degrees != projectile.heading_degrees {
            let delta = ((advanced.movement.heading_degrees - projectile.heading_degrees + 540.0)
                % 360.0)
                - 180.0;
            *turn = fround(delta + *turn).clamp(-35.0, 35.0);
        }
    }
    projectile.common.age_ticks += 1;
    projectile.common.flight_ticks += 1;
    if let Some(phase) = &mut projectile.base_presentation_phase_degrees {
        *phase = fround(*phase + speed * 3.0);
    }
    projectile.common.direction = advanced.movement.direction;
    projectile.heading_degrees = advanced.movement.heading_degrees;
    projectile.common.position = advanced.movement.position;
    projectile.reacquires_target = advanced.reacquires_target;
    projectile.target_id = advanced.target_id;
    projectile.turn_accumulator = advanced.movement.turn_accumulator;
    projectile.common.velocity = Vec2::new(
        fround(projectile.common.direction.x * next_speed),
        fround(projectile.common.direction.y * next_speed),
    );
}

/// Private xorshift/multiply word, including the native signed absolute value.
#[must_use]
pub fn native_ground_spark_private_word(source: u32) -> u32 {
    (native_weld_private_stage(source) as i32).unsigned_abs()
}

pub(super) fn native_weld_private_stage(source: u32) -> u32 {
    let mut value = source ^ (source << 21);
    value ^= value >> 11;
    (value ^ (value << 4)).wrapping_mul(0x0a67_cfcf)
}

fn ball_lightning_movement_speed(speed: f64, acceleration: f64) -> f64 {
    fround(fround(acceleration + 1.0) * speed).min(6.0)
}
fn normalize_degrees(value: f64) -> f64 {
    fround(((value % 360.0) + 360.0) % 360.0)
}
fn missile_reacquires(build_id: u32, speed: f64, underpowered: bool) -> bool {
    if underpowered || build_id == 1009 {
        return false;
    }
    match build_id {
        1000 => speed > 1.255_f32 as f64,
        1001 => speed > 1.0,
        1002 => speed > 0.8600000143051147,
        _ => unreachable!(),
    }
}
fn underpowered_vector(build_id: u32, source: &[f64], underpowered: bool) -> Vec<f64> {
    let mut vector = source.to_vec();
    if !underpowered {
        return vector;
    }
    match build_id {
        1000 => {
            vector[3] = 1.0;
            vector[4] = 0.8_f32 as f64;
            vector[5..=8].fill(0.0);
        }
        1001 => {
            vector[3] = 1.0;
            vector[4] = 0.8_f32 as f64;
            vector[5..=6].fill(0.0);
        }
        1002 => {
            vector[3] = 1.0;
            vector[4] = 0.8_f32 as f64;
            vector[5] = 0.0;
            vector[6] = 1.0;
        }
        1009 => {
            vector[2] = 0.0;
            vector[3] = 1.0;
            vector[4] = 0.0;
        }
        _ => unreachable!(),
    }
    vector
}
