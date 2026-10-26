//! Earthquake's ordered scenery disturbances and the shared articulated Golem.

use super::casting::GolemWorld;
use super::golem::{NativeGolemKernelActor, NativeGolemKernelTarget, step_native_secondary_golem};
use super::program::*;
use super::spawns;
use super::step::StepEngine;
use super::{
    NativeSecondaryActorKind as Kind, NativeSecondaryAudioCue as Cue,
    NativeSecondaryDamageKind as Damage, NativeSecondaryEventKind as Event,
    NativeSecondaryHeadingMode, NativeSecondaryTargetEffectPatch as Patch,
    NativeSecondaryTargetHeadingChange, NativeSecondaryWorld,
};
use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::secondary::{SecondaryActorRead, SecondaryActorWrite};

pub(super) fn step<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &impl SecondaryActorRead,
    actor: &mut impl SecondaryActorWrite,
    retain: &mut bool,
) {
    match actor.kind {
        Kind::Earthquake => quake(engine, source, actor),
        Kind::EarthquakeSceneryWobble => {
            actor.frame = 0.0;
            actor.phase = engine.wobble_phases.get(&actor.id).copied().unwrap_or(source.phase);
            *retain = true;
        }
        Kind::EarthquakeQuake => {
            actor.phase = fround(source.phase + 2.0);
            actor.scale = fround(source.scale + (0.005_f32 as f64));
            actor.slow_factor = fround(source.slow_factor + (0.005_f32 as f64));
            if float::sin(actor.phase * std::f64::consts::PI / 180.0).abs() < 0.01 {
                let delta = fround(0.25 + engine.state.rng.draw_scaled_float(0.75, false));
                actor.scale = fround(actor.scale + delta);
                actor.slow_factor = fround(actor.slow_factor + delta);
                actor.alpha = fround(source.alpha * (0.95_f32 as f64));
            }
            *retain = actor.phase < 360.0;
        }
        Kind::EarthquakeDust => {
            // Its constructor already ran one dust pre-tick; common actor movement
            // excludes this family so the float32 position store happens only here.
            actor.phase = source.phase;
            actor.position = source.position;
            spawns::advance_dust(actor);
            *retain = actor.phase < 180.0;
        }
        Kind::EarthquakeDebris => debris(engine, source, actor, retain),
        Kind::Golem => golem(engine, source, actor, retain),
        Kind::GolemKnockback => golem_knockback(engine, source, actor, retain),
        Kind::GolemAssemblyDebris => assembly_debris(engine, source, actor, retain),
        _ => unreachable!("earth family dispatcher"),
    }
}

fn quake<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &impl SecondaryActorRead,
    actor: &mut impl SecondaryActorWrite,
) {
    let before = source.lifetime_ticks.saturating_sub(source.age_ticks);
    let after = before.saturating_sub(1);
    let intensity = fround(before.min(200) as f64 / 200.0);
    if source.age_ticks == 0 {
        engine.event(actor, Some(Cue::RockHit), Event::Pulse);
        engine.event(actor, Some(Cue::QuakeCracks), Event::Pulse);
    }
    let horizontal = engine.state.rng.draw_scaled_float(3.0, true);
    let phase = fround(source.phase + (0.05_f32 as f64));
    let mut green = fround(source.quantity - (0.05_f32 as f64)).max(0.0);
    if source.phase < 0.6 && phase > 0.6 || source.phase < 3.0 && phase > 3.0 {
        green = 1.0;
    }
    if source.phase < 3.0 && phase > 3.0 {
        engine.event(actor, Some(Cue::QuakeCrackSmall), Event::Pulse);
    }
    actor.alpha = intensity;
    actor.frame = source.frame;
    actor.phase = phase;
    actor.quantity = green;
    actor.velocity = Vec2::new(
        horizontal,
        fround(float::sin(before as f64 * 20.0 * std::f64::consts::PI / 180.0) * 10.0 * intensity),
    );
    if after.is_multiple_of(30) {
        if intensity >= 0.99 {
            spawns::quake(engine.state, actor, intensity);
        }
        let mut targets = engine.candidates(actor, 512.0);
        targets.retain(|target| squared_distance(actor.position, target.position) < 512.0 * 512.0);
        shuffle_fixed_bound(&mut targets, &mut engine.state.rng);
        let count = targets.len() / 2;
        for target in targets.into_iter().take(count) {
            engine.output.disrupted_target_ids.push(target.id);
            let mut ticks = 1;
            if engine.state.rng.draw_integer(2) == 1 {
                let pause = engine.state.rng.draw_integer(50);
                let time = engine
                    .effect(&actor.world_key, target.id)
                    .map_or(1.0, |effect| effect.time_scale);
                ticks = ((50.0 + f64::from(pause)) / time.max(f64::EPSILON) + 0.5).floor() as u64;
            }
            let heading = engine.state.rng.draw_sign(15.0);
            engine.output.target_heading_changes.push(NativeSecondaryTargetHeadingChange {
                degrees: heading,
                mode: NativeSecondaryHeadingMode::Relative,
                target_id: target.id,
            });
            engine.merge(
                actor,
                target.id,
                Patch { disrupted_ticks: Some(ticks), ..Patch::default() },
            );
        }
    }
    let scenery = source
        .hit_target_ids
        .get(source.frame as usize)
        .and_then(|id| engine.sources.get(id))
        .filter(|actor| actor.kind == Kind::EarthquakeSceneryWobble && actor.target_id.is_some())
        .cloned();
    if let Some(scenery) = scenery {
        let key = (actor.world_key.clone(), scenery.target_id.unwrap());
        let current = engine.scenery_phases.get(&key).copied().unwrap_or(scenery.phase);
        let direction = engine.state.rng.draw_sign(1.0);
        let adjustment = engine.state.rng.draw_scaled_float(1.5, false);
        let multiplier = if current < -2.0 {
            1.0
        } else if current > 2.0 {
            -1.0
        } else {
            direction
        };
        let next = fround(current + fround(adjustment * multiplier));
        engine.scenery_phases.insert(key, next);
        engine.wobble_phases.insert(scenery.id, next);
        actor.frame = source.frame + 1.0;
        if engine.context.enhanced_effects_for(&actor.owner_id)
            && engine.state.rng.draw_integer(30) == 1
        {
            spawns::dust(engine.state, actor, &scenery);
        }
    } else {
        actor.frame = 0.0;
    }
    if engine.state.rng.draw_integer(15) == 1 {
        spawns::quake_debris(engine.state, actor);
    }
}

fn debris<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &impl SecondaryActorRead,
    actor: &mut impl SecondaryActorWrite,
    retain: &mut bool,
) {
    let mut alpha = source.alpha;
    let mut bounce = source.quantity;
    let mut height = source.phase;
    let mut position = source.position;
    let mut rotation_step = source.endpoint.y;
    let mut rotation = source.rotation_radians;
    let mut vertical = source.endpoint.x;
    let mut velocity = source.velocity;
    if !(height != 0.0 && engine.context.tick.is_multiple_of(3)) {
        if height != 0.0 {
            position = Vec2::new(fround(position.x + velocity.x), fround(position.y + velocity.y));
            height = fround(height + vertical);
            vertical = fround(vertical + (0.4_f32 as f64));
            if height > 0.0 {
                if engine.world.effect_position_blocked(&actor.world_key, position) {
                    *retain = false;
                }
                rotation_step = fround(1.0 + engine.state.rng.draw_scaled_float(10.0, false));
                bounce = fround(bounce * (0.3_f32 as f64));
                vertical = bounce;
                if engine.state.rng.draw_integer(2) == 1 {
                    velocity = Vec2::new(
                        fround(velocity.x * (0.65_f32 as f64)),
                        fround(velocity.y * (0.65_f32 as f64)),
                    );
                }
                if vertical > (-0.75_f32 as f64) {
                    bounce = 0.0;
                    rotation_step = 0.0;
                    vertical = 0.0;
                    velocity = Vec2::ZERO;
                }
                height = vertical;
                actor.frame = source.frame + 1.0;
            }
            rotation = fround(rotation + rotation_step * std::f64::consts::PI / 180.0);
        }
        alpha = fround(alpha - (0.015_f32 as f64));
    }
    alpha = fround(alpha - (0.025_f32 as f64));
    actor.alpha = alpha.max(0.0);
    actor.endpoint = Vec2::new(vertical, rotation_step);
    actor.phase = height;
    actor.position = position;
    actor.quantity = bounce;
    actor.rotation_radians = rotation;
    actor.velocity = velocity;
    *retain = *retain && alpha > 0.0;
}

fn golem<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &impl SecondaryActorRead,
    actor: &mut impl SecondaryActorWrite,
    retain: &mut bool,
) {
    let mut kernel = NativeGolemKernelActor {
        age_ticks: source.age_ticks,
        damage_minimum: source.damage,
        golem: source.golem().copied().expect("Golem actor retains its articulated state"),
        id: source.id,
        owner_id: source.owner_id.clone(),
        position: source.position,
        rotation_radians: source.rotation_radians,
        target_id: source.target_id,
    };
    let targets = engine
        .candidates(source, 1000.0)
        .into_iter()
        .map(|target| NativeGolemKernelTarget {
            id: target.id,
            position: target.position,
            radius: target.radius,
            registration_order: target.registration_order,
        })
        .collect::<Vec<_>>();
    let mut world = GolemWorld {
        world: engine.world,
        owner: &source.owner_id,
        world_key: &source.world_key,
        origin: source.position,
    };
    let owner = &engine.context.players[&source.owner_id];
    let result = step_native_secondary_golem(
        &mut kernel,
        Some(owner.character.position),
        &targets,
        &mut world,
        &mut engine.state.rng,
    );
    actor.age_ticks = kernel.age_ticks;
    actor.frame = source.frame + 1.0;
    *actor.golem_mut().expect("Golem retains its articulation") = kernel.golem;
    actor.position = kernel.position;
    actor.rotation_radians = kernel.rotation_radians;
    actor.target_id = kernel.target_id;
    if let Some(milestone) = result.assembly_milestone {
        let enhanced = engine.context.enhanced_effects_for(&actor.owner_id);
        let debris = super::golem_debris::create_native_golem_assembly_debris(
            &mut engine.state.rng,
            actor.scale,
            enhanced,
        );
        for particle in debris {
            let body = particle.common;
            let mut child = super::program::actor(
                Kind::GolemAssemblyDebris,
                &actor.owner_id,
                &actor.world_key,
                Some(45),
                1,
                Vec2::new(
                    fround(actor.position.x + body.position.x),
                    fround(actor.position.y + body.position.y),
                ),
            );
            child.enhanced = enhanced;
            child.scale = body.scale;
            child.alpha = body.alpha;
            child.frame = body.record as f64;
            child.phase = body.height;
            child.quantity = body.index as f64;
            child.endpoint = Vec2::new(particle.bounce_velocity, body.vertical_velocity);
            child.rotation_radians = body.rotation_degrees * std::f64::consts::PI / 180.0;
            child.slow_factor = body.rotation_step_degrees * std::f64::consts::PI / 180.0;
            child.velocity = body.velocity;
            child.lifetime_ticks = if enhanced { 400 } else { 80 };
            engine.state.spawn_actor(child);
        }
        engine.event(actor, Some(Cue::QuakeCrackSmall), Event::Pulse);
        let mut event = actor_event(
            actor,
            engine.context.tick,
            Event::Pulse,
            Some(if milestone == 0 { Cue::FlameLashStart } else { Cue::RockHit }),
        );
        event.pitch = if milestone == 0 { 0.8 } else { 1.0 };
        engine.state.emit_event(event);
    }
    if result.provoke_started {
        engine.event(actor, Some(Cue::GolemProvoke), Event::Pulse);
    }
    if result.footstep {
        engine.event(actor, Some(Cue::StoneStep), Event::Pulse);
    }
    if let Some(contact) = result.contact {
        engine.event(actor, Some(Cue::KnockbackGolem), Event::Impact);
        let mut child = super::program::actor(
            Kind::GolemKnockback,
            &actor.owner_id,
            &actor.world_key,
            Some(45),
            1,
            contact.origin,
        );
        child.damage = contact.damage;
        child.quantity = contact.movement_budget;
        child.rotation_radians = contact.heading_degrees * std::f64::consts::PI / 180.0;
        child.radius = 120.0;
        child.hit_target_ids = contact.target_ids.into_iter().map(|id| id as u64).collect();
        child.lifetime_ticks = 6;
        engine.state.spawn_actor(child);
    }
    *retain = true;
}

fn assembly_debris<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &impl SecondaryActorRead,
    actor: &mut impl SecondaryActorWrite,
    retain: &mut bool,
) {
    use crate::spells::earth_debris::{
        NativeBoulderDebrisMotion, step_native_boulder_debris_motion,
    };
    let motion = step_native_boulder_debris_motion(
        NativeBoulderDebrisMotion {
            alpha: source.alpha,
            bounce_velocity: source.endpoint.x,
            height: source.phase,
            position: source.position,
            rotation_degrees: source.rotation_radians * 180.0 / std::f64::consts::PI,
            rotation_step_degrees: source.slow_factor * 180.0 / std::f64::consts::PI,
            velocity: source.velocity,
            vertical_velocity: source.endpoint.y,
        },
        engine.context.tick,
        &mut engine.state.rng,
    );
    *retain = motion.is_some();
    if let Some(motion) = motion {
        actor.alpha = motion.alpha;
        actor.phase = motion.height;
        actor.endpoint = Vec2::new(motion.bounce_velocity, motion.vertical_velocity);
        actor.position = motion.position;
        actor.velocity = motion.velocity;
        actor.rotation_radians = motion.rotation_degrees * std::f64::consts::PI / 180.0;
        actor.slow_factor = motion.rotation_step_degrees * std::f64::consts::PI / 180.0;
    }
}

/// The shared area callback commits each move before the next target query.
/// A target reached through body contact joins this same five-update area.
fn golem_knockback<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &impl SecondaryActorRead,
    actor: &mut impl SecondaryActorWrite,
    retain: &mut bool,
) {
    use crate::knockback::{
        NativeKnockbackArea, NativeKnockbackAreaWorld, step_native_knockback_area,
    };

    struct AreaWorld<'a, W> {
        world: &'a mut W,
        world_key: &'a str,
        source_actor_id: u64,
    }
    impl<W: NativeSecondaryWorld> NativeKnockbackAreaWorld<usize> for AreaWorld<'_, W> {
        fn position(&self, id: &usize) -> Option<Vec2> {
            self.world.target(self.world_key, *id).map(|target| target.position)
        }
        fn move_actor(&mut self, id: &usize, delta: Vec2) -> Vec<usize> {
            self.world.knockback_area_movement(self.world_key, *id, delta, self.source_actor_id)
        }
    }
    let ids: Vec<_> = source.hit_target_ids.iter().map(|id| *id as usize).collect();
    let area = step_native_knockback_area(
        NativeKnockbackArea {
            origin: actor.position,
            remaining_distance: source.quantity,
            target_ids: &ids,
        },
        &mut engine.state.rng,
        &mut AreaWorld {
            world: engine.world,
            world_key: &actor.world_key,
            source_actor_id: actor.id,
        },
    );
    engine.state.record_camera_displacement(
        &actor.world_key,
        area.camera_displacement,
        engine.context.tick,
    );
    actor.quantity = area.remaining_distance;
    actor.hit_target_ids = area.target_ids.into_iter().map(|id| id as u64).collect();
    *retain = !area.terminal;
    for perturbation in area.heading_perturbations {
        let Some(target) = engine.world.target(&actor.world_key, perturbation.target_id) else {
            continue;
        };
        engine.damage(actor, &target, actor.damage, Damage::Physical);
        engine.merge(actor, target.id, Patch { dazzle_ticks: Some(200), ..Patch::default() });
        engine.output.target_heading_changes.push(NativeSecondaryTargetHeadingChange {
            degrees: perturbation.heading_degrees,
            mode: NativeSecondaryHeadingMode::Relative,
            target_id: target.id,
        });
    }
}
