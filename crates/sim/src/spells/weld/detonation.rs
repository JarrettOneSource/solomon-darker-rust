//! Materialize the authored Weld Fire/Steam payloads through the canonical Fire
//! factory. Direct Weld damage remains owned by its projectile/contact phase.

use super::*;
use crate::Rng;
use crate::painter::{WorldManagerLane, WorldPainterOrder};
use crate::profile::WeldBuildId;
use crate::profile::primary::SpentEmber;
use crate::spells::effects::{PrimarySpellFireDetonation, fire_explosion};
use crate::spells::fire::{FireProjectilePayload, create_fire_detonation};
use crate::spells::model::PrimarySpellTransient;

pub fn create_primary_spell_weld_fire_detonation(
    birth: WeldActorBirth<'_>,
    rng: &mut Rng,
    private_seed: u32,
    include_impact: bool,
    order: &mut WorldPainterOrder,
) -> PrimarySpellFireDetonation {
    let offset = match birth.build_id {
        WeldBuildId::BurningBolt | WeldBuildId::MeteorSwarm => 5,
        WeldBuildId::FlameLash => 4,
        _ => panic!("Weld build has no authored Fire payload"),
    };
    let value = |index| birth.vector.get(index).copied().unwrap_or(0.0);
    let payload = FireProjectilePayload {
        burn_damage: 0.0,
        ember_damage: value(offset + 2),
        ember_fragments: value(offset + 3).round().max(0.0) as u32,
        explode_damage: value(offset),
        explode_radius: value(offset + 1),
        private_seed: private_seed as i32,
        spent_ember: SpentEmber::None,
    };
    let mut transients = Vec::new();
    if include_impact {
        transients.push(PrimarySpellTransient::Weld(WeldWorldActor::Impact(
            create_primary_spell_weld_impact(birth, rng),
        )));
    }
    let first_effect_id = birth.id + u64::from(include_impact);
    let has_explosion = payload.explode_radius > 0.0 && payload.explode_damage > 0.0;
    let detonation = create_fire_detonation(
        first_effect_id + u64::from(has_explosion),
        payload,
        birth.origin,
        birth.owner_id,
        birth.world_key,
        rng,
    );
    if let Some(explosion) = detonation.explosion {
        transients.push(PrimarySpellTransient::FireExplosion(fire_explosion(
            first_effect_id,
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

/// The effects a Weld Steam detonation adds to the spell queue, and the next free ID.
pub struct WeldSteamSpellDetonation {
    pub next_id: u64,
    pub transients: Vec<PrimarySpellTransient>,
}

pub fn create_primary_spell_weld_steam_detonation(
    input: WeldSteamDetonationInput<'_>,
    rng: &mut Rng,
    order: &mut WorldPainterOrder,
) -> WeldSteamSpellDetonation {
    let source_next_id = input.first_fragment_id - 1;
    let detonation = create_weld_steam_detonation(input, rng);
    let Some(steam) = detonation.explosion else {
        return WeldSteamSpellDetonation { next_id: source_next_id, transients: Vec::new() };
    };
    let mut transients = vec![PrimarySpellTransient::FireExplosion(fire_explosion(
        source_next_id,
        steam.explosion,
        steam.sound_pitch,
        order,
    ))];
    for mut fragment in detonation.fragments {
        fragment.common.owner.painter_registrations.push(order.register(WorldManagerLane::Actor));
        transients.push(PrimarySpellTransient::Weld(WeldWorldActor::Steam(fragment)));
    }
    WeldSteamSpellDetonation { next_id: detonation.next_id, transients }
}
