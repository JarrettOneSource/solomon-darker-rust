//! Presentation actors whose native tick is local to their own state.

use super::dampened::{NativeDampenedSpellState, step_native_dampened_spell};
use super::program::*;
use super::step::StepEngine;
use super::{
    NativeSecondaryActorKind as Kind, NativeSecondaryDamageKind as Damage, NativeSecondaryWorld,
};
use crate::Vec2;
use crate::float::fround;
use crate::secondary::{SecondaryActorRead, SecondaryActorWrite};

pub(super) fn step<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &impl SecondaryActorRead,
    actor: &mut impl SecondaryActorWrite,
    retain: &mut bool,
) {
    match actor.kind {
        Kind::PlanewalkerMote => {
            let alpha = fround(source.alpha - source.phase);
            actor.alpha = alpha.max(0.0);
            actor.position = Vec2::new(
                fround(source.position.x + source.velocity.x),
                fround(source.position.y + source.velocity.y),
            );
            actor.velocity = Vec2::new(
                fround(source.velocity.x * (0.95_f32 as f64)),
                fround(source.velocity.y * (0.95_f32 as f64)),
            );
            *retain = alpha > 0.0;
        }
        Kind::PrismaticWave => {
            if actor.age_ticks <= 100 {
                advance_rng(&mut engine.state.rng, 19);
                let owner = &engine.context.players[&actor.owner_id];
                actor.alpha = fround(source.alpha + (0.025_f32 as f64)).min(1.0);
                actor.phase = fround(source.phase + source.slow_factor * 6.0);
                actor.position =
                    Vec2::new(owner.character.position.x, owner.character.position.y - 25.0);
                actor.scale = fround(
                    source.scale
                        + if source.age_ticks < 50 {
                            0.065_f32 as f64
                        } else {
                            -(0.075_f32 as f64)
                        },
                );
            } else {
                actor.alpha = 0.0;
            }
        }
        Kind::PhaseBurst => actor.alpha = fround(source.alpha - (0.1_f32 as f64)).max(0.0),
        Kind::IceBlast | Kind::StormStrike | Kind::TurnUndead => {
            actor.alpha = (actor.alpha - 0.05).max(0.0);
            actor.scale *= 1.1;
        }
        Kind::DampenedProjectile => {
            let mut projectile = NativeDampenedSpellState {
                heading_degrees: source.rotation_radians * 180.0 / std::f64::consts::PI,
                phase_degrees: source.phase,
                position: source.position,
                velocity: source.velocity,
            };
            let smoke =
                step_native_dampened_spell(&mut projectile, actor.variant, &mut engine.state.rng);
            actor.frame = 0.0;
            actor.phase = projectile.phase_degrees;
            actor.position = projectile.position;
            actor.rotation_radians = projectile.heading_degrees * std::f64::consts::PI / 180.0;
            *retain = engine.world.effect_visible(&actor.world_key, actor.position, 50.0);
            let mut child = super::program::actor(
                Kind::DampenedSmoke,
                &actor.owner_id,
                &actor.world_key,
                actor.skill_id,
                1,
                smoke.position,
            );
            child.frame = f64::from(smoke.entry);
            child.lifetime_ticks = 101;
            child.phase = smoke.color_alpha;
            child.quantity = f64::from(smoke.tint);
            child.rotation_radians = smoke.rotation_degrees * std::f64::consts::PI / 180.0;
            child.scale = smoke.scale;
            child.slow_factor = smoke.alpha_loss;
            child.variant = actor.variant;
            engine.state.spawn_actor(child);
        }
        Kind::DampenedSmoke => {
            actor.frame = source.frame;
            actor.alpha = fround(source.alpha - source.slow_factor).max(0.0);
            *retain = actor.alpha > 0.0;
        }
        Kind::FlashResponseGrow | Kind::FlashResponseFade | Kind::ShieldBreak => {
            actor.alpha = fround(source.alpha - (0.05_f32 as f64)).max(0.0);
            if actor.kind == Kind::FlashResponseGrow {
                actor.scale = fround(source.scale * (1.05_f32 as f64));
            }
            *retain = actor.alpha > 0.0;
        }
        Kind::TeleportBurst => {
            actor.alpha = fround(source.alpha - (0.1_f32 as f64)).max(0.0);
            actor.scale = fround(
                source.scale * if source.variant == 0 { 1.1_f32 as f64 } else { 0.96_f32 as f64 },
            );
        }
        Kind::ShieldExplosion => {
            if actor.age_ticks == 1 {
                for target in engine.candidates(actor, 110.0) {
                    engine.damage(actor, &target, actor.damage, Damage::Magic);
                }
            }
        }
        _ => unreachable!("local presentation dispatcher"),
    }
}
