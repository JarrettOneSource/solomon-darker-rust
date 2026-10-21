//! Enemy display samples from `client/boneyard-enemy-samples.ts`.
//! Only the retained presentation clone is written. Brains, RNG and authority are never stepped.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::{PI, TAU};

use solomon_sim::Vec2;
use solomon_sim::enemies::EnemyToken;
use solomon_sim::enemies::actor_presentation::{
    EnemyActorAppearanceContext, NativeEnemyPresentationInput, NativeMaggotPresentationInput,
    native_enemy_presentation_input, native_maggot_presentation_input,
};
use solomon_sim::enemies::model::{
    EnemyActor, EnemyBrain, EnemyDeathEffect, EnemyEvent, SpiderPhase,
};
use solomon_sim::enemies::presentation::NativeEnemyEffectPresentationInput;
use solomon_sim::enemies::projectiles::{EnemyProjectile, EnemyProjectileKind, MageLightningPulse};
use solomon_sim::enemies::world::EnemyWorld;

use super::math::{interpolate_members, lerp, lerp_cycle, lerp_vec};

// The source sample module imports these stock frame/lifetime counts.
const IMP_UPPER_EFFECT_FRAME_COUNT: f64 = 10.0;
const MAGE_LIGHTNING_MAX_PULSE_AGES: f64 = 5.0;
const ENEMY_GAIT_POSE_COUNT: f64 = 8.0;
const GUIDED_MISSILE_VISUAL_PHASE_PERIOD: f64 = 720.0;

/// Fractional source snapshot fields that cannot be stored in the simulation's whole ticks.
#[derive(Clone, Debug, Default)]
pub struct EnemyPresentationMetadata {
    pub presentation_tick: f64,
    pub visible_event_ids: BTreeSet<usize>,
    pub actors: BTreeMap<usize, NativeEnemyPresentationInput>,
    pub maggots: BTreeMap<usize, NativeMaggotPresentationInput>,
    pub projectile_age_ticks: BTreeMap<usize, f64>,
    pub projectile_heading_deg: BTreeMap<usize, f64>,
    pub projectile_visual_phase_deg: BTreeMap<usize, f64>,
    pub projectile_effects: BTreeMap<usize, NativeEnemyEffectPresentationInput>,
    pub death_effects: BTreeMap<usize, NativeEnemyEffectPresentationInput>,
    pub silk_age_ticks: BTreeMap<usize, f64>,
    pub crows: Vec<EnemyCrowPresentationInput>,
}

/// Crows join the source death-effect snapshots after normal effects, with discrete art/tint.
#[derive(Clone, Debug, PartialEq)]
pub struct EnemyCrowPresentationInput {
    pub id: usize,
    pub age_ticks: f64,
    pub position: Vec2,
    pub height: f64,
    pub entry: usize,
    pub tint: u32,
}

/// Capture once at an accepted endpoint. Spider light and target tint belong to that endpoint,
/// and must not be recomputed from the interpolated camera or an interpolated world.
#[must_use]
pub(super) fn from_world(
    world: &EnemyWorld,
    presentation_tick: f64,
    enemy_events: &[EnemyEvent],
    actor_context: &dyn Fn(&EnemyActor) -> EnemyActorAppearanceContext,
) -> EnemyPresentationMetadata {
    let mut metadata = EnemyPresentationMetadata {
        presentation_tick,
        visible_event_ids: enemy_events.iter().map(|event| event.event_id).collect(),
        ..Default::default()
    };
    for actor in world.actors.iter().filter(|actor| visible_actor(actor)) {
        let mut sample = native_enemy_presentation_input(actor, actor_context(actor));
        sample.presentation_tick = presentation_tick;
        metadata.actors.insert(actor.id, sample);
    }
    let tick = world.last_step_tick.unwrap_or(presentation_tick as u64);
    for maggot in &world.maggots {
        metadata.maggots.insert(maggot.id, native_maggot_presentation_input(maggot, tick));
    }
    for projectile in &world.projectiles {
        metadata
            .projectile_age_ticks
            .insert(projectile.state().id, projectile.state().age_ticks as f64);
    }
    for projectile in &world.projectiles {
        let id = projectile.state().id;
        metadata.projectile_heading_deg.insert(id, projectile_heading(projectile));
        metadata
            .projectile_visual_phase_deg
            .insert(id, positive_phase(projectile.state().visual_phase_deg));
    }
    for effect in &world.projectile_effects {
        metadata.projectile_effects.insert(
            effect.state.id,
            NativeEnemyEffectPresentationInput {
                age_ticks: effect.state.age_ticks as f64,
                rotation_radians: effect.state.rotation_deg * PI / 180.0,
            },
        );
    }
    for effect in &world.death_effects {
        metadata.death_effects.insert(
            effect.id,
            NativeEnemyEffectPresentationInput {
                age_ticks: effect.age_ticks as f64,
                rotation_radians: effect.rotation_deg * PI / 180.0,
            },
        );
    }
    for silk in &world.silks {
        metadata.silk_age_ticks.insert(silk.id, silk.state.age_ticks as f64);
    }
    metadata.crows = crow_samples(world);
    metadata
}

/// `output` is the timeline's selected endpoint clone. Matched residents keep older order;
/// births and removals occur only at the newer endpoint, exactly like the source snapshots.
#[must_use]
#[allow(clippy::too_many_arguments)] // Both endpoint worlds and their sampled animation belong to one bracket.
pub(super) fn interpolate(
    output: &mut EnemyWorld,
    older: &EnemyWorld,
    newer: &EnemyWorld,
    older_metadata: &EnemyPresentationMetadata,
    newer_metadata: &EnemyPresentationMetadata,
    blend: f64,
    presentation_tick: f64,
) -> EnemyPresentationMetadata {
    let first: Vec<_> = older.actors.iter().filter(|actor| visible_actor(actor)).cloned().collect();
    let second: Vec<_> =
        newer.actors.iter().filter(|actor| visible_actor(actor)).cloned().collect();
    output.actors = interpolate_members(
        &first,
        &second,
        blend,
        |actor| actor.id,
        |first, second, blend| {
            let mut actor = endpoint(first, second, blend).clone();
            actor.current_health = lerp(
                first.current_health.min(first.config.maximum_health),
                second.current_health.min(second.config.maximum_health),
                blend,
            );
            actor.heading_deg = lerp_cycle(first.heading_deg, second.heading_deg, blend, 360.0);
            actor.position = lerp_vec(first.position, second.position, blend);
            actor.shield_health = lerp(first.shield_health, second.shield_health, blend);
            actor.shield_maximum_health =
                lerp(first.shield_maximum_health, second.shield_maximum_health, blend);
            if let (
                EnemyBrain::DemonSkull(first),
                EnemyBrain::DemonSkull(second),
                EnemyBrain::DemonSkull(brain),
            ) = (&first.brain, &second.brain, &mut actor.brain)
            {
                brain.body_heading_deg =
                    lerp_cycle(first.body_heading_deg, second.body_heading_deg, blend, 360.0);
                brain.body_offset = lerp_vec(first.body_offset, second.body_offset, blend);
                brain.jitter = lerp_vec(first.jitter, second.jitter, blend);
                brain.eye_charge = lerp(first.eye_charge, second.eye_charge, blend);
                brain.charge_glow = lerp(first.charge_glow, second.charge_glow, blend);
                brain.flair_glow = lerp(first.flair_glow, second.flair_glow, blend);
                brain.body_phase_deg =
                    lerp_cycle(first.body_phase_deg, second.body_phase_deg, blend, 360.0);
                brain.flicker_phase_deg =
                    lerp_cycle(first.flicker_phase_deg, second.flicker_phase_deg, blend, 360.0);
                brain.spin = lerp(first.spin, second.spin, blend);
            }
            actor
        },
    );
    let mut metadata = EnemyPresentationMetadata {
        presentation_tick,
        visible_event_ids: endpoint(older_metadata, newer_metadata, blend)
            .visible_event_ids
            .clone(),
        ..Default::default()
    };
    for actor in &output.actors {
        let mut sample =
            match (older_metadata.actors.get(&actor.id), newer_metadata.actors.get(&actor.id)) {
                (Some(first), Some(second)) => {
                    interpolate_animation(first, second, blend, actor.config.enemy_token)
                }
                (first, second) => endpoint_option(first, second, blend)
                    .expect("retained enemy has its endpoint animation")
                    .clone(),
            };
        sample.presentation_tick = presentation_tick;
        metadata.actors.insert(actor.id, sample);
    }
    output.projectiles = interpolate_members(
        &older.projectiles,
        &newer.projectiles,
        blend,
        |projectile| projectile.state().id,
        |first, second, blend| {
            let mut projectile = endpoint(first, second, blend).clone();
            let first_heading = projectile_heading(first);
            let second_heading = projectile_heading(second);
            let period = if first.kind() == EnemyProjectileKind::GuidedMissile {
                GUIDED_MISSILE_VISUAL_PHASE_PERIOD
            } else {
                360.0
            };
            let first = first.state();
            let second = second.state();
            let state = projectile.state_mut();
            state.heading_deg = lerp_cycle(first_heading, second_heading, blend, 360.0);
            state.position = lerp_vec(first.position, second.position, blend);
            state.speed = lerp(first.speed, second.speed, blend);
            state.vertical_offset = lerp(first.vertical_offset, second.vertical_offset, blend);
            state.visual_phase_deg = lerp_cycle(
                positive_phase(first.visual_phase_deg),
                positive_phase(second.visual_phase_deg),
                blend,
                period,
            );
            state.visual_scale = lerp(first.visual_scale, second.visual_scale, blend);
            projectile
        },
    );
    for projectile in &output.projectiles {
        let id = projectile.state().id;
        metadata.projectile_age_ticks.insert(
            id,
            interpolate_age(
                id,
                &older_metadata.projectile_age_ticks,
                &newer_metadata.projectile_age_ticks,
                blend,
            ),
        );
    }
    for projectile in &output.projectiles {
        let state = projectile.state();
        metadata.projectile_heading_deg.insert(state.id, state.heading_deg);
        metadata.projectile_visual_phase_deg.insert(state.id, state.visual_phase_deg);
    }
    output.projectile_effects = interpolate_members(
        &older.projectile_effects,
        &newer.projectile_effects,
        blend,
        |effect| effect.state.id,
        |first, second, blend| {
            let mut effect = endpoint(first, second, blend).clone();
            effect.state.alpha = lerp(first.state.alpha, second.state.alpha, blend);
            effect.state.position = lerp_vec(first.state.position, second.state.position, blend);
            effect.state.rotation_deg = lerp_cycle(
                first.state.rotation_deg * PI / 180.0,
                second.state.rotation_deg * PI / 180.0,
                blend,
                TAU,
            ) * 180.0
                / PI;
            effect.state.scale = lerp(first.state.scale, second.state.scale, blend);
            effect
        },
    );
    for effect in &output.projectile_effects {
        let id = effect.state.id;
        let sample = match (
            older_metadata.projectile_effects.get(&id),
            newer_metadata.projectile_effects.get(&id),
        ) {
            (Some(first), Some(second)) => NativeEnemyEffectPresentationInput {
                age_ticks: lerp(first.age_ticks, second.age_ticks, blend),
                rotation_radians: lerp_cycle(
                    first.rotation_radians,
                    second.rotation_radians,
                    blend,
                    TAU,
                ),
            },
            (first, second) => *endpoint_option(first, second, blend)
                .expect("retained projectile effect has its endpoint sample"),
        };
        metadata.projectile_effects.insert(id, sample);
    }
    let (effects, ages) =
        interpolate_death_effects(&older.death_effects, &newer.death_effects, blend);
    output.death_effects = effects;
    metadata.death_effects = ages;
    output.maggots = interpolate_members(
        &older.maggots,
        &newer.maggots,
        blend,
        |maggot| maggot.id,
        |first, second, blend| {
            let mut maggot = endpoint(first, second, blend).clone();
            let sample = interpolate_maggot(
                &older_metadata.maggots[&first.id],
                &newer_metadata.maggots[&second.id],
                blend,
            );
            maggot.current_health = sample.current_health;
            maggot.emergence_phase = sample.emergence_phase;
            maggot.heading_deg = sample.heading_deg;
            maggot.gait_pose = sample.pose;
            maggot.position = sample.position;
            maggot.vertical_offset = sample.vertical_offset;
            maggot.visual_scale = sample.visual_scale;
            maggot
        },
    );
    for maggot in &output.maggots {
        let sample = match (
            older_metadata.maggots.get(&maggot.id),
            newer_metadata.maggots.get(&maggot.id),
        ) {
            (Some(first), Some(second)) => interpolate_maggot(first, second, blend),
            (first, second) => *endpoint_option(first, second, blend)
                .expect("retained Maggot has its endpoint sample"),
        };
        metadata.maggots.insert(maggot.id, sample);
    }
    output.mage_lightning_pulses =
        merge_pulses(&older.mage_lightning_pulses, &newer.mage_lightning_pulses, presentation_tick);
    interpolate_spiders(output, older, newer, blend, &mut metadata);
    metadata.crows = interpolate_members(
        &older_metadata.crows,
        &newer_metadata.crows,
        blend,
        |crow| crow.id,
        |first, second, blend| {
            let mut crow = endpoint(first, second, blend).clone();
            crow.age_ticks = lerp(first.age_ticks, second.age_ticks, blend);
            crow.position = lerp_vec(first.position, second.position, blend);
            crow.height = lerp(first.height, second.height, blend);
            crow
        },
    );
    metadata
}

fn visible_actor(actor: &EnemyActor) -> bool {
    !matches!(&actor.brain, EnemyBrain::Spider(brain) if brain.phase == SpiderPhase::Captured)
}

fn endpoint<'a, T>(first: &'a T, second: &'a T, blend: f64) -> &'a T {
    if blend < 1.0 { first } else { second }
}

fn endpoint_option<'a, T>(
    first: Option<&'a T>,
    second: Option<&'a T>,
    blend: f64,
) -> Option<&'a T> {
    if blend < 1.0 { first } else { second }
}

fn interpolate_age(
    id: usize,
    older: &BTreeMap<usize, f64>,
    newer: &BTreeMap<usize, f64>,
    blend: f64,
) -> f64 {
    match (older.get(&id), newer.get(&id)) {
        (Some(first), Some(second)) => lerp(*first, *second, blend),
        (first, second) => {
            *endpoint_option(first, second, blend).expect("retained effect has its endpoint age")
        }
    }
}

fn positive_phase(phase: f64) -> f64 {
    ((phase % 720.0) + 720.0) % 720.0
}

fn projectile_heading(projectile: &EnemyProjectile) -> f64 {
    if let EnemyProjectile::Arrow { state, velocity } = projectile
        && state.speed > 0.0
    {
        return solomon_sim::heading::heading_degrees_from_vector(velocity.x, velocity.y);
    }
    projectile.state().heading_deg
}

#[must_use]
pub(crate) fn interpolate_death_effects(
    older: &[EnemyDeathEffect],
    newer: &[EnemyDeathEffect],
    blend: f64,
) -> (Vec<EnemyDeathEffect>, BTreeMap<usize, NativeEnemyEffectPresentationInput>) {
    let mut ages = BTreeMap::new();
    let effects = interpolate_members(
        older,
        newer,
        blend,
        |effect| effect.id,
        |first, second, blend| {
            let mut effect = endpoint(first, second, blend).clone();
            ages.insert(
                effect.id,
                NativeEnemyEffectPresentationInput {
                    age_ticks: lerp(first.age_ticks as f64, second.age_ticks as f64, blend),
                    rotation_radians: lerp_cycle(
                        first.rotation_deg * PI / 180.0,
                        second.rotation_deg * PI / 180.0,
                        blend,
                        TAU,
                    ),
                },
            );
            effect.alpha = lerp(first.alpha, second.alpha, blend);
            effect.height = lerp(first.height, second.height, blend);
            effect.position = lerp_vec(first.position, second.position, blend);
            effect.rotation_deg = lerp_cycle(
                first.rotation_deg * PI / 180.0,
                second.rotation_deg * PI / 180.0,
                blend,
                TAU,
            ) * 180.0
                / PI;
            effect.scale = lerp(first.scale, second.scale, blend);
            effect.scale_y = lerp(first.scale_y, second.scale_y, blend);
            effect
        },
    );
    for effect in &effects {
        ages.entry(effect.id).or_insert(NativeEnemyEffectPresentationInput {
            age_ticks: effect.age_ticks as f64,
            rotation_radians: effect.rotation_deg * PI / 180.0,
        });
    }
    (effects, ages)
}

fn interpolate_animation(
    first: &NativeEnemyPresentationInput,
    second: &NativeEnemyPresentationInput,
    blend: f64,
    token: EnemyToken,
) -> NativeEnemyPresentationInput {
    let mut sample = endpoint(first, second, blend).clone();
    if first.state != second.state
        || first.action != second.action
        || first.death_epoch != second.death_epoch
    {
        return sample;
    }
    if let (Some(first), Some(second), Some(spider)) =
        (first.spider, second.spider, &mut sample.spider)
    {
        spider.body_heading_deg =
            lerp_cycle(first.body_heading_deg, second.body_heading_deg, blend, 360.0);
        spider.outline_alpha = lerp(first.outline_alpha, second.outline_alpha, blend);
    }
    sample.action_progress = lerp(first.action_progress, second.action_progress, blend);
    sample.alpha = lerp(first.alpha, second.alpha, blend);
    sample.body_gait_phase = lerp_cycle(first.body_gait_phase, second.body_gait_phase, blend, 4.0);
    if let (Some(first), Some(second)) = (first.pike_target_offset, second.pike_target_offset) {
        sample.pike_target_offset = Some(lerp_vec(first, second, blend));
    }
    sample.coffin_pose = lerp(first.coffin_pose, second.coffin_pose, blend);
    sample.coffin_rotation_radians =
        lerp(first.coffin_rotation_radians, second.coffin_rotation_radians, blend);
    sample.death_tick = lerp(first.death_tick, second.death_tick, blend);
    sample.demon_front_extremity_offset =
        lerp_vec(first.demon_front_extremity_offset, second.demon_front_extremity_offset, blend);
    sample.demon_front_rotation_radians =
        lerp(first.demon_front_rotation_radians, second.demon_front_rotation_radians, blend);
    sample.demon_rear_extremity_offset =
        lerp_vec(first.demon_rear_extremity_offset, second.demon_rear_extremity_offset, blend);
    sample.demon_rear_rotation_radians =
        lerp(first.demon_rear_rotation_radians, second.demon_rear_rotation_radians, blend);
    sample.demon_shadow_offset =
        lerp_vec(first.demon_shadow_offset, second.demon_shadow_offset, blend);
    sample.shadow_lateral_offset =
        lerp_cycle(first.shadow_lateral_offset, second.shadow_lateral_offset, blend, 4.0);
    sample.gait_pose = lerp_cycle(first.gait_pose, second.gait_pose, blend, ENEMY_GAIT_POSE_COUNT);
    sample.hit_flash = lerp(first.hit_flash, second.hit_flash, blend);
    sample.imp_effect_alpha = lerp(first.imp_effect_alpha, second.imp_effect_alpha, blend);
    if token == EnemyToken::Imp
        && blend < 1.0
        && first.imp_effect_frame >= 0.0
        && second.imp_effect_frame >= 0.0
    {
        sample.imp_effect_frame = lerp_cycle(
            first.imp_effect_frame,
            second.imp_effect_frame,
            blend,
            IMP_UPPER_EFFECT_FRAME_COUNT,
        );
    }
    sample.stride_phase_deg = lerp(first.stride_phase_deg, second.stride_phase_deg, blend);
    sample.vertical_offset = lerp(first.vertical_offset, second.vertical_offset, blend);
    sample.zombie_angular_offset_deg =
        lerp(first.zombie_angular_offset_deg, second.zombie_angular_offset_deg, blend);
    sample.zombie_body_rotation_radians =
        lerp(first.zombie_body_rotation_radians, second.zombie_body_rotation_radians, blend);
    sample.zombie_arm_socket_rotation_radians = lerp(
        first.zombie_arm_socket_rotation_radians,
        second.zombie_arm_socket_rotation_radians,
        blend,
    );
    sample.zombie_front_arm_pose =
        lerp(first.zombie_front_arm_pose, second.zombie_front_arm_pose, blend);
    sample.zombie_front_arm_rotation_radians = lerp(
        first.zombie_front_arm_rotation_radians,
        second.zombie_front_arm_rotation_radians,
        blend,
    );
    sample.zombie_head_rotation_radians =
        lerp(first.zombie_head_rotation_radians, second.zombie_head_rotation_radians, blend);
    sample.zombie_rear_arm_pose =
        lerp(first.zombie_rear_arm_pose, second.zombie_rear_arm_pose, blend);
    sample.zombie_rear_arm_rotation_radians = lerp(
        first.zombie_rear_arm_rotation_radians,
        second.zombie_rear_arm_rotation_radians,
        blend,
    );
    sample.effects = interpolate_members(
        &first.effects,
        &second.effects,
        blend,
        |effect| effect.id,
        |first, second, blend| {
            if first.layer.role != second.layer.role {
                return first.clone();
            }
            let mut effect = endpoint(first, second, blend).clone();
            effect.layer.alpha = lerp(first.layer.alpha, second.layer.alpha, blend);
            effect.layer.offset = lerp_vec(first.layer.offset, second.layer.offset, blend);
            effect.layer.rotation_radians =
                lerp(first.layer.rotation_radians, second.layer.rotation_radians, blend);
            effect.layer.scale = lerp(first.layer.scale, second.layer.scale, blend);
            effect
        },
    );
    sample
}

fn interpolate_maggot(
    first: &NativeMaggotPresentationInput,
    second: &NativeMaggotPresentationInput,
    blend: f64,
) -> NativeMaggotPresentationInput {
    let mut sample = *endpoint(first, second, blend);
    sample.alpha = lerp(first.alpha, second.alpha, blend);
    sample.current_health = lerp(first.current_health, second.current_health, blend);
    sample.death_tick = lerp(first.death_tick, second.death_tick, blend);
    sample.emergence_phase = lerp_cycle(first.emergence_phase, second.emergence_phase, blend, 5.0);
    sample.emergence_tick = lerp(first.emergence_tick, second.emergence_tick, blend);
    sample.heading_deg = lerp_cycle(first.heading_deg, second.heading_deg, blend, 360.0);
    sample.hit_flash = lerp(first.hit_flash, second.hit_flash, blend);
    sample.pose = lerp_cycle(first.pose, second.pose, blend, 2.0);
    sample.position = lerp_vec(first.position, second.position, blend);
    sample.vertical_offset = lerp(first.vertical_offset, second.vertical_offset, blend);
    sample.visual_scale = lerp(first.visual_scale, second.visual_scale, blend);
    sample
}

fn merge_pulses(
    older: &[MageLightningPulse],
    newer: &[MageLightningPulse],
    tick: f64,
) -> Vec<MageLightningPulse> {
    let mut pulses = BTreeMap::new();
    for pulse in older.iter().chain(newer) {
        if pulse.tick as f64 > tick || tick - pulse.tick as f64 >= MAGE_LIGHTNING_MAX_PULSE_AGES {
            continue;
        }
        pulses.insert(pulse.id, pulse.clone());
    }
    let mut pulses: Vec<_> = pulses.into_values().collect();
    pulses.sort_by_key(|pulse| (pulse.tick, pulse.id));
    pulses
}

fn interpolate_spiders(
    output: &mut EnemyWorld,
    older: &EnemyWorld,
    newer: &EnemyWorld,
    blend: f64,
    metadata: &mut EnemyPresentationMetadata,
) {
    // The Spider source returns the newer collection verbatim at the endpoint, including order.
    if blend >= 1.0 {
        output.silks.clone_from(&newer.silks);
        output.silk_fragments.clone_from(&newer.silk_fragments);
        output.spider_remains.clone_from(&newer.spider_remains);
    } else {
        let by_id: BTreeMap<_, _> = newer.silks.iter().map(|silk| (silk.id, silk)).collect();
        output.silks = older.silks.clone();
        for silk in &mut output.silks {
            if let Some(second) = by_id.get(&silk.id) {
                let first = &mut silk.state;
                first.phase = lerp(first.phase, second.state.phase, blend);
                first.height = lerp(first.height, second.state.height, blend);
                first.alpha = lerp(first.alpha, second.state.alpha, blend);
                first.wave_scale = lerp(first.wave_scale, second.state.wave_scale, blend);
                first.position = lerp_vec(first.position, second.state.position, blend);
                first.drift = lerp_vec(first.drift, second.state.drift, blend);
            }
        }
        let by_id: BTreeMap<_, _> =
            newer.silk_fragments.iter().map(|fragment| (fragment.id, fragment)).collect();
        output.silk_fragments = older.silk_fragments.clone();
        for fragment in &mut output.silk_fragments {
            if let Some(second) = by_id.get(&fragment.id) {
                fragment.state.start = lerp_vec(fragment.state.start, second.state.start, blend);
                fragment.state.middle = lerp_vec(fragment.state.middle, second.state.middle, blend);
                fragment.state.end = lerp_vec(fragment.state.end, second.state.end, blend);
                fragment.state.opacity = lerp(fragment.state.opacity, second.state.opacity, blend);
            }
        }
        let by_id: BTreeMap<_, _> =
            newer.spider_remains.iter().map(|remains| (remains.id, remains)).collect();
        output.spider_remains = older.spider_remains.clone();
        for remains in &mut output.spider_remains {
            if let Some(second) = by_id.get(&remains.id) {
                remains.state.life = lerp(remains.state.life, second.state.life, blend);
                remains.state.position =
                    lerp_vec(remains.state.position, second.state.position, blend);
                if let (Some(first), Some(second)) = (&mut remains.state.decal, &second.state.decal)
                {
                    first.scale = lerp(first.scale, second.scale, blend);
                    first.alpha = lerp(first.alpha, second.alpha, blend);
                }
            }
        }
    }
    for silk in &output.silks {
        let first = older.silks.iter().find(|first| first.id == silk.id);
        let second = newer.silks.iter().find(|second| second.id == silk.id);
        let age = match (first, second) {
            (Some(first), Some(second)) => {
                lerp(first.state.age_ticks as f64, second.state.age_ticks as f64, blend)
            }
            _ => silk.state.age_ticks as f64,
        };
        metadata.silk_age_ticks.insert(silk.id, age);
    }
    output.webbed_players.clone_from(&endpoint(older, newer, blend).webbed_players);
}

fn crow_samples(world: &EnemyWorld) -> Vec<EnemyCrowPresentationInput> {
    let mut samples = Vec::new();
    let mut append = |crow: &solomon_sim::enemies::model::CrowState| {
        let Some(position) = crow.position else { return };
        let facing = (((crow.heading_deg + 10.0) / 20.0).trunc() as i64).rem_euclid(18) as usize;
        let channel = (crow.light_intensity.clamp(0.0, 1.0) * 255.0).round() as u32;
        samples.push(EnemyCrowPresentationInput {
            id: crow.id,
            age_ticks: crow.age as f64,
            position,
            height: -crow.height,
            entry: 2 + crow.flap_phase.trunc() as usize * 18 + facing,
            tint: channel * 0x010101,
        });
    };
    for actor in &world.actors {
        if let EnemyBrain::Heartmonger(brain) = &actor.brain {
            for crow in &brain.crows {
                append(crow);
            }
        }
    }
    for crow in &world.detached_crows {
        append(&crow.flight);
    }
    samples
}

#[cfg(test)]
#[path = "enemies/tests.rs"]
mod tests;
