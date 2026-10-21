//! One wizard's canonical components, composed as in `core-server/player-entity-store.ts`
//! and `game-simulation.ts`. Movement belongs to the active world; secondary clocks belong
//! to `NativeSecondarySimulationState::players`. Menus never own copies of these values.

mod console;

use crate::NativeRng;
use crate::belt::PlayerBeltComponent;
use crate::combat::PlayerCombatState;
use crate::economy::{
    HubEconomyContext, HubEconomyResult, HubEconomyState, HubInventoryAction,
    NativeUnforgeOutcomeKind, NativeUnforgeVitals, find_inventory_item,
};
use crate::equipment::native_equipment_recipe_effects;
use crate::hub::interaction::BoastFailureProducer;
use crate::profile::{
    NativePotionKind, NativeSkillBookState, NewGameSelection, PlayerProgressionState,
    PlayerSkillChoice, PlayerSkillDerivedStats, PlayerSkillRuntimeState, PlayerStatBookComponent,
    draw_native_player_creation_offer_seed, native_skill_category,
    player_skill_derived_stats_with_stat_book, refresh_player_combat_from_skill_stats,
};

/// Authored Hagatha selector values from `native-hagatha-effects.ts`.
const REVELATION: u32 = 6;
const WEIRD_CASTER: u32 = 14;
const SORCERORS_CHARM: u32 = 17;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerState {
    pub selection: NewGameSelection,
    pub book: NativeSkillBookState,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub stat_book: PlayerStatBookComponent,
    pub economy: HubEconomyState,
    pub progression: PlayerProgressionState,
    pub combat: PlayerCombatState,
    pub belt: PlayerBeltComponent,
    pub skill_runtime: PlayerSkillRuntimeState,
}

/// Effects owned by the surrounding world, returned after a committed inventory command.
pub struct PlayerInventoryResult {
    pub transaction: HubEconomyResult,
    pub reset_secondary_cooldowns: bool,
    pub consumed_stock_potion: bool,
    pub mod_consumption: Option<crate::modding::inventory::GameSimulationModConsumption>,
}

impl PlayerState {
    pub(crate) fn fail_boast_with_extensions(
        &mut self,
        producer: BoastFailureProducer,
        extensions: Option<&dyn crate::modding::extensions::GameSimulationExtensions>,
    ) {
        let before = self.economy.npc.boast.clone();
        let rules = before.selected.as_ref().and_then(|selection| {
            crate::modding::extensions::game_boast_rules(selection, extensions)
        });
        self.economy.npc.boast.fail_with_rules(producer, rules);
        if self.economy.npc.boast != before {
            self.economy.revision += 1;
            self.refresh_economy();
        }
    }

    #[must_use]
    pub fn new(selection: NewGameSelection, gameplay_rng: &mut NativeRng) -> Self {
        let offer_seed = draw_native_player_creation_offer_seed(gameplay_rng);
        let economy = HubEconomyState::new(i64::from(offer_seed), selection.element);
        let mut book = NativeSkillBookState::new(selection.element, selection.discipline);
        let stat_book = PlayerStatBookComponent::default();
        let skill_runtime =
            PlayerSkillRuntimeState::new_with_stat_book(&mut book, &stat_book, &economy);
        let belt = PlayerBeltComponent::new(&book);
        Self {
            selection,
            book,
            stat_book,
            economy,
            progression: PlayerProgressionState::new(offer_seed),
            combat: PlayerCombatState::new(),
            belt,
            skill_runtime,
        }
    }

    #[must_use]
    pub fn derived_stats(&self) -> PlayerSkillDerivedStats {
        player_skill_derived_stats_with_stat_book(
            &self.skill_runtime,
            &self.book,
            &self.stat_book,
            &self.progression,
            &self.economy,
        )
    }

    #[must_use]
    pub fn concentrations(&self) -> Vec<usize> {
        [self.skill_runtime.concentration_skill_id_a, self.skill_runtime.concentration_skill_id_b]
            .into_iter()
            .flatten()
            .collect()
    }

    /// `autofillPlayerEntitySkillSelections`: choices and derived combat capacities
    /// commit together. The caller resets the wizard's cast only if its primary changed.
    pub fn autofill_skill_selections(&mut self, rng: &mut NativeRng) -> bool {
        let previous = self.book.clone();
        self.skill_runtime.autofill_selections_with_stat_book(
            &mut self.book,
            &self.stat_book,
            &self.economy,
            rng,
        );
        self.refresh(&previous);
        previous.primary_skill_id != self.book.primary_skill_id
            || (self.book.primary_skill_id == 52
                && previous.weld_build_id != self.book.weld_build_id)
    }

    pub fn select_primary(&mut self, skill_id: usize) -> bool {
        if native_skill_category(skill_id) != Some(1)
            || self.book.effective_ranks.get(skill_id).is_none_or(|rank| *rank == 0)
        {
            return false;
        }
        let previous = self.book.clone();
        self.book.select_primary(skill_id);
        self.refresh(&previous);
        true
    }

    pub fn select_concentration(&mut self, skill_id: usize) -> bool {
        if self.progression.mind_chug_ticks_remaining > 0 {
            return false;
        }
        let previous = self.book.clone();
        if !self.skill_runtime.set_concentration_with_stat_book(
            &mut self.book,
            &self.stat_book,
            &self.economy,
            skill_id,
        ) {
            return false;
        }
        self.refresh(&previous);
        true
    }

    pub fn select_concentration_slot(
        &mut self,
        skill_id: usize,
        slot: crate::profile::runtime::PlayerConcentrationSlot,
    ) -> bool {
        if self.progression.mind_chug_ticks_remaining > 0 {
            return false;
        }
        let previous = self.book.clone();
        if !self.skill_runtime.set_concentration_slot_with_stat_book(
            &mut self.book,
            &self.stat_book,
            &self.economy,
            skill_id,
            slot,
        ) {
            return false;
        }
        self.refresh(&previous);
        true
    }

    pub(crate) fn economy_context(&self) -> HubEconomyContext {
        HubEconomyContext {
            player_level: self.progression.level,
            // Equipment admission uses learned Creativity before equipment bonuses.
            creativity_rank: self.book.permanent_ranks[63],
            advanced_unlocks: self.book.advanced_unlocks,
            vitals: NativeUnforgeVitals {
                current_health: self.combat.current_health,
                current_mana: self.combat.current_mana,
                maximum_health: self.combat.maximum_health,
                maximum_mana: self.combat.maximum_mana,
            },
        }
    }

    /// World phase, doorway and NPC range admission is checked by the session before here.
    pub fn apply_inventory(
        &mut self,
        action: &HubInventoryAction,
        gameplay_rng: &mut NativeRng,
    ) -> PlayerInventoryResult {
        self.apply_inventory_with_extensions(action, gameplay_rng, None)
    }

    pub fn apply_inventory_with_extensions(
        &mut self,
        action: &HubInventoryAction,
        gameplay_rng: &mut NativeRng,
        extensions: Option<&dyn crate::modding::extensions::GameSimulationExtensions>,
    ) -> PlayerInventoryResult {
        self.apply_inventory_with_dye_session(action, gameplay_rng, extensions, None)
    }

    pub fn apply_inventory_with_dye_session(
        &mut self,
        action: &HubInventoryAction,
        gameplay_rng: &mut NativeRng,
        extensions: Option<&dyn crate::modding::extensions::GameSimulationExtensions>,
        dye_session: Option<&crate::economy::NativeInventoryDyeSession>,
    ) -> PlayerInventoryResult {
        let previous_book = self.book.clone();
        let previous_revision = self.economy.revision;
        let consumed = match action {
            HubInventoryAction::Consume { item_id } => {
                find_inventory_item(&self.economy.backpack, *item_id).cloned()
            }
            _ => None,
        };
        let read_book = match action {
            HubInventoryAction::ReadSkillBook { item_id } => {
                find_inventory_item(&self.economy.backpack, *item_id).cloned()
            }
            _ => None,
        };
        let magical_equipment = match action {
            HubInventoryAction::Equip { item_id, .. } => {
                find_inventory_item(&self.economy.backpack, *item_id).is_some_and(|item| {
                    item.native_effects.as_ref().map_or_else(
                        || {
                            item.recipe_index.is_some_and(|index| {
                                !native_equipment_recipe_effects(index).is_empty()
                            })
                        },
                        |effects| !effects.is_empty(),
                    )
                })
            }
            _ => false,
        };
        let purchased_selectors = match action {
            HubInventoryAction::BuyHagatha { selector } => self
                .economy
                .hagatha_offers()
                .into_iter()
                .find(|offer| offer.selector == *selector)
                .map_or_else(Vec::new, |offer| offer.members)
                .into_iter()
                .filter(|selector| {
                    ![REVELATION, WEIRD_CASTER].contains(selector)
                        || !self.economy.first_mixed_selectors.contains(selector)
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        let context = self.economy_context();
        let transaction =
            self.economy.apply_with_dye_session(action, &context, extensions, dye_session);
        self.economy.record_action_feedback(action, &transaction);
        // The host publishes feedback for rejected commands too, without consuming item RNG.
        if !matches!(action, HubInventoryAction::AcknowledgeNpcHint { .. }) {
            self.economy.revision = self.economy.revision.max(previous_revision + 1);
        }
        self.refresh(&previous_book);
        let mut result = PlayerInventoryResult {
            transaction,
            reset_secondary_cooldowns: false,
            consumed_stock_potion: false,
            mod_consumption: None,
        };
        if !result.transaction.accepted {
            return result;
        }
        match action {
            HubInventoryAction::BuyHagatha { .. } => {
                self.apply_hagatha_purchase(&purchased_selectors, gameplay_rng)
            }
            HubInventoryAction::RemoveHagatha { selector } => {
                let previous = self.progression.hagatha_runtime;
                self.progression.hagatha_runtime.remove(*selector);
                if self.progression.hagatha_runtime != previous {
                    self.progression.revision += 1;
                }
                self.refresh(&self.book.clone());
            }
            HubInventoryAction::BuyTeacherSpell { skill_id } => {
                let previous = self.book.clone();
                assert!(
                    self.book.unlock_advanced(*skill_id),
                    "accepted teacher purchase must unlock its skill"
                );
                self.refresh(&previous);
            }
            _ => {}
        }
        if let Some(item) = consumed {
            if item.native_type_id == 7001 {
                self.fail_boast_with_extensions(BoastFailureProducer::PotionUse, extensions);
                result.consumed_stock_potion = item.mod_content.is_none();
            }
            if item.mod_content.is_none()
                && let Some(kind) =
                    item.native_subtype.and_then(NativePotionKind::from_native_subtype)
            {
                self.progression.apply_potion(kind, &mut self.combat);
            }
        }
        if magical_equipment {
            self.fail_boast_with_extensions(BoastFailureProducer::MagicalEquipment, extensions);
        }
        if let Some(outcome) = &result.transaction.unforge_outcome {
            match outcome.kind {
                NativeUnforgeOutcomeKind::FullRejuvenation => {
                    self.combat.current_health = self.combat.maximum_health;
                    self.combat.current_mana = self.combat.maximum_mana;
                    self.progression.revision += 1;
                    result.reset_secondary_cooldowns = true;
                }
                NativeUnforgeOutcomeKind::MindDredge => self.progression.grant_mind_dredge(),
                _ => {}
            }
        }
        if let Some(item) = read_book {
            let outcome = match item.native_subtype {
                Some(2) => {
                    self.grant_bonus_skill_choice(gameplay_rng);
                    crate::economy::NativeSkillBookOutcome::Choice
                }
                Some(3) => {
                    let skill_id = self.increase_random_skill(gameplay_rng);
                    crate::economy::NativeSkillBookOutcome::Rank { skill_id }
                }
                _ => unreachable!("an accepted skill book has a native choice or rank subtype"),
            };
            self.economy
                .action_feedback
                .as_mut()
                .expect("an accepted inventory read has its transaction receipt")
                .skill_book_outcome = Some(outcome);
        }
        result
    }

    pub fn grant_experience(&mut self, amount: f64, gameplay_rng: &mut NativeRng) {
        let previous = self.progression.pending_offer.as_ref().map(|offer| offer.sequence);
        self.progression.grant_experience(
            &self.book,
            &mut self.combat,
            amount,
            gameplay_rng,
            self.economy.owned_perk_selectors.contains(&SORCERORS_CHARM),
        );
        self.mark_new_offer_insight(previous, gameplay_rng);
    }

    pub fn synchronize_level_milestone(
        &mut self,
        milestone: &crate::profile::SharedPlayerLevelMilestone,
        rng: &mut NativeRng,
    ) -> Result<(), String> {
        let previous = self.progression.pending_offer.as_ref().map(|offer| offer.sequence);
        let charm = self.owns_sorcerors_charm();
        self.progression.synchronize_level_milestone(
            &self.book,
            self.combat.maximum_mana,
            milestone,
            rng,
            charm,
        )?;
        self.mark_new_offer_insight(previous, rng);
        Ok(())
    }

    pub fn choose_skill(
        &mut self,
        choice: PlayerSkillChoice,
        gameplay_rng: &mut NativeRng,
    ) -> bool {
        let previous = self.book.clone();
        if !self.progression.choose_skill(
            &mut self.book,
            choice,
            gameplay_rng,
            &self.economy.owned_perk_selectors,
        ) {
            return false;
        }
        self.refresh(&previous);
        self.skill_runtime.autofill_selections_with_stat_book(
            &mut self.book,
            &self.stat_book,
            &self.economy,
            gameplay_rng,
        );
        self.refresh(&self.book.clone());
        self.progression.open_next_offer(
            &self.book,
            self.combat.maximum_mana,
            gameplay_rng,
            self.owns_sorcerors_charm(),
        );
        self.mark_new_offer_insight(None, gameplay_rng);
        true
    }

    /// `replacePlayerEntitySkillChoiceWithMod` opens/finalizes the next pending native
    /// offer after the authored host has accepted its own skill reward transaction.
    pub fn replace_skill_with_mod(
        &mut self,
        offer_sequence: u64,
        gameplay_rng: &mut NativeRng,
    ) -> bool {
        if !self.progression.replace_choice_with_mod(offer_sequence) {
            return false;
        }
        self.progression.open_next_offer(
            &self.book,
            self.combat.maximum_mana,
            gameplay_rng,
            self.owns_sorcerors_charm(),
        );
        self.mark_new_offer_insight(Some(offer_sequence), gameplay_rng);
        true
    }

    /// The Game host commits the seed draw only when the current offer accepts a reroll.
    pub fn reroll_skill_offer(
        &mut self,
        offer_sequence: u64,
        gameplay_rng: &mut NativeRng,
    ) -> bool {
        if !self.owns_sorcerors_charm() {
            return false;
        }
        let mut next_rng = gameplay_rng.clone();
        let next_seed = next_rng.draw_integer(1_000_000) as u32;
        if !self.progression.reroll_offer(
            &self.book,
            self.combat.maximum_mana,
            offer_sequence,
            next_seed,
            &mut next_rng,
        ) {
            return false;
        }
        self.mark_new_offer_insight(Some(offer_sequence), &mut next_rng);
        *gameplay_rng = next_rng;
        true
    }

    pub fn defer_skill_choice(
        &mut self,
        offer_sequence: u64,
        gameplay_rng: &mut NativeRng,
    ) -> bool {
        let owned = self.owns_sorcerors_charm();
        if !self.progression.defer_choice(
            &self.book,
            self.combat.maximum_mana,
            offer_sequence,
            gameplay_rng,
            owned,
        ) {
            return false;
        }
        self.mark_new_offer_insight(Some(offer_sequence), gameplay_rng);
        true
    }

    fn apply_hagatha_purchase(&mut self, selectors: &[u32], gameplay_rng: &mut NativeRng) {
        self.progression.hagatha_runtime.purchase(selectors);
        self.refresh_hagatha_skill_effects(gameplay_rng);
    }

    /// `refreshPlayerEntityHagathaSkillEffects`: restore owned skill effects without
    /// replaying purchase-only charges. An actual Weird Caster acquisition reseeds offers.
    pub fn refresh_hagatha_skill_effects(&mut self, gameplay_rng: &mut NativeRng) -> Option<usize> {
        let previous = self.book.clone();
        if self.economy.owned_perk_selectors.contains(&REVELATION) {
            self.book.apply_starting_revelation(self.selection.element);
        }
        let granted_skill_id = if self.economy.owned_perk_selectors.contains(&WEIRD_CASTER) {
            self.book.grant_weird_caster_skill(gameplay_rng, &self.economy.owned_perk_selectors)
        } else {
            None
        };
        if granted_skill_id.is_some() {
            self.progression.apply_acquisition_offer_seeds(gameplay_rng, 1);
        }
        self.refresh(&previous);
        self.skill_runtime.autofill_selections_with_stat_book(
            &mut self.book,
            &self.stat_book,
            &self.economy,
            gameplay_rng,
        );
        self.refresh(&self.book.clone());
        if granted_skill_id.is_some()
            && let Some(offer) = &self.progression.pending_offer
        {
            self.progression.pending_offer = Some(crate::profile::build_player_skill_offer(
                &self.progression,
                &self.book,
                self.combat.maximum_mana,
                offer.sequence,
                gameplay_rng,
            ));
            self.progression.revision += 1;
            self.mark_new_offer_insight(None, gameplay_rng);
        }
        granted_skill_id
    }

    /// A world transition keeps the learned book but replaces transient combat/runtime state.
    /// Entering a Boneyard preserves selected concentrations; returning to College clears them.
    pub fn reset_for_new_run(&mut self, preserve_concentrations: bool) {
        let previous = &self.skill_runtime;
        let concentrations = (
            previous.concentration_skill_id_a,
            previous.concentration_skill_id_b,
            previous.next_concentration_replacement_slot,
        );
        self.skill_runtime = PlayerSkillRuntimeState::new_with_stat_book(
            &mut self.book,
            &self.stat_book,
            &self.economy,
        );
        if preserve_concentrations {
            self.skill_runtime.concentration_skill_id_a = concentrations.0;
            self.skill_runtime.concentration_skill_id_b = concentrations.1;
            self.skill_runtime.next_concentration_replacement_slot = concentrations.2;
            self.skill_runtime.refresh_with_stat_book(
                &mut self.book,
                &self.stat_book,
                &self.economy,
            );
        }
        self.combat.reset_for_new_run();
        self.progression.reset_potion_effects();
        let derived = self.derived_stats();
        refresh_player_combat_from_skill_stats(&mut self.combat, &derived);
    }

    /// The stock Tutorial replaces only its starter equipment and two starter potions.
    pub fn prepare_tutorial_loadout(&mut self) {
        use crate::economy::project_inventory_items;
        use crate::profile::native_hagatha_revelation_rank;

        let previous = self.book.clone();
        self.book.permanent_ranks[11] = 0;
        self.book.effective_ranks[11] = 0;
        let rank = native_hagatha_revelation_rank(1, &self.economy.owned_perk_selectors);
        self.book.permanent_ranks[72] = self.book.permanent_ranks[72].max(rank);
        self.book.effective_ranks[72] = self.book.effective_ranks[72].max(rank);
        self.book.learned_skill_order.retain(|id| *id != 11 && *id != 72);
        self.book.learned_skill_order.push(72);
        self.book.primary_skill_id = 8;
        self.book.weld_build_id = None;
        self.book.weld_component_ranks = None;
        for subtype in [0, 1] {
            let potion_id = project_inventory_items(&self.economy.backpack)
                .into_iter()
                .find(|entry| {
                    entry.item.native_type_id == 7001 && entry.item.native_subtype == Some(subtype)
                })
                .map(|entry| entry.item.id);
            if let Some(item_id) = potion_id {
                let result = self
                    .economy
                    .apply(&HubInventoryAction::Discard { item_id }, &HubEconomyContext::default());
                assert!(result.accepted, "Tutorial starter potion must be discardable");
            }
        }
        self.economy
            .apply_starter_appearance(crate::appearance::NATIVE_TUTORIAL_EQUIPMENT_APPEARANCE);
        self.refresh(&previous);
        self.belt = PlayerBeltComponent::new(&self.book);
    }

    pub fn grant_bonus_skill_choice(&mut self, rng: &mut NativeRng) {
        let previous = self.progression.pending_offer.as_ref().map(|offer| offer.sequence);
        let charm = self.owns_sorcerors_charm();
        self.progression.grant_bonus_skill_choice(&self.book, self.combat.maximum_mana, rng, charm);
        self.mark_new_offer_insight(previous, rng);
    }

    pub fn increase_random_skill(&mut self, rng: &mut NativeRng) -> Option<usize> {
        let previous = self.book.clone();
        let id = self.book.increase_random_learned_skill(rng)?;
        self.progression.apply_acquisition_offer_seeds(rng, 1);
        self.refresh(&previous);
        self.skill_runtime.autofill_selections_with_stat_book(
            &mut self.book,
            &self.stat_book,
            &self.economy,
            rng,
        );
        if let Some(offer) = &self.progression.pending_offer {
            let sequence = offer.sequence;
            self.progression.pending_offer = Some(crate::profile::build_player_skill_offer(
                &self.progression,
                &self.book,
                self.combat.maximum_mana,
                sequence,
                rng,
            ));
            self.progression.revision += 1;
            self.mark_new_offer_insight(None, rng);
        }
        Some(id)
    }

    /// `replacePlayerLoadout` retains durable unlocks and economy while replacing
    /// learned/runtime/combat components. A new loadout rearms its owned one-use perks.
    pub fn replace_loadout(
        &mut self,
        selection: NewGameSelection,
        offer_seed: u32,
        appearance_owner: Option<crate::appearance::StarterColorOwner>,
    ) {
        if let Some(owner) = appearance_owner {
            let appearance = crate::appearance::roll_native_starter_equipment_appearance(
                &mut NativeRng::new(i64::from(offer_seed)),
                owner,
            );
            self.economy.apply_starter_appearance(appearance);
            self.economy.revision += 1;
        }
        let advanced_unlocks = self.book.advanced_unlocks;
        let revision = self.progression.revision;
        self.selection = selection;
        self.book = NativeSkillBookState::new(self.selection.element, self.selection.discipline);
        self.book.advanced_unlocks = advanced_unlocks;
        self.stat_book = PlayerStatBookComponent::default();
        self.skill_runtime = PlayerSkillRuntimeState::new_with_stat_book(
            &mut self.book,
            &self.stat_book,
            &self.economy,
        );
        self.belt = PlayerBeltComponent::new(&self.book);
        self.progression = PlayerProgressionState::new(offer_seed);
        self.progression.hagatha_runtime.purchase(&self.economy.owned_perk_selectors);
        self.progression.revision = revision + 1;
        self.progression.welding_offer_bias =
            self.skill_runtime.equipment_modifiers.feature_bits & 0x1000 != 0;
        self.progression.discipline_offer_bias =
            self.economy.owned_perk_selectors.contains(&WEIRD_CASTER);
        self.combat = PlayerCombatState::new();
        let derived = self.derived_stats();
        refresh_player_combat_from_skill_stats(&mut self.combat, &derived);
    }

    pub(crate) fn refresh_economy(&mut self) {
        let previous = self.book.clone();
        self.refresh(&previous);
    }

    pub fn set_mindstar(&mut self, active: bool) {
        let previous = self.book.clone();
        self.skill_runtime.set_mindstar_active_with_stat_book(
            active,
            &mut self.book,
            &self.stat_book,
            &self.economy,
        );
        self.refresh(&previous);
    }

    fn refresh(&mut self, previous: &NativeSkillBookState) {
        self.skill_runtime.refresh_with_stat_book(&mut self.book, &self.stat_book, &self.economy);
        self.belt.autofill_newly_learned(previous, &self.book);
        self.belt.refresh(&self.book, &self.economy);
        // Native equipment feature kind 39 is Weld Calling.
        self.progression.welding_offer_bias =
            self.skill_runtime.equipment_modifiers.feature_bits & 0x1000 != 0;
        self.progression.discipline_offer_bias =
            self.economy.owned_perk_selectors.contains(&WEIRD_CASTER);
        if previous != &self.book {
            self.progression.revision += 1;
        }
        let derived = self.derived_stats();
        refresh_player_combat_from_skill_stats(&mut self.combat, &derived);
    }

    fn owns_sorcerors_charm(&self) -> bool {
        self.economy.owned_perk_selectors.contains(&SORCERORS_CHARM)
    }

    fn mark_new_offer_insight(
        &mut self,
        previous_sequence: Option<u64>,
        gameplay_rng: &mut NativeRng,
    ) {
        if let Some(offer) = &mut self.progression.pending_offer
            && Some(offer.sequence) != previous_sequence
        {
            self.skill_runtime.mark_creativity_insight_with_stat_book(
                offer,
                &self.book,
                &self.stat_book,
                gameplay_rng,
            );
        }
    }
}
