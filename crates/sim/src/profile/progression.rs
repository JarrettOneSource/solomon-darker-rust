//! Native experience thresholds, skill offers and acquisition from Website
//! `core-kernels/player-progression.ts`. Combat resources stay in their canonical state.

use crate::NativeRng;
use crate::combat::PlayerCombatState;
use crate::float::fround;

use super::{
    NATIVE_HAGATHA_FACTORS, NATIVE_SKILL_CATALOG, NATIVE_WELD_BUILDS, NativeSkillBookState,
    SPELL_WELDING_SKILL_ID, WizardElement, native_hagatha_revelation_rank, native_skill_category,
    native_skill_rule, native_skill_stat, native_weld_build,
};

pub const MAX_PLAYER_LEVEL: u32 = 75;
pub const MAX_PLAYER_EXPERIENCE: f64 = 10_000_000.0;
pub const NATIVE_LEVEL_THRESHOLDS: [u32; 76] = [
    0, 90, 160, 275, 390, 520, 650, 800, 1060, 1300, 1600, 2000, 2400, 2850, 3400, 4200, 4800,
    5650, 6000, 6500, 7200, 7850, 8900, 9900, 11000, 12000, 13000, 14000, 15000, 16000, 20000,
    25000, 30000, 35000, 40000, 45000, 51000, 57000, 64000, 71000, 79000, 88000, 98000, 110000,
    120000, 130000, 135000, 150000, 175000, 200000, 300000, 400000, 500000, 600000, 700000, 800000,
    900000, 1000000, 1200000, 1400000, 1700000, 2000000, 2300000, 2600000, 3000000, 3500000,
    4000000, 4500000, 5000000, 5500000, 6000000, 6500000, 7000000, 7500000, 8500000, 10000000,
];
const ELEMENTAL_PRIMARIES: [usize; 5] = [8, 16, 24, 32, 40];
const WELDING_RELATED: [[usize; 2]; 5] = [[10, 9], [18, 17], [25, 26], [34, 33], [43, 42]];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeHagathaRuntimeState {
    pub cheat_death_charges: u32,
    pub reverie_active: bool,
    pub serendipity_active: bool,
}

impl NativeHagathaRuntimeState {
    pub fn purchase(&mut self, selectors: &[u32]) {
        if selectors.contains(&7) {
            self.cheat_death_charges = 1;
        }
        if selectors.contains(&24) {
            self.serendipity_active = true;
        }
        if selectors.contains(&25) {
            self.reverie_active = true;
        }
    }

    pub fn remove(&mut self, selector: u32) {
        match selector {
            7 => self.cheat_death_charges = 0,
            24 => self.serendipity_active = false,
            25 => self.reverie_active = false,
            _ => {}
        }
    }

    pub fn hurt(&mut self, remaining_damage: f64) {
        assert!(
            remaining_damage.is_finite() && remaining_damage >= 0.0,
            "remaining Hagatha damage is finite and non-negative"
        );
        if remaining_damage > 0.0 {
            self.reverie_active = false;
            self.serendipity_active = false;
        }
    }

    pub fn consume_cheat_death(&mut self, maximum_health: f64) -> Option<f64> {
        assert!(
            maximum_health.is_finite() && maximum_health > 0.0,
            "Cheat Death uses positive finite maximum health"
        );
        if self.cheat_death_charges == 0 {
            return None;
        }
        assert_eq!(self.cheat_death_charges, 1, "Cheat Death has one native charge");
        self.cheat_death_charges = 0;
        Some(fround(maximum_health * NATIVE_HAGATHA_FACTORS.cheat_death_recovery))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativePotionKind {
    Health,
    Mana,
    WizardChug,
    Antidote,
    MindChug,
    Rejuvenation,
}

impl NativePotionKind {
    #[must_use]
    pub fn from_native_subtype(subtype: u32) -> Option<Self> {
        Some(match subtype {
            0 => Self::Health,
            1 => Self::Mana,
            2 => Self::WizardChug,
            3 => Self::Antidote,
            4 => Self::MindChug,
            5 => Self::Rejuvenation,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerSkillOfferOption {
    pub insight: bool,
    pub skill_id: usize,
    pub target_rank: u32,
    pub weld_build_id: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerSkillOffer {
    pub automatic_choice_index: Option<usize>,
    pub level: u32,
    pub options: Vec<PlayerSkillOfferOption>,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerSkillChoice {
    pub choice_index: usize,
    pub offer_sequence: u64,
    pub skill_id: usize,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SharedPlayerLevelMilestone {
    pub crossed_levels: Vec<u32>,
    pub experience: f64,
    pub level: u32,
}

impl SharedPlayerLevelMilestone {
    pub fn validate(&self) -> Result<(), String> {
        if !self.experience.is_finite() || self.experience < 0.0 {
            return Err("shared milestone experience must be finite and non-negative".into());
        }
        if !(1..=MAX_PLAYER_LEVEL).contains(&self.level) {
            return Err("shared milestone level is out of range".into());
        }
        let mut previous = None;
        for level in &self.crossed_levels {
            if *level < 2 || *level > self.level || previous.is_some_and(|value| value >= *level) {
                return Err("shared crossed levels must be sorted and in range".into());
            }
            previous = Some(*level);
        }
        Ok(())
    }
}

/// The progression fields of PlayerProgressionComponent. Current resources and statuses
/// remain in PlayerCombatState and are passed only to commands that change them.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerProgressionState {
    pub deferred_skill_choices: usize,
    pub damage_x4_ticks_remaining: u64,
    pub discipline_offer_bias: bool,
    pub exclude_active_weld_build_from_offers: bool,
    pub experience: f64,
    pub forced_offer_skill_ids: Vec<usize>,
    pub hagatha_runtime: NativeHagathaRuntimeState,
    pub level: u32,
    pub mind_chug_ticks_remaining: u64,
    // Native/portable disk imports may retain fractional thresholds until XP advances.
    pub next_threshold: f64,
    pub offer_cycle: u32,
    pub offer_seed: u32,
    pub pending_levels: Vec<u32>,
    pub pending_offer: Option<PlayerSkillOffer>,
    pub previous_threshold: f64,
    pub poison_immunity_ticks_remaining: u64,
    pub revision: u64,
    pub sorcerors_charm_available: bool,
    pub weld_offer_marker: u32,
    pub welding_offer_bias: bool,
}

impl PlayerProgressionState {
    #[must_use]
    pub fn new(offer_seed: u32) -> Self {
        assert!(offer_seed < 1_000_000, "a player offer seed is below one million");
        Self {
            deferred_skill_choices: 0,
            damage_x4_ticks_remaining: 0,
            discipline_offer_bias: false,
            exclude_active_weld_build_from_offers: false,
            experience: 0.0,
            forced_offer_skill_ids: Vec::new(),
            hagatha_runtime: NativeHagathaRuntimeState::default(),
            level: 1,
            mind_chug_ticks_remaining: 0,
            next_threshold: f64::from(NATIVE_LEVEL_THRESHOLDS[1]),
            offer_cycle: 0,
            offer_seed,
            pending_levels: Vec::new(),
            pending_offer: None,
            previous_threshold: 0.0,
            poison_immunity_ticks_remaining: 0,
            revision: 0,
            sorcerors_charm_available: false,
            weld_offer_marker: 9999,
            welding_offer_bias: false,
        }
    }

    #[must_use]
    pub fn experience_progress(&self) -> f64 {
        let span = self.next_threshold - self.previous_threshold;
        if span <= 0.0 {
            return 1.0;
        }
        ((self.experience - self.previous_threshold) / span).clamp(0.0, 1.0)
    }

    pub fn apply_acquisition_offer_seeds(&mut self, gameplay_rng: &mut NativeRng, count: usize) {
        for _ in 0..count {
            self.offer_seed = gameplay_rng.draw_integer(1_000_000) as u32;
        }
    }

    /// Native level thresholds are strict: exactly 90XP is still level one.
    pub fn grant_experience(
        &mut self,
        book: &NativeSkillBookState,
        combat: &mut PlayerCombatState,
        amount: f64,
        gameplay_rng: &mut NativeRng,
        sorcerors_charm_owned: bool,
    ) {
        assert!(
            amount.is_finite() && amount >= 0.0,
            "experience awards are finite and non-negative"
        );
        if amount == 0.0 || self.level >= MAX_PLAYER_LEVEL {
            let experience = MAX_PLAYER_EXPERIENCE.min(self.experience + amount);
            if experience != self.experience {
                self.experience = experience;
                self.revision += 1;
            }
            return;
        }
        self.experience = MAX_PLAYER_EXPERIENCE.min(self.experience + amount);
        let old_level = self.level;
        let mut crossed_count = 0;
        while self.level < MAX_PLAYER_LEVEL
            && self.experience > f64::from(NATIVE_LEVEL_THRESHOLDS[self.level as usize])
        {
            self.level += 1;
            crossed_count += 1;
        }
        self.next_threshold = f64::from(NATIVE_LEVEL_THRESHOLDS[self.level as usize]);
        self.previous_threshold = f64::from(NATIVE_LEVEL_THRESHOLDS[self.level as usize - 1]);
        if self.level != old_level {
            combat.current_health = combat.maximum_health;
            combat.current_mana = combat.maximum_mana;
        }
        if crossed_count > 0 {
            self.pending_levels =
                vec![
                    self.level;
                    self.pending_levels.len() + self.deferred_skill_choices + crossed_count
                ];
            self.deferred_skill_choices = 0;
            self.pending_offer = None;
            self.sorcerors_charm_available = false;
        }
        self.revision += 1;
        self.open_next_offer(book, combat.maximum_mana, gameplay_rng, sorcerors_charm_owned);
    }

    pub fn open_next_offer(
        &mut self,
        book: &NativeSkillBookState,
        maximum_mana: f64,
        gameplay_rng: &mut NativeRng,
        sorcerors_charm_available: bool,
    ) {
        if self.pending_offer.is_some() || self.pending_levels.is_empty() {
            return;
        }
        self.offer_cycle = self.offer_cycle.wrapping_add(1);
        let sequence = self.revision + 1;
        self.pending_offer =
            Some(build_player_skill_offer(self, book, maximum_mana, sequence, gameplay_rng));
        self.revision = sequence;
        self.sorcerors_charm_available = sorcerors_charm_available;
    }

    /// `synchronizePlayerLevelMilestone` copies the source player's milestone and
    /// opens the receiver's own offer. It does not heal the receiver; rejoin has
    /// a separate resource-recovery boundary after this shared progression step.
    pub fn synchronize_level_milestone(
        &mut self,
        book: &NativeSkillBookState,
        maximum_mana: f64,
        milestone: &SharedPlayerLevelMilestone,
        rng: &mut NativeRng,
        charm_owned: bool,
    ) -> Result<(), String> {
        milestone.validate()?;
        if !milestone.crossed_levels.is_empty() {
            self.pending_levels = vec![
                milestone.level;
                self.pending_levels.len()
                    + self.deferred_skill_choices
                    + milestone.crossed_levels.len()
            ];
            self.deferred_skill_choices = 0;
            self.pending_offer = None;
            self.sorcerors_charm_available = false;
        }
        self.experience = milestone.experience.min(MAX_PLAYER_EXPERIENCE);
        self.level = milestone.level;
        self.next_threshold = f64::from(NATIVE_LEVEL_THRESHOLDS[self.level as usize]);
        self.previous_threshold = f64::from(NATIVE_LEVEL_THRESHOLDS[self.level as usize - 1]);
        self.revision += 1;
        self.open_next_offer(book, maximum_mana, rng, charm_owned);
        Ok(())
    }

    pub fn choose_skill(
        &mut self,
        book: &mut NativeSkillBookState,
        choice: PlayerSkillChoice,
        gameplay_rng: &mut NativeRng,
        owned_hagatha_selectors: &[u32],
    ) -> bool {
        let Some(offer) = &self.pending_offer else {
            return false;
        };
        if offer.sequence != choice.offer_sequence {
            return false;
        }
        let Some(chosen) = offer.options.get(choice.choice_index).copied() else {
            return false;
        };
        if chosen.skill_id != choice.skill_id {
            return false;
        }
        let Some(entry) = NATIVE_SKILL_CATALOG.get(chosen.skill_id) else {
            return false;
        };
        let rank = book.permanent_ranks[chosen.skill_id];
        let weld = chosen.weld_build_id.and_then(native_weld_build);
        if chosen.skill_id == SPELL_WELDING_SKILL_ID {
            if chosen.insight || weld.is_none() || rank > 0 || chosen.target_rank != 1 {
                return false;
            }
        } else if chosen.weld_build_id.is_some()
            || entry.maximum_level < 1
            || rank >= entry.maximum_level
            || (chosen.insight && rank + 2 > entry.maximum_level)
            || chosen.target_rank != rank + 1
        {
            return false;
        }
        let selected_rank = if chosen.skill_id == SPELL_WELDING_SKILL_ID {
            1
        } else {
            rank + if chosen.insight { 2 } else { 1 }
        };
        let next_rank = if chosen.skill_id == SPELL_WELDING_SKILL_ID {
            selected_rank
        } else {
            entry.maximum_level.min(if owned_hagatha_selectors.contains(&6) {
                selected_rank.max(2)
            } else {
                selected_rank
            })
        };
        book.permanent_ranks[chosen.skill_id] = next_rank;
        book.effective_ranks[chosen.skill_id] = next_rank;
        if rank == 0 && !book.learned_skill_order.contains(&chosen.skill_id) {
            book.learned_skill_order.push(chosen.skill_id);
        }
        if let Some(build) = weld {
            book.primary_skill_id = SPELL_WELDING_SKILL_ID;
            book.weld_build_id = Some(build.id);
            book.weld_component_ranks =
                Some(build.component_skill_ids.map(|id| book.effective_ranks[id]));
        }
        self.apply_acquisition_offer_seeds(gameplay_rng, if chosen.insight { 2 } else { 1 });
        self.forced_offer_skill_ids.clear();
        if !self.pending_levels.is_empty() {
            self.pending_levels.remove(0);
        }
        self.pending_offer = None;
        self.revision += 1;
        self.sorcerors_charm_available = false;
        if self.weld_offer_marker >= 1001 && welding_eligible(self.level, book) {
            self.weld_offer_marker = self.offer_cycle;
        }
        true
    }

    pub fn set_automatic_choice(&mut self, choice_index: usize) -> bool {
        let Some(offer) = &mut self.pending_offer else {
            return false;
        };
        if choice_index >= offer.options.len() {
            return false;
        }
        if offer.automatic_choice_index != Some(choice_index) {
            offer.automatic_choice_index = Some(choice_index);
            self.revision += 1;
        }
        true
    }

    /// `replacePlayerSkillChoiceWithMod`: the admitted authored reward consumes this
    /// native offer without awarding any of its skills or drawing native offer RNG.
    pub fn replace_choice_with_mod(&mut self, offer_sequence: u64) -> bool {
        if self.pending_offer.as_ref().is_none_or(|offer| offer.sequence != offer_sequence) {
            return false;
        }
        self.forced_offer_skill_ids.clear();
        if !self.pending_levels.is_empty() {
            self.pending_levels.remove(0);
        }
        self.pending_offer = None;
        self.revision += 1;
        self.sorcerors_charm_available = false;
        true
    }

    pub fn reroll_offer(
        &mut self,
        book: &NativeSkillBookState,
        maximum_mana: f64,
        offer_sequence: u64,
        next_offer_seed: u32,
        gameplay_rng: &mut NativeRng,
    ) -> bool {
        if !self.sorcerors_charm_available
            || self.pending_offer.as_ref().is_none_or(|offer| offer.sequence != offer_sequence)
        {
            return false;
        }
        assert!(next_offer_seed < 1_000_000, "a player offer seed is below one million");
        self.offer_seed = next_offer_seed;
        self.pending_offer = None;
        self.sorcerors_charm_available = false;
        self.open_next_offer(book, maximum_mana, gameplay_rng, false);
        true
    }

    pub fn defer_choice(
        &mut self,
        book: &NativeSkillBookState,
        maximum_mana: f64,
        offer_sequence: u64,
        gameplay_rng: &mut NativeRng,
        sorcerors_charm_owned: bool,
    ) -> bool {
        if !sorcerors_charm_owned
            || !self.sorcerors_charm_available
            || self.pending_offer.as_ref().is_none_or(|offer| offer.sequence != offer_sequence)
        {
            return false;
        }
        self.deferred_skill_choices += 1;
        if !self.pending_levels.is_empty() {
            self.pending_levels.remove(0);
        }
        self.pending_offer = None;
        self.revision += 1;
        self.sorcerors_charm_available = false;
        self.open_next_offer(book, maximum_mana, gameplay_rng, true);
        true
    }

    pub fn grant_bonus_skill_choice(
        &mut self,
        book: &NativeSkillBookState,
        maximum_mana: f64,
        gameplay_rng: &mut NativeRng,
        sorcerors_charm_owned: bool,
    ) {
        self.pending_levels.push(self.level);
        if self.pending_offer.is_some() {
            self.revision += 1;
        } else {
            self.open_next_offer(book, maximum_mana, gameplay_rng, sorcerors_charm_owned);
        }
    }

    pub fn apply_potion(&mut self, kind: NativePotionKind, combat: &mut PlayerCombatState) {
        match kind {
            NativePotionKind::Health => combat.current_health = combat.maximum_health,
            NativePotionKind::Mana => combat.current_mana = combat.maximum_mana,
            NativePotionKind::WizardChug => self.damage_x4_ticks_remaining = 6000,
            NativePotionKind::Antidote => {
                combat.poison_before_cold = false;
                combat.poison_damage_per_tick = 0.0;
                combat.poison_ticks_remaining = 0;
                self.poison_immunity_ticks_remaining = 1000;
            }
            NativePotionKind::MindChug => self.mind_chug_ticks_remaining = 6000,
            NativePotionKind::Rejuvenation => {
                combat.current_health = combat.maximum_health;
                combat.current_mana = combat.maximum_mana;
            }
        }
        self.revision += 1;
    }

    pub fn step_potion_effects(&mut self) {
        self.damage_x4_ticks_remaining = self.damage_x4_ticks_remaining.saturating_sub(1);
        self.mind_chug_ticks_remaining = self.mind_chug_ticks_remaining.saturating_sub(1);
        self.poison_immunity_ticks_remaining =
            self.poison_immunity_ticks_remaining.saturating_sub(1);
    }

    pub fn reset_potion_effects(&mut self) {
        self.damage_x4_ticks_remaining = 0;
        self.mind_chug_ticks_remaining = 0;
        self.poison_immunity_ticks_remaining = 0;
    }

    pub fn grant_mind_dredge(&mut self) {
        self.deferred_skill_choices += 1;
        self.revision += 1;
    }
    pub fn apply_damage_x4_bonus(&mut self) {
        self.damage_x4_ticks_remaining = 1500;
        self.revision += 1;
    }
}

/// Native creation consumes 13 acquisition seeds, retaining only the last one.
pub fn draw_native_player_creation_offer_seed(gameplay_rng: &mut NativeRng) -> u32 {
    let mut seed = 0;
    for _ in 0..13 {
        seed = gameplay_rng.draw_integer(1_000_000) as u32;
    }
    seed
}

#[must_use]
pub fn boneyard_enemy_experience_award(
    evaluated_actor_reward: f64,
    arena_player_count: u32,
    receiver_level: u32,
    gameplay_scalar: f64,
    receiver_xp_bonus: f64,
) -> f64 {
    assert!(
        evaluated_actor_reward.is_finite()
            && evaluated_actor_reward >= 0.0
            && arena_player_count > 0
            && (1..=75).contains(&receiver_level)
            && gameplay_scalar.is_finite()
            && gameplay_scalar >= 0.0
            && receiver_xp_bonus.is_finite()
            && receiver_xp_bonus >= 0.0,
        "native experience award inputs are within their domains"
    );
    let level_factor = match receiver_level {
        1 => 1.0,
        2..=5 => 0.9,
        6..=15 => 0.72,
        16..=30 => 0.504,
        _ => 0.3024,
    };
    evaluated_actor_reward
        * f64::from(arena_player_count)
        * gameplay_scalar
        * level_factor
        * (1.0 + receiver_xp_bonus)
}

#[must_use]
pub fn native_skill_passes_offer_eligibility(
    skill_id: usize,
    level: u32,
    book: &NativeSkillBookState,
) -> bool {
    let Some(rule) = native_skill_rule(skill_id) else {
        return false;
    };
    let Some(entry) = NATIVE_SKILL_CATALOG.get(skill_id) else {
        return false;
    };
    if entry.cap_level < 1
        || !has_dependencies_and_unlock(skill_id, book)
        || book.permanent_ranks[skill_id] >= entry.cap_level
    {
        return false;
    }
    let reduction = if learned(book, 63) { 2 } else { 0 };
    learned(book, skill_id) || level >= rule.minimum_level.saturating_sub(reduction)
}

/// `buildPlayerSkillOffer`: local offer-seed draws choose pools; the gameplay RNG owns
/// Weld selection and final full-range shuffle, so the two streams cannot be merged.
#[must_use]
pub fn build_player_skill_offer(
    progression: &PlayerProgressionState,
    book: &NativeSkillBookState,
    maximum_mana: f64,
    sequence: u64,
    gameplay_rng: &mut NativeRng,
) -> PlayerSkillOffer {
    let mut rng = NativeRng::new(i64::from(progression.offer_seed));
    let desired = if learned(book, 63) { 4 } else { 3 };
    let category_one_owned = count_owned_category(book, 1);
    let category_two_owned = count_owned_category(book, 2);
    let mut focus_one = progression.level < 5 || (category_one_owned > 1 && progression.level < 10);
    let draw = rng.draw_integer(2);
    focus_one |= draw != 0;
    if category_one_owned > 1 {
        let draw = rng.draw_integer(4);
        focus_one |= draw != 1;
    }
    let mut focus_two = false;
    if !progression.discipline_offer_bias {
        if category_two_owned > 1 {
            let draw = rng.draw_integer(2);
            focus_two |= draw != 1;
        }
        if category_two_owned > 2 {
            let draw = rng.draw_integer(3);
            focus_two |= draw != 1;
        }
        if category_two_owned > 3 {
            let draw = rng.draw_integer(6);
            focus_two |= draw != 2;
        }
        focus_two |= (category_two_owned > 0 && progression.level < 9)
            || (category_two_owned > 1 && progression.level < 16)
            || (category_two_owned > 2 && progression.level < 26)
            || (category_two_owned > 3 && progression.level < 36);
    }
    let mut root_priority = Vec::new();
    let mut general = Vec::new();
    for skill_id in 8..=79 {
        if skill_id == SPELL_WELDING_SKILL_ID
            || !native_skill_passes_offer_eligibility(skill_id, progression.level, book)
        {
            continue;
        }
        let rule = native_skill_rule(skill_id).expect("the eligible native skill rule");
        let rank = book.effective_ranks[skill_id];
        let focused_out = (focus_one && rule.category == 1 && rank == 0)
            || (focus_two && rule.category == 2 && rank == 0);
        let root_match = rule.root == book.element_root || rule.root == book.discipline_root;
        if root_match
            && (!focused_out || (rule.category == 2 && book.discipline_root == 7))
            && (rule.category != 2 || mana_cost(skill_id, rank + 1) <= maximum_mana)
        {
            root_priority.push(skill_id);
        }
        let general_match =
            !root_match || (progression.discipline_offer_bias && rule.root == book.discipline_root);
        if general_match
            && !focused_out
            && (rule.category != 2 || mana_cost(skill_id, progression.level + 1) <= maximum_mana)
        {
            let weight = if progression.discipline_offer_bias && rule.root == book.discipline_root {
                2
            } else {
                1
            };
            for _ in 0..weight {
                general.push(skill_id);
            }
        }
    }
    full_range_shuffle(&mut general, &mut rng);
    let mut selected = Vec::new();
    for skill_id in progression.forced_offer_skill_ids.iter().take(desired) {
        if (8..=79).contains(skill_id)
            && *skill_id != SPELL_WELDING_SKILL_ID
            && has_dependencies_and_unlock(*skill_id, book)
        {
            insert_unique(&mut selected, offer_option(*skill_id, book));
        }
    }
    if !root_priority.is_empty() && selected.len() < desired {
        let priority_pool = if progression.welding_offer_bias && rng.draw_integer(2) == 1 {
            welding_related_pool(book, progression.level)
        } else {
            root_priority.clone()
        };
        let draw = rng.draw_integer(priority_pool.len() as i32) as usize;
        insert_unique(&mut selected, offer_option(priority_pool[draw], book));
    }
    let welding_option = if selected.len() < desired {
        draw_welding_option(progression, book, gameplay_rng)
    } else {
        None
    };
    if let Some(option) = welding_option {
        insert_unique(&mut selected, option);
    }
    let owned_count = book.permanent_ranks[8..82].iter().filter(|rank| **rank > 0).count();
    let mut keep_started = false;
    if owned_count > 8 {
        keep_started = rng.draw_integer(2) == 1;
    }
    if owned_count > 12 {
        keep_started = rng.draw_integer(5) != 2;
    }
    if owned_count > 20 {
        keep_started = rng.draw_integer(10) != 2;
    }
    if keep_started {
        root_priority.retain(|id| started_or_dependent(*id, book));
        general.retain(|id| started_or_dependent(*id, book));
        selected.retain(|option| started_or_dependent(option.skill_id, book));
    }
    general.extend(root_priority);
    let mut attempts = 0;
    let mut category_one_collisions = 0;
    while selected.len() < desired && !general.is_empty() && attempts < 200 {
        attempts += 1;
        if attempts == 100 {
            for skill_id in 8..=79 {
                if skill_id != SPELL_WELDING_SKILL_ID
                    && native_skill_passes_offer_eligibility(skill_id, progression.level, book)
                {
                    general.push(skill_id);
                }
            }
        }
        let skill_id = general[rng.draw_integer(general.len() as i32) as usize];
        let category = native_skill_category(skill_id);
        if category == Some(4)
            && selected.iter().any(|option| native_skill_category(option.skill_id) == Some(4))
        {
            continue;
        }
        if category == Some(1)
            && selected.iter().any(|option| native_skill_category(option.skill_id) == Some(1))
            && category_one_collisions < 50
        {
            category_one_collisions += 1;
            continue;
        }
        insert_unique(&mut selected, offer_option(skill_id, book));
    }
    full_range_shuffle(&mut selected, gameplay_rng);
    PlayerSkillOffer {
        automatic_choice_index: None,
        level: progression.level,
        options: selected,
        sequence,
    }
}

fn learned(book: &NativeSkillBookState, skill_id: usize) -> bool {
    book.permanent_ranks.get(skill_id).is_some_and(|rank| *rank >= 1)
}
fn count_owned_category(book: &NativeSkillBookState, category: u32) -> usize {
    (8..=79).filter(|id| native_skill_category(*id) == Some(category) && learned(book, *id)).count()
}
fn has_dependencies_and_unlock(skill_id: usize, book: &NativeSkillBookState) -> bool {
    let Some(rule) = native_skill_rule(skill_id) else {
        return false;
    };
    if (72..=79).contains(&skill_id)
        && !book.advanced_unlocks[skill_id - 72]
        && !(skill_id == 72 && learned(book, 72))
    {
        return false;
    }
    rule.all.iter().all(|id| learned(book, *id))
        && (rule.any.is_empty() || rule.any.iter().any(|id| learned(book, *id)))
        && !rule.forbidden.iter().any(|id| learned(book, *id))
}
fn welding_eligible(level: u32, book: &NativeSkillBookState) -> bool {
    !learned(book, SPELL_WELDING_SKILL_ID)
        && native_skill_passes_offer_eligibility(SPELL_WELDING_SKILL_ID, level, book)
        && ELEMENTAL_PRIMARIES.iter().filter(|id| learned(book, **id)).count() > 1
}
fn draw_welding_option(
    progression: &PlayerProgressionState,
    book: &NativeSkillBookState,
    rng: &mut NativeRng,
) -> Option<PlayerSkillOfferOption> {
    if progression.weld_offer_marker > 999 || !welding_eligible(progression.level, book) {
        return None;
    }
    let cycles = progression.offer_cycle.wrapping_sub(progression.weld_offer_marker);
    if !cycles.is_multiple_of(5)
        && u64::from(progression.offer_cycle) > u64::from(progression.weld_offer_marker) + 1
    {
        return None;
    }
    let candidates: Vec<_> = NATIVE_WELD_BUILDS
        .iter()
        .filter(|build| {
            build.primary_skill_ids.iter().all(|id| learned(book, *id))
                && (!progression.exclude_active_weld_build_from_offers
                    || Some(build.id) != book.weld_build_id)
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let build = candidates[rng.draw_integer(candidates.len() as i32) as usize];
    Some(PlayerSkillOfferOption {
        insight: false,
        skill_id: SPELL_WELDING_SKILL_ID,
        target_rank: 1,
        weld_build_id: Some(build.id),
    })
}
fn welding_related_pool(book: &NativeSkillBookState, level: u32) -> Vec<usize> {
    let mut related = Vec::new();
    for should_be_learned in [true, false] {
        for (index, primary) in ELEMENTAL_PRIMARIES.iter().enumerate() {
            if learned(book, *primary) != should_be_learned {
                continue;
            }
            related.push(*primary);
            for skill_id in WELDING_RELATED[index] {
                if native_skill_passes_offer_eligibility(skill_id, level, book) {
                    related.push(skill_id);
                }
            }
        }
        if related.len() >= 6 {
            break;
        }
    }
    related
}
fn started_or_dependent(skill_id: usize, book: &NativeSkillBookState) -> bool {
    book.effective_ranks[skill_id] > 0
        || native_skill_rule(skill_id)
            .and_then(|rule| rule.all.first())
            .is_some_and(|required| learned(book, *required))
}
fn mana_cost(skill_id: usize, rank: u32) -> f64 {
    native_skill_stat(skill_id, "mManaCost", rank).unwrap_or(0.0)
}
fn offer_option(skill_id: usize, book: &NativeSkillBookState) -> PlayerSkillOfferOption {
    PlayerSkillOfferOption {
        insight: false,
        skill_id,
        target_rank: book.permanent_ranks[skill_id] + 1,
        weld_build_id: None,
    }
}
fn insert_unique(selected: &mut Vec<PlayerSkillOfferOption>, option: PlayerSkillOfferOption) {
    if !selected.iter().any(|selected| selected.skill_id == option.skill_id) {
        selected.push(option);
    }
}
fn full_range_shuffle<T>(values: &mut [T], rng: &mut NativeRng) {
    for index in 0..values.len() {
        let draw = rng.draw_integer(values.len() as i32) as usize;
        values.swap(index, draw);
    }
}

impl NativeSkillBookState {
    pub fn increase_random_learned_skill(&mut self, rng: &mut NativeRng) -> Option<usize> {
        let eligible: Vec<_> = (8..82)
            .filter(|id| {
                self.effective_ranks[*id] > 0
                    && self.permanent_ranks[*id] < NATIVE_SKILL_CATALOG[*id].maximum_level
            })
            .collect();
        if eligible.is_empty() {
            return None;
        }
        let skill_id = eligible[rng.draw_integer(eligible.len() as i32) as usize];
        self.permanent_ranks[skill_id] += 1;
        self.effective_ranks[skill_id] += 1;
        Some(skill_id)
    }

    pub fn grant_weird_caster_skill(
        &mut self,
        rng: &mut NativeRng,
        owned: &[u32],
    ) -> Option<usize> {
        let category_two: Vec<_> =
            (8..=79).filter(|id| native_skill_category(*id) == Some(2)).collect();
        if category_two.iter().filter(|id| self.permanent_ranks[**id] > 0).count() >= 2 {
            return None;
        }
        let candidates: Vec<_> =
            category_two.into_iter().filter(|id| self.permanent_ranks[*id] == 0).collect();
        if candidates.is_empty() {
            return None;
        }
        let skill_id = candidates[rng.draw_integer(candidates.len() as i32) as usize];
        let rank = NATIVE_SKILL_CATALOG[skill_id]
            .maximum_level
            .min(native_hagatha_revelation_rank(1, owned));
        assert!(rank > 0, "the native Weird Caster skill has an authored rank");
        self.permanent_ranks[skill_id] = rank;
        self.effective_ranks[skill_id] = rank;
        if !self.learned_skill_order.contains(&skill_id) {
            self.learned_skill_order.push(skill_id);
        }
        Some(skill_id)
    }

    pub fn apply_starting_revelation(&mut self, element: WizardElement) {
        let starters = match element {
            WizardElement::Ether => [8, 11],
            WizardElement::Fire => [16, 21],
            WizardElement::Air => [24, 27],
            WizardElement::Water => [32, 35],
            WizardElement::Earth => [40, 45],
        };
        for skill_id in starters {
            if self.permanent_ranks[skill_id] >= 2
                || NATIVE_SKILL_CATALOG[skill_id].maximum_level < 2
            {
                continue;
            }
            self.permanent_ranks[skill_id] = 2;
            self.effective_ranks[skill_id] = self.effective_ranks[skill_id].max(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::WizardDiscipline;

    #[test]
    fn crossing_the_first_experience_threshold_heals_and_opens_a_real_skill_offer() {
        let book = NativeSkillBookState::new(WizardElement::Ether, WizardDiscipline::Arcane);
        let mut progression = PlayerProgressionState::new(1);
        let mut combat = PlayerCombatState {
            current_health: 5.0,
            current_mana: 1.0,
            ..PlayerCombatState::new()
        };
        let mut rng = NativeRng::new(1);
        progression.grant_experience(&book, &mut combat, 90.0, &mut rng, false);
        assert_eq!(progression.level, 1);
        assert_eq!(combat.current_health, 5.0);
        progression.grant_experience(&book, &mut combat, 1.0, &mut rng, false);
        assert_eq!(progression.level, 2);
        assert_eq!(combat.current_health, combat.maximum_health);
        assert_eq!(combat.current_mana, combat.maximum_mana);
        assert_eq!(
            progression.pending_offer.as_ref().expect("the level-two offer").options.len(),
            3
        );
    }
}
