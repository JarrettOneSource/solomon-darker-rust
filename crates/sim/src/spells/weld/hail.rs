//! Native Hail terrain/contact children from `core-kernels/native-weld-hail-contact.ts`.

use super::model::{NativeWeldActorBase, NativeWeldActorBirth};
use super::persistent::NativeWeldHailstonesState;
use crate::float;
use crate::float::{atan2, fround};
use crate::{NativeRng, Vec2};

pub const NATIVE_WELD_HAIL_TARGET_RADIUS_FACTOR: f64 = 1.5;
pub const NATIVE_WELD_HAIL_COLD_SLOW_FACTOR: f64 = 0.5;
pub const NATIVE_WELD_HAIL_COLD_SLOW_TICKS: u64 = 250;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldHailTerrainParticleState {
    pub common: NativeWeldActorBase,
    pub alpha: f64,
    pub alpha_step: f64,
    pub position: Vec2,
    pub record: usize,
    pub rotation_degrees: f64,
    pub scale: f64,
    pub tint: u32,
    pub velocity: Vec2,
    pub velocity_factor: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldHailTerrainBouncerState {
    pub common: NativeWeldActorBase,
    pub alpha: f64,
    pub bounce_velocity: f64,
    pub enhanced_shadow: bool,
    pub height: f64,
    pub position: Vec2,
    pub record: usize,
    pub rotation_degrees: f64,
    pub rotation_step_degrees: f64,
    pub scale: f64,
    pub velocity: Vec2,
    pub vertical_velocity: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldHailLineState {
    pub common: NativeWeldActorBase,
    pub alpha: f64,
    pub alpha_step: f64,
    pub end: Vec2,
    pub end_alpha: f64,
    pub start: Vec2,
    pub width: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldHailFlashState {
    pub common: NativeWeldActorBase,
    pub alpha: f64,
    pub alpha_step: f64,
    pub position: Vec2,
    pub record: usize,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldHailKnockbackState {
    pub common: NativeWeldActorBase,
    pub delta: Vec2,
    pub remaining_ticks: u64,
    pub target_id: String,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeWeldHailChildActorState {
    Flash(NativeWeldHailFlashState),
    Knockback(NativeWeldHailKnockbackState),
    Line(NativeWeldHailLineState),
    TerrainBouncer(NativeWeldHailTerrainBouncerState),
    TerrainParticle(NativeWeldHailTerrainParticleState),
}

impl NativeWeldHailChildActorState {
    #[must_use]
    pub fn common(&self) -> &NativeWeldActorBase {
        match self {
            Self::Flash(actor) => &actor.common,
            Self::Knockback(actor) => &actor.common,
            Self::Line(actor) => &actor.common,
            Self::TerrainBouncer(actor) => &actor.common,
            Self::TerrainParticle(actor) => &actor.common,
        }
    }

    pub fn common_mut(&mut self) -> &mut NativeWeldActorBase {
        match self {
            Self::Flash(actor) => &mut actor.common,
            Self::Knockback(actor) => &mut actor.common,
            Self::Line(actor) => &mut actor.common,
            Self::TerrainBouncer(actor) => &mut actor.common,
            Self::TerrainParticle(actor) => &mut actor.common,
        }
    }
}

pub fn create_native_weld_hail_terrain_impact(
    actor: &NativeWeldHailstonesState,
    enhanced_effects: bool,
    first_id: u64,
    tick: u64,
    rng: &mut NativeRng,
) -> Vec<NativeWeldHailChildActorState> {
    let mut actors = Vec::new();
    if actor.rocks.is_empty() {
        return actors;
    }
    let mut angle = fround(normalize_degrees(direction_heading(actor.common.direction) + 100.0));
    let rock_step = fround(160.0 / actor.rocks.len() as f64);
    for _ in &actor.rocks {
        for _ in 0..15 {
            let angle_step = rng.draw_integer(5);
            angle = fround(angle + f64::from(angle_step) + 24.0);
            let speed = rng.draw_scaled_float(4.0, false);
            let lead = rng.draw_scaled_float(5.0, false);
            let scale = rng.draw_scaled_float(0.25, false);
            let alpha = rng.draw_scaled_float(0.75, false);
            let radial = direction_from_heading(angle);
            let velocity = Vec2::new(
                fround(radial.x * fround(speed + 1.0)),
                fround(radial.y * fround(speed + 1.0)),
            );
            let distance = fround(lead + 3.0);
            actors.push(NativeWeldHailChildActorState::TerrainParticle(
                NativeWeldHailTerrainParticleState {
                    common: hail_actor_base(actor, first_id + actors.len() as u64, tick),
                    alpha: fround(alpha + 0.25),
                    alpha_step: 0.125,
                    position: Vec2::new(
                        fround(actor.common.origin.x + velocity.x * distance),
                        fround(actor.common.origin.y - 35.0 + velocity.y * distance),
                    ),
                    record: 45,
                    rotation_degrees: angle,
                    scale: fround(scale + 0.25),
                    tint: 0xff_ffff,
                    velocity,
                    velocity_factor: 0.92_f32 as f64,
                },
            ));
        }
        let bounce = rng.draw_scaled_float(3.0, false);
        let height = rng.draw_scaled_float(20.0, false);
        let rotation = rng.draw_scaled_float(360.0, false);
        let rotation_step = rng.draw_scaled_float(10.0, false);
        let scale = rng.draw_scaled_float(0.25, false);
        let distance = rng.draw_scaled_float(10.0, false);
        let speed = rng.draw_scaled_float(1.0, false);
        let angle_jitter = rng.draw_scaled_float(10.0, false);
        let radial = direction_from_heading(angle);
        let initial_velocity = Vec2::new(fround(radial.x * 1.5), radial.y);
        let lead_distance = fround(distance + 10.0);
        let velocity_scale = fround(speed + 1.0);
        let velocity = Vec2::new(
            fround(initial_velocity.x * velocity_scale),
            fround(initial_velocity.y * velocity_scale),
        );
        actors.push(NativeWeldHailChildActorState::TerrainBouncer(
            NativeWeldHailTerrainBouncerState {
                common: hail_actor_base(actor, first_id + actors.len() as u64, tick),
                alpha: 1.5,
                bounce_velocity: fround(-(bounce + 2.0)),
                enhanced_shadow: enhanced_effects,
                height: fround(-height),
                position: Vec2::new(
                    fround(
                        actor.common.origin.x
                            + initial_velocity.x * lead_distance
                            + velocity.x * 2.0,
                    ),
                    fround(
                        actor.common.origin.y
                            + initial_velocity.y * lead_distance
                            + velocity.y * 2.0,
                    ),
                ),
                record: 32,
                rotation_degrees: rotation,
                rotation_step_degrees: fround(rotation_step + 1.0),
                scale: fround(scale + 0.5),
                velocity,
                vertical_velocity: fround(-(bounce + 2.0)),
            },
        ));
        angle = fround(angle + rock_step + angle_jitter);
    }
    actors
}

pub fn create_native_weld_hail_contact_presentation(
    actor: &NativeWeldHailstonesState,
    start: Vec2,
    end: Vec2,
    first_id: u64,
    tick: u64,
    rng: &mut NativeRng,
) -> [NativeWeldHailChildActorState; 2] {
    let end_alpha = rng.draw_scaled_float(0.25, false);
    [
        NativeWeldHailChildActorState::Line(NativeWeldHailLineState {
            common: hail_actor_base(actor, first_id, tick),
            alpha: 1.0,
            alpha_step: 0.075_f32 as f64,
            end,
            end_alpha: fround(end_alpha + 0.25),
            start,
            width: 6.0,
        }),
        NativeWeldHailChildActorState::Flash(NativeWeldHailFlashState {
            common: hail_actor_base(actor, first_id + 1, tick),
            alpha: 1.0,
            alpha_step: 0.1_f32 as f64,
            position: end,
            record: 15,
        }),
    ]
}

#[must_use]
pub fn create_native_weld_hail_knockback(
    actor: &NativeWeldHailstonesState,
    id: u64,
    target_id: &str,
    tick: u64,
) -> Option<NativeWeldHailKnockbackState> {
    let remaining_ticks = (actor.pushback * 20.0).round_ties_even();
    if remaining_ticks <= 0.0 {
        return None;
    }
    Some(NativeWeldHailKnockbackState {
        common: hail_actor_base(actor, id, tick),
        delta: actor.common.direction,
        remaining_ticks: remaining_ticks as u64,
        target_id: target_id.into(),
    })
}

pub fn step_native_weld_hail_child(
    actor: &mut NativeWeldHailChildActorState,
    rng: &mut NativeRng,
) -> bool {
    match actor {
        // The contact transaction, rather than the visual epoch, owns knockback.
        NativeWeldHailChildActorState::Knockback(_) => true,
        NativeWeldHailChildActorState::Line(actor) => {
            actor.alpha = fround(actor.alpha - actor.alpha_step);
            if actor.alpha <= 0.0 {
                return false;
            }
            actor.common.owner.age_ticks += 1;
            true
        }
        NativeWeldHailChildActorState::Flash(actor) => {
            actor.alpha = fround(actor.alpha - actor.alpha_step);
            if actor.alpha <= 0.0 {
                return false;
            }
            actor.common.owner.age_ticks += 1;
            true
        }
        NativeWeldHailChildActorState::TerrainParticle(actor) => {
            actor.alpha = fround(actor.alpha - actor.alpha_step);
            if actor.alpha <= 0.0 {
                return false;
            }
            actor.common.owner.age_ticks += 1;
            actor.position = Vec2::new(
                fround(actor.position.x + actor.velocity.x),
                fround(actor.position.y + actor.velocity.y),
            );
            actor.velocity = Vec2::new(
                fround(actor.velocity.x * actor.velocity_factor),
                fround(actor.velocity.y * actor.velocity_factor),
            );
            true
        }
        NativeWeldHailChildActorState::TerrainBouncer(actor) => {
            let global_tick = actor.common.birth_tick + actor.common.owner.age_ticks + 1;
            if actor.height != 0.0 && global_tick.is_multiple_of(3) {
                actor.common.owner.age_ticks += 1;
                return true;
            }
            actor.position = Vec2::new(
                fround(actor.position.x + actor.velocity.x),
                fround(actor.position.y + actor.velocity.y),
            );
            let previous_height = actor.height;
            actor.height = fround(actor.height + actor.vertical_velocity);
            actor.vertical_velocity = fround(actor.vertical_velocity + 0.4_f32 as f64);
            actor.rotation_degrees = fround(actor.rotation_degrees + actor.rotation_step_degrees);
            if previous_height != 0.0 && actor.height > 0.0 {
                let spin = rng.draw_scaled_float(10.0, false);
                let damping = rng.draw_integer(2);
                actor.rotation_step_degrees = fround(spin + 1.0);
                actor.bounce_velocity = fround(actor.bounce_velocity * 0.65_f32 as f64);
                actor.vertical_velocity = actor.bounce_velocity;
                if damping == 1 {
                    actor.velocity = Vec2::new(
                        fround(actor.velocity.x * 0.65_f32 as f64),
                        fround(actor.velocity.y * 0.65_f32 as f64),
                    );
                }
                if actor.vertical_velocity > -0.75 {
                    actor.velocity = Vec2::ZERO;
                    actor.vertical_velocity = 0.0;
                    actor.bounce_velocity = 0.0;
                    actor.rotation_step_degrees = 0.0;
                }
                actor.height = actor.vertical_velocity;
            }
            actor.alpha = fround(actor.alpha - 0.015_f32 as f64);
            if actor.alpha <= 0.0 {
                return false;
            }
            actor.common.owner.age_ticks += 1;
            true
        }
    }
}

fn hail_actor_base(actor: &NativeWeldHailstonesState, id: u64, tick: u64) -> NativeWeldActorBase {
    NativeWeldActorBirth {
        build_id: 1008,
        direction: actor.common.direction,
        id,
        origin: actor.common.origin,
        owner_id: &actor.common.owner.owner_id,
        tick,
        vector: &actor.common.vector,
        world_key: &actor.common.owner.world_key,
    }
    .actor()
}

fn direction_heading(direction: Vec2) -> f64 {
    normalize_degrees(atan2(direction.x, -direction.y) * 180.0 / std::f64::consts::PI)
}

fn direction_from_heading(degrees: f64) -> Vec2 {
    let radians = degrees * std::f64::consts::PI / 180.0;
    Vec2::new(fround(float::sin(radians)), fround(-float::cos(radians)))
}

fn normalize_degrees(degrees: f64) -> f64 {
    (degrees % 360.0 + 360.0) % 360.0
}
