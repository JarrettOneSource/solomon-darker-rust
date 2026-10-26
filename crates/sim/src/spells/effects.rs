//! Effect births and painter membership from `core-kernels/primary-spells.ts`.

use super::earth_debris::{NativeWeldMeteorDebrisSeed, create_native_weld_boulder_debris_particle};
use super::earth_fragments::{
    EarthImpactSeed, earth_impact_fragment_count, earth_impact_lifetime_ticks,
};
use super::ether::NATIVE_ETHER_BLAST_PARTICLE_LIFETIME_TICKS;
use super::fire::{
    NATIVE_FIRE_EXPLOSION_LIFETIME_TICKS, NATIVE_FIRE_IMPACT_LIFETIME_TICKS,
    NativeFireExplosionState, create_native_fire_detonation, native_fire_particle_lifetime_ticks,
    native_fire_particle_variant,
};
use super::model::*;
use super::water::water_frost_jet_lifetime_ticks;
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::{NativeRng, Vec2};

pub(super) fn transient_base(
    id: u64,
    owner_id: &str,
    world_key: &str,
    light_registration: Option<PainterRegistration>,
) -> PrimarySpellTransientBase {
    PrimarySpellTransientBase {
        age_ticks: 0,
        id,
        light_registration,
        owner_id: owner_id.into(),
        painter_registrations: light_registration.into_iter().collect(),
        world_key: world_key.into(),
    }
}

pub struct CreatePrimarySpellEarthBoulderBitInput<'a> {
    pub debris: NativeWeldMeteorDebrisSeed,
    pub enhanced_effects: bool,
    pub id: u64,
    pub origin: Vec2,
    pub owner_id: &'a str,
    pub tick: u64,
    pub world_key: &'a str,
}

#[must_use]
pub fn create_primary_spell_earth_boulder_bit(
    input: CreatePrimarySpellEarthBoulderBitInput<'_>,
    order: &mut WorldPainterOrder,
) -> PrimarySpellEarthBoulderBit {
    let mut common = transient_base(input.id, input.owner_id, input.world_key, None);
    common.painter_registrations.push(order.register(WorldManagerLane::Actor));
    PrimarySpellEarthBoulderBit {
        common,
        birth_tick: input.tick,
        debris: create_native_weld_boulder_debris_particle(input.debris, input.enhanced_effects),
        origin: input.origin,
        position: input.origin,
    }
}

#[must_use]
pub fn create_primary_spell_earth_impact(
    id: u64,
    spell: &PrimarySpellEarthProjectile,
    tick: u64,
    order: &mut WorldPainterOrder,
    enhanced_effects: bool,
) -> PrimarySpellEarthImpact {
    let seed =
        EarthImpactSeed { birth_tick: tick, charge: spell.common.charge, enhanced_effects, id };
    let mut common = transient_base(id, &spell.common.owner_id, &spell.common.world_key, None);
    common.painter_registrations =
        order.register_many(WorldManagerLane::Actor, earth_impact_fragment_count(seed.charge));
    PrimarySpellEarthImpact {
        common,
        birth_tick: tick,
        charge: seed.charge,
        enhanced_effects,
        lifetime_ticks: earth_impact_lifetime_ticks(seed),
        origin: spell.common.position,
    }
}

#[must_use]
pub fn create_primary_spell_ether_impact(
    id: u64,
    spell: &PrimarySpellEtherProjectile,
    tick: u64,
    order: &mut WorldPainterOrder,
) -> PrimarySpellEtherImpact {
    PrimarySpellEtherImpact {
        common: transient_base(
            id,
            &spell.common.owner_id,
            &spell.common.world_key,
            Some(order.register(WorldManagerLane::Transient)),
        ),
        birth_tick: tick,
        origin: spell.common.position,
        visual_scale: spell.visual_scale,
    }
}

pub(super) fn fire_impact_at(
    id: u64,
    origin: Vec2,
    owner_id: &str,
    world_key: &str,
    order: &mut WorldPainterOrder,
) -> PrimarySpellFireImpact {
    PrimarySpellFireImpact {
        common: transient_base(
            id,
            owner_id,
            world_key,
            Some(order.register(WorldManagerLane::Transient)),
        ),
        origin,
    }
}

pub(super) fn fire_explosion(
    id: u64,
    seed: NativeFireExplosionState,
    sound_pitch: f64,
    order: &mut WorldPainterOrder,
) -> PrimarySpellFireExplosion {
    PrimarySpellFireExplosion {
        common: transient_base(
            id,
            &seed.owner_id,
            &seed.world_key,
            Some(order.register(WorldManagerLane::Transient)),
        ),
        burn_damage: seed.burn_damage,
        damage: seed.damage,
        footprint_dimension: seed.footprint_dimension,
        origin: seed.origin,
        presentation: seed.presentation,
        sound_pitch,
        visual_scale: seed.visual_scale,
    }
}

pub struct PrimarySpellFireDetonation {
    pub contacts: Vec<FireRadialContact>,
    pub next_id: u64,
    pub transients: Vec<PrimarySpellTransient>,
}

#[must_use]
pub fn create_primary_spell_fire_detonation(
    next_id: u64,
    spell: &PrimarySpellFireProjectile,
    origin: Vec2,
    rng: &mut NativeRng,
    order: &mut WorldPainterOrder,
) -> PrimarySpellFireDetonation {
    let impact =
        fire_impact_at(next_id, origin, &spell.common.owner_id, &spell.common.world_key, order);
    let has_explosion = spell.explode_radius > 0.0 && spell.explode_damage > 0.0;
    let detonation = create_native_fire_detonation(
        next_id + 1 + u64::from(has_explosion),
        spell.into(),
        origin,
        &spell.common.owner_id,
        &spell.common.world_key,
        rng,
    );
    let mut transients = vec![PrimarySpellTransient::FireImpact(impact)];
    if let Some(explosion) = detonation.explosion {
        transients.push(PrimarySpellTransient::FireExplosion(fire_explosion(
            next_id + 1,
            explosion,
            detonation.sound_pitch,
            order,
        )));
    }
    for mut ember in detonation.embers {
        let registration = order.register(WorldManagerLane::Actor);
        ember.common.light_registration = Some(registration);
        ember.common.painter_registrations = vec![registration];
        transients.push(PrimarySpellTransient::FireEmber(ember));
    }
    PrimarySpellFireDetonation {
        contacts: detonation.contacts,
        next_id: detonation.next_id,
        transients,
    }
}

pub(super) fn create_fire_particle(
    id: u64,
    fireball: &PrimarySpellProjectileBase,
    order: &mut WorldPainterOrder,
    enhanced_effects: bool,
) -> PrimarySpellFireParticle {
    let mut common = transient_base(id, &fireball.owner_id, &fireball.world_key, None);
    common.painter_registrations.push(order.register(WorldManagerLane::Actor));
    PrimarySpellFireParticle {
        common,
        enhanced_effects,
        direction: fireball.direction,
        origin: fireball.position,
        variant: native_fire_particle_variant(id),
    }
}

pub(super) fn transient_lifetime(effect: &PrimarySpellTransient) -> u64 {
    match effect {
        PrimarySpellTransient::WaterAura(spell) => spell.duration_ticks,
        PrimarySpellTransient::Air(spell) => super::air::native_air_contact_lifetime_ticks(
            spell.underpowered,
            spell.chained,
            spell.enhanced_effects,
        ),
        PrimarySpellTransient::Water(spell) => water_frost_jet_lifetime_ticks(spell.common.id),
        PrimarySpellTransient::EarthImpact(spell) => spell.lifetime_ticks,
        PrimarySpellTransient::EtherImpact(_) => 19,
        PrimarySpellTransient::EtherPierceStreak(_) => 10,
        PrimarySpellTransient::EtherBlast(_) => NATIVE_ETHER_BLAST_PARTICLE_LIFETIME_TICKS,
        PrimarySpellTransient::FireParticle(spell) => {
            native_fire_particle_lifetime_ticks(spell.common.id, spell.enhanced_effects)
        }
        PrimarySpellTransient::FireExplosion(_) => NATIVE_FIRE_EXPLOSION_LIFETIME_TICKS,
        PrimarySpellTransient::FireImpact(_) => NATIVE_FIRE_IMPACT_LIFETIME_TICKS,
        // These actors have their own native lifecycle rather than a fixed duration.
        PrimarySpellTransient::Harden(_)
        | PrimarySpellTransient::EarthCalledRock(_)
        | PrimarySpellTransient::AirHurricane(_)
        | PrimarySpellTransient::WaterHail(_)
        | PrimarySpellTransient::Staff(_)
        | PrimarySpellTransient::Weld(_)
        | PrimarySpellTransient::EarthBoulderBit(_)
        | PrimarySpellTransient::FireEmber(_)
        | PrimarySpellTransient::FireGoodImp(_)
        | PrimarySpellTransient::FirePatch(_) => unreachable!("actor has a native step"),
    }
}

pub(super) fn standalone_painter_order(source: &SpellState) -> WorldPainterOrder {
    let mut order = WorldPainterOrder::default();
    let mut include = |registration: PainterRegistration| {
        let next = match registration.manager_lane {
            WorldManagerLane::Actor => &mut order.next_actor_ordinal,
            WorldManagerLane::Transient => &mut order.next_transient_ordinal,
        };
        *next = (*next).max(registration.registration_ordinal + 1);
    };
    for projectile in source.projectiles.iter() {
        include(projectile.common().registration);
    }
    for effect in &source.transients {
        if let Some(registration) = effect.common().light_registration {
            include(registration);
        }
        for &registration in &effect.common().painter_registrations {
            include(registration);
        }
    }
    order
}
