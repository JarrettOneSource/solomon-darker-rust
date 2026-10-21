//! Portal materialization and recurring Imp ejections from `native-survival-portal.ts`.
use super::config::EnemyFamilyConfig;
use super::effects::DeathEffectOwner;
use super::model::*;
use super::movement::positive_modulo;
use super::world::{EnemyMovementPurpose, EnemyMovementRequest, EnemyStepContext, EnemyWorld};
use super::{EnemySpawnIntent, EnemyToken, PortalEjection};
use crate::Vec2;
use crate::float::fround;

pub fn step_portal(world: &mut EnemyWorld, actor: &mut EnemyActor, context: &EnemyStepContext<'_>) {
    let EnemyBrain::Portal(mut brain) = actor.brain.clone() else {
        panic!("Portal brain required");
    };
    let EnemyFamilyConfig::Portal { frequency } = actor.config.family else {
        panic!("Portal config required");
    };
    // Portal::Tick settles its placement body before capturing +0x218/+0x21C.
    if brain.age_ticks < 10 {
        actor.position = context.geometry.resolve_movement(EnemyMovementRequest {
            actor_id: actor.id,
            delta: Vec2::ZERO,
            position: actor.position,
            purpose: EnemyMovementPurpose::Movement,
            radius: 45.0,
            requested_position: actor.position,
        });
    }
    brain.age_ticks += 1;
    if brain.age_ticks <= 10 {
        if brain.age_ticks == 10 {
            brain.anchor_position = Some(actor.position);
            let mut event =
                EnemyEvent::new(EnemyEventKind::EnemyActionSound, actor.id, context.tick);
            event.gain_scale = Some(0.5);
            event.pitch = Some(1.0);
            event.sound = Some(EnemySound::PortalOpen);
            event.source_position = Some(actor.position);
            world.emit_event(event);
        }
    } else {
        brain.body_phase =
            fround(positive_modulo(brain.body_phase + 0.15 + world.draw_unit() * 0.15, 32.0));
        brain.aura_phase =
            fround(positive_modulo(brain.aura_phase + 0.05 + world.draw_unit() * 0.2, 20.0));
        brain.alpha = fround((brain.alpha + fround(0.025)).min(1.0));
        if brain.ticks_until_ejection > 1 {
            brain.ticks_until_ejection -= 1;
        } else {
            let heading = world.draw_unit() * 360.0;
            let velocity = fround(-(10.0 + world.draw_unit() * 5.0));
            let (lower, upper) =
                [(800, 1000), (600, 800), (300, 400), (200, 300), (100, 200), (25, 50)]
                    [frequency as usize];
            let regular = lower + world.draw_integer(upper - lower + 1);
            brain.ticks_until_ejection = u64::from(if world.draw_integer(8) == 1 {
                world.draw_integer(upper)
            } else {
                regular
            });
            world.emit_action_sound(
                DeathEffectOwner::from(&*actor),
                context.tick,
                EnemySound::FireballHit,
                1.0,
            );
            let mut intent = EnemySpawnIntent::new(
                EnemyToken::Imp,
                actor.position,
                world.next_synthetic_spawn_intent_id,
                context.tick,
                actor.wave_ordinal,
            );
            intent.portal_ejection = Some(PortalEjection {
                child_heading_deg: heading,
                inherited_primary_damage: actor.config.primary_damage.unwrap_or(0.0),
                parent_heading_deg: actor.heading_deg,
                parent_position: actor.position,
                vertical_velocity: velocity,
            });
            world.pending_spawn_intents.push(intent);
            world.next_synthetic_spawn_intent_id += 1;
        }
    }
    brain.hurt_ticks_remaining = brain.hurt_ticks_remaining.saturating_sub(1);
    brain.phase = PortalPhase::Active;
    actor.body_pose = brain.body_phase;
    actor.gait_pose = brain.aura_phase;
    actor.stride_phase_deg = brain.fixed_scale;
    // External impulses leave the settled root unchanged after materialization.
    if let Some(anchor) = brain.anchor_position {
        actor.position = anchor;
    }
    actor.brain = EnemyBrain::Portal(brain);
}
