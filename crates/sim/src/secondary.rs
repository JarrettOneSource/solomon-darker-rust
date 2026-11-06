//! Native secondary ability profiles and cast timing from the pinned Website.
//! The same canonical skillbook, equipment lanes and gameplay RNG serve primary spells.

mod actors;
mod casting;
mod context;
mod cosmic;
pub mod dampened;
mod earth;
pub mod electric_burn;
mod elemental;
pub mod ether_drain;
mod events;
mod feedback;
mod fields;
pub mod geometry;
pub mod golem;
pub mod golem_debris;
pub mod leviathan;
mod local_tick;
pub mod motion;
mod player;
pub mod presentation;
mod program;
mod simulation;
mod spawns;
mod status;
mod step;
mod stoneskin;

pub use actors::{
    SecondaryActorKind, SecondaryActorState, SecondaryDamageContact, SecondaryDamageKind,
    SecondaryHeadingMode, SecondaryKnockbackContact, SecondaryLightDisposition, SecondaryTarget,
    SecondaryTargetHeadingChange, enroll_secondary_light_owners, enroll_secondary_painter_owners,
    secondary_light_disposition, secondary_painter_manager_lane,
};
pub use context::{
    SecondaryDampenCandidates, SecondaryDampenPayload, SecondaryDampenProjectileCandidate,
    SecondaryDampenProjectileKind, SecondaryInput, SecondaryPlayerAuthority,
    SecondaryPlayerSnapshot, SecondarySceneryTarget, SecondaryTickContext, SecondaryTickResult,
    SecondaryWorld,
};
pub use events::{
    SecondaryAudioCue, SecondaryEventKind, SecondaryEventState, SecondaryScreenFlashState,
};
pub use feedback::{
    PlayerFlashResponseInput, PlayerMindblastInput, PlayerMindblastTriggerResult,
    PlayerRescueInput, ScriptFire,
};
pub use player::{
    HandCast2, SECONDARY_GLOBAL_COOLDOWN_TICKS, SecondaryCastAction, SecondaryCastStep,
    SecondaryPlayerState, plane_orb_damage, step_secondary_cast_action,
};
pub use simulation::{
    SecondaryCameraDisplacement, SecondaryEtherBurnInput, SecondaryFireBurnInput,
    SecondaryGolemDamageResult, SecondaryPlayerDamageInput, SecondaryPlayerDamagePresentation,
    SecondaryPlayerDamageResult, SecondarySimulationState,
};
pub use status::{
    SecondaryElectricBurnEffectState, SecondaryMovementModifierKind, SecondarySteamedEffectState,
    SecondaryTargetEffectPatch, SecondaryTargetEffectState, apply_secondary_dazzle,
    apply_secondary_target_effect, secondary_target_effect, secondary_target_material_tint,
};

use crate::profile::offensive::{
    OffensiveSpellFactors, resolve_skill_damage_value, resolve_skill_mana_cost_value,
};
use crate::profile::{SKILL_CATALOG, SkillBookState, skill_stat};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SecondaryAbilityId {
    CallLeviathan = 11,
    Planewalker = 12,
    Phasing = 15,
    RingOfFire = 21,
    Firewalker = 23,
    MagicStorm = 27,
    PrismaticShock = 30,
    RingOfIce = 35,
    Earthquake = 41,
    RaiseGolem = 45,
    Stoneskin = 46,
    Teleport = 48,
    MagicCircle = 49,
    MagicTrap = 50,
    Dampen = 51,
    MagicShield = 54,
    AcidRain = 72,
    FireWall = 73,
    EtherDrain = 74,
    CallComet = 76,
    TurnUndead = 77,
    Mindstar = 78,
    Regenerate = 79,
}

impl SecondaryAbilityId {
    pub const ALL: [Self; 23] = [
        Self::CallLeviathan,
        Self::Planewalker,
        Self::Phasing,
        Self::RingOfFire,
        Self::Firewalker,
        Self::MagicStorm,
        Self::PrismaticShock,
        Self::RingOfIce,
        Self::Earthquake,
        Self::RaiseGolem,
        Self::Stoneskin,
        Self::Teleport,
        Self::MagicCircle,
        Self::MagicTrap,
        Self::Dampen,
        Self::MagicShield,
        Self::AcidRain,
        Self::FireWall,
        Self::EtherDrain,
        Self::CallComet,
        Self::TurnUndead,
        Self::Mindstar,
        Self::Regenerate,
    ];

    #[must_use]
    pub const fn skill_id(self) -> usize {
        self as usize
    }

    #[must_use]
    pub fn from_skill_id(skill_id: usize) -> Option<Self> {
        Self::ALL.into_iter().find(|ability| ability.skill_id() == skill_id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SecondaryStatValue {
    pub name: &'static str,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SecondaryAbilityRankStats {
    pub rank: u32,
    pub ability: SecondaryAbilityId,
    pub values: Vec<SecondaryStatValue>,
}

impl SecondaryAbilityRankStats {
    #[must_use]
    pub fn value(&self, property: &str) -> Option<f64> {
        self.values.iter().find(|entry| entry.name == property).map(|entry| entry.value)
    }
}

/// Every authored secondary stat except the two rank-limit metadata properties.
#[must_use]
pub fn effective_secondary_ability_rank_stats(
    skill_book: &SkillBookState,
    ability: SecondaryAbilityId,
) -> SecondaryAbilityRankStats {
    secondary_ability_rank_stats(ability, skill_book.effective_ranks[ability.skill_id()])
}

/// `nativeSecondaryAbilityRankStats` admits Revelation's rank two even for a
/// normally single-rank ability. Short authored arrays retain their final row.
#[must_use]
pub fn secondary_ability_rank_stats(
    ability: SecondaryAbilityId,
    rank: u32,
) -> SecondaryAbilityRankStats {
    let skill_id = ability.skill_id();
    let definition = &SKILL_CATALOG[skill_id];
    assert!(
        rank > 0 && rank <= definition.maximum_level.max(2),
        "secondary ability has a valid learned rank"
    );
    let mut values = Vec::new();
    for property in definition.numeric_properties {
        if property.name == "mCapLevel" || property.name == "mMaxLevel" {
            continue;
        }
        let value = property.value.at_rank(rank).expect("authored secondary stat has a rank");
        assert!(value.is_finite(), "authored secondary stat is finite");
        values.push(SecondaryStatValue { name: property.name, value });
    }
    SecondaryAbilityRankStats { rank, ability, values }
}

#[must_use]
pub fn resolved_secondary_ability_rank_stats(
    skill_book: &SkillBookState,
    ability: SecondaryAbilityId,
    offensive_factors: &OffensiveSpellFactors,
) -> SecondaryAbilityRankStats {
    let mut stats = effective_secondary_ability_rank_stats(skill_book, ability);
    // Source keeps Leviathan's ranked base on the retained parent. Each bolt
    // resolves the live caster's modifiers when that appendage actually fires.
    if ability == SecondaryAbilityId::CallLeviathan {
        return stats;
    }
    for property in &mut stats.values {
        if matches!(property.name, "mDamage" | "mDamage1" | "mDamage2") {
            property.value =
                resolve_skill_damage_value(ability.skill_id(), property.value, offensive_factors);
        }
    }
    stats
}

/// Raw cost additions are derived by the skill runtime before offensive/equipment lanes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SecondaryManaCostBonuses {
    pub explosive_shield: f64,
    pub golem: f64,
    pub magic_storm: f64,
}

#[must_use]
pub fn secondary_ability_mana_cost(
    skill_book: &SkillBookState,
    ability: SecondaryAbilityId,
    offensive_factors: &OffensiveSpellFactors,
    bonuses: SecondaryManaCostBonuses,
) -> f64 {
    let stats = resolved_secondary_ability_rank_stats(skill_book, ability, offensive_factors);
    let mut cost = stats.value("mManaCost").unwrap_or(0.0);
    cost += match ability {
        SecondaryAbilityId::MagicStorm => bonuses.magic_storm,
        SecondaryAbilityId::RaiseGolem => bonuses.golem,
        SecondaryAbilityId::MagicShield => bonuses.explosive_shield,
        _ => 0.0,
    };
    resolve_skill_mana_cost_value(ability.skill_id(), cost, offensive_factors)
}

/// The actual snapshot/authority supplement uses the player's stat book,
/// retaining rank-zero upgrade costs (`game-snapshot.ts` and `game-simulation.ts`).
/// Base secondary rank tables intentionally remain the source shared catalog.
#[must_use]
pub fn secondary_ability_mana_cost_with_stat_book(
    skill_book: &SkillBookState,
    stat_book: &crate::profile::PlayerStatBookComponent,
    ability: SecondaryAbilityId,
    offensive_factors: &OffensiveSpellFactors,
) -> f64 {
    let ranked =
        |id, property| stat_book.stat(id, property, skill_book.effective_ranks[id]).unwrap_or(0.0);
    secondary_ability_mana_cost(
        skill_book,
        ability,
        offensive_factors,
        SecondaryManaCostBonuses {
            explosive_shield: ranked(55, "mManaCost"),
            golem: ranked(75, "mManaCost"),
            magic_storm: ranked(28, "mManaCost"),
        },
    )
}

#[must_use]
pub fn secondary_cooldown_capacity_ticks(
    skill_book: &SkillBookState,
    ability: SecondaryAbilityId,
) -> f64 {
    use SecondaryAbilityId::*;
    match ability {
        Phasing | Teleport => {
            (effective_skill_value(skill_book, ability.skill_id(), "mCooldown") * 100.0 + 0.5)
                .floor()
        }
        CallLeviathan => 833.0,
        Planewalker | RingOfFire | RingOfIce | Earthquake | RaiseGolem | MagicCircle
        | MagicShield | AcidRain => 2500.0,
        MagicTrap => 625.0,
        Firewalker | Mindstar | Regenerate => 50.0,
        MagicStorm | PrismaticShock | CallComet => 1250.0,
        Stoneskin => 10000.0,
        Dampen => 2000.0,
        FireWall => 277.0,
        EtherDrain => 3750.0,
        TurnUndead => 1875.0,
    }
}

fn effective_skill_value(skill_book: &SkillBookState, skill_id: usize, property: &str) -> f64 {
    let rank = skill_book.effective_ranks[skill_id];
    if rank == 0 {
        return 0.0;
    }
    skill_stat(skill_id, property, rank).unwrap_or(0.0)
}
