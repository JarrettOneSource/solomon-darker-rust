//! Primary display membership, ages and fields from `client/primary-spell-*-presentation.ts`.
//! Integer simulation clocks stay intact; the fractional ages travel with the display frame.

mod transients;
mod weld;

use std::collections::{BTreeMap, BTreeSet};

use solomon_sim::painter::PainterRegistration;
use solomon_sim::spells::ether::NATIVE_ETHER_BLAST_PARTICLE_LIFETIME_TICKS;
use solomon_sim::spells::fire::{
    NATIVE_FIRE_EXPLOSION_LIFETIME_TICKS, NATIVE_FIRE_IMPACT_LIFETIME_TICKS,
    native_fire_particle_lifetime_ticks,
};
use solomon_sim::spells::model::{PrimarySpellProjectile, PrimarySpellTransient, SpellState};
use solomon_sim::spells::water::water_frost_jet_lifetime_ticks;
use solomon_sim::spells::weld::effects::NativeWeldWorldActor;

use super::PresentationClocks;
use super::math::{interpolate_members, lerp, lerp_option, lerp_vec};

#[derive(Default)]
pub(super) struct PrimaryPresentationMemory {
    hail_registrations: BTreeMap<u64, Vec<PainterRegistration>>,
}

/// A single snapshot is copied without the bracket's fixed-lifetime admission filter.
pub(super) fn copy(
    output: &mut SpellState,
    snapshot_tick: u64,
    memory: &mut PrimaryPresentationMemory,
    clocks: &mut PresentationClocks,
) {
    for effect in &mut output.transients {
        retain_hail(effect, snapshot_tick, memory, clocks);
    }
    prune_memory(output, memory);
}

#[allow(clippy::too_many_arguments)] // One bracket owns its endpoint clocks and retained Hail registrations.
pub(super) fn project(
    output: &mut SpellState,
    older: &SpellState,
    newer: &SpellState,
    older_tick: u64,
    newer_tick: u64,
    tick: f64,
    blend: f64,
    memory: &mut PrimaryPresentationMemory,
    clocks: &mut PresentationClocks,
) {
    clocks.primary.clear();
    clocks.primary_contact.clear();
    output.projectiles = interpolate_members(
        &older.projectiles,
        &newer.projectiles,
        blend,
        |spell| spell.common().id,
        |first, next, blend| projectile(first, next, blend, clocks),
    );
    for spell in &output.projectiles {
        clocks.primary.entry(spell.common().id).or_insert(spell.common().age_ticks as f64);
    }
    let first: Vec<_> = older
        .transients
        .iter()
        .filter(|effect| fixed_timing(effect, older_tick).is_none())
        .cloned()
        .collect();
    let next: Vec<_> = newer
        .transients
        .iter()
        .filter(|effect| fixed_timing(effect, newer_tick).is_none())
        .cloned()
        .collect();
    output.transients = interpolate_members(
        &first,
        &next,
        blend,
        |effect| effect.common().id,
        |first, next, blend| {
            let mut first = first.clone();
            let mut next = next.clone();
            retain_hail(&mut first, older_tick, memory, clocks);
            retain_hail(&mut next, newer_tick, memory, clocks);
            transients::project(&first, &next, blend, clocks)
        },
    );
    // Source retains an older-only Hail at its older snapshot age until removal.
    for effect in &mut output.transients {
        if !clocks.primary.contains_key(&effect.common().id) {
            let snapshot = if blend < 1.0 { older_tick } else { newer_tick };
            retain_hail(effect, snapshot, memory, clocks);
        }
    }
    let mut fixed: BTreeMap<
        u64,
        (Option<&PrimarySpellTransient>, Option<&PrimarySpellTransient>, FixedTiming),
    > = BTreeMap::new();
    for effect in &older.transients {
        if let Some(timing) = fixed_timing(effect, older_tick) {
            fixed.insert(effect.common().id, (Some(effect), None, timing));
        }
    }
    for effect in &newer.transients {
        if let Some(timing) = fixed_timing(effect, newer_tick) {
            fixed.entry(effect.common().id).and_modify(|entry| entry.1 = Some(effect)).or_insert((
                None,
                Some(effect),
                timing,
            ));
        }
    }
    for (id, (first, next, timing)) in fixed {
        let age = tick - timing.age_zero_tick;
        if age < timing.first_visible_age || age >= timing.lifetime_ticks {
            continue;
        }
        let mut effect = match (first, next) {
            (Some(first), Some(next)) => transients::project(first, next, blend, clocks),
            (Some(effect), None) | (None, Some(effect)) => effect.clone(),
            (None, None) => unreachable!("fixed bracket has an endpoint"),
        };
        let age = if matches!(effect, PrimarySpellTransient::Air(_)) { age.floor() } else { age };
        effect.common_mut().age_ticks = age.floor() as u64;
        clocks.primary.insert(id, age);
        output.transients.push(effect);
    }
    output.transients.sort_by_key(|effect| effect.common().id);
    for effect in &output.transients {
        clocks.primary.entry(effect.common().id).or_insert(effect.common().age_ticks as f64);
        if let PrimarySpellTransient::FireGoodImp(imp) = effect
            && let Some(age) = imp.contact_age_ticks
        {
            clocks.primary_contact.entry(imp.common.id).or_insert(age as f64);
        }
    }
    prune_memory(output, memory);
}

fn projectile(
    first: &PrimarySpellProjectile,
    next: &PrimarySpellProjectile,
    blend: f64,
    clocks: &mut PresentationClocks,
) -> PrimarySpellProjectile {
    let mut output = if blend < 1.0 { first.clone() } else { next.clone() };
    if std::mem::discriminant(first) != std::mem::discriminant(next) {
        return output;
    }
    let age = lerp(first.common().age_ticks as f64, next.common().age_ticks as f64, blend);
    clocks.primary.insert(first.common().id, age);
    let common = output.common_mut();
    common.age_ticks = age.floor() as u64;
    common.direction = lerp_vec(first.common().direction, next.common().direction, blend);
    common.position = lerp_vec(first.common().position, next.common().position, blend);
    common.velocity = lerp_vec(first.common().velocity, next.common().velocity, blend);
    match (&mut output, first, next) {
        (
            PrimarySpellProjectile::Weld(output),
            PrimarySpellProjectile::Weld(first),
            PrimarySpellProjectile::Weld(next),
        ) => {
            output.ball_lightning_acceleration = lerp_option(
                first.ball_lightning_acceleration,
                next.ball_lightning_acceleration,
                blend,
            );
            output.base_presentation_phase_degrees = lerp_option(
                first.base_presentation_phase_degrees,
                next.base_presentation_phase_degrees,
                blend,
            );
            output.frost_turn_degrees =
                lerp_option(first.frost_turn_degrees, next.frost_turn_degrees, blend);
            let age = lerp_option(
                first.ground_spark_native_age_ticks.map(|age| age as f64),
                next.ground_spark_native_age_ticks.map(|age| age as f64),
                blend,
            );
            output.ground_spark_native_age_ticks = age.map(|age| age.floor() as u64);
            if let Some(age) = age {
                clocks.primary_ground_spark.insert(output.common.id, age);
            }
        }
        (
            PrimarySpellProjectile::Earth(output),
            PrimarySpellProjectile::Earth(first),
            PrimarySpellProjectile::Earth(next),
        ) => {
            output.common.charge = lerp(first.common.charge, next.common.charge, blend);
            output.shell_charge = lerp(first.shell_charge, next.shell_charge, blend);
        }
        _ => output.common_mut().charge = lerp(first.common().charge, next.common().charge, blend),
    }
    output
}

fn retain_hail(
    effect: &mut PrimarySpellTransient,
    snapshot_tick: u64,
    memory: &mut PrimaryPresentationMemory,
    clocks: &mut PresentationClocks,
) {
    let PrimarySpellTransient::WaterHail(hail) = effect else {
        return;
    };
    let retained = memory
        .hail_registrations
        .entry(hail.common.id)
        .or_insert_with(|| hail.common.painter_registrations.clone());
    assert_eq!(
        *retained, hail.common.painter_registrations,
        "retained Hail painter registration changed"
    );
    let age =
        snapshot_tick.checked_sub(hail.birth_tick).expect("Hail was born before its snapshot");
    hail.common.age_ticks = age;
    hail.life = solomon_sim::spells::air_water::native_water_hail_life_at_age(age);
    clocks.primary.insert(hail.common.id, age as f64);
}

fn prune_memory(output: &SpellState, memory: &mut PrimaryPresentationMemory) {
    let ids: BTreeSet<_> = output
        .transients
        .iter()
        .filter_map(|effect| {
            if let PrimarySpellTransient::WaterHail(hail) = effect {
                Some(hail.common.id)
            } else {
                None
            }
        })
        .collect();
    memory.hail_registrations.retain(|id, _| ids.contains(id));
}

struct FixedTiming {
    age_zero_tick: f64,
    lifetime_ticks: f64,
    first_visible_age: f64,
}

fn fixed_timing(effect: &PrimarySpellTransient, snapshot_tick: u64) -> Option<FixedTiming> {
    let inferred_birth = snapshot_tick as f64 - effect.common().age_ticks as f64;
    let (birth, lifetime, first) = match effect {
        PrimarySpellTransient::Air(effect) => (
            effect.birth_tick as f64,
            solomon_sim::spells::air::native_air_contact_lifetime_ticks(
                effect.underpowered,
                effect.chained,
                effect.enhanced_effects,
            ) as f64,
            0.0,
        ),
        PrimarySpellTransient::EarthImpact(effect) => {
            (effect.birth_tick as f64, effect.lifetime_ticks as f64, 0.0)
        }
        PrimarySpellTransient::EtherImpact(effect) => (effect.birth_tick as f64, 19.0, 0.0),
        PrimarySpellTransient::EtherBlast(effect) => {
            (effect.birth_tick as f64, NATIVE_ETHER_BLAST_PARTICLE_LIFETIME_TICKS as f64, 0.0)
        }
        PrimarySpellTransient::EtherPierceStreak(_) => (inferred_birth, 10.0, 0.0),
        PrimarySpellTransient::FireParticle(effect) => (
            inferred_birth,
            native_fire_particle_lifetime_ticks(effect.common.id, effect.enhanced_effects) as f64,
            0.0,
        ),
        PrimarySpellTransient::FireExplosion(_) => {
            (inferred_birth, NATIVE_FIRE_EXPLOSION_LIFETIME_TICKS as f64, 0.0)
        }
        PrimarySpellTransient::FireImpact(_) => {
            (inferred_birth, NATIVE_FIRE_IMPACT_LIFETIME_TICKS as f64, 0.0)
        }
        PrimarySpellTransient::Water(effect) => {
            (inferred_birth, water_frost_jet_lifetime_ticks(effect.common.id) as f64, 1.0)
        }
        PrimarySpellTransient::WaterAura(effect) => {
            (effect.birth_tick as f64, effect.duration_ticks as f64, 0.0)
        }
        PrimarySpellTransient::Weld(NativeWeldWorldActor::BlizzardChainFrost(effect)) => (
            effect.common.birth_tick as f64,
            water_frost_jet_lifetime_ticks(effect.common.owner.id) as f64,
            0.0,
        ),
        PrimarySpellTransient::Weld(NativeWeldWorldActor::BlizzardGlow(effect)) => {
            (effect.common.birth_tick as f64, 1.0, 0.0)
        }
        PrimarySpellTransient::Weld(NativeWeldWorldActor::FrostFade(effect)) => (
            effect.common.birth_tick as f64,
            if effect.common.build_id == 1004 { 3.0 } else { 20.0 },
            0.0,
        ),
        _ => return None,
    };
    Some(FixedTiming { age_zero_tick: birth, lifetime_ticks: lifetime, first_visible_age: first })
}
