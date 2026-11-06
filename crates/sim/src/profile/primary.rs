//! Cast-time primary spell payloads for each element and Weld, the spell colors, and the
//! Inventory's spell name, units and summary.
//!
//! The retained stat catalog supplies authored ranks. Rounding remains visible where Water
//! geometry and weld vectors are narrowed. The differential fixture covers every elemental
//! branch, Fire's exclusive ember outcome, and all ten welded builds.

use crate::float::fround;

use super::offensive::{
    OffensiveSpellFactors, resolve_skill_damage_value, resolve_skill_mana_cost_value,
};
use super::weld::{WeldCastKind, WeldPrimaryVector, weld_primary_vector_with_stat_book};
use super::{
    PlayerStatBookComponent, SKILL_CATALOG, SPELL_WELDING_SKILL_ID, SkillBookState, WeldBuildId,
    weld_build,
};

pub const PRIMARY_EARTH_INITIAL_CHARGE: f64 = 0.18_f32 as f64;

/// Unrounded color channels of the eight skill roots. Different consumers apply their own
/// authored luminance mix or float32 narrowing before making tint bytes.
pub const SKILL_ROOT_COLORS: [[f64; 3]; 8] = [
    [1.0, 0.1, 1.0],
    [1.0, 0.35, 0.1],
    [0.1, 1.0, 1.0],
    [0.1, 0.5, 1.0],
    [0.1, 1.0, 0.1],
    [1.0, 0.5, 0.1],
    [0.1, 0.5, 0.5],
    [0.75, 0.75, 0.75],
];

/// The selected spell's color, which Spider's captured wizard outline uses independent of
/// clothing dye. Each byte narrows its channel to float32, then truncates. The Weld color
/// table has fifteen rows, five beyond the ten builds, so it is indexed by the build number.
#[must_use]
pub fn primary_spell_tint(skill_id: usize, weld_build_id: Option<u32>) -> u32 {
    const WELD_COLORS: [[f64; 3]; 15] = [
        [1.0, 0.1, 0.5],
        [1.0, 0.5, 1.0],
        [1.0, 0.75, 1.0],
        [1.0, 0.75, 0.5],
        [1.0, 0.75, 1.0],
        [0.75, 0.75, 0.75],
        [1.0, 0.75, 1.0],
        [1.0, 0.75, 0.5],
        [0.8, 1.0, 1.0],
        [0.9, 1.0, 1.0],
        [1.0, 0.1, 0.5],
        [1.0, 0.35, 0.1],
        [0.1, 0.5, 1.0],
        [0.1, 1.0, 1.0],
        [0.1, 1.0, 0.1],
    ];
    let root = if skill_id < 8 { Some(skill_id) } else { super::skill_root(skill_id) };
    let weld = if skill_id == SPELL_WELDING_SKILL_ID {
        weld_build_id
            .and_then(|build| build.checked_sub(1000))
            .and_then(|index| WELD_COLORS.get(index as usize))
    } else {
        None
    };
    let Some(color) = weld.or_else(|| root.and_then(|root| SKILL_ROOT_COLORS.get(root))) else {
        return 0xffffff;
    };
    let channel = |value| (fround(value) * 255.0).trunc() as u32;
    channel(color[0]) << 16 | channel(color[1]) << 8 | channel(color[2])
}

/// The damage roll, mana cost and rank that every primary spell payload shares.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrimarySkillProfileBase {
    pub damage_maximum: f64,
    pub damage_minimum: f64,
    pub damage_roll_count: f64,
    pub mana_cost: f64,
    pub rank: u32,
    pub skill_id: u32,
}

/// Ether's cast payload: bolt quantity, piercing, damage retention, retargeting, speed and
/// blast charges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EtherPrimarySkillProfile {
    pub common: PrimarySkillProfileBase,
    pub blast_charge_capacity: u32,
    pub damage_retention: f64,
    pub pierces: u32,
    pub quantity: u32,
    pub reacquires_target: bool,
    pub speed_factor: f64,
}

/// Fire selects Imps before Immolate when both upgrades are present.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SpentEmber {
    None,
    Immolate { damage: f64 },
    Imp { damage: f64, lifetime_ticks: u64 },
}

/// Fire's cast payload: burn, ember and explosion damage, and what a spent ember becomes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FirePrimarySkillProfile {
    pub common: PrimarySkillProfileBase,
    pub burn_damage: f64,
    pub ember_damage: f64,
    pub ember_fragments: u32,
    pub explode_damage: f64,
    pub explode_radius: f64,
    pub spent_ember: SpentEmber,
}

/// Air's cast payload: arcs, disintegration chance, Hurricane damage and stun slowdown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirPrimarySkillProfile {
    pub common: PrimarySkillProfileBase,
    pub arc_count: u32,
    pub disintegrate_chance: f64,
    pub hurricane_damage_maximum: f64,
    pub hurricane_damage_minimum: f64,
    pub stun_movement_factor: f64,
}

/// Water's cast payload: cone geometry, cold and slow effects, hail, armor and pushback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterPrimarySkillProfile {
    pub common: PrimarySkillProfileBase,
    pub armor_maximum: f64,
    pub armor_per_second: f64,
    pub aura_radius_scale: f64,
    pub aura_movement_factor: f64,
    pub aura_slow_factor: f64,
    pub cold_duration_ticks: u64,
    pub cold_movement_factor: f64,
    pub hail_chance: f64,
    pub hail_damage_maximum: f64,
    pub hail_damage_minimum: f64,
    pub hail_threshold: u32,
    pub half_angle_degrees: f64,
    pub minimum_cold_duration_ticks: u64,
    pub pushback_factor: f64,
    pub reach: f64,
    pub slowdown_scale: f64,
    pub widen_half_degrees: f64,
}

/// Earth's cast payload: boulder growth and maximum charge, Rock Surge and toughness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarthPrimarySkillProfile {
    pub common: PrimarySkillProfileBase,
    pub growth_factor: f64,
    pub maximum_charge: f64,
    pub rock_surge_chance: f64,
    pub rock_surge_mana_cost: f64,
    pub toughness: f64,
}

/// A welded primary's cast payload: its build, cast kind, damage factor and weld vector.
#[derive(Clone, Debug, PartialEq)]
pub struct WeldPrimarySkillProfile {
    pub common: PrimarySkillProfileBase,
    pub build_id: WeldBuildId,
    pub cast_kind: WeldCastKind,
    pub damage_factor: f64,
    pub vector: WeldPrimaryVector,
}

/// The complete cast payload of the selected primary spell, by element or Weld.
#[derive(Clone, Debug, PartialEq)]
pub enum PrimarySkillProfile {
    Ether(EtherPrimarySkillProfile),
    Fire(FirePrimarySkillProfile),
    Air(AirPrimarySkillProfile),
    Water(WaterPrimarySkillProfile),
    Earth(EarthPrimarySkillProfile),
    Weld(WeldPrimarySkillProfile),
}

impl PrimarySkillProfile {
    #[must_use]
    pub fn common(&self) -> &PrimarySkillProfileBase {
        match self {
            Self::Ether(profile) => &profile.common,
            Self::Fire(profile) => &profile.common,
            Self::Air(profile) => &profile.common,
            Self::Water(profile) => &profile.common,
            Self::Earth(profile) => &profile.common,
            Self::Weld(profile) => &profile.common,
        }
    }
}

/// Whether an elemental primary fires once per cast or channels continuously.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimaryCastMode {
    Continuous,
    OneShot,
}

#[must_use]
pub fn primary_cast_mode(skill_id: usize) -> PrimaryCastMode {
    match skill_id {
        8 | 16 => PrimaryCastMode::OneShot,
        24 | 32 | 40 => PrimaryCastMode::Continuous,
        _ => panic!("skill {skill_id} is not an elemental primary"),
    }
}

/// Resolves the complete immutable payload used by one cast or channel.
#[must_use]
pub fn primary_skill_profile(
    book: &SkillBookState,
    factors: &OffensiveSpellFactors,
) -> PrimarySkillProfile {
    primary_skill_profile_with_stat_book(book, &PlayerStatBookComponent::default(), factors)
}

#[must_use]
pub fn primary_skill_profile_with_stat_book(
    book: &SkillBookState,
    stat_book: &PlayerStatBookComponent,
    factors: &OffensiveSpellFactors,
) -> PrimarySkillProfile {
    factors.validate();
    let skill_id = book.primary_skill_id;
    if skill_id == SPELL_WELDING_SKILL_ID {
        return PrimarySkillProfile::Weld(weld_profile(book, stat_book, factors));
    }
    assert!(
        [8, 16, 24, 32, 40].contains(&skill_id),
        "skill {skill_id} is not an elemental primary"
    );
    let rank = book.effective_ranks[skill_id];
    assert!(rank > 0, "primary skill {skill_id} must be learned");
    let raw_minimum = damage_value(stat_book, skill_id, rank, false);
    let raw_maximum = damage_value(stat_book, skill_id, rank, true);
    let common = PrimarySkillProfileBase {
        damage_minimum: resolve_skill_damage_value(skill_id, raw_minimum, factors),
        damage_maximum: resolve_skill_damage_value(skill_id, raw_maximum, factors),
        damage_roll_count: raw_maximum - raw_minimum + 1.0,
        mana_cost: resolve_skill_mana_cost_value(
            skill_id,
            primary_mana_cost(stat_book, book, skill_id),
            factors,
        ),
        rank,
        skill_id: skill_id as u32,
    };

    match skill_id {
        8 => {
            let speed_factor = 1.0 + ranked_or(stat_book, book, 9, "mSpeed", 0.0) / 100.0;
            let damage_retention = if book.effective_ranks[13] > 0 {
                1.0 - ranked(stat_book, 13, "mLoss", book.effective_ranks[13]) / 100.0
            } else {
                1.0
            };
            PrimarySkillProfile::Ether(EtherPrimarySkillProfile {
                common,
                blast_charge_capacity: ranked_or(stat_book, book, 14, "mCharges", 0.0).round()
                    as u32,
                damage_retention,
                pierces: ranked_or(stat_book, book, 13, "mPierces", 0.0).round() as u32,
                quantity: ranked_or(stat_book, book, 10, "mQuantity", 1.0).round() as u32,
                reacquires_target: speed_factor > 1.01_f32 as f64,
                speed_factor,
            })
        }
        16 => {
            let spent_ember = if book.effective_ranks[19] > 0 {
                SpentEmber::Imp {
                    damage: ranked(stat_book, 19, "mDamage", book.effective_ranks[19])
                        * factors.damage,
                    lifetime_ticks: 300,
                }
            } else if book.effective_ranks[20] > 0 {
                SpentEmber::Immolate {
                    damage: ranked(stat_book, 20, "mDamage", book.effective_ranks[20])
                        * factors.damage,
                }
            } else {
                SpentEmber::None
            };
            PrimarySkillProfile::Fire(FirePrimarySkillProfile {
                common,
                burn_damage: ranked_or(stat_book, book, 22, "mDamage", 0.0) * factors.damage,
                ember_damage: ranked_or(stat_book, book, 17, "mDamage", 0.0) * factors.damage,
                ember_fragments: ranked_or(stat_book, book, 17, "mFragments", 0.0).round() as u32,
                explode_damage: ranked_or(stat_book, book, 18, "mDamage", 0.0) * factors.damage,
                explode_radius: ranked_or(stat_book, book, 18, "mRadius", 0.0),
                spent_ember,
            })
        }
        24 => PrimarySkillProfile::Air(AirPrimarySkillProfile {
            common,
            arc_count: ranked_or(stat_book, book, 25, "mArcs", 0.0).round() as u32,
            disintegrate_chance: ranked_or(stat_book, book, 31, "mChance", 0.0),
            hurricane_damage_maximum: ranked_or(stat_book, book, 29, "mDamage2", 0.0)
                * factors.damage,
            hurricane_damage_minimum: ranked_or(stat_book, book, 29, "mDamage1", 0.0)
                * factors.damage,
            stun_movement_factor: 1.0 - ranked_or(stat_book, book, 26, "mStunAmount", 0.0) / 100.0,
        }),
        32 => {
            let widen_half_degrees = fround(ranked_or(stat_book, book, 34, "mWiden", 0.0) * 0.5);
            let pushback_factor =
                fround(ranked_or(stat_book, book, 33, "mPushback", 0.0) * (0.01_f32 as f64));
            let slowdown_scale =
                fround(1.0 + ranked_or(stat_book, book, 39, "mSlowdown", 0.0) / 100.0);
            let minimum_cold_duration_ticks = if book.effective_ranks[39] > 0 { 200 } else { 0 };
            let aura_slow_factor =
                fround(1.0 - ranked_or(stat_book, book, 37, "mPercent", 0.0) / 100.0);
            let hail_chance = ranked_or(stat_book, book, 38, "mToHit", 0.0);
            PrimarySkillProfile::Water(WaterPrimarySkillProfile {
                common,
                armor_maximum: ranked_or(stat_book, book, 36, "mMaxArmor", 0.0),
                armor_per_second: ranked_or(stat_book, book, 36, "mArmorPlus", 0.0),
                aura_radius_scale: crate::cold_aura::cold_aura_radius_scale(ranked_or(
                    stat_book, book, 37, "mRadius", 0.0,
                )),
                aura_movement_factor: fround(aura_slow_factor / slowdown_scale),
                aura_slow_factor,
                cold_duration_ticks: 25.max(minimum_cold_duration_ticks),
                cold_movement_factor: fround(fround(0.5) / slowdown_scale),
                hail_chance,
                hail_damage_maximum: ranked_or(stat_book, book, 38, "mDamage2", 0.0)
                    * factors.damage,
                hail_damage_minimum: ranked_or(stat_book, book, 38, "mDamage1", 0.0)
                    * factors.damage,
                hail_threshold: fround(hail_chance).trunc() as u32,
                half_angle_degrees: 15.0 + widen_half_degrees * 0.5,
                minimum_cold_duration_ticks,
                pushback_factor,
                reach: 205.0 + 4.0 * widen_half_degrees,
                slowdown_scale,
                widen_half_degrees,
            })
        }
        40 => {
            let toughness = if book.effective_ranks[43] > 0 {
                ranked(stat_book, 43, "mStrength", book.effective_ranks[43]) / 100.0
            } else {
                1.0
            };
            PrimarySkillProfile::Earth(EarthPrimarySkillProfile {
                common,
                growth_factor: 1.0 + ranked_or(stat_book, book, 42, "mSpeedUp", 0.0) / 100.0,
                maximum_charge: fround(1.0 + ranked_or(stat_book, book, 47, "mSize", 0.0) / 100.0),
                rock_surge_chance: ranked_or(stat_book, book, 44, "mChance", 0.0),
                rock_surge_mana_cost: ranked_or(stat_book, book, 44, "mManaCost", 0.0)
                    * factors.mana_cost,
                toughness,
            })
        }
        _ => unreachable!("elemental primary ids were checked above"),
    }
}

fn weld_profile(
    book: &SkillBookState,
    stat_book: &PlayerStatBookComponent,
    factors: &OffensiveSpellFactors,
) -> WeldPrimarySkillProfile {
    let build_id = book.weld_build_id.expect("Spell Welding has an active native build");
    let weld_effect = factors.equipment.as_ref().map_or(1.0, |equipment| equipment.weld_effect);
    let vector = weld_primary_vector_with_stat_book(book, stat_book, build_id, weld_effect);
    let rank = book.effective_ranks[SPELL_WELDING_SKILL_ID];
    assert!(rank > 0, "Spell Welding must be learned");
    let maximum_index = match build_id {
        WeldBuildId::BurningBolt
        | WeldBuildId::FrostMissile
        | WeldBuildId::BallLightning
        | WeldBuildId::MeteorSwarm => 1,
        _ => 0,
    };
    let mana_index =
        if vector.cast_kind == WeldCastKind::OneShot && build_id != WeldBuildId::CrawlingShock {
            2
        } else {
            1
        };
    WeldPrimarySkillProfile {
        common: PrimarySkillProfileBase {
            damage_minimum: resolve_skill_damage_value(
                SPELL_WELDING_SKILL_ID,
                vector.values[0],
                factors,
            ),
            damage_maximum: resolve_skill_damage_value(
                SPELL_WELDING_SKILL_ID,
                vector.values[maximum_index],
                factors,
            ),
            damage_roll_count: 1.0,
            mana_cost: resolve_skill_mana_cost_value(
                SPELL_WELDING_SKILL_ID,
                vector.values[mana_index],
                factors,
            ),
            rank,
            skill_id: build_id.id(),
        },
        build_id,
        cast_kind: vector.cast_kind,
        damage_factor: factors.damage,
        vector,
    }
}

/// The unit the Inventory shows a primary spell's damage in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimarySpellDamageUnit {
    Bolt,
    Boulder,
    Impact,
    Rock,
    Second,
}

/// Whether the Inventory shows a primary spell's mana cost per cast or per second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimarySpellManaUnit {
    Cast,
    Second,
}

/// The Inventory's name and units for the selected primary spell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrimarySpellDescriptor {
    pub damage_range: bool,
    pub damage_unit: PrimarySpellDamageUnit,
    pub mana_unit: PrimarySpellManaUnit,
    pub name: &'static str,
}

/// Inventory's authored spell name and units, including all ten welded builds.
#[must_use]
pub fn primary_spell_descriptor(
    selected_primary_skill_id: usize,
    weld_build_id: Option<WeldBuildId>,
) -> PrimarySpellDescriptor {
    use PrimarySpellDamageUnit::{Bolt, Boulder, Impact, Rock, Second};
    use PrimarySpellManaUnit::Cast;

    let (name, damage_range, damage_unit, mana_unit) = match selected_primary_skill_id {
        8 => (SKILL_CATALOG[8].name, true, Bolt, Cast),
        16 => (SKILL_CATALOG[16].name, false, Bolt, Cast),
        24 | 32 => (
            SKILL_CATALOG[selected_primary_skill_id].name,
            false,
            Second,
            PrimarySpellManaUnit::Second,
        ),
        40 => (SKILL_CATALOG[40].name, true, Boulder, PrimarySpellManaUnit::Second),
        SPELL_WELDING_SKILL_ID => {
            let id = weld_build_id.expect("selected Spell Welding has a native build");
            let (damage_range, damage_unit, mana_unit) = match id {
                WeldBuildId::BurningBolt
                | WeldBuildId::FrostMissile
                | WeldBuildId::BallLightning => (true, Bolt, Cast),
                WeldBuildId::FlameLash | WeldBuildId::BlizzardBeam | WeldBuildId::SteamJet => {
                    (false, Second, PrimarySpellManaUnit::Second)
                }
                WeldBuildId::EtherealBoulder => (true, Boulder, PrimarySpellManaUnit::Second),
                WeldBuildId::MeteorSwarm => (true, Impact, PrimarySpellManaUnit::Second),
                WeldBuildId::Hailstones => (false, Rock, PrimarySpellManaUnit::Second),
                WeldBuildId::CrawlingShock => (false, Bolt, Cast),
            };
            (weld_build(id).synthetic_name, damage_range, damage_unit, mana_unit)
        }
        _ => panic!("skill {selected_primary_skill_id} is not a native selected primary"),
    };
    PrimarySpellDescriptor { damage_range, damage_unit, mana_unit, name }
}

/// The damage range and mana cost shown for the selected primary spell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrimarySpellSummary {
    pub damage_maximum: f64,
    pub damage_minimum: f64,
    pub mana_cost: f64,
}

/// Boulder's lower displayed damage uses its initial charge, not its fully charged payload.
#[must_use]
pub fn primary_spell_summary(
    book: &SkillBookState,
    factors: &OffensiveSpellFactors,
) -> PrimarySpellSummary {
    primary_spell_summary_with_stat_book(book, &PlayerStatBookComponent::default(), factors)
}

#[must_use]
pub fn primary_spell_summary_with_stat_book(
    book: &SkillBookState,
    stat_book: &PlayerStatBookComponent,
    factors: &OffensiveSpellFactors,
) -> PrimarySpellSummary {
    let profile = primary_skill_profile_with_stat_book(book, stat_book, factors);
    let common = profile.common();
    let damage_minimum = match &profile {
        PrimarySkillProfile::Earth(_) => {
            boulder_display_minimum(damage_value(stat_book, 40, common.rank, false), 40, factors)
        }
        PrimarySkillProfile::Weld(weld) if weld.build_id == WeldBuildId::EtherealBoulder => {
            boulder_display_minimum(weld.vector.values[0], SPELL_WELDING_SKILL_ID, factors)
        }
        _ => common.damage_minimum,
    };
    PrimarySpellSummary {
        damage_minimum,
        damage_maximum: common.damage_maximum,
        mana_cost: common.mana_cost,
    }
}

fn boulder_display_minimum(damage: f64, skill_id: usize, factors: &OffensiveSpellFactors) -> f64 {
    let charge = PRIMARY_EARTH_INITIAL_CHARGE;
    let initial_damage = fround(damage * charge * charge);
    resolve_skill_damage_value(skill_id, initial_damage, factors).max(1.0)
}

fn primary_mana_cost(
    stat_book: &PlayerStatBookComponent,
    book: &SkillBookState,
    skill_id: usize,
) -> f64 {
    let upgrades: &[usize] = match skill_id {
        8 => &[9, 10, 13],
        16 => &[17, 18, 19, 20],
        24 => &[25, 26, 29, 31],
        32 => &[33, 34, 36, 37, 38],
        40 => &[42, 43, 47],
        _ => unreachable!("mana calculation receives an elemental primary"),
    };
    // Sum the upgrades separately, then add the primary. Regrouping these
    // additions changes the final bit for fractional authored mana costs.
    let mut upgrade_cost = 0.0;
    for &upgrade in upgrades {
        let rank = book.effective_ranks[upgrade];
        if rank > 0 {
            upgrade_cost += stat_book.stat(upgrade, "mManaCost", rank).unwrap_or(0.0);
        }
    }
    stat_book.stat(skill_id, "mManaCost", book.effective_ranks[skill_id]).unwrap_or(0.0)
        + upgrade_cost
}

fn damage_value(
    stat_book: &PlayerStatBookComponent,
    skill_id: usize,
    rank: u32,
    maximum: bool,
) -> f64 {
    if let Some(value) = stat_book.stat(skill_id, "mDamage", rank) {
        return value;
    }
    ranked(stat_book, skill_id, if maximum { "mDamage2" } else { "mDamage1" }, rank)
}

fn ranked_or(
    stat_book: &PlayerStatBookComponent,
    book: &SkillBookState,
    skill_id: usize,
    property: &str,
    fallback: f64,
) -> f64 {
    let rank = book.effective_ranks[skill_id];
    if rank == 0 {
        return fallback;
    }
    ranked(stat_book, skill_id, property, rank)
}

fn ranked(stat_book: &PlayerStatBookComponent, skill_id: usize, property: &str, rank: u32) -> f64 {
    stat_book
        .stat(skill_id, property, rank)
        .unwrap_or_else(|| panic!("native skill {skill_id} is missing {property} at rank {rank}"))
}
