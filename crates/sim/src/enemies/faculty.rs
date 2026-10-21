//! Faculty actor epochs from `core-server/enemies/faculty.ts`.

use super::boss_spells::spawn_faculty_spell;
use super::config::EnemyFamilyConfig;
use super::effects::DeathEffectOwner;
use super::faculty_controller::{
    NativeFacultyContext, advance_native_faculty_orbit, step_native_faculty,
};
use super::faculty_effects::{FacultySmokePhase, spawn_faculty_smoke};
use super::model::*;
use super::movement::{move_toward_target, staff_attack_speed};
use super::path::{RadialDirection, TangentDirection};
use super::targeting::{target_by_id, target_eligible};
use super::world::{EnemyStepContext, EnemyWorld};
use crate::Vec2;
use crate::float;
use crate::float::{fround, hypot};
use crate::heading::heading_degrees_from_vector;
use crate::painter::WorldPainterOrder;

pub fn step_faculty(
    world: &mut EnemyWorld,
    actor: &mut EnemyActor,
    context: &EnemyStepContext<'_>,
    order: &mut WorldPainterOrder,
) {
    let EnemyBrain::Faculty(source_brain) = &actor.brain else { panic!("Faculty brain required") };
    let EnemyFamilyConfig::Faculty(family) = &actor.config.family else {
        panic!("Faculty config required")
    };
    let primary = family.primary;
    let source_heading = actor.heading_deg;
    let orbit_sign = source_brain.orbit_sign;
    advance_native_faculty_orbit(&mut actor.path, orbit_sign, &mut world.steering_rng);
    let effect = context.ability_effect(actor.id);
    let disrupted = effect.disrupted_ticks > 0;
    let fleeing = effect.flee_ticks > 0;
    let frozen = staff_attack_speed(actor) < 9.999_999_747_378_752e-5;
    let reorienting = actor.path.reorientation_ticks_remaining > 0;
    if disrupted || fleeing {
        reset_faculty(actor);
    }
    let EnemyBrain::Faculty(mut brain) = actor.brain.clone() else {
        panic!("Faculty action retains its owner")
    };
    let source_action = brain.action.clone();
    if !disrupted && !frozen {
        if reorienting {
            let target = target_by_id(actor.target_player_id.as_deref(), context.players)
                .map(|target| target.position);
            actor.heading_deg =
                actor.path.step_reorientation(actor.heading_deg, actor.position, target);
        } else if fleeing {
            move_toward_target(
                world,
                actor,
                context,
                RadialDirection::Retreat,
                TangentDirection::Hold,
            );
        } else if brain.action.is_none() {
            move_toward_target(
                world,
                actor,
                context,
                RadialDirection::Approach,
                TangentDirection::Hold,
            );
        }
    }
    let advance_action = !disrupted && !frozen && !fleeing && !reorienting;
    let target = target_by_id(actor.target_player_id.as_deref(), context.players)
        .filter(|target| target_eligible(target));
    let delta = target.map_or(Vec2::ZERO, |target| {
        Vec2::new(target.position.x - actor.position.x, target.position.y - actor.position.y)
    });
    let line_of_sight = target.is_some_and(|target| {
        let clipped = context.geometry.clip_spell_segment(actor.position, target.position);
        hypot(clipped.x - target.position.x, clipped.y - target.position.y) < 0.0001
    });
    let step = step_native_faculty(
        &mut brain,
        &mut world.steering_rng,
        primary,
        NativeFacultyContext {
            advance_action,
            actor_time_scale: staff_attack_speed(actor),
            line_of_sight,
            target_distance_squared: target.map(|_| delta.x * delta.x + delta.y * delta.y),
            tick: context.tick,
        },
    );
    if advance_action
        && source_action.as_ref().is_some_and(|action| action.kind == FacultyActionKind::Lightning)
        && let Some(target) = target
    {
        actor.heading_deg = if brain.heading_locked {
            source_heading
        } else {
            heading_degrees_from_vector(
                delta.x + target.velocity_per_tick.x * 65.0,
                delta.y + target.velocity_per_tick.y * 65.0 - 25.0,
            )
        };
    }
    spawn_faculty_smoke(
        world,
        DeathEffectOwner::from(&*actor),
        context.tick,
        FacultySmokePhase::Living,
        order,
    );
    let camera_angle =
        world.steering_rng.draw_scaled_float(360.0, false) * std::f64::consts::PI / 180.0;
    let mut event = EnemyEvent::new(EnemyEventKind::EnemyCameraShake, actor.id, context.tick);
    event.source_position = Some(actor.position);
    event.camera_shake = Some(EnemyCameraShake {
        attenuation: CameraShakeAttenuation::HitSquared,
        displacement: Vec2::new(
            fround(float::sin(camera_angle)),
            fround(-float::cos(camera_angle)),
        ),
    });
    world.emit_event(event);
    if step.marker
        && source_action.as_ref().is_some_and(|action| action.kind == FacultyActionKind::Lightning)
    {
        let owner = DeathEffectOwner::from(&*actor);
        world.emit_action_sound(owner, context.tick, EnemySound::LightningStart, 0.5);
        world.emit_action_sound(owner, context.tick, EnemySound::LightningStart, 0.75);
        world.emit_action_sound(owner, context.tick, EnemySound::FlameLashStart, 1.0);
        let mut event = EnemyEvent::new(EnemyEventKind::EnemyScreenFlash, actor.id, context.tick);
        event.source_position = Some(actor.position);
        event.screen_flash = Some(EnemyScreenFlash {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
            decay_per_tick: 0.025,
            point_attenuated: true,
        });
        world.emit_event(event);
    }
    brain.body_heading_deg = if brain.heading_locked || target.is_none() {
        actor.heading_deg
    } else {
        heading_degrees_from_vector(delta.x, delta.y)
    };
    actor.body_pose = brain.body_pose;
    // The dispatch observes the new pose/hand state while the phase label still belongs
    // to the incoming actor. The final phase is assigned after the spell producer.
    actor.brain = EnemyBrain::Faculty(brain.clone());
    if let Some(dispatch) = step.dispatch {
        spawn_faculty_spell(world, actor, dispatch, context, order);
    }
    brain.phase = if brain.action.is_none() { BossPhase::RangeControl } else { BossPhase::Cast };
    actor.brain = EnemyBrain::Faculty(brain);
}

pub fn reset_faculty(actor: &mut EnemyActor) {
    let EnemyBrain::Faculty(brain) = &mut actor.brain else { panic!("Faculty brain required") };
    actor.body_pose = 0.0;
    brain.action = None;
    brain.body_pose = 0.0;
    brain.hand_mask = 0;
    brain.heading_locked = false;
    brain.lightning_active = false;
    brain.phase = BossPhase::RangeControl;
}
