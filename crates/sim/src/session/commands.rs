//! Public player commands from `core-server/game-simulation.ts`.
//! Browser and native host share these admission checks and world side effects.

use super::*;
use crate::belt::{
    NativeBeltCommand, NativeBeltEntry, native_belt_entry_item, native_belt_equipment_slots,
};
use crate::boneyard::loot::BoneyardGoodieInteraction;
use crate::economy::{
    EquipmentSlot, HubEconomyRejection, HubEconomyResult, HubInventoryAction, find_inventory_item,
};
use crate::game_run::GameRunPhase;
use crate::player::PlayerInventoryResult;
use crate::profile::runtime::PlayerConcentrationSlot;

impl GameSession {
    #[must_use]
    pub fn can_edit_books(&self, id: &str) -> bool {
        matches!(self.run.phase, GameRunPhase::Hub | GameRunPhase::Active)
            && self.level_up_barrier.is_none()
            && self
                .participants
                .get(id)
                .is_some_and(|participant| participant.player.combat.can_accept_input())
    }

    pub fn bind_belt(&mut self, id: &str, command: NativeBeltCommand) -> bool {
        if matches!(command, NativeBeltCommand::BindItem { .. }) {
            self.close_inventory_dye_session(id, None);
        }
        if !self.can_edit_books(id) {
            return false;
        }
        let participant = self.participants.get_mut(id).unwrap();
        participant.player.belt.apply(
            command,
            &participant.player.book,
            &participant.player.economy,
        )
    }

    pub fn select_primary(&mut self, id: &str, skill_id: usize) -> bool {
        if !self.can_edit_books(id) || self.hub_primary_unset(id) {
            return false;
        }
        let mut participant = self.participants.get_mut(id).unwrap();
        let previous = participant.selected_primary_id();
        let accepted = participant.player.select_primary(skill_id);
        participant.synchronize_primary_selection(previous);
        accepted
    }

    pub fn select_concentration(&mut self, id: &str, skill_id: usize) -> bool {
        if !self.can_edit_books(id) {
            return false;
        }
        let mut participant = self.participants.get_mut(id).unwrap();
        let previous = participant.selected_primary_id();
        let accepted = participant.player.select_concentration(skill_id);
        participant.synchronize_primary_selection(previous);
        accepted
    }

    pub fn select_concentration_slot(
        &mut self,
        id: &str,
        skill_id: usize,
        slot: PlayerConcentrationSlot,
    ) -> bool {
        if !self.can_edit_books(id) {
            return false;
        }
        let mut participant = self.participants.get_mut(id).unwrap();
        let previous = participant.selected_primary_id();
        let accepted = participant.player.select_concentration_slot(skill_id, slot);
        participant.synchronize_primary_selection(previous);
        accepted
    }

    pub fn apply_inventory(
        &mut self,
        id: &str,
        action: &HubInventoryAction,
    ) -> PlayerInventoryResult {
        self.apply_inventory_with_extensions(id, action, None)
            .expect("stock inventory commands have finite native mutations")
    }

    pub fn apply_inventory_with_extensions(
        &mut self,
        id: &str,
        action: &HubInventoryAction,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) -> Result<PlayerInventoryResult, String> {
        if !matches!(
            action,
            HubInventoryAction::OpenDye { .. }
                | HubInventoryAction::CloseDye { .. }
                | HubInventoryAction::Dye { .. }
        ) {
            self.close_inventory_dye_session(id, None);
        }
        let Some(participant) = self.participants.get(id) else {
            return Ok(rejected(HubEconomyRejection::ServiceUnavailable));
        };
        if self.level_up_barrier.is_some()
            || (self.boneyard.is_none() && participant.wizard.transition.is_some())
            || (action.requires_hub_service() && !self.hub_service_available(id))
        {
            return Ok(rejected(HubEconomyRejection::ServiceUnavailable));
        }
        let mut participant = self.participants.get_mut(id).unwrap();
        let consumed = match action {
            HubInventoryAction::Consume { item_id } => {
                find_inventory_item(&participant.player.economy.backpack, *item_id).cloned()
            }
            _ => None,
        };
        if consumed.as_ref().and_then(|item| item.mod_content.as_ref()).is_some_and(|content| {
            !extensions.is_some_and(|extensions| extensions.has_consumable(&content.content_id))
        }) {
            return Ok(rejected(HubEconomyRejection::ServiceUnavailable));
        }
        let before_mana = participant.player.combat.current_mana;
        let bonus_book = match action {
            HubInventoryAction::ReadSkillBook { item_id } => {
                find_inventory_item(&participant.player.economy.backpack, *item_id)
                    .is_some_and(|item| item.native_subtype == Some(2))
            }
            _ => false,
        };
        let previous = participant.selected_primary_id();
        let mut result = participant.player.apply_inventory_with_dye_session(
            action,
            &mut self.gameplay_rng,
            extensions,
            self.inventory_dye_sessions.get(id),
        );
        if result.transaction.accepted {
            if let Some(session) = &result.transaction.dye_session {
                self.inventory_dye_sessions.insert(id.into(), session.clone());
            }
            if let HubInventoryAction::CloseDye { session_id } = action {
                self.inventory_dye_sessions
                    .retain(|owner, session| owner != id || &session.id != session_id);
            }
        }
        participant.synchronize_primary_selection(previous);
        if result.transaction.accepted
            && result.consumed_stock_potion
            && participant.player.combat.current_mana != before_mana
        {
            let delta = super::extensions::filter_mana_input(
                crate::modding::extensions::GameSimulationManaFilterInput {
                    current_mana: before_mana,
                    delta: participant.player.combat.current_mana - before_mana,
                    maximum_mana: participant.player.combat.maximum_mana,
                    player_id: id,
                    source: crate::modding::extensions::GameSimulationManaSource::StockPotion,
                    tick: self.ticks,
                },
                &participant.player.economy.equipment,
                extensions,
                "filtered mana delta",
            )?;
            participant
                .player
                .combat
                .set_mana((before_mana + delta).clamp(0.0, participant.player.combat.maximum_mana));
        }
        if result.reset_secondary_cooldowns {
            self.secondary.cooldown_rejuvenation(id);
        }
        if result.consumed_stock_potion && self.boneyard.is_some() {
            participant.score.reset_kill_streak();
        }
        if result.transaction.accepted && bonus_book {
            self.assign_automatic_skill_choices_with_extensions(&[id.into()], extensions);
            let cohort = self.level_up_participant_ids();
            self.refresh_level_up_barrier(id, &cohort);
        }
        if result.transaction.accepted
            && let Some(content) = consumed.and_then(|item| item.mod_content)
        {
            let consumption = crate::modding::inventory::GameSimulationModConsumption {
                content,
                player_id: id.into(),
                tick: self.ticks,
                use_id: self.next_mod_consumable_use_id,
            };
            if let Some(effect) = &consumption.content.consume_vfx {
                self.mod_effects.retain(|effect| {
                    effect.player_id != id || effect.content_id != consumption.content.content_id
                });
                let lifetime = (consumption.content.duration_ms / 10.0).ceil().max(1.0) as u64;
                self.mod_effects.push(crate::modding::inventory::GameSimulationModEffect {
                    color: effect.color,
                    content_id: consumption.content.content_id.clone(),
                    expires_tick: self.ticks + lifetime,
                    player_id: id.into(),
                    started_tick: self.ticks,
                    use_id: consumption.use_id,
                });
            }
            self.next_mod_consumable_use_id += 1;
            result.mod_consumption = Some(consumption);
        }
        Ok(result)
    }

    pub fn activate_belt_slot(
        &mut self,
        id: &str,
        slot: usize,
        aim: Option<crate::Vec2>,
    ) -> PlayerInventoryResult {
        self.activate_belt_slot_with_extensions(id, slot, aim, None)
            .expect("stock belt commands have finite native mutations")
    }

    pub fn activate_belt_slot_with_extensions(
        &mut self,
        id: &str,
        slot: usize,
        aim: Option<crate::Vec2>,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) -> Result<PlayerInventoryResult, String> {
        self.close_inventory_dye_session(id, None);
        if slot >= crate::belt::PLAYER_BELT_SLOT_COUNT || !self.can_edit_books(id) {
            return Ok(rejected(HubEconomyRejection::ServiceUnavailable));
        }
        let participant = &self.participants.get(id).expect("participant exists");
        let economy = &participant.player.economy;
        let Some(entry) = participant.player.belt.entries[slot] else {
            return Ok(rejected(HubEconomyRejection::IneligibleItem));
        };
        if let NativeBeltEntry::Skill { skill_id } = entry {
            if participant.player.book.effective_ranks.get(skill_id).copied().unwrap_or(0) < 1 {
                return Ok(rejected(HubEconomyRejection::IneligibleItem));
            }
            let accepted = match crate::profile::native_skill_category(skill_id) {
                Some(1) => self.select_primary(id, skill_id),
                Some(3) => self.select_concentration(id, skill_id),
                Some(2) if self.run.phase == GameRunPhase::Active && self.boneyard.is_some() => {
                    let Some(ability) =
                        crate::secondary::NativeSecondaryAbilityId::from_skill_id(skill_id)
                    else {
                        return Ok(rejected(HubEconomyRejection::ServiceUnavailable));
                    };
                    self.activate_secondary_belt_skill(id, ability, slot, aim, extensions)?;
                    true
                }
                _ => false,
            };
            return Ok(if accepted {
                accepted_inventory()
            } else {
                rejected(HubEconomyRejection::ServiceUnavailable)
            });
        }
        let Some(item) = native_belt_entry_item(entry, economy) else {
            return Ok(rejected(HubEconomyRejection::ItemNotFound));
        };
        let item_id = item.id;
        if item.native_type_id == 7001 {
            return self.apply_inventory_with_extensions(
                id,
                &HubInventoryAction::Consume { item_id },
                extensions,
            );
        }
        if item.native_type_id == 7008 {
            return self.apply_inventory_with_extensions(
                id,
                &HubInventoryAction::EquipSackContents { item_id },
                extensions,
            );
        }
        if item.equipment_type.is_none() {
            return Ok(rejected(HubEconomyRejection::IneligibleItem));
        }
        if find_inventory_item(&economy.backpack, item_id).is_some() {
            let slots =
                native_belt_equipment_slots(item, economy.owned_perk_selectors.contains(&19));
            if let Some(target) = slots
                .iter()
                .copied()
                .find(|slot| equipment_empty(economy, *slot))
                .or_else(|| slots.first().copied())
            {
                return self.apply_inventory_with_extensions(
                    id,
                    &HubInventoryAction::Equip { item_id, slot: target },
                    extensions,
                );
            }
        }
        Ok(accepted_inventory())
    }

    pub fn interact_goodie(&mut self, id: &str) -> PlayerInventoryResult {
        self.close_inventory_dye_session(id, None);
        let Some(participant) = self.participants.get_mut(id) else {
            return rejected(HubEconomyRejection::ServiceUnavailable);
        };
        if self.run.phase != GameRunPhase::Active
            || self.level_up_barrier.is_some()
            || participant.player.combat.life_state != crate::combat::PlayerLifeState::Alive
        {
            return rejected(HubEconomyRejection::ServiceUnavailable);
        }
        let Some(world) = &mut self.boneyard else {
            return rejected(HubEconomyRejection::ServiceUnavailable);
        };
        match world.loot.interact_goodie(
            &mut participant.player.economy,
            id,
            participant.wizard.position,
            participant.wizard.heading_index,
            self.ticks,
        ) {
            BoneyardGoodieInteraction::NoTarget => rejected(HubEconomyRejection::InvalidTarget),
            BoneyardGoodieInteraction::MissingKey { event } => {
                if let Some(event) = event {
                    self.retain_world_events(&[], vec![event], &[], self.ticks);
                }
                rejected(HubEconomyRejection::ItemNotFound)
            }
            BoneyardGoodieInteraction::Opened { .. } => {
                participant.player.refresh_economy();
                PlayerInventoryResult {
                    transaction: HubEconomyResult {
                        accepted: true,
                        reason: None,
                        dowsing_pitch: None,
                        dye_session: None,
                        unforge_outcome: None,
                    },
                    reset_secondary_cooldowns: false,
                    consumed_stock_potion: false,
                    mod_consumption: None,
                }
            }
        }
    }

    fn hub_service_available(&self, id: &str) -> bool {
        self.run.phase == GameRunPhase::Hub
            && self.boneyard.is_none()
            && self
                .participants
                .get(id)
                .is_some_and(|participant| participant.wizard.transition.is_none())
    }
}

fn rejected(reason: HubEconomyRejection) -> PlayerInventoryResult {
    PlayerInventoryResult {
        transaction: HubEconomyResult {
            accepted: false,
            reason: Some(reason),
            dowsing_pitch: None,
            dye_session: None,
            unforge_outcome: None,
        },
        reset_secondary_cooldowns: false,
        consumed_stock_potion: false,
        mod_consumption: None,
    }
}

fn accepted_inventory() -> PlayerInventoryResult {
    PlayerInventoryResult {
        transaction: HubEconomyResult {
            accepted: true,
            reason: None,
            dowsing_pitch: None,
            dye_session: None,
            unforge_outcome: None,
        },
        reset_secondary_cooldowns: false,
        consumed_stock_potion: false,
        mod_consumption: None,
    }
}

fn equipment_empty(economy: &crate::economy::HubEconomyState, slot: EquipmentSlot) -> bool {
    match slot {
        EquipmentSlot::Amulet => economy.equipment.amulet.is_none(),
        EquipmentSlot::Hat => economy.equipment.hat.is_none(),
        EquipmentSlot::Robe => economy.equipment.robe.is_none(),
        EquipmentSlot::Weapon => economy.equipment.weapon.is_none(),
        EquipmentSlot::Ring0 => economy.equipment.rings[0].is_none(),
        EquipmentSlot::Ring1 => economy.equipment.rings[1].is_none(),
        EquipmentSlot::Ring2 => economy.equipment.rings[2].is_none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{NewGameSelection, WizardDiscipline, WizardElement};

    #[test]
    fn every_hotbar_bank_activates_the_actual_owned_potion() {
        for slot in [7, 8, 15, 16, 23] {
            let mut state = GameSession::new(
                "owner".into(),
                NewGameSelection {
                    name: "Aurelia".into(),
                    discipline: WizardDiscipline::Arcane,
                    element: WizardElement::Ether,
                },
                GameSessionOptions::default(),
            );
            assert!(state.bind_belt("owner", NativeBeltCommand::BindItem { item_id: 1, slot }));
            let player = &mut state.participants.get_mut("owner").unwrap().player;
            player.combat.current_health = 1.0;
            let result = state.activate_belt_slot("owner", slot, None);
            assert!(
                result.transaction.accepted,
                "bank slot {slot}: {:?}",
                result.transaction.reason
            );
            assert!(result.consumed_stock_potion);
            let player = &state.participants.get("owner").expect("participant exists").player;
            assert!(player.combat.current_health > 1.0);
            assert_eq!(player.belt.entries[slot], Some(NativeBeltEntry::HealthPotion));
            assert_eq!(
                state.activate_belt_slot("owner", 24, None).transaction.reason,
                Some(HubEconomyRejection::ServiceUnavailable)
            );
        }
    }
}
