//! Source-ordered player visual phases from `core-server/air-water-player-visual-system.ts`.
//! These actors remain in SpellState; the session calls synchronize before contacts
//! and finalize afterward so Hail and Aura consume the native combat stream in order.
use super::{
    create_native_water_aura_actor, create_native_water_hail_actor, step_native_water_hail_actor,
};
use crate::hurricane::{
    NativeHurricanePresentationProgram, create_native_hurricane_presentation,
    step_native_hurricane_presentation,
};
use crate::painter::{WorldManagerLane, WorldPainterOrder};
use crate::profile::primary::NativePrimarySkillProfile;
use crate::spells::effects::transient_base;
use crate::spells::model::*;
use crate::spells::water::{
    WaterFrostJetKind, water_frost_jet_kind, water_frost_jet_particle_count,
};
use crate::{NativeRng, Vec2};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct AirWaterPlayerVisualOwner {
    pub hurricane_contact_charge: f64,
    pub hurricane_charge: f64,
    pub hurricane_damage_maximum: f64,
    pub hurricane_damage_minimum: f64,
    pub owner_id: String,
    pub position: Vec2,
    pub world_key: String,
}

pub fn synchronize_air_water_player_visual_actors(
    spells: &mut SpellState,
    owners: &[AirWaterPlayerVisualOwner],
    tick: u64,
    rng: &mut NativeRng,
    channel_emissions: &[PrimarySpellChannelEmission],
    order: Option<&mut WorldPainterOrder>,
    enhanced_effects: bool,
) {
    let mut standalone = WorldPainterOrder {
        next_actor_ordinal: spells.next_id as usize,
        next_transient_ordinal: spells.next_id as usize,
    };
    let order = order.unwrap_or(&mut standalone);
    let mut owner_by_id = BTreeMap::new();
    for owner in owners {
        assert!(
            owner_by_id.insert(owner.owner_id.as_str(), owner).is_none(),
            "Air/Water owner IDs are unique"
        );
        assert!(
            (0.0..=1.0).contains(&owner.hurricane_charge)
                && (0.0..=owner.hurricane_charge).contains(&owner.hurricane_contact_charge),
            "Hurricane contact charge is within presentation charge"
        );
        assert!(
            owner.hurricane_damage_minimum.is_finite()
                && owner.hurricane_damage_maximum.is_finite()
                && owner.hurricane_damage_minimum >= 0.0
                && owner.hurricane_damage_maximum >= owner.hurricane_damage_minimum,
            "Hurricane damage range is finite and ordered"
        );
        assert!(
            owner.position.x.is_finite() && owner.position.y.is_finite(),
            "Air/Water owner position is finite"
        );
    }
    let mut hurricane_by_owner = BTreeMap::new();
    for mut effect in std::mem::take(&mut spells.transients) {
        if let PrimarySpellTransient::AirHurricane(actor) = effect {
            assert!(
                hurricane_by_owner.insert(actor.common.owner_id.clone(), actor).is_none(),
                "Hurricane owners are unique"
            );
            continue;
        }
        if let PrimarySpellTransient::WaterAura(actor) = &mut effect {
            let Some(owner) = owner_by_id.get(actor.common.owner_id.as_str()) else {
                continue;
            };
            if owner.world_key != actor.common.world_key {
                continue;
            }
            actor.origin = owner.position;
        }
        spells.transients.push(effect);
    }
    // Owner input order is significant: births draw their lanes before the next owner.
    for owner in owners {
        if owner.hurricane_charge <= 0.0 {
            continue;
        }
        if let Some(mut actor) = hurricane_by_owner.remove(&owner.owner_id)
            && actor.common.world_key == owner.world_key
            && owner.hurricane_contact_charge > 0.0
        {
            let mut program = NativeHurricanePresentationProgram {
                lanes: actor.lanes,
                phase_degrees: actor.phase_degrees,
            };
            step_native_hurricane_presentation(&mut program, owner.hurricane_contact_charge, rng);
            actor.common.age_ticks += 1;
            actor.enhanced_effects = enhanced_effects;
            actor.charge = owner.hurricane_charge;
            actor.contact_charge = owner.hurricane_contact_charge;
            actor.damage_maximum = owner.hurricane_damage_maximum;
            actor.damage_minimum = owner.hurricane_damage_minimum;
            actor.lanes = program.lanes;
            actor.phase_degrees = program.phase_degrees;
            actor.position = owner.position;
            spells.transients.push(PrimarySpellTransient::AirHurricane(actor));
            continue;
        }
        let program = create_native_hurricane_presentation(rng);
        let mut common = transient_base(spells.next_id, &owner.owner_id, &owner.world_key, None);
        common.painter_registrations.push(order.register(WorldManagerLane::Actor));
        spells.next_id += 1;
        spells.transients.push(PrimarySpellTransient::AirHurricane(PrimarySpellAirHurricane {
            common,
            birth_tick: tick,
            charge: owner.hurricane_charge,
            contact_charge: owner.hurricane_contact_charge,
            damage_maximum: owner.hurricane_damage_maximum,
            damage_minimum: owner.hurricane_damage_minimum,
            enhanced_effects,
            lanes: program.lanes,
            phase_degrees: program.phase_degrees,
            position: owner.position,
        }));
    }
    for emission in channel_emissions {
        let NativePrimarySkillProfile::Water(profile) = &emission.primary_skill else {
            continue;
        };
        if emission.kind != PrimarySpellChannelKind::Water
            || emission.underpowered
            || profile.hail_threshold == 0
        {
            continue;
        }
        let count = u64::from(water_frost_jet_particle_count(profile.widen_half_degrees, true));
        let mut frost: Vec<_> = spells
            .transients
            .iter()
            .filter_map(|effect| {
                let PrimarySpellTransient::Water(actor) = effect else {
                    return None;
                };
                let common = &actor.common;
                (common.owner_id == emission.owner_id
                    && common.world_key == emission.world_key
                    && common.id >= emission.id
                    && common.id < emission.id + count)
                    .then_some(actor.clone())
            })
            .collect();
        frost.sort_by_key(|actor| actor.common.id);
        for actor in frost {
            if water_frost_jet_kind(actor.common.id, actor.underpowered)
                != WaterFrostJetKind::Normal
            {
                continue;
            }
            if rng.draw_integer(250) as u32 >= profile.hail_threshold {
                continue;
            }
            let common =
                transient_base(spells.next_id, &emission.owner_id, &emission.world_key, None);
            let mut hail =
                create_native_water_hail_actor(common, tick, emission.origin, actor.direction, rng);
            hail.common.painter_registrations.push(order.register(WorldManagerLane::Actor));
            spells.transients.push(PrimarySpellTransient::WaterHail(hail));
            spells.next_id += 1;
        }
    }
}

pub fn finalize_air_water_player_visual_actors(
    spells: &mut SpellState,
    channel_emissions: &[PrimarySpellChannelEmission],
    tick: u64,
    rng: &mut NativeRng,
    order: &mut WorldPainterOrder,
) {
    if tick.is_multiple_of(crate::cold_aura::NATIVE_COLD_AURA_PERIOD_TICKS) {
        for emission in channel_emissions {
            let NativePrimarySkillProfile::Water(profile) = &emission.primary_skill else {
                continue;
            };
            if emission.kind != PrimarySpellChannelKind::Water
                || emission.underpowered
                || profile.aura_radius_scale <= 0.0
            {
                continue;
            }
            let common =
                transient_base(spells.next_id, &emission.owner_id, &emission.world_key, None);
            let mut aura = create_native_water_aura_actor(
                common,
                tick,
                emission.query_origin,
                profile.aura_radius_scale,
                rng,
            );
            aura.common.painter_registrations.push(order.register(WorldManagerLane::Actor));
            spells.transients.push(PrimarySpellTransient::WaterAura(aura));
            spells.next_id += 1;
        }
    }
    for effect in std::mem::take(&mut spells.transients) {
        if let PrimarySpellTransient::WaterHail(actor) = effect {
            if actor.birth_tick == tick {
                spells.transients.push(PrimarySpellTransient::WaterHail(actor));
            } else if let Some(actor) = step_native_water_hail_actor(actor, rng) {
                spells.transients.push(PrimarySpellTransient::WaterHail(actor));
            }
        } else {
            spells.transients.push(effect);
        }
    }
}
