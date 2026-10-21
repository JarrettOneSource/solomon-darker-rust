//! Ordered EtherDrain actor epoch from `native-secondary-abilities.ts:3271..3445`.

use super::kernel::inside_broad_ellipse;
use super::*;
use crate::float::fround;
use crate::secondary::program::MAX_NATIVE_TICKS;
use crate::secondary::program::{actor as new_actor, actor_event, screen_flash};
use crate::secondary::step::StepEngine;
use crate::secondary::{
    NativeSecondaryActorKind as Kind, NativeSecondaryActorState as Actor,
    NativeSecondaryAudioCue as Cue, NativeSecondaryEventKind as Event,
    NativeSecondarySimulationState as State, NativeSecondaryWorld,
};

pub(in crate::secondary) fn step_field<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &Actor,
    actor: &mut Actor,
    retain: &mut bool,
) {
    let phase_at_entry = source.phase;
    let mut countdown = source.freeze_ticks;
    let mut refresh = source.quantity;
    let mut scale = source.scale;
    if phase_at_entry == 0.0 {
        refresh -= 5.0;
        scale = fround(scale + (0.025_f32 as f64));
        if scale >= 1.0 {
            scale = 1.0;
            actor.phase = 1.0;
        }
        actor.alpha = fround(source.alpha + (0.005_f32 as f64)).min(1.0);
    } else if phase_at_entry == 1.0 {
        countdown = countdown.saturating_sub(1);
        actor.alpha = fround(source.alpha + (0.01_f32 as f64)).min(1.0);
        if countdown < 1 {
            countdown = 100;
            actor.phase = 2.0;
            refresh = 100.0;
        }
    } else {
        countdown = countdown.saturating_sub(1);
        refresh -= 10.0;
        scale = fround(scale - (0.05_f32 as f64));
        actor.alpha = fround(source.alpha - (0.05_f32 as f64)).max(0.0);
        if scale <= 0.0 {
            *retain = false;
        }
    }
    actor.freeze_ticks = countdown;
    actor.rotation_radians = source.rotation_radians + scale * 2.0 * std::f64::consts::PI / 180.0;
    actor.scale = scale.max(0.0);
    actor.slow_factor = source.slow_factor;
    if actor.age_ticks == 1 {
        let mut event = actor_event(actor, engine.context.tick, Event::Pulse, None);
        event.screen_flash = Some(screen_flash(1.0, 0.5, 1.0, 0.05, true));
        engine.state.emit_event(event);
    }
    if phase_at_entry == 1.0
        && countdown > 50
        && engine.state.rng.draw_integer(if engine.context.enhanced_effects_for(&actor.owner_id) {
            3
        } else {
            5
        }) == 1
    {
        crate::secondary::spawns::drain_cloud(engine.state, actor);
    }
    refresh -= 1.0;
    let targets = match engine.world.ether_drain_targets(&actor.world_key) {
        Some(targets) => targets,
        None => fallback_targets(engine, source, actor, refresh < 1.0),
    };
    let (refreshed, mut field) = refresh_native_ether_drain_targets(
        source.ether_drain.as_ref(),
        &targets,
        actor.position,
        refresh < 1.0,
    );
    let world_animations = engine.world.ether_drain_world_animations(&actor.world_key);
    refresh_native_ether_drain_world_animations(
        &mut field,
        &world_animations,
        actor.position,
        source.ether_drain.as_ref().is_some_and(|field| field.targets_initialized),
    );
    let scenery = engine.world.scenery_targets(
        &actor.world_key,
        actor.position,
        NATIVE_ETHER_DRAIN_BROAD_RADIUS,
    );
    if refreshed {
        field.scenery = scenery
            .iter()
            .filter(|target| inside_broad_ellipse(actor.position, target.position, 0.8_f32 as f64))
            .map(|target| target.id as u64)
            .collect();
        refresh = 100.0;
    }
    actor.hit_target_ids.clear();
    actor.quantity = refresh;
    if actor.phase != 2.0 && countdown > 50 {
        let input = NativeEtherDrainField {
            alpha: actor.alpha,
            damage: actor.damage,
            id: actor.id,
            owner_id: &actor.owner_id,
            position: actor.position,
        };
        for reference in field.loose.iter().chain(&field.queried) {
            let target = targets.iter().rev().find(|target| &target.reference == reference);
            let Some(mut contact) =
                target.and_then(|target| native_ether_drain_contact(input, target))
            else {
                continue;
            };
            if contact.magic_contact {
                contact.hit_strength =
                    fround(0.5 + engine.state.rng.draw_scaled_float(0.5_f32 as f64, false));
            }
            engine.output.ether_drain_contacts.push(contact);
        }
        for reference in &field.world_animation_refs {
            let target = world_animations.iter().find(|target| target.reference == *reference);
            if let Some(contact) =
                target.and_then(|target| native_ether_drain_world_animation_contact(input, *target))
            {
                if contact.consume {
                    actor.slow_factor = 2.0;
                }
                engine.output.ether_drain_world_animation_contacts.push(contact);
            }
        }
        for scenery_id in &field.scenery {
            let Some(target) = scenery.iter().find(|target| target.id as u64 == *scenery_id) else {
                continue;
            };
            let key = (actor.world_key.clone(), target.id);
            let previous = engine.scenery_phases.get(&key).copied().unwrap_or(0.0);
            let sign = engine.state.rng.draw_sign(1.0);
            let step = engine.state.rng.draw_scaled_float(1.0, false);
            let direction = if previous < -1.0 {
                1.0
            } else if previous > 1.0 {
                -1.0
            } else {
                sign
            };
            let phase = fround(previous + direction * step);
            let existing = engine
                .sources
                .values()
                .chain(&engine.state.actors)
                .find(|candidate| {
                    candidate.kind == Kind::EarthquakeSceneryWobble
                        && candidate.world_key == actor.world_key
                        && candidate.target_id == Some(target.id)
                })
                .map(|carrier| carrier.id);
            let carrier_id = match existing {
                Some(id) => id,
                None => {
                    let mut carrier = new_actor(
                        Kind::EarthquakeSceneryWobble,
                        &actor.owner_id,
                        &actor.world_key,
                        Some(74),
                        1,
                        target.position,
                    );
                    carrier.lifetime_ticks = MAX_NATIVE_TICKS;
                    carrier.target_id = Some(target.id);
                    carrier.variant = target.type_id as i32;
                    carrier.phase = phase;
                    let id = engine.state.next_actor_id;
                    engine.state.spawn_actor(carrier);
                    id
                }
            };
            engine.wobble_phases.insert(carrier_id, phase);
            engine.scenery_phases.insert(key, phase);
            if target.type_id != 2001 || engine.state.rng.draw_integer(100) != 1 {
                continue;
            }
            let secondary_variant = target.secondary_variant.unwrap_or(0);
            let variant = match secondary_variant {
                0..=2 => 0,
                3..=5 => 1,
                7 => 2,
                _ => continue,
            };
            let oscillation = engine.state.rng.draw_scaled_float(360.0, false);
            let rotation = engine.state.rng.draw_scaled_float(360.0, false);
            let radius = engine.state.rng.draw_scaled_float(150.0, false);
            let direction = engine.state.rng.draw_unit_vector();
            let center = native_ether_drain_leaf_center(
                super::trees::TREE_POLYGONS[secondary_variant as usize],
            );
            let position = Vec2::new(
                fround(target.position.x + fround(center.x + direction.x * radius)),
                fround(target.position.y + fround(center.y + direction.y * radius)),
            );
            field.animations.push(create_native_ether_drain_debris(
                actor.position,
                position,
                oscillation,
                rotation,
                variant,
            ));
        }
        if engine.state.rng.draw_integer(50) == 1 {
            let oscillation = engine.state.rng.draw_scaled_float(360.0, false);
            let rotation = engine.state.rng.draw_scaled_float(360.0, false);
            let direction = engine.state.rng.draw_unit_vector();
            let variant = engine.state.rng.draw_integer(3) as u32;
            let position = Vec2::new(
                fround(actor.position.x + direction.x * 1024.0),
                fround(actor.position.y + direction.y * 1024.0),
            );
            field.animations.push(create_native_ether_drain_debris(
                actor.position,
                position,
                oscillation,
                rotation,
                variant,
            ));
        }
    }
    actor.slow_factor = fround(actor.slow_factor - (0.1_f32 as f64)).max(0.0);
    let stepped = step_native_ether_drain_animations(
        &field.animations,
        actor.position,
        &mut engine.state.rng,
    );
    field.animations = stepped.animations;
    for parameter in stepped.pulses {
        spawn_pulse(engine.state, actor, parameter, engine.context.tick);
        actor.slow_factor = 2.0;
    }
    actor.ether_drain = Some(field);
}

fn fallback_targets<W: NativeSecondaryWorld>(
    engine: &StepEngine<'_, '_, '_, W>,
    source: &Actor,
    actor: &Actor,
    refresh: bool,
) -> Vec<NativeEtherDrainTarget> {
    let targets = match source.ether_drain.as_ref().filter(|_| !refresh) {
        None => engine.candidates(actor, NATIVE_ETHER_DRAIN_BROAD_RADIUS),
        Some(field) => field
            .queried
            .iter()
            .filter_map(|reference| {
                let NativeEtherDrainTargetRef::Enemy { id, .. } = reference else { return None };
                engine.world.target(&actor.world_key, *id as usize)
            })
            .collect(),
    };
    targets
        .into_iter()
        .map(|target| NativeEtherDrainTarget {
            activation_delay_ticks: 0,
            force_factor: native_ether_drain_enemy_force_factor(&target.family),
            native_flags: target.native_flags.unwrap_or(2),
            position: target.position,
            reference: NativeEtherDrainTargetRef::Enemy {
                id: target.id as u64,
                registration_ordinal: target.light_registration.registration_ordinal,
            },
        })
        .collect()
}

fn spawn_pulse(state: &mut State, parent: &Actor, parameter: f64, tick: u64) {
    if parameter < 1.0 {
        return;
    }
    let mut flare = new_actor(
        Kind::EtherDrainCaptureFlare,
        &parent.owner_id,
        &parent.world_key,
        Some(74),
        parent.rank,
        Vec2::new(parent.position.x, fround(parent.position.y - parameter * 10.0)),
    );
    flare.alpha = parameter;
    flare.enhanced = parent.enhanced;
    flare.lifetime_ticks = (parameter / (0.05_f32 as f64)).ceil() as u64 + 1;
    flare.scale = parameter;
    state.spawn_actor(flare);
    let mut event = actor_event(parent, tick, Event::Pulse, Some(Cue::Phase));
    event.pitch = 1.5;
    state.emit_event(event);
}

impl State {
    pub fn pulse_ether_drain(
        &mut self,
        actor_id: u64,
        parameter: f64,
        tick: u64,
        after_parent_tick: bool,
    ) {
        let Some(parent) = self
            .actors
            .iter_mut()
            .find(|actor| actor.id == actor_id && actor.kind == Kind::EtherDrain)
        else {
            return;
        };
        parent.slow_factor = if after_parent_tick { 2.0 } else { fround(2.0 - (0.1_f32 as f64)) };
        let parent = parent.clone();
        spawn_pulse(self, &parent, parameter, tick);
    }

    pub fn capture_ether_drain_image(
        &mut self,
        field_id: u64,
        image: Option<NativeEtherDrainCapturedImage>,
        tick: u64,
    ) {
        let Some(parent) = self
            .actors
            .iter()
            .find(|actor| actor.id == field_id && actor.kind == Kind::EtherDrain)
            .cloned()
        else {
            return;
        };
        if let Some(field) = &parent.ether_drain {
            for animation in &field.animations {
                self.pulse_ether_drain(
                    field_id,
                    if matches!(animation, NativeEtherDrainAnimation::Captured { .. }) {
                        1.5
                    } else {
                        0.5
                    },
                    tick,
                    true,
                );
            }
        }
        let field = self
            .actors
            .iter_mut()
            .find(|actor| actor.id == field_id)
            .unwrap()
            .ether_drain
            .get_or_insert_default();
        field.animations.clear();
        if let Some(image) = image {
            field.animations.push(NativeEtherDrainAnimation::Captured {
                alpha: fround(1.25 - (0.2_f32 as f64)),
                atlas: image.atlas,
                entry: image.entry,
                tint: image.tint,
                body_y_offset: image.body_y_offset,
            });
            let pitch = self.rng.draw_float_range(0.9_f32 as f64, 1.0);
            let mut event = actor_event(&parent, tick, Event::Pulse, Some(Cue::CrunchDrain));
            event.pitch = pitch;
            self.emit_event(event);
        }
    }
}
