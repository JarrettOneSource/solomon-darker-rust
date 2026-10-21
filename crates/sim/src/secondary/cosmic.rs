//! Leviathan articulation, Planewalker shots and Ether Drain's pressure field.

use super::leviathan::*;
use super::program::*;
use super::spawns;
use super::step::StepEngine;
use super::{
    NativeSecondaryActorKind as Kind, NativeSecondaryActorState as Actor,
    NativeSecondaryDamageKind as Damage, NativeSecondaryEventKind as Event, NativeSecondaryWorld,
};
use crate::Vec2;
use crate::float;
use crate::float::{fround, hypot};
use crate::profile::offensive::resolve_native_skill_damage_value;

pub(super) fn step<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &Actor,
    actor: &mut Actor,
    retain: &mut bool,
) {
    match actor.kind {
        Kind::Leviathan => {
            actor.phase = f64::from(native_leviathan_phase(actor.age_ticks));
            actor.scale =
                fround(actor.slow_factor * native_leviathan_current_scale(actor.age_ticks));
            if actor.age_ticks == 1 {
                let mut event = actor_event(actor, engine.context.tick, Event::Pulse, None);
                event.screen_flash = Some(screen_flash(1.0, 0.5, 1.0, 0.05, true));
                engine.state.emit_event(event);
            }
        }
        Kind::LeviathanAppendage => appendage(engine, source, actor, retain),
        Kind::EtherBolt => {
            actor.quantity = source.quantity - 1.0;
            actor.alpha = if actor.quantity < 1.0 {
                fround(source.alpha - (0.01_f32 as f64))
            } else {
                source.alpha
            };
            if let Some(target) = engine.candidates(actor, actor.radius).first() {
                spawns::ether_fade(
                    engine.state,
                    actor,
                    actor.position,
                    spawns::EtherFadePresentation::Impact,
                    engine.context.tick,
                );
                engine.damage(actor, target, actor.damage, Damage::Magic);
                *retain = false;
            } else {
                *retain = actor.alpha > 0.0;
            }
        }
        Kind::LeviathanMote => {
            actor.alpha = fround(source.alpha - source.slow_factor);
            actor.velocity = Vec2::new(
                fround(source.velocity.x * (0.95_f32 as f64)),
                fround(source.velocity.y * (0.95_f32 as f64)),
            );
            *retain = actor.alpha > 0.0;
        }
        Kind::PlaneOrbShot => {
            let travel = source.phase + 1.0;
            actor.position = Vec2::new(
                fround(source.position.x + travel * source.velocity.x),
                fround(source.position.y + travel * source.velocity.y),
            );
            if actor.age_ticks < 1000 {
                actor.phase = fround(source.phase * (0.98_f32 as f64));
                actor.scale = source.slow_factor.min(fround(source.scale + (0.01_f32 as f64)));
                actor.quantity =
                    if source.quantity + 1.0 >= 6.0 { 0.0 } else { source.quantity + 1.0 };
                actor.radius = fround(2.0 * actor.scale);
                if engine.context.enhanced_effects_for(&actor.owner_id) {
                    spawns::plane_orb_mote(engine.state, actor);
                }
                if actor.quantity == 0.0 {
                    let position = Vec2::new(actor.position.x, fround(actor.position.y - 15.0));
                    for target in stable_targets(engine.world.targets(
                        &actor.world_key,
                        position,
                        actor.radius,
                    )) {
                        engine.damage(actor, &target, actor.damage * 5.0, Damage::Magic);
                    }
                }
            } else {
                actor.scale = fround(source.scale - (0.02_f32 as f64)).max(0.0);
                *retain = actor.scale > 0.0;
            }
        }
        Kind::PlaneOrbParticle => {
            let alpha = fround(source.alpha - source.slow_factor);
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
        Kind::EtherDrain => super::ether_drain::step_field(engine, source, actor, retain),
        Kind::EtherDrainCloud => {
            actor.phase = fround(source.phase + source.slow_factor);
            actor.position = Vec2::new(
                fround(source.position.x + source.velocity.x * source.quantity),
                fround(source.position.y + source.velocity.y * source.quantity),
            );
            *retain = actor.phase <= 180.0;
        }
        Kind::EtherDrainDebris => {
            let oscillation = engine.state.rng.draw_scaled_float(17.0, false);
            let speed_gate = engine.state.rng.draw_integer(100);
            let rotation = engine.state.rng.draw_scaled_float(5.0, false);
            let remaining = fround(source.quantity - source.slow_factor);
            let mut speed = fround(source.slow_factor + (0.05_f32 as f64));
            if speed_gate == 3 {
                speed = fround(speed * 0.5);
            }
            let phase = fround(source.phase + 3.0 + oscillation);
            let perpendicular =
                fround(float::sin(phase * std::f64::consts::PI / 180.0) * remaining / 7.0);
            actor.phase = phase;
            actor.position = Vec2::new(
                fround(
                    source.endpoint.x
                        + remaining * source.velocity.x
                        + perpendicular * source.velocity.y,
                ),
                fround(
                    source.endpoint.y + remaining * source.velocity.y
                        - perpendicular * source.velocity.x,
                ),
            );
            actor.quantity = remaining;
            actor.rotation_radians = normalize_radians(
                source.rotation_radians + (3.0 + rotation) * std::f64::consts::PI / 180.0,
            );
            actor.slow_factor = speed;
            if remaining <= 0.0 {
                if let Some(parent) = source.hit_target_ids.first() {
                    engine.drain_pulses.insert(*parent);
                }
                *retain = false;
            }
        }
        Kind::EtherDrainCaptureFlare => {
            actor.alpha = fround(source.alpha - (0.05_f32 as f64)).max(0.0);
            *retain = actor.alpha > 0.0;
        }
        _ => unreachable!("cosmic family dispatcher"),
    }
}

fn appendage<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &Actor,
    actor: &mut Actor,
    retain: &mut bool,
) {
    let Some(parent_id) = source.hit_target_ids.first().copied() else {
        *retain = false;
        return;
    };
    let Some(parent) =
        engine.sources.get(&parent_id).filter(|actor| actor.kind == Kind::Leviathan).cloned()
    else {
        *retain = false;
        return;
    };
    let age = parent.age_ticks + 1;
    let current_scale = native_leviathan_current_scale(age);
    let composite = fround(parent.slow_factor * current_scale);
    let mut spin = actor.velocity.x;
    let mut heading = actor.rotation_radians * 180.0 / std::f64::consts::PI;
    let mut deployment = actor.slow_factor;
    let mut recoil = actor.midpoint;
    let mut bank = actor.phase;
    let mut countdown = actor.quantity;
    let mut target_id = actor.target_id;
    let mut depth = actor.frame - 100.0;
    if native_leviathan_active(age) {
        spin = fround(spin + actor.velocity.y);
        recoil =
            Vec2::new(fround(recoil.x * (0.9_f32 as f64)), fround(recoil.y * (0.9_f32 as f64)));
        let wander = engine.state.rng.draw_scaled_float(5.8_f32 as f64, false);
        heading = fround(heading + (0.2_f32 as f64) + wander);
        if heading > 360.0 {
            heading = fround(heading - 360.0);
        }
        deployment = if 1640_i64 - age as i64 + 1 > 15 {
            fround(deployment * (0.95_f32 as f64))
        } else {
            fround(deployment + (0.07_f32 as f64))
        };
        depth = (native_leviathan_heading_vector(heading).y * 100.0 + 0.5).floor();
        if deployment < (0.05_f32 as f64) {
            let origin = Vec2::new(
                fround(parent.position.x + actor.endpoint.x),
                fround(parent.position.y + actor.endpoint.y),
            );
            if let Some(id) = target_id {
                if let Some(target) = engine.world.target(&actor.world_key, id) {
                    heading = native_leviathan_heading_degrees(origin, target.position);
                    countdown -= 1.0;
                    if countdown < 1.0 {
                        let root = native_leviathan_appendage_local_root(
                            actor.endpoint,
                            recoil,
                            spin,
                            deployment,
                        );
                        let muzzle = native_leviathan_muzzle_position(
                            parent.position,
                            root,
                            bank as u32,
                            heading,
                            actor.radius,
                        );
                        let direction = native_leviathan_heading_vector(heading);
                        spawns::ether_fade(
                            engine.state,
                            actor,
                            muzzle,
                            spawns::EtherFadePresentation::Muzzle,
                            engine.context.tick,
                        );
                        let mut shot = super::program::actor(
                            Kind::EtherBolt,
                            &actor.owner_id,
                            &actor.world_key,
                            Some(11),
                            actor.rank,
                            Vec2::new(muzzle.x, fround(muzzle.y + 25.0)),
                        );
                        shot.damage = fround(resolve_native_skill_damage_value(
                            11,
                            parent.damage,
                            &engine.context.players[&actor.owner_id].offensive_factors,
                        ));
                        shot.lifetime_ticks = NATIVE_ETHER_BOLT_LIFETIME_TICKS;
                        shot.quantity = 100.0;
                        shot.radius = 10.0;
                        shot.rotation_radians = heading * std::f64::consts::PI / 180.0;
                        shot.velocity =
                            Vec2::new(fround(direction.x * 10.0), fround(direction.y * 10.0));
                        engine.state.spawn_actor(shot);
                        countdown = 75.0 + f64::from(engine.state.rng.draw_integer(26));
                        if engine.state.rng.draw_integer(2) == 1 {
                            bank = if bank == 0.0 { 1.0 } else { 0.0 };
                        }
                        recoil = Vec2::new(fround(direction.x * 10.0), fround(direction.y * 10.0));
                    }
                } else {
                    target_id = None;
                }
            } else {
                let mut targets = stable_targets(engine.world.targets(
                    &actor.world_key,
                    origin,
                    NATIVE_LEVIATHAN_TARGET_RANGE,
                ));
                targets.retain(|target| {
                    target.native_flags.unwrap_or(2) & 2 != 0
                        && native_leviathan_inside_target_lane(origin, heading, target.position)
                        && !engine.world.line_obstruction(&actor.world_key, origin, target.position)
                });
                targets.sort_by(|a, b| {
                    hypot(a.position.x - origin.x, a.position.y - origin.y)
                        .total_cmp(&hypot(b.position.x - origin.x, b.position.y - origin.y))
                        .then(a.id.cmp(&b.id))
                });
                target_id = targets.first().map(|target| target.id);
            }
        }
    }
    let root = native_leviathan_appendage_local_root(actor.endpoint, recoil, spin, deployment);
    actor.frame = depth + 100.0;
    actor.midpoint = recoil;
    actor.phase = bank;
    actor.position = native_leviathan_appendage_presentation_root(parent.position, composite, root);
    actor.quantity = countdown;
    actor.rotation_radians = heading * std::f64::consts::PI / 180.0;
    actor.scale = composite;
    actor.slow_factor = deployment;
    actor.target_id = target_id;
    actor.velocity = Vec2::new(spin, actor.velocity.y);
    *retain = age < NATIVE_LEVIATHAN_LIFETIME_TICKS;
    if engine.last_appendages.get(&parent_id) == Some(&actor.id)
        && engine.context.enhanced_effects_for(&actor.owner_id)
    {
        spawns::leviathan_mote(engine.state, &parent, current_scale);
    }
}
