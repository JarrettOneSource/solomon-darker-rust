//! Discorporeal outer actor epoch from `enemies/demon-skull.ts`.

use super::demon_skull_actions::{begin_demon_skull_action, shake, step_demon_skull_actions};
use super::demon_skull_controller::*;
use super::effects::DeathEffectOwner;
use super::model::*;
use super::movement::move_toward_target;
use super::path::{RadialDirection, TangentDirection};
use super::targeting::{target_by_id, target_eligible};
use super::world::{
    EnemyMovementPurpose, EnemyMovementRequest, EnemyProjectileWorldQuery, EnemyStepContext,
    EnemyWorld,
};
use crate::Vec2;
use crate::float::fround;
use crate::heading::heading_degrees_from_vector;
use crate::painter::WorldPainterOrder;

pub fn step_demon_skull(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    authored_attack_speed: f64,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::DemonSkull(brain) = &mut actor.brain else {
        panic!("DemonSkull brain required")
    };
    let admitted = context.native_visibility.is_none_or(|visible| visible(actor.position).admitted);
    prepare_native_demon_skull_tick(brain, admitted);
    let effect = context.ability_effect(actor.id);
    let disrupted = effect.disrupted_ticks > 0;
    let fleeing = effect.flee_ticks > 0;
    let active = effect.time_scale >= 9.999_999_747_378_752e-5;
    if disrupted || fleeing {
        brain.actions.clear();
        brain.phase = BossPhase::RangeControl;
    }
    if active && !disrupted {
        if !brain.actions.is_empty() {
            step_demon_skull_actions(world, actor, context, order);
        } else {
            if actor.path.reorientation_ticks_remaining > 0 {
                let target = target_by_id(actor.target_player_id.as_deref(), context.players)
                    .map(|target| target.position);
                actor.heading_deg =
                    actor.path.step_reorientation(actor.heading_deg, actor.position, target);
            } else {
                move_toward_target(
                    world,
                    actor,
                    context,
                    if fleeing { RadialDirection::Retreat } else { RadialDirection::Approach },
                    TangentDirection::Hold,
                );
            }
            let target = target_by_id(actor.target_player_id.as_deref(), context.players)
                .filter(|target| target_eligible(target));
            let EnemyBrain::DemonSkull(brain) = &actor.brain else { unreachable!() };
            if !brain.scream_active
                && !fleeing
                && actor.path.reorientation_ticks_remaining == 0
                && let Some(target) = target
                && squared(actor.position, target.position) < 120.0 * 120.0
                && clear_line(context, actor, target.position)
            {
                actor.heading_deg = heading_degrees_from_vector(
                    target.position.x - actor.position.x,
                    target.position.y - actor.position.y,
                );
                begin_demon_skull_action(
                    world,
                    actor,
                    DemonSkullActionKind::Bite,
                    context,
                    authored_attack_speed,
                );
            }
        }
    }
    let EnemyBrain::DemonSkull(brain) = &mut actor.brain else { unreachable!() };
    let recoil = brain.recoil;
    if recoil.x * recoil.x + recoil.y * recoil.y > 1.0 {
        actor.position = context.geometry.resolve_movement(EnemyMovementRequest {
            actor_id: actor.id,
            position: actor.position,
            delta: recoil,
            radius: actor.config.collision_radius,
            purpose: EnemyMovementPurpose::Movement,
            requested_position: Vec2::new(actor.position.x + recoil.x, actor.position.y + recoil.y),
        });
        brain.recoil = Vec2::new(fround(recoil.x * 0.5), fround(recoil.y * 0.5));
    } else {
        brain.recoil = Vec2::ZERO;
    }
    let target = target_by_id(actor.target_player_id.as_deref(), context.players)
        .filter(|target| target_eligible(target));
    let line_of_sight = target.is_some_and(|target| clear_line(context, actor, target.position));
    let EnemyBrain::DemonSkull(brain) = &mut actor.brain else { unreachable!() };
    let step = step_native_demon_skull_controller(
        brain,
        &mut world.steering_rng,
        NativeDemonSkullControllerContext {
            alternate_player: false,
            has_action: !brain.actions.is_empty(),
            heading_deg: actor.heading_deg,
            health_ratio: actor.current_health / actor.config.maximum_health,
            line_of_sight,
            movement_scale: actor.config.scale,
            scream_playing: world.demon_skull_encounter.scream_stream_ticks_remaining > 0,
            target_distance_squared: target.map(|target| squared(actor.position, target.position)),
        },
    );
    if step.increment_attack_seed {
        actor.loot_seed = actor.loot_seed.wrapping_add(1);
    }
    if let Some(kind) = step.begin_action {
        begin_demon_skull_action(world, actor, kind, context, authored_attack_speed);
    }
    if step.landing {
        world.emit_action_sound(
            DeathEffectOwner::from(&*actor),
            context.tick,
            EnemySound::Jaw,
            1.0,
        );
    }
    if step.scream_shake {
        shake(world, actor, context.tick, CameraShakeAttenuation::Hit, 8.0);
    }
    let EnemyBrain::DemonSkull(brain) = &mut actor.brain else { unreachable!() };
    actor.body_pose = brain.body_pose;
    brain.phase = if brain.actions.is_empty() { BossPhase::RangeControl } else { BossPhase::Cast };
}

fn clear_line(context: &EnemyStepContext<'_>, actor: &EnemyActor, end: Vec2) -> bool {
    !context.geometry.projectile_world_blocked(EnemyProjectileWorldQuery::Line {
        projectile_id: actor.id,
        start: actor.position,
        end,
        native_exclusion_mask: 0x380,
    })
}
fn squared(a: Vec2, b: Vec2) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}
