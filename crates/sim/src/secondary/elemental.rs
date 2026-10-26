//! Elemental fields, waves, burns, weather and Comet. Contacts are emitted in
//! native actor order; target effects are applied before later actors query them.

use crate::float;
use crate::secondary::{SecondaryActorRead, SecondaryActorWrite};
use std::collections::BTreeSet;

use super::program::*;
use super::step::{BurnRequest, StepEngine};
use super::{
    NativeSecondaryActorKind as Kind, NativeSecondaryAudioCue as Cue, NativeSecondaryDamageContact,
    NativeSecondaryDamageKind as Damage, NativeSecondaryEventKind as Event,
    NativeSecondaryKnockbackContact, NativeSecondaryTarget,
    NativeSecondaryTargetEffectPatch as Patch, NativeSecondaryTargetEffectState,
    NativeSecondaryWorld,
};
use crate::Vec2;
use crate::float::{fround, hypot};
use crate::profile::primary::NativeSpentEmber;
use crate::spells::air_water::draw_native_spell_damage;
use crate::spells::fire::{
    NativeFireProjectilePayload, create_native_fire_detonation, step_native_fire_ember,
};
use crate::spells::model::{PrimarySpellFireEmber, PrimarySpellTransientBase};
use crate::spells::special_death::{NativeSpellSpecialDeathKind, NativeSpellSpecialDeathPayload};

pub(super) fn step_status_pulses<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    effects: &[NativeSecondaryTargetEffectState],
) {
    // Read the previous effects, including their final tick. The stored effects
    // have already advanced, which matters for simultaneous Prismatic expiry.
    for effect in effects {
        if let Some(burn) = &effect.electric_burn
            && let Some(origin) = engine.world.target(&effect.world_key, effect.target_id)
        {
            let mut arcs = engine.world.targets(&effect.world_key, origin.position, 200.0);
            arcs.retain(|target| target.id != origin.id);
            arcs.sort_by(|a, b| {
                squared_distance(a.position, origin.position)
                    .total_cmp(&squared_distance(b.position, origin.position))
            });
            arcs.truncate(burn.arc_count);
            let origin_position = origin.position;
            arcs.insert(0, origin);
            for (index, target) in arcs.into_iter().enumerate() {
                let source = index == 0;
                let mut hit_strength = 0.0;
                if !source || engine.state.rng.draw_integer(3) == 1 {
                    hit_strength = fround(0.25 + engine.state.rng.draw_scaled_float(0.5, false));
                }
                let prism = engine
                    .effect(&effect.world_key, target.id)
                    .is_some_and(|effect| effect.prismatic_ticks > 0);
                if burn.arc_count > 0 {
                    let enhanced = engine.context.enhanced_effects_for(&burn.owner_id);
                    if !source {
                        super::electric_burn::spawn_arc(
                            engine.state,
                            &burn.owner_id,
                            &effect.world_key,
                            origin_position,
                            &target,
                            enhanced,
                            engine.context.tick,
                        );
                    }
                    super::electric_burn::spawn_flare(
                        engine.state,
                        &burn.owner_id,
                        &effect.world_key,
                        &target,
                        if source { 0.5 } else { 0.75 },
                        enhanced,
                    );
                }
                engine.output.damage.push(NativeSecondaryDamageContact {
                    amount: burn.damage_per_tick * if prism { 2.0 } else { 1.0 },
                    ether_drain: false,
                    hit_strength: Some(hit_strength),
                    kind: Damage::Lightning,
                    owner_id: burn.owner_id.clone(),
                    source_actor_id: burn.source_actor_id,
                    special_death_payload: None,
                    suppress_hit_reaction: true,
                    suppress_hurt_sound: false,
                    target_id: target.id,
                });
                if burn.stun_factor < 1.0 {
                    super::apply_native_secondary_target_effect(
                        &mut engine.state.target_effects,
                        &effect.world_key,
                        target.id,
                        Patch {
                            stun_factor: Some(burn.stun_factor),
                            stun_ticks: Some(25),
                            ..Patch::default()
                        },
                    );
                }
            }
        }
        if let Some(burn) = &effect.steamed
            && let Some(target) = engine.world.target(&effect.world_key, effect.target_id)
        {
            let hit_strength = fround(0.25 + engine.state.rng.draw_scaled_float(0.5, false));
            // Mod_Steamed samples before submitting every hit, including survivors.
            let special_death_payload =
                (burn.explode_radius > 0.0).then(|| NativeSpellSpecialDeathPayload {
                    kind: NativeSpellSpecialDeathKind::Steam,
                    ember_damage: burn.ember_damage,
                    ember_fragments: burn.ember_fragments as u32,
                    explode_damage: burn.explode_damage,
                    explode_radius: burn.explode_radius,
                    private_seed: engine.state.rng.draw_integer(1_000_000) as u32,
                });
            engine.output.damage.push(NativeSecondaryDamageContact {
                amount: burn.damage_per_tick,
                ether_drain: false,
                hit_strength: Some(hit_strength),
                kind: Damage::Fire,
                owner_id: burn.owner_id.clone(),
                source_actor_id: burn.source_actor_id,
                special_death_payload,
                suppress_hit_reaction: true,
                suppress_hurt_sound: false,
                target_id: target.id,
            });
        }
        let (Some(owner), Some(skill), Some(source_id)) = (
            &effect.frost_burn_owner_id,
            effect.frost_burn_skill_id,
            effect.frost_burn_source_actor_id,
        ) else {
            continue;
        };
        if effect.frost_burn_ticks == 0 {
            continue;
        }
        let Some(target) = engine.world.target(&effect.world_key, effect.target_id) else {
            continue;
        };
        engine.output.damage.push(NativeSecondaryDamageContact {
            amount: effect.frost_burn_damage_per_tick,
            ether_drain: false,
            hit_strength: Some(0.0),
            kind: Damage::Ice,
            owner_id: owner.clone(),
            source_actor_id: source_id,
            special_death_payload: None,
            suppress_hit_reaction: true,
            suppress_hurt_sound: true,
            target_id: target.id,
        });
        if engine.state.rng.draw_integer(2) != 1 {
            continue;
        }
        let record = engine.state.rng.draw_integer(2);
        let rotation = engine.state.rng.draw_scaled_float(360.0, false);
        let scale = engine.state.rng.draw_scaled_float(0.5, false);
        let radius = engine.state.rng.draw_scaled_float(10.0, false);
        let offset = native_unit_vector(&mut engine.state.rng);
        let vertical = engine.state.rng.draw_scaled_float(35.0, false);
        let speed = engine.state.rng.draw_scaled_float(1.0, false);
        let velocity = native_unit_vector(&mut engine.state.rng);
        let alpha = engine.state.rng.draw_scaled_float(0.5, false);
        let mut child = actor(
            Kind::FrostBurnFlare,
            owner,
            &effect.world_key,
            Some(skill),
            1,
            Vec2::new(
                fround(target.position.x + offset.x * radius),
                fround(target.position.y - vertical + offset.y * radius),
            ),
        );
        child.alpha = fround(1.0 - alpha);
        child.frame = f64::from(10 + record);
        child.lifetime_ticks = 20;
        child.quantity = 0x408080 as f64;
        child.rotation_radians = rotation * std::f64::consts::PI / 180.0;
        child.scale = fround(0.5 + scale);
        child.target_id = Some(target.id);
        child.velocity =
            Vec2::new(fround(velocity.x * (0.5 + speed)), fround(velocity.y * (0.5 + speed)));
        engine.state.spawn_actor(child);
    }
}

pub(super) fn step<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &impl SecondaryActorRead,
    actor: &mut impl SecondaryActorWrite,
    retain: &mut bool,
) {
    match actor.kind {
        Kind::MovingFire | Kind::FirePatch => {
            actor.slow_factor = fround(source.slow_factor - FIRE_LIFE_STEP);
            actor.phase = fround(
                source.phase + if actor.kind == Kind::MovingFire { 0.12_f32 as f64 } else { 0.25 },
            );
            if actor.phase >= 32.0 {
                actor.phase -= 32.0;
            }
            actor.frame = 46.0 + actor.phase.round_ties_even();
            actor.radius = fround(source.radius + (0.05_f32 as f64)).min(1.0);
            if actor.kind == Kind::MovingFire {
                actor.velocity = Vec2::new(
                    fround(source.velocity.x * (1.01_f32 as f64)),
                    fround(source.velocity.y * (1.01_f32 as f64)),
                );
            }
            *retain = actor.slow_factor > 0.0;
            if actor.enhanced && actor.damage > 0.0 && engine.context.tick.is_multiple_of(3) {
                let radius = fround(32.0 * actor.scale);
                for target in engine.candidates(actor, radius) {
                    if hypot(
                        actor.position.x - target.position.x,
                        actor.position.y - target.position.y,
                    ) >= radius
                    {
                        continue;
                    }
                    let response = engine.state.rng.draw_scaled_float(0.5, false);
                    let burn = engine.context.players[&actor.owner_id].fire_burn_damage;
                    engine.fire_burns.push(BurnRequest {
                        actor: actor.snapshot(),
                        damage: burn,
                        target: target.clone(),
                    });
                    let lane = fround(fround(fround(actor.damage / 100.0) * 3.0) * 0.5);
                    let contact_index = engine.output.damage.len();
                    engine.damage(actor, &target, lane + lane, Damage::Fire);
                    if let Some(contact) = engine.output.damage.get_mut(contact_index) {
                        contact.hit_strength = Some(fround(0.25 + response));
                    }
                }
            }
        }
        Kind::Shockwave | Kind::MindblastShockwave | Kind::RescueShockwave => {
            wave(engine, source, actor, retain)
        }
        Kind::FreezeWave => {
            if !advance_wave(engine, source, actor, 6.0, 0.12375_f32 as f64) {
                *retain = false;
                return;
            }
            if actor.age_ticks.is_multiple_of(10) {
                let mut hit = ordered_hit_target_ids(&source.hit_target_ids);
                for target in engine.candidates(actor, actor.radius) {
                    if hit.contains(&(target.id as u64)) {
                        continue;
                    }
                    hit.push(target.id as u64);
                    let owner = &engine.context.players[&actor.owner_id];
                    let patch = if target.native_flags.unwrap_or(0) & 0x40 != 0 {
                        Patch {
                            cold_slow_factor: Some(owner.cold_slow_factor),
                            cold_slow_material: Some(true),
                            cold_slow_ticks: Some(actor.freeze_ticks),
                            ..Patch::default()
                        }
                    } else {
                        Patch {
                            frozen_ticks: Some(actor.freeze_ticks),
                            frozen_time_scale: Some(0.0),
                            ..Patch::default()
                        }
                    };
                    engine.merge(actor, target.id, patch);
                    if actor.variant == 1 {
                        engine.merge(
                            actor,
                            target.id,
                            Patch {
                                frost_burn_damage_per_tick: Some(0.01_f32 as f64),
                                frost_burn_owner_id: Some(actor.owner_id.clone()),
                                frost_burn_skill_id: Some(if actor.skill_id == Some(76) {
                                    76
                                } else {
                                    35
                                }),
                                frost_burn_source_actor_id: Some(actor.id),
                                frost_burn_ticks: Some(actor.freeze_ticks * 100),
                                ..Patch::default()
                            },
                        );
                    }
                }
                actor.hit_target_ids = hit;
            }
        }
        Kind::FrostBurnFlare => {
            let alpha = fround(source.alpha - (0.05_f32 as f64));
            actor.alpha = alpha.max(0.0);
            actor.position = Vec2::new(
                fround(source.position.x + source.velocity.x),
                fround(source.position.y + source.velocity.y),
            );
            actor.velocity = Vec2::new(
                fround(source.velocity.x * (0.96_f32 as f64)),
                fround(source.velocity.y * (0.96_f32 as f64)),
            );
            *retain = alpha > 0.0;
        }
        Kind::RingFireFragment => {
            step_fragment(source, actor);
            if actor.variant == 0
                && let Some(target) = engine
                    .candidates(actor, actor.radius)
                    .into_iter()
                    .find(|target| !actor.hit_target_ids.contains(&(target.id as u64)))
            {
                engine.damage(actor, &target, actor.damage, Damage::Fire);
                engine.fire_burns.push(BurnRequest {
                    actor: actor.snapshot(),
                    damage: engine.context.players[&actor.owner_id].fire_burn_damage,
                    target,
                });
                *retain = false;
                return;
            }
            *retain = actor.alpha > 0.0;
        }
        Kind::FireBurn | Kind::EtherBurn => burn(engine, actor, retain),
        Kind::FireBurnFlame | Kind::EtherBurnFlare => {
            actor.alpha = fround(source.alpha - (0.01_f32 as f64)).max(0.0);
            actor.frame = source.frame;
            *retain = actor.alpha > 0.0;
        }
        Kind::StormCloud => storm(engine, source, actor, retain),
        Kind::AcidRain => acid(engine, source, actor, retain),
        Kind::StormDrop | Kind::AcidDrop => {
            let height = (source.phase + 20.0).min(0.0);
            let factor =
                if actor.kind == Kind::StormDrop { 1.1_f32 as f64 } else { 1.100000023841858 };
            let scale = if height >= 0.0 { fround(source.scale * factor) } else { source.scale };
            if height >= 0.0 && scale >= 1.0 {
                *retain = false;
                return;
            }
            actor.phase = height;
            actor.quantity = fround(source.quantity + 4.0);
            actor.scale = scale;
        }
        Kind::AcidSplash => {
            let life = fround(source.alpha - (0.0125_f32 as f64));
            if life <= 0.0 {
                *retain = false;
                return;
            }
            actor.alpha = life;
            actor.velocity = Vec2::new(
                fround(source.velocity.x * (0.95_f32 as f64)),
                fround(source.velocity.y * (0.95_f32 as f64)),
            );
        }
        Kind::Comet => comet(engine, source, actor, retain),
        Kind::CometTrail => {
            let life = fround(source.phase - (0.025_f32 as f64));
            if life <= 0.0 {
                *retain = false;
                return;
            }
            actor.phase = life;
            actor.rotation_radians = fround(source.rotation_radians * source.quantity);
        }
        Kind::CometDebris | Kind::StoneskinChip => {
            *retain = super::motion::step_secondary_bouncer(
                source,
                actor,
                engine.context.tick,
                &mut engine.state.rng,
            );
        }
        _ => unreachable!("elemental family dispatcher"),
    }
}

fn burn<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    actor: &mut impl SecondaryActorWrite,
    retain: &mut bool,
) {
    let Some(target) = actor.target_id.and_then(|id| engine.world.target(&actor.world_key, id))
    else {
        *retain = false;
        return;
    };
    let remaining = actor.lifetime_ticks - actor.age_ticks + 1;
    let fade = if remaining < 50 { fround(remaining as f64 / 50.0) } else { 1.0 };
    let fire = actor.kind == Kind::FireBurn;
    if fire {
        engine.damage(actor, &target, actor.damage, Damage::Fire);
    }
    let scale = engine.state.rng.draw_scaled_float(0.25, !fire);
    let radius = engine.state.rng.draw_scaled_float(if fire { 0.1 } else { 0.1_f32 as f64 }, false);
    let mut child = super::program::actor(
        if fire { Kind::FireBurnFlame } else { Kind::EtherBurnFlare },
        &actor.owner_id,
        &actor.world_key,
        if fire { actor.skill_id } else { Some(14) },
        actor.rank,
        Vec2::new(target.position.x, fround(target.position.y - 15.0)),
    );
    child.alpha = fround(fade * 0.125);
    child.frame = if fire {
        333.0 + (engine.context.tick / 3 % 10) as f64
    } else {
        246.0 + (engine.context.tick / 6 % 5) as f64
    };
    child.lifetime_ticks = 13;
    child.scale = fround((1.0 + scale) * target.scale);
    child.target_id = Some(target.id);
    engine.state.spawn_actor(child);
    actor.alpha = fade;
    actor.light_registration = Some(target.light_registration);
    actor.position = target.position;
    actor.radius = fround(0.1 + radius);
    actor.scale = target.scale;
    *retain = actor.age_ticks < actor.lifetime_ticks;
}

/// JavaScript Set preserves first insertion order. The retained hit list is part
/// of the wave's future behavior and cannot be sorted by numeric target ID.
fn ordered_hit_target_ids(ids: &[u64]) -> Vec<u64> {
    let mut hit = Vec::with_capacity(ids.len());
    for id in ids {
        if !hit.contains(id) {
            hit.push(*id);
        }
    }
    hit
}

fn wave<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &impl SecondaryActorRead,
    actor: &mut impl SecondaryActorWrite,
    retain: &mut bool,
) {
    assert!(source.quantity > 0.0, "native shockwave retains radius growth");
    if !advance_wave(engine, source, actor, source.quantity, source.slow_factor) {
        *retain = false;
        return;
    }
    actor.scale = fround(1.0 + actor.age_ticks as f64 * 0.08);
    if actor.age_ticks.is_multiple_of(10) {
        let mut hit = ordered_hit_target_ids(&actor.hit_target_ids);
        for target in engine.candidates(actor, actor.radius) {
            if hit.contains(&(target.id as u64)) {
                continue;
            }
            hit.push(target.id as u64);
            if matches!(actor.kind, Kind::Shockwave | Kind::RescueShockwave) {
                engine.damage(actor, &target, actor.damage, Damage::Fire);
            }
            if actor.kind == Kind::Shockwave {
                engine.fire_burns.push(BurnRequest {
                    actor: actor.snapshot(),
                    damage: engine.context.players[&actor.owner_id].fire_burn_damage,
                    target: target.clone(),
                });
            }
            engine.merge(actor, target.id, Patch { dazzle_ticks: Some(400), ..Patch::default() });
            if actor.kind == Kind::Shockwave && actor.skill_id == Some(21) && actor.variant == 1 {
                maximum_explosion(engine, actor, &target);
            }
        }
        actor.hit_target_ids = hit;
    }
    if actor.age_ticks.is_multiple_of(2) {
        for target in engine.candidates(actor, actor.radius) {
            if !actor.hit_target_ids.contains(&(target.id as u64)) {
                continue;
            }
            let dx = fround(target.position.x - actor.position.x);
            let dy = fround(target.position.y - actor.position.y);
            let distance = hypot(dx, dy);
            let delta = if distance == 0.0 {
                Vec2::ZERO
            } else {
                Vec2::new(
                    fround(dx / distance * actor.alpha * source.quantity),
                    fround(dy / distance * actor.alpha * source.quantity),
                )
            };
            engine.output.knockbacks.push(NativeSecondaryKnockbackContact {
                delta,
                source_actor_id: actor.id,
                target_id: target.id,
            });
        }
    }
}

fn advance_wave<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &impl SecondaryActorRead,
    actor: &mut impl SecondaryActorWrite,
    growth: f64,
    fade_threshold: f64,
) -> bool {
    let wave = super::motion::step_native_expanding_wave(
        super::motion::NativeExpandingWave {
            radius: source.radius,
            growth,
            scalar: source.alpha,
            life: source.phase,
            fade_threshold,
        },
        &mut engine.state.rng,
    );
    engine.state.record_camera_displacement(
        &actor.world_key,
        wave.displacement,
        engine.context.tick,
    );
    actor.alpha = wave.scalar;
    actor.phase = wave.life;
    actor.radius = wave.radius;
    actor.velocity = Vec2::ZERO;
    wave.retain
}

fn maximum_explosion<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    wave: &impl SecondaryActorRead,
    target: &NativeSecondaryTarget,
) {
    let seed = engine.state.rng.draw_integer(1_000_001);
    let burn = engine.context.players[&wave.owner_id].fire_burn_damage;
    let result = create_native_fire_detonation(
        engine.state.next_actor_id + 1,
        NativeFireProjectilePayload {
            burn_damage: burn,
            ember_damage: fround(wave.damage / 3.0),
            ember_fragments: 3,
            explode_damage: wave.damage,
            explode_radius: 10.0 + (1.5 - 1.0) / 0.18,
            private_seed: seed,
            spent_ember: NativeSpentEmber::None,
        },
        target.position,
        &wave.owner_id,
        &wave.world_key,
        &mut engine.state.rng,
    );
    let mut explosion = actor(
        Kind::RingFireExplosion,
        &wave.owner_id,
        &wave.world_key,
        Some(21),
        wave.rank,
        target.position,
    );
    explosion.damage = fround(wave.damage * 0.5);
    explosion.lifetime_ticks = 37;
    explosion.radius = 165.0;
    explosion.scale = 1.5;
    explosion.target_id = Some(target.id);
    explosion.id = engine.state.spawn_actor(explosion.clone());
    let mut event =
        actor_event(&explosion, engine.context.tick, Event::Pulse, Some(Cue::FireballHit));
    event.pitch = result.sound_pitch;
    engine.state.emit_event(event);
    let mut event =
        actor_event(&explosion, engine.context.tick, Event::Pulse, Some(Cue::ThrowFire));
    event.pitch = 0.8_f32 as f64;
    engine.state.emit_event(event);
    for ember in result.embers {
        let mut child = actor(
            Kind::RingFireFragment,
            &wave.owner_id,
            &wave.world_key,
            Some(21),
            wave.rank,
            ember.position,
        );
        child.age_ticks = ember.common.age_ticks;
        child.alpha = ember.life;
        child.damage = ember.damage;
        child.frame = ember.phase;
        child.lifetime_ticks = 400;
        child.phase = ember.height;
        child.quantity = if ember.vertical_velocity == 0.0 { 1.0 } else { 0.0 };
        child.radius = 7.0;
        child.slow_factor = ember.vertical_velocity;
        child.variant = ember.contact_cadence as i32;
        child.velocity = ember.horizontal_velocity;
        engine.state.spawn_actor(child);
    }
    for target in stable_targets(engine.world.targets(&wave.world_key, target.position, 165.0)) {
        engine.damage(&explosion, &target, explosion.damage, Damage::Fire);
    }
    let mut consumed = BTreeSet::new();
    for contact in result.contacts {
        if consumed.contains(&contact.spell_id) {
            continue;
        }
        let Some(mut fragment) = engine
            .state
            .deferred_births
            .as_ref()
            .expect("actor phase owns birth commands")
            .iter()
            .find(|actor| actor.id == contact.spell_id && actor.kind == Kind::RingFireFragment)
            .map(|actor| actor.snapshot())
        else {
            continue;
        };
        let Some(target) = stable_targets(engine.world.targets(
            &contact.world_key,
            contact.position,
            contact.radius,
        ))
        .into_iter()
        .next() else {
            continue;
        };
        fragment.position = contact.position;
        engine.damage(&fragment, &target, contact.amount, Damage::Fire);
        engine.fire_burns.push(BurnRequest {
            actor: fragment,
            damage: contact.burn_damage,
            target,
        });
        consumed.insert(contact.spell_id);
    }
    engine
        .state
        .deferred_births
        .as_mut()
        .expect("actor phase owns birth commands")
        .retain(|actor| !consumed.contains(&actor.id));
}

fn step_fragment(source: &impl SecondaryActorRead, actor: &mut impl SecondaryActorWrite) {
    let mut ember = PrimarySpellFireEmber {
        common: PrimarySpellTransientBase {
            age_ticks: source.age_ticks,
            id: 1,
            light_registration: None,
            owner_id: source.owner_id.clone(),
            painter_registrations: Vec::new(),
            world_key: source.world_key.clone(),
        },
        burn_damage: 0.0,
        contact_cadence: source.variant as u64,
        contact_due: source.variant == 0,
        damage: source.damage,
        height: source.phase,
        horizontal_velocity: source.velocity,
        life: source.alpha,
        phase: source.frame,
        position: source.position,
        spent_ember: NativeSpentEmber::None,
        vertical_velocity: source.slow_factor,
    };
    if !step_native_fire_ember(&mut ember).retained {
        **actor = (**source).clone();
        actor.alpha = 0.0;
        return;
    };
    **actor = (**source).clone();
    actor.age_ticks = ember.common.age_ticks;
    actor.alpha = ember.life;
    actor.frame = ember.phase;
    actor.phase = ember.height;
    actor.position = ember.position;
    actor.quantity = if ember.vertical_velocity == 0.0 { 1.0 } else { 0.0 };
    actor.slow_factor = ember.vertical_velocity;
    actor.variant = ember.contact_cadence as i32;
    actor.velocity = ember.horizontal_velocity;
}

fn storm<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &impl SecondaryActorRead,
    actor: &mut impl SecondaryActorWrite,
    retain: &mut bool,
) {
    let mut flash = source.frame;
    if actor.age_ticks > actor.freeze_ticks {
        actor.alpha = fround(source.alpha - 0.01);
        *retain = actor.alpha > 0.0;
    } else {
        let drops = (if engine.context.enhanced_effects_for(&actor.owner_id) { 5 } else { 2 })
            / if actor.variant == 1 { 2 } else { 1 };
        for index in 0..drops {
            weather_drop(engine, source, index, false);
        }
        if source.variant == 1 {
            let turn = engine.state.rng.draw_scaled_float(2.0, false);
            actor.rotation_radians =
                normalize_radians(source.rotation_radians + turn * std::f64::consts::PI / 180.0);
            actor.position = Vec2::new(
                fround(source.position.x + float::cos(actor.rotation_radians) * (0.35_f32 as f64)),
                fround(source.position.y + float::sin(actor.rotation_radians) * (0.35_f32 as f64)),
            );
        }
        actor.alpha = fround(source.alpha + 0.05).min(1.0);
        actor.quantity = source.quantity - 1.0;
        actor.scale = fround(source.scale * 1.2).min(1.0);
        if actor.quantity <= 0.0 {
            let reset = engine.state.rng.draw_integer(91);
            actor.quantity = ((30.0 + f64::from(reset)) / actor.slow_factor).trunc().max(1.0);
            let targets = engine.candidates(actor, 500.0);
            if !targets.is_empty() {
                let index = engine.state.rng.draw_integer(targets.len() as i32) as usize;
                let target = &targets[index];
                let source_distance = engine.state.rng.draw_scaled_float(100.0, false);
                let source_direction = native_unit_vector(&mut engine.state.rng);
                let middle_distance = engine.state.rng.draw_scaled_float(200.0, false);
                let middle_direction = native_unit_vector(&mut engine.state.rng);
                let damage =
                    draw_native_spell_damage(&mut engine.state.rng, actor.damage, actor.phase);
                let origin = Vec2::new(
                    fround(actor.position.x + source_direction.x * source_distance),
                    fround(actor.position.y - 175.0 + source_direction.y * source_distance),
                );
                let middle = Vec2::new(
                    fround(actor.position.x + middle_direction.x * middle_distance),
                    fround(actor.position.y - 90.0 + middle_direction.y * middle_distance),
                );
                let endpoint =
                    Vec2::new(fround(target.position.x), fround(target.position.y - 15.0));
                flash = 1.0;
                engine.damage(actor, target, damage, Damage::Lightning);
                let mut strike = super::program::actor(
                    Kind::StormStrike,
                    &actor.owner_id,
                    &actor.world_key,
                    Some(27),
                    1,
                    origin,
                );
                strike.enhanced = engine.context.enhanced_effects_for(&actor.owner_id);
                strike.endpoint = endpoint;
                strike.lifetime_ticks = 1;
                strike.midpoint = middle;
                strike.phase = engine.context.tick as f64;
                strike.target_id = Some(target.id);
                strike.id = engine.state.spawn_actor(strike.clone());
                for cue in [Cue::LightningStart, Cue::Thunder] {
                    let mut event =
                        actor_event(&strike, engine.context.tick, Event::Impact, Some(cue));
                    event.position = endpoint;
                    engine.state.emit_event(event);
                }
            }
        }
    }
    actor.frame = fround(flash - (0.1_f32 as f64)).max(0.0);
    if engine.state.rng.draw_integer(1000) == 3 {
        engine.state.rng.draw_scaled_float(0.35, false);
        actor.frame = 1.0;
        engine.event(actor, Some(Cue::Thunder), Event::Pulse);
    }
}

fn weather_drop<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    parent: &impl SecondaryActorRead,
    index: i32,
    acid: bool,
) {
    let distance = engine.state.rng.draw_scaled_float(200.0, false);
    let direction = native_unit_vector(&mut engine.state.rng);
    let mut child = actor(
        if acid { Kind::AcidDrop } else { Kind::StormDrop },
        &parent.owner_id,
        &parent.world_key,
        Some(if acid { 72 } else { 27 }),
        1,
        Vec2::new(
            fround(parent.position.x + direction.x * distance),
            fround(parent.position.y + direction.y * distance * 0.8),
        ),
    );
    child.lifetime_ticks = 64;
    child.phase = -175.0;
    child.scale = 0.1_f32 as f64;
    child.variant = index;
    engine.state.spawn_actor(child);
}

fn acid<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &impl SecondaryActorRead,
    actor: &mut impl SecondaryActorWrite,
    retain: &mut bool,
) {
    let residue = if source.scale >= 1.0 && source.age_ticks < 1500 {
        (source.alpha + 0.005).min(1.0)
    } else {
        source.alpha
    };
    if actor.age_ticks > 1500 {
        let cloud = (source.phase - 0.01).max(0.0);
        actor.alpha = if cloud > 0.0 { residue } else { (residue - 0.0005).max(0.0) };
        actor.phase = cloud;
        *retain = cloud > 0.0 || actor.alpha > 0.0;
        return;
    }
    for index in 0..if engine.context.enhanced_effects_for(&actor.owner_id) { 5 } else { 2 } {
        weather_drop(engine, actor, index, true);
    }
    if engine.state.rng.draw_integer(4) == 3 {
        engine.state.rng.draw_scaled_float(360.0, false);
        let rotation = engine.state.rng.draw_scaled_float(360.0, false);
        let scale = engine.state.rng.draw_scaled_float(0.75, false);
        let distance = engine.state.rng.draw_scaled_float(200.0, false);
        let direction = native_unit_vector(&mut engine.state.rng);
        let mut child = super::program::actor(
            Kind::AcidSplash,
            &actor.owner_id,
            &actor.world_key,
            Some(72),
            1,
            Vec2::new(
                fround(actor.position.x + direction.x * distance),
                fround(actor.position.y + direction.y * distance),
            ),
        );
        child.alpha = 0.25;
        child.lifetime_ticks = 21;
        child.rotation_radians = rotation * std::f64::consts::PI / 180.0;
        child.scale = fround(fround(0.75 + scale) * 0.5);
        child.velocity = Vec2::new(0.0, -1.5);
        engine.state.spawn_actor(child);
    }
    let scale = fround(source.scale * 1.2).min(1.0);
    let mut countdown = source.quantity;
    if scale >= 1.0 {
        countdown -= 1.0;
    }
    if scale >= 1.0 && countdown <= 0.0 {
        let mut targets = engine.candidates(actor, 200.0);
        targets.retain(|target| squared_distance(actor.position, target.position) < 200.0 * 200.0);
        shuffle_fixed_bound(&mut targets, &mut engine.state.rng);
        let count = if targets.is_empty() { 0 } else { targets.len() / 3 + 1 };
        for target in targets.into_iter().take(count) {
            engine.damage(actor, &target, actor.damage, Damage::Acid);
        }
        if count > 0 && engine.state.rng.draw_integer(2) == 1 {
            engine.state.rng.draw_scaled_float(0.5, false);
            let pitch = engine.state.rng.draw_scaled_float(0.45_f32 as f64, false);
            let mut event =
                actor_event(actor, engine.context.tick, Event::Pulse, Some(Cue::AcidSizzle));
            event.pitch = fround(0.8 + pitch);
            engine.state.emit_event(event);
        }
        countdown = 25.0;
    }
    actor.alpha = residue;
    actor.phase = fround(source.phase + 0.05).min(1.0);
    actor.quantity = countdown;
    actor.scale = scale;
}

fn comet<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    source: &impl SecondaryActorRead,
    actor: &mut impl SecondaryActorWrite,
    retain: &mut bool,
) {
    let brightness = engine.state.rng.draw_scaled_float(0.5, false);
    let rotation = engine.state.rng.draw_scaled_float(360.0, false);
    let direction = engine.state.rng.draw_integer(2);
    let scale = engine.state.rng.draw_scaled_float(0.5, false);
    let mut trail = super::program::actor(
        Kind::CometTrail,
        &actor.owner_id,
        &actor.world_key,
        Some(76),
        1,
        actor.position,
    );
    // Source's trail actor keeps actorSeed's default flag; only the impact's
    // FreezeWave program below receives the current global quality.
    trail.alpha = fround(0.5 + brightness);
    trail.lifetime_ticks = 21;
    trail.phase = fround(0.5 * fround(0.5 + scale));
    trail.quantity = if direction == 1 { 0.99_f32 as f64 } else { 1.015_f32 as f64 };
    trail.rotation_radians = rotation * std::f64::consts::PI / 180.0;
    trail.scale = 2.5;
    engine.state.spawn_actor(trail);
    let remaining = 400_i64 - actor.age_ticks as i64;
    if source.variant == 0 && remaining < 175 {
        actor.variant = 1;
        engine.event(actor, Some(Cue::CometWhistle), Event::Whistle);
    }
    if remaining > 0 {
        return;
    }
    for target in engine.candidates(actor, 400.0) {
        engine.damage(actor, &target, actor.damage, Damage::Ice);
    }
    let mut wave = super::program::actor(
        Kind::FreezeWave,
        &actor.owner_id,
        &actor.world_key,
        Some(76),
        actor.rank,
        actor.position,
    );
    wave.freeze_ticks = actor.freeze_ticks;
    freeze_wave(
        engine.state,
        wave,
        engine.context.enhanced_effects_for(&actor.owner_id),
        actor.quantity == 1.0,
    );
    let mut impact = super::program::actor(
        Kind::CometImpact,
        &actor.owner_id,
        &actor.world_key,
        Some(76),
        actor.rank,
        actor.position,
    );
    impact.lifetime_ticks = 1000;
    engine.state.spawn_actor(impact);
    let initial = engine.state.rng.draw_scaled_float(360.0, false);
    let mut angle = 0.0;
    loop {
        let vertical = engine.state.rng.draw_scaled_float(3.0, false);
        let height = engine.state.rng.draw_scaled_float(20.0, false);
        let rotation = engine.state.rng.draw_scaled_float(360.0, false);
        let spin = engine.state.rng.draw_scaled_float(10.0, false);
        let record = engine.state.rng.draw_integer(5);
        let scale = engine.state.rng.draw_scaled_float(0.25, true);
        let offset = engine.state.rng.draw_scaled_float(10.0, false);
        let speed = engine.state.rng.draw_scaled_float(2.5, false);
        let life = engine.state.rng.draw_scaled_float(1.0, true);
        let step = engine.state.rng.draw_scaled_float(3.0, true);
        let radians = (initial + angle) * std::f64::consts::PI / 180.0;
        let direction = Vec2::new(float::cos(radians), -float::sin(radians));
        let speed = fround(0.5 + speed);
        let bounce = fround(fround(-(2.0 + vertical)) * 1.25);
        let distance = fround(80.0 + offset);
        let mut child = super::program::actor(
            Kind::CometDebris,
            &actor.owner_id,
            &actor.world_key,
            Some(76),
            actor.rank,
            Vec2::new(
                fround(actor.position.x + direction.x * distance),
                fround(actor.position.y + direction.y * distance),
            ),
        );
        child.alpha = fround(1.5 * fround(1.0 + life));
        child.endpoint = Vec2::new(bounce, bounce);
        child.lifetime_ticks = 1000;
        child.phase = fround(-height);
        child.rotation_radians = rotation * std::f64::consts::PI / 180.0;
        child.scale = fround(0.8 + scale);
        child.slow_factor = (1.0 + spin) * std::f64::consts::PI / 180.0;
        child.variant = record;
        child.velocity = Vec2::new(fround(direction.x * 1.5 * speed), fround(direction.y * speed));
        engine.state.spawn_actor(child);
        angle = fround(angle + 8.0 + step);
        if angle >= 360.0 {
            break;
        }
    }
    let mut event = actor_event(actor, engine.context.tick, Event::Impact, Some(Cue::ExplodeSteam));
    event.screen_flash = Some(screen_flash(1.0, 1.0, 1.0, 0.005, false));
    engine.state.emit_event(event);
    for cue in [Cue::MagicShieldExplode, Cue::BigFire, Cue::RingOfIce] {
        engine.event(actor, Some(cue), Event::Impact);
    }
    *retain = false;
}
