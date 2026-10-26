//! Source `ml-bot-policy/own-effects.ts`: admitted effect rows and belt activity.

use crate::Vec2;
use crate::float::hypot;
use crate::secondary::{NativeSecondaryActorKind as SecondaryKind, NativeSecondarySimulationState};
use crate::spells::model::{
    PrimaryProjectileRef, PrimarySpellProjectilePhase, PrimarySpellTransient, SpellState,
};
use crate::spells::weld::{
    NativeWeldHailChildActorState, NativeWeldPersistentActorState, NativeWeldWorldActor,
};

use super::numbers::{normalized, signed, unsigned, write};
use super::spec::scales;

#[derive(Clone, Debug)]
pub struct MlBotPolicyOwnEffectObservation {
    pub block_r: Vec<f32>,
    pub primary_effect_active: bool,
    pub secondary_effect_active: [bool; 8],
}

pub fn observe_ml_bot_policy_own_effects(
    primary: &SpellState,
    secondary: &NativeSecondarySimulationState,
    player_id: &str,
    position: Vec2,
    quickbar: &[Option<usize>; 8],
    world_key: &str,
) -> MlBotPolicyOwnEffectObservation {
    let mut rows = Vec::new();
    for projectile in primary.projectiles.iter() {
        let common = projectile.common();
        if common.owner_id != player_id || common.world_key != world_key {
            continue;
        }
        let (target, radius) = match projectile {
            PrimaryProjectileRef::Ether(state) => (state.target_id.is_some(), 0.0),
            PrimaryProjectileRef::Weld(state) => (state.target_id.is_some(), 0.0),
            PrimaryProjectileRef::Fire(state) => (false, state.explode_radius),
            PrimaryProjectileRef::Earth(_) => (false, 0.0),
        };
        rows.push(EffectRow {
            damage: common.damage,
            family: Family::Projectile,
            target,
            held: common.phase == PrimarySpellProjectilePhase::Held,
            id: common.id,
            position: common.position,
            radius,
            remaining_seconds: scales::EFFECT_LIFETIME_SECONDS,
            primary: true,
            slot: None,
            velocity: common.velocity,
        });
    }
    for transient in &primary.transients {
        let common = transient.common();
        if common.owner_id != player_id || common.world_key != world_key {
            continue;
        }
        if let Some(row) = primary_transient_row(transient) {
            rows.push(row);
        }
    }
    for actor in secondary.actors.iter() {
        if actor.owner_id != player_id
            || actor.world_key != world_key
            || !secondary_effect(actor.kind)
        {
            continue;
        }
        let slot =
            actor.skill_id.and_then(|id| quickbar.iter().position(|skill| *skill == Some(id)));
        rows.push(EffectRow {
            damage: actor.damage,
            family: if matches!(
                actor.kind,
                SecondaryKind::Comet
                    | SecondaryKind::EtherBolt
                    | SecondaryKind::MovingFire
                    | SecondaryKind::PlaneOrbShot
            ) {
                Family::Projectile
            } else {
                Family::Area
            },
            target: actor.target_id.is_some(),
            held: false,
            id: actor.id,
            position: actor.position,
            radius: actor.radius.max(0.0),
            remaining_seconds: if actor.lifetime_ticks > 0 {
                actor.lifetime_ticks.saturating_sub(actor.age_ticks) as f64 / scales::TICK_RATE
            } else {
                scales::EFFECT_LIFETIME_SECONDS
            },
            primary: false,
            slot,
            velocity: actor.velocity,
        });
    }
    rows.sort_by(|left, right| {
        edge_distance(left, position)
            .total_cmp(&edge_distance(right, position))
            .then(right.primary.cmp(&left.primary))
            .then(left.id.cmp(&right.id))
    });
    let mut block_r = vec![0.0; 6 * 23 + 3];
    for (slot, row) in rows.iter().take(6).enumerate() {
        let start = slot * 23;
        let direction = normalized(row.position.x - position.x, row.position.y - position.y);
        block_r[start] = 1.0;
        block_r[start + 1] = f32::from(row.primary);
        if let Some(slot) = row.slot {
            block_r[start + 2 + slot] = 1.0;
        }
        write(
            &mut block_r,
            start + 10,
            &[
                f64::from(row.family == Family::Projectile),
                f64::from(row.family == Family::Area),
                f64::from(row.family == Family::Channel),
                direction.x,
                direction.y,
                unsigned(edge_distance(row, position), scales::RANGE),
                signed(row.velocity.x, scales::VELOCITY),
                signed(row.velocity.y, scales::VELOCITY),
                unsigned(row.radius, scales::RADIUS),
                unsigned(row.remaining_seconds, scales::EFFECT_LIFETIME_SECONDS),
                unsigned(row.damage, scales::SKILL_DAMAGE),
                f64::from(row.held),
                f64::from(row.target),
            ],
        );
    }
    let mut secondary_effect_active = [false; 8];
    let mut primary_effect_active = false;
    let mut projectiles = 0;
    let mut areas = 0;
    for row in &rows {
        primary_effect_active |= row.primary;
        if let Some(slot) = row.slot {
            secondary_effect_active[slot] = true;
        }
        projectiles += usize::from(row.family == Family::Projectile);
        areas += usize::from(row.family == Family::Area);
    }
    write(
        &mut block_r,
        138,
        &[
            unsigned(rows.len() as f64, scales::OWN_EFFECT_COUNT),
            unsigned(projectiles as f64, scales::OWN_EFFECT_COUNT),
            unsigned(areas as f64, scales::OWN_EFFECT_COUNT),
        ],
    );
    MlBotPolicyOwnEffectObservation { block_r, primary_effect_active, secondary_effect_active }
}

#[derive(Clone, Copy, PartialEq)]
enum Family {
    Area,
    Channel,
    Projectile,
}

struct EffectRow {
    damage: f64,
    family: Family,
    target: bool,
    held: bool,
    id: u64,
    position: Vec2,
    radius: f64,
    remaining_seconds: f64,
    primary: bool,
    slot: Option<usize>,
    velocity: Vec2,
}

fn primary_transient_row(transient: &PrimarySpellTransient) -> Option<EffectRow> {
    let common = transient.common();
    let mut row = EffectRow {
        damage: 0.0,
        family: Family::Area,
        target: false,
        held: false,
        id: common.id,
        position: Vec2::ZERO,
        radius: 0.0,
        remaining_seconds: scales::EFFECT_LIFETIME_SECONDS,
        primary: true,
        slot: None,
        velocity: Vec2::ZERO,
    };
    match transient {
        PrimarySpellTransient::Air(state) => {
            row.family = Family::Channel;
            row.position = state.midpoint;
            row.target = state.target_id.is_some();
        }
        PrimarySpellTransient::Water(state) => {
            row.family = Family::Channel;
            row.position = state.origin;
        }
        PrimarySpellTransient::AirHurricane(state) => {
            row.damage = state.damage_maximum;
            row.position = state.position;
        }
        PrimarySpellTransient::WaterHail(state) => {
            row.position = state.position;
            row.velocity = state.horizontal_velocity;
        }
        PrimarySpellTransient::EtherBlast(state) => row.position = state.origin,
        PrimarySpellTransient::FireEmber(state) => {
            row.damage = state.damage;
            row.position = state.position;
            row.velocity = state.horizontal_velocity;
        }
        PrimarySpellTransient::FireExplosion(state) => {
            row.damage = state.damage;
            row.position = state.origin;
        }
        PrimarySpellTransient::FirePatch(state) => {
            row.damage = state.damage;
            row.position = state.position;
            row.velocity = state.velocity;
        }
        PrimarySpellTransient::Weld(state) => {
            row.position = state.common().origin;
            match state {
                NativeWeldWorldActor::Channel(state) => {
                    row.family = Family::Channel;
                    row.position = state.midpoint.unwrap_or(state.common.origin);
                    row.target = state.target_id.is_some();
                }
                NativeWeldWorldActor::Persistent(state) => match state {
                    NativeWeldPersistentActorState::EtherealBoulder(state) => {
                        row.damage = state.damage;
                        row.held = state.phase == PrimarySpellProjectilePhase::Held;
                        row.velocity = state.velocity;
                    }
                    NativeWeldPersistentActorState::Hailstones(state) => {
                        row.damage = state.damage;
                        row.radius = state.collision_radius;
                        row.held = state.phase == PrimarySpellProjectilePhase::Held;
                    }
                    NativeWeldPersistentActorState::MeteorField(_) => row.held = true,
                },
                NativeWeldWorldActor::Meteor(state) => {
                    row.damage = state.damage;
                    row.position = state.position;
                }
                NativeWeldWorldActor::Steam(state) => {
                    row.position = state.position;
                    row.velocity = state.velocity;
                }
                NativeWeldWorldActor::HailChild(NativeWeldHailChildActorState::Knockback(_)) => {
                    row.target = true
                }
                _ => return None,
            }
        }
        // The Source's explicit PRIMARY_EFFECT_KINDS set excludes these real
        // presentation rows, including Water aura and the friendly Fire Imp.
        _ => return None,
    }
    Some(row)
}

fn edge_distance(row: &EffectRow, observer: Vec2) -> f64 {
    (hypot(row.position.x - observer.x, row.position.y - observer.y) - row.radius).max(0.0)
}

fn secondary_effect(kind: SecondaryKind) -> bool {
    use SecondaryKind::*;
    // Exhaustive classification from Source closed-unions.ts. Presentation,
    // minion and status-carrier rows never activate a policy ability slot.
    match kind {
        AcidRain | Comet | DampenWave | Earthquake | EtherBolt | EtherDrain | FirePatch
        | FreezeWave | IceBlast | Leviathan | LeviathanAppendage | MagicCircle | MagicTrap
        | MagicTrapBurst | MindblastBurst | MindblastShockwave | RescueShockwave | MovingFire
        | PlaneOrbShot | PrismaticWave | RingFireExplosion | ShieldExplosion | Shockwave
        | StormCloud | StormStrike | TurnUndead => true,
        AcidDrop
        | AcidSplash
        | CometDebris
        | CometImpact
        | CometTrail
        | DampenedProjectile
        | DampenedSmoke
        | EarthquakeDebris
        | EarthquakeDust
        | EarthquakeQuake
        | EarthquakeSceneryWobble
        | ElectricBurn
        | ElectricBurnFlare
        | ElectricBurnArc
        | EtherBurn
        | EtherBurnFlare
        | EtherDrainCaptureFlare
        | EtherDrainCloud
        | EtherDrainDebris
        | EtherFade
        | FireBurn
        | FireBurnFlame
        | FlashResponseFade
        | FlashResponseGrow
        | FreezeWaveVisual
        | FrostBurnFlare
        | Golem
        | GolemAssemblyDebris
        | GolemDeath
        | GolemKnockback
        | LeviathanMote
        | MagicCirclePlayerFlash
        | MagicTrapShimmer
        | PhaseBurst
        | PlaneOrbParticle
        | PlanewalkerMote
        | RingFireFragment
        | ShieldBreak
        | StormDrop
        | StoneskinChip
        | TeleportBurst => false,
    }
}
