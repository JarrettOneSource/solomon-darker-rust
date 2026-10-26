//! Twenty native Coffin BoulderBits from `native-coffin-emergence.ts` and
//! `core-server/enemies/coffin-emergence.ts`. Births and bounces share steering RNG.

use super::effects::{DeathEffectOverrides, DeathEffectOwner, SimpleDeathEffectSpec};
use super::model::{
    EnemyActor, EnemyDeathEffect, EnemyDeathEffectKind, EnemyEffectAtlas, EnemyEffectBlend,
};
use super::world::EnemyWorld;
use crate::float::{self, fround};
use crate::painter::WorldPainterOrder;
use crate::spells::earth_debris::{NativeBoulderDebrisMotion, step_native_boulder_debris_motion};
use crate::{NativeRng, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeCoffinEmergenceParticle {
    #[cfg_attr(feature = "persistence", serde(flatten))]
    pub motion: NativeBoulderDebrisMotion,
    pub record: usize,
    pub scale: f64,
    pub enhanced_shadow: bool,
}

#[must_use]
pub fn create_native_coffin_emergence(
    rng: &mut NativeRng,
    position: Vec2,
    scale: f64,
    radius: f64,
    enhanced: bool,
) -> Vec<NativeCoffinEmergenceParticle> {
    assert!(
        scale.is_finite() && scale > 0.0 && radius.is_finite() && radius > 0.0,
        "Coffin emergence requires its live native scale and radius"
    );
    let mut heading = rng.draw_scaled_float(360.0, false);
    let mut particles = Vec::with_capacity(20);
    for _ in 0..20 {
        let bounce = fround(-(rng.draw_scaled_float(3.0, false) + 2.0));
        // Native BoulderBit's initial height is overwritten by Coffin's factory.
        rng.draw_scaled_float(20.0, false);
        let rotation_degrees = rng.draw_scaled_float(360.0, false);
        let rotation_step_degrees = fround(1.0 + rng.draw_scaled_float(10.0, false));
        let record = if rng.draw_integer(2) == 0 { 1834 } else { 1835 };
        let radians = heading * std::f64::consts::PI / 180.0;
        let direction =
            Vec2::new(fround(float::sin(radians)), fround(-float::cos(radians) * (0.8_f32 as f64)));
        let vertical_scale = fround(scale.min(1.0) * rng.draw_scaled_float(1.5, false) + 0.75);
        let vertical_velocity = fround(fround(vertical_scale * bounce) * 0.5);
        let height = fround(-rng.draw_scaled_float(fround(scale.min(1.0) * 50.0), false));
        let distance = rng.draw_scaled_float(radius, false);
        let root = Vec2::new(
            fround(position.x + fround(direction.x * distance)),
            fround(position.y + fround(direction.y * distance)),
        );
        let size = fround(fround(0.75 + rng.draw_scaled_float(0.5, false)) * (0.8_f32 as f64));
        let speed = fround(1.5 + rng.draw_scaled_float(fround(scale * 2.0), false));
        let velocity = Vec2::new(
            fround(fround(direction.x * speed) * 0.75),
            fround(fround(direction.y * speed) * 0.75),
        );
        particles.push(NativeCoffinEmergenceParticle {
            record,
            scale: size,
            enhanced_shadow: enhanced,
            motion: NativeBoulderDebrisMotion {
                alpha: if enhanced { 10.0 } else { 2.0 },
                bounce_velocity: vertical_velocity,
                height,
                position: root,
                rotation_degrees,
                rotation_step_degrees,
                velocity,
                vertical_velocity,
            },
        });
        heading = fround(heading + 18.0 + rng.draw_scaled_float(6.0, true));
    }
    particles
}

pub(super) fn spawn_coffin_emergence(
    world: &mut EnemyWorld,
    actor: &EnemyActor,
    tick: u64,
    enhanced: bool,
    order: &mut WorldPainterOrder,
) {
    let particles = create_native_coffin_emergence(
        &mut world.steering_rng,
        actor.position,
        actor.config.scale,
        actor.config.collision_radius,
        enhanced,
    );
    for particle in particles {
        let motion = particle.motion;
        world.spawn_simple_death_effect(
            DeathEffectOwner::from(actor),
            tick,
            SimpleDeathEffectSpec {
                alpha: 1.0,
                alpha_loss_per_tick: 0.025_f32 as f64,
                atlas: EnemyEffectAtlas::BadGuys,
                blend_mode: EnemyEffectBlend::Normal,
                entry: particle.record,
                kind: EnemyDeathEffectKind::BoulderBit,
                lifetime_ticks: if enhanced { 400 } else { 80 },
                role: "coffin-emergence-rock",
                scale: particle.scale,
            },
            DeathEffectOverrides {
                angular_velocity_deg: Some(motion.rotation_step_degrees),
                opacity_timer: Some(motion.alpha),
                position: Some(motion.position),
                rotation_deg: Some(motion.rotation_degrees),
                velocity: Some(motion.velocity),
                ..Default::default()
            },
            order,
        );
        let effect = world.death_effects.last_mut().unwrap();
        effect.bounce_retention = 0.3_f32 as f64;
        effect.bounce_velocity = motion.bounce_velocity;
        effect.height = motion.height;
        effect.shadow = particle.enhanced_shadow;
        effect.vertical_velocity = motion.vertical_velocity;
    }
}

pub(super) fn step_coffin_emergence(
    effect: &mut EnemyDeathEffect,
    tick: u64,
    rng: &mut NativeRng,
) -> bool {
    let Some(motion) = step_native_boulder_debris_motion(
        NativeBoulderDebrisMotion {
            alpha: effect.opacity_timer,
            bounce_velocity: effect.bounce_velocity,
            height: effect.height,
            position: effect.position,
            rotation_degrees: effect.rotation_deg,
            rotation_step_degrees: effect.angular_velocity_deg,
            velocity: effect.velocity,
            vertical_velocity: effect.vertical_velocity,
        },
        tick,
        rng,
    ) else {
        return false;
    };
    effect.age_ticks = tick - effect.spawn_tick;
    effect.last_step_tick = Some(tick);
    effect.alpha = motion.alpha.min(1.0);
    effect.opacity_timer = motion.alpha;
    effect.bounce_velocity = motion.bounce_velocity;
    effect.height = motion.height;
    effect.position = motion.position;
    effect.rotation_deg = motion.rotation_degrees;
    effect.angular_velocity_deg = motion.rotation_step_degrees;
    effect.velocity = motion.velocity;
    effect.vertical_velocity = motion.vertical_velocity;
    true
}
