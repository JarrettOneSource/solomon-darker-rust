//! Canonical wizard identity and learned skills, from Website `core-kernels/player-character.ts`
//! and `player-progression.ts`. `profile-goldens.json` pins every stock stat-book row.

mod catalog;
mod class_title;
pub mod create;
mod hagatha;
mod progression;
pub mod runtime;
pub mod seeker;
mod stat_book;

pub use stat_book::{PlayerStatBookComponent, SkillStatBookEntry, SkillStatBookValue};

pub use hagatha::{
    HAGATHA_FACTORS, HAGATHA_LAST_WORD_DEATH_TICK, HagathaDerivedModifiers, HagathaFactors,
    hagatha_boss_damage_factor, hagatha_derived_modifiers,
    hagatha_drinker_should_use_health_potion, hagatha_drinker_should_use_mana_potion,
    hagatha_revelation_rank,
};
pub use progression::{
    HagathaRuntimeState, LEVEL_THRESHOLDS, MAX_PLAYER_EXPERIENCE, MAX_PLAYER_LEVEL,
    PlayerProgressionState, PlayerSkillChoice, PlayerSkillOffer, PlayerSkillOfferOption,
    PotionKind, SharedPlayerLevelMilestone, boneyard_enemy_experience_award,
    build_player_skill_offer, draw_player_creation_offer_seed, skill_passes_offer_eligibility,
};
pub use runtime::{
    CONCENTRATABLE_SKILL_IDS, HardenChannel, HardenState, HurricaneChargeTick,
    PlayerConcentrationSlot, PlayerSkillActivity, PlayerSkillDerivedStats, PlayerSkillRuntimeState,
    PlayerSkillRuntimeTickResult, PlayerStaffAttackOutcome, PlayerStaffAttackResult,
    effective_skill_numeric_value, effective_skill_numeric_value_with_stat_book,
    harden_break_angle_step, hurricane_charge_tick, interpolate_harden_coating,
    player_poison_duration_seconds, player_skill_derived_stats,
    player_skill_derived_stats_with_stat_book, player_staff_damage,
    refresh_player_combat_from_skill_stats, resolve_player_staff_attack,
};
pub mod offensive;
pub mod primary;
pub mod weld;

pub use catalog::{SKILL_CATALOG, SKILL_RULES, WELD_BUILDS};
pub use class_title::wizard_class_title;

use crate::appearance::StarterColorOwner;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum WizardElement {
    Air,
    Earth,
    Ether,
    Fire,
    Water,
}

impl WizardElement {
    pub const ALL: [Self; 5] = [Self::Air, Self::Earth, Self::Ether, Self::Fire, Self::Water];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Air => "air",
            Self::Earth => "earth",
            Self::Ether => "ether",
            Self::Fire => "fire",
            Self::Water => "water",
        }
    }
}

impl From<WizardElement> for StarterColorOwner {
    fn from(element: WizardElement) -> Self {
        match element {
            WizardElement::Air => Self::Air,
            WizardElement::Earth => Self::Earth,
            WizardElement::Ether => Self::Ether,
            WizardElement::Fire => Self::Fire,
            WizardElement::Water => Self::Water,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum WizardDiscipline {
    Arcane,
    Body,
    Mind,
    Air,
    Earth,
    Ether,
    Fire,
    Water,
}

impl WizardDiscipline {
    /// `WIZARD_DISCIPLINE_CHOICES`: the three ordinary manual Create choices.
    pub const CHOICES: [Self; 3] = [Self::Arcane, Self::Body, Self::Mind];
    /// Accepted identities also include Create's randomized second elemental root.
    pub const ALL: [Self; 8] = [
        Self::Arcane,
        Self::Body,
        Self::Mind,
        Self::Air,
        Self::Earth,
        Self::Ether,
        Self::Fire,
        Self::Water,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Arcane => "arcane",
            Self::Body => "body",
            Self::Mind => "mind",
            Self::Air => "air",
            Self::Earth => "earth",
            Self::Ether => "ether",
            Self::Fire => "fire",
            Self::Water => "water",
        }
    }

    #[must_use]
    pub const fn root(self) -> usize {
        match self {
            Self::Ether => 0,
            Self::Fire => 1,
            Self::Air => 2,
            Self::Water => 3,
            Self::Earth => 4,
            Self::Body => 5,
            Self::Mind => 6,
            Self::Arcane => 7,
        }
    }
}

impl From<WizardElement> for WizardDiscipline {
    fn from(element: WizardElement) -> Self {
        match element {
            WizardElement::Air => Self::Air,
            WizardElement::Earth => Self::Earth,
            WizardElement::Ether => Self::Ether,
            WizardElement::Fire => Self::Fire,
            WizardElement::Water => Self::Water,
        }
    }
}

/// The local create-wizard screen's complete identity selection.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NewGameSelection {
    pub name: String,
    pub element: WizardElement,
    pub discipline: WizardDiscipline,
}

pub const SKILL_ROW_COUNT: usize = 83;
pub const SPELL_WELDING_SKILL_ID: usize = 52;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkillRule {
    pub skill_id: usize,
    pub minimum_level: u32,
    pub root: usize,
    pub category: u32,
    pub all: &'static [usize],
    pub any: &'static [usize],
    pub forbidden: &'static [usize],
}

#[must_use]
pub fn skill_rule(skill_id: usize) -> Option<&'static SkillRule> {
    SKILL_RULES.get(skill_id.checked_sub(8)?)
}

#[must_use]
pub fn skill_root(skill_id: usize) -> Option<usize> {
    if skill_id == 80 {
        return Some(0);
    }
    skill_rule(skill_id).map(|rule| rule.root)
}

#[must_use]
pub fn skill_category(skill_id: usize) -> Option<u32> {
    skill_rule(skill_id).map(|rule| rule.category)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SkillValue {
    Scalar(f64),
    Ranks(&'static [f64]),
}

impl SkillValue {
    /// Native rank tables include row zero; ranks beyond a short table retain its last row.
    #[must_use]
    pub fn at_rank(self, rank: u32) -> Option<f64> {
        match self {
            Self::Scalar(value) => Some(value),
            Self::Ranks(values) => {
                values.get((rank as usize).min(values.len().saturating_sub(1))).copied()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkillProperty {
    pub name: &'static str,
    pub value: SkillValue,
}

/// The complete numeric stat row and authored description of one native skill.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkillDefinition {
    pub id: usize,
    pub name: &'static str,
    pub family: &'static str,
    pub icon_record: usize,
    pub cap_level: u32,
    pub maximum_level: u32,
    pub description: &'static str,
    pub quick_description: &'static str,
    /// Authored tooltip templates retain the native `%d:property%` formatting tokens.
    pub stat_lines: &'static [&'static str],
    pub bonus_lines: &'static [&'static str],
    pub numeric_properties: &'static [SkillProperty],
}

impl SkillDefinition {
    #[must_use]
    pub fn stat(&self, property: &str, rank: u32) -> Option<f64> {
        self.numeric_properties
            .iter()
            .find(|value| value.name == property)
            .and_then(|value| value.value.at_rank(rank))
    }
}

/// Looks up an authored numeric property without inventing a missing-stat default.
#[must_use]
pub fn skill_stat(skill_id: usize, property: &str, rank: u32) -> Option<f64> {
    SKILL_CATALOG.get(skill_id)?.stat(property, rank)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeldBuild {
    pub color_roots: [usize; 2],
    pub component_skill_ids: [usize; 6],
    pub id: u32,
    pub pair_description: &'static str,
    pub primary_skill_ids: [usize; 2],
    pub icon_record: usize,
    pub synthetic_name: &'static str,
}

#[must_use]
pub fn weld_build(build_id: u32) -> Option<&'static WeldBuild> {
    let index = build_id.checked_sub(1000)? as usize;
    WELD_BUILDS.get(index)
}

/// Permanent acquisition and equipment-adjusted ranks share one canonical skillbook.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SkillBookState {
    pub advanced_unlocks: [bool; 8],
    pub discipline_root: usize,
    #[cfg_attr(feature = "persistence", serde(with = "crate::persistence::fixed_array"))]
    pub effective_ranks: [u32; SKILL_ROW_COUNT],
    pub element_root: usize,
    pub learned_skill_order: Vec<usize>,
    #[cfg_attr(feature = "persistence", serde(with = "crate::persistence::fixed_array"))]
    pub permanent_ranks: [u32; SKILL_ROW_COUNT],
    pub primary_skill_id: usize,
    pub weld_build_id: Option<u32>,
    pub weld_component_ranks: Option<[u32; 6]>,
}

impl SkillBookState {
    /// `createPlayerSkillBook`: root rows and exactly two element-owned starter skills.
    #[must_use]
    pub fn new(element: WizardElement, discipline: WizardDiscipline) -> Self {
        let (element_root, primary, secondary) = match element {
            WizardElement::Ether => (0, 8, 11),
            WizardElement::Fire => (1, 16, 21),
            WizardElement::Air => (2, 24, 27),
            WizardElement::Water => (3, 32, 35),
            WizardElement::Earth => (4, 40, 45),
        };
        let discipline_root = discipline.root();
        let mut ranks = [0; SKILL_ROW_COUNT];
        ranks[..8].fill(1);
        ranks[primary] = 1;
        ranks[secondary] = 1;
        Self {
            advanced_unlocks: [false; 8],
            discipline_root,
            effective_ranks: ranks,
            element_root,
            learned_skill_order: vec![primary, secondary],
            permanent_ranks: ranks,
            primary_skill_id: primary,
            weld_build_id: None,
            weld_component_ranks: None,
        }
    }

    /// `unlockPlayerAdvancedSkill`: buying a spell unlocks acquisition; it grants no rank.
    pub fn unlock_advanced(&mut self, skill_id: usize) -> bool {
        if !(72..=79).contains(&skill_id) || self.advanced_unlocks[skill_id - 72] {
            return false;
        }
        self.advanced_unlocks[skill_id - 72] = true;
        true
    }

    /// `grantPlayerSkillRanks`: acquisition order is stable and ranks cap at the stock maximum.
    pub fn grant_ranks(&mut self, skill_id: usize, ranks: u32) {
        assert!(ranks > 0, "granted skill ranks must be positive");
        assert!(
            (8..=79).contains(&skill_id) && skill_id != SPELL_WELDING_SKILL_ID,
            "grant ordinary native skills separately from a Weld build"
        );
        let maximum = SKILL_CATALOG[skill_id].maximum_level;
        assert!(maximum > 0, "a grantable native skill has an authored maximum rank");
        let current = self.permanent_ranks[skill_id];
        let target = maximum.min(current.saturating_add(ranks));
        let unlock_changed = skill_id >= 72 && !self.advanced_unlocks[skill_id - 72];
        if target == current && !unlock_changed {
            return;
        }
        self.permanent_ranks[skill_id] = target;
        self.effective_ranks[skill_id] = target;
        if skill_id >= 72 {
            self.advanced_unlocks[skill_id - 72] = true;
        }
        if current == 0 && !self.learned_skill_order.contains(&skill_id) {
            self.learned_skill_order.push(skill_id);
        }
    }

    /// `grantPlayerWeldBuild`: acquire its missing components before the synthetic skill.
    pub fn grant_weld_build(&mut self, build_id: u32) {
        let build = weld_build(build_id).expect("an authored native Weld build");
        for skill_id in build.component_skill_ids {
            if self.permanent_ranks[skill_id] < 1 {
                self.grant_ranks(skill_id, 1);
            }
        }
        let learned = self.permanent_ranks[SPELL_WELDING_SKILL_ID] == 0;
        self.permanent_ranks[SPELL_WELDING_SKILL_ID] = 1;
        self.effective_ranks[SPELL_WELDING_SKILL_ID] = 1;
        if learned && !self.learned_skill_order.contains(&SPELL_WELDING_SKILL_ID) {
            self.learned_skill_order.push(SPELL_WELDING_SKILL_ID);
        }
        self.primary_skill_id = SPELL_WELDING_SKILL_ID;
        self.weld_build_id = Some(build.id);
        self.weld_component_ranks =
            Some(build.component_skill_ids.map(|skill_id| self.effective_ranks[skill_id]));
    }

    pub fn select_primary(&mut self, skill_id: usize) {
        assert!(
            [8, 16, 24, 32, 40, SPELL_WELDING_SKILL_ID].contains(&skill_id),
            "the selected native skill must be a primary attack"
        );
        assert!(self.effective_ranks[skill_id] > 0, "the selected primary skill must be learned");
        assert!(
            skill_id != SPELL_WELDING_SKILL_ID || self.weld_build_id.is_some(),
            "Spell Welding must have an acquired build"
        );
        self.primary_skill_id = skill_id;
    }

    #[must_use]
    pub fn skill_icon_record(&self, skill_id: usize) -> usize {
        if skill_id != SPELL_WELDING_SKILL_ID {
            return SKILL_CATALOG[skill_id].icon_record;
        }
        match self.weld_build_id {
            Some(id) => weld_build(id).expect("the learned native Weld build").icon_record,
            None => SKILL_CATALOG[skill_id].icon_record,
        }
    }
}
