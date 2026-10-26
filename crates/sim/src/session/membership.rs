//! Participant add/remove, Hub transfer and active-run detach/rejoin from the public
//! Game API. Connection generations and reservation deadlines belong to the host.

use super::*;
use crate::game_run::GameRunPhase;
use crate::painter::WorldManagerLane;
use crate::player::PlayerState;
use crate::profile::{NativeHagathaRuntimeState, NewGameSelection, SharedPlayerLevelMilestone};

/// The durable wizard account state carried across character creation and retirement.
#[derive(Clone, Debug)]
pub struct ParticipantProfile {
    pub economy: crate::economy::HubEconomyState,
    pub hagatha_runtime: NativeHagathaRuntimeState,
    pub advanced_unlocks: [bool; 8],
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct DetachedSessionParticipant {
    pub participant: SessionParticipant,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub next_entity_id: u64,
    pub run_id: String,
}

impl GameSession {
    /// `partitionGameSimulationPlayers` retains both source world values and removes
    /// the opposite cohort in entity order. No constructor, RNG draw or painter birth
    /// occurs while selecting a party for a hosted run or a save document.
    pub fn partition(&self, selected_ids: &[String]) -> Result<SessionPartition, String> {
        let selected_set: std::collections::BTreeSet<_> = selected_ids.iter().collect();
        if selected_set.len() != selected_ids.len()
            || selected_ids.iter().any(|id| !self.participants.contains_key(id))
        {
            return Err("game simulation partition requires unique existing players".into());
        }
        let mut selected = self.clone();
        let mut remaining = self.clone();
        for id in &self.participant_order {
            if selected_set.contains(id) {
                remaining.remove_participant(id);
            } else {
                selected.remove_participant(id);
            }
        }
        Ok(SessionPartition { remaining, selected })
    }

    pub fn add_participant(&mut self, id: String, selection: NewGameSelection) -> bool {
        if self.participants.contains_key(&id) {
            return false;
        }
        let player = PlayerState::new(selection, &mut self.gameplay_rng);
        let light = self.painter_order.register(WorldManagerLane::Actor);
        let mut participant = super::lifecycle::new_hub_participant(
            id.clone(),
            self.next_entity_id,
            player,
            light,
            self.hub_trader_animation_seed,
            self.hall_of_fame_clock_started_at_tick,
        );
        if let Some(world) = &self.boneyard {
            participant.wizard = world.spawn_wizard();
        }
        self.participant_order.push(id.clone());
        self.participants.insert(id, participant);
        self.next_entity_id += 1;
        self.run.synchronize_participants(&self.participant_order);
        true
    }

    /// Retire the wizard, then hydrate the fresh character from its durable
    /// profile (`retireGameSaveWizard` / `hydrateGameSaveProfile`). A shared
    /// active run must transfer that profile to its Hub rather than reset peers.
    pub fn reset_participant(&mut self, id: &str, selection: NewGameSelection) -> bool {
        if !self.participants.contains_key(id) {
            return false;
        }
        let shared_active = self.participants.len() > 1
            && (self.boneyard.is_some() || self.run.phase != GameRunPhase::Hub);
        if shared_active {
            return false;
        }
        let profile = self.retired_participant_profile(id).unwrap();
        if self.boneyard.is_some() || self.run.phase != GameRunPhase::Hub {
            self.return_to_hub();
        }
        self.remove_participant(id);
        self.add_participant_from_profile(id.into(), selection, profile)
    }

    pub fn remove_participant(&mut self, id: &str) -> bool {
        if self.participants.remove(id).is_none() {
            return false;
        }
        self.close_inventory_dye_session(id, None);
        self.participant_order.retain(|participant| participant != id);
        self.spells.projectiles.retain(|actor| actor.common().owner_id != id);
        self.spells.transients.retain(|actor| actor.common().owner_id != id);
        self.secondary.remove_owner(id);
        if let Some(world) = &mut self.boneyard {
            world.enemies.remove_cocoon_owner(id);
        }
        if let Some(barrier) = &mut self.level_up_barrier
            && barrier.participant_ids.iter().any(|participant| participant == id)
        {
            barrier.participant_ids.retain(|participant| participant != id);
            barrier.pending_player_ids.retain(|participant| participant != id);
            if barrier.participant_ids.is_empty() || barrier.pending_player_ids.is_empty() {
                self.level_up_barrier = None;
            } else if barrier.source_player_id == id {
                barrier.source_player_id = barrier.participant_ids[0].clone();
            }
        }
        self.run.synchronize_participants(&self.participant_order);
        true
    }

    pub fn detach_participant(&self, id: &str) -> Result<DetachedSessionParticipant, String> {
        if self.boneyard.is_none() || self.run.phase != GameRunPhase::Active {
            return Err("active-run detach requires a live Boneyard".into());
        }
        let participant =
            self.participants.get(id).ok_or("active-run detach has no matching player")?;
        Ok(DetachedSessionParticipant {
            participant: participant.snapshot(),
            next_entity_id: self.next_entity_id,
            run_id: self.run.run_id.clone().ok_or("active-run detach has no run identity")?,
        })
    }

    /// `game-host.ts::synchronizePartyRejoinMilestones` uses the first retained
    /// entity's current progression, rather than recreating policy from wall time.
    #[must_use]
    pub fn rejoin_milestone(
        &self,
        detached: &DetachedSessionParticipant,
    ) -> Option<SharedPlayerLevelMilestone> {
        if self.run.phase != GameRunPhase::Active
            || self.run.run_id.as_deref() != Some(&detached.run_id)
        {
            return None;
        }
        let source = self.participants.get(self.participant_order.first()?)?;
        let source = &source.player.progression;
        let previous = detached.participant.player.progression.level;
        if source.level <= previous {
            return None;
        }
        Some(SharedPlayerLevelMilestone {
            crossed_levels: (previous + 1..=source.level).collect(),
            experience: source.experience,
            level: source.level,
        })
    }

    pub fn rejoin_participant(
        &mut self,
        mut detached: DetachedSessionParticipant,
        milestone: Option<&SharedPlayerLevelMilestone>,
    ) -> Result<(), String> {
        self.validate_rejoin(&detached, milestone)?;
        let participant = &mut detached.participant;
        import_detached_character(
            participant,
            self.boneyard.as_ref().unwrap(),
            &mut self.painter_order,
        );
        if let Some(milestone) = milestone
            && !milestone.crossed_levels.is_empty()
        {
            synchronize_detached_player(participant, milestone, &mut self.gameplay_rng)?;
        }
        let id = participant.id.clone();
        participant.entity_id = self.next_entity_id;
        self.next_entity_id += 1;
        self.participant_order.push(id.clone());
        self.participants.insert(id.clone(), detached.participant);
        self.assign_automatic_skill_choices(std::slice::from_ref(&id));
        self.refresh_rejoin_barrier(&id, milestone);
        self.run.synchronize_participants(&self.participant_order);
        Ok(())
    }

    /// Offline transactions commit the gameplay RNG and that one participant;
    /// private painter allocation never consumes the active world's order.
    pub fn synchronize_detached(
        &mut self,
        detached: &mut DetachedSessionParticipant,
        milestone: &SharedPlayerLevelMilestone,
    ) -> Result<(), String> {
        let projected = self.stage_detached(detached, Some(milestone))?;
        self.gameplay_rng = projected.gameplay_rng;
        detached.participant = projected.participant;
        detached.next_entity_id = projected.next_entity_id;
        Ok(())
    }

    pub fn merge_hub(&mut self, source: &GameSession) -> Result<(), String> {
        if self.boneyard.is_some() || source.boneyard.is_some() {
            return Err("game simulation merge requires two Hub worlds".into());
        }
        for id in &source.participant_order {
            if self.participants.contains_key(id) {
                return Err(format!("Hub already contains player {id}"));
            }
        }
        for id in &source.participant_order {
            self.import_hub_participant(
                source.participants.get(id).expect("participant exists").snapshot(),
            )?;
        }
        Ok(())
    }

    /// Hub reconnect/transfer retains the actual profile and character. It
    /// enrolls a fresh lighting owner and resets source cast occupancy only.
    pub fn import_hub_participant(
        &mut self,
        mut participant: SessionParticipant,
    ) -> Result<(), String> {
        if self.boneyard.is_some() {
            return Err("Hub participant import requires a Hub world".into());
        }
        if self.participants.contains_key(&participant.id) {
            return Err("Hub participant import requires an absent identity".into());
        }
        participant.lighting =
            PlayerLightingState::new(self.painter_order.register(WorldManagerLane::Actor));
        participant.wizard.primary_cast.reset_for_selection(participant.selected_primary_id());
        participant.entity_id = self.next_entity_id;
        self.next_entity_id += 1;
        let id = participant.id.clone();
        self.participant_order.push(id.clone());
        self.participants.insert(id, participant);
        Ok(())
    }

    /// A retired save retains its economy, charges and permanent advanced unlocks.
    /// Loading it creates a fresh wizard with those durable entitlements.
    pub fn add_participant_from_profile(
        &mut self,
        id: String,
        selection: NewGameSelection,
        profile: ParticipantProfile,
    ) -> bool {
        if self.boneyard.is_some() || !self.add_participant(id.clone(), selection) {
            return false;
        }
        self.hydrate_participant_profile(&id, profile).expect("the new profile owner is present");
        true
    }

    /// `hydrateGameSaveProfile`: replace the durable economy, refresh its derived
    /// book and resources, migrate starter tints, then restore unlocks and owned effects.
    pub fn hydrate_participant_profile(
        &mut self,
        id: &str,
        profile: ParticipantProfile,
    ) -> Result<(), String> {
        let participant = self
            .participants
            .get_mut(id)
            .ok_or("game save profile owner is absent from the fresh game")?;
        let player = &mut *participant.player;
        player.economy = profile.economy;
        player.refresh_economy();
        migrate_starter_equipment_appearance(player);
        for (offset, unlocked) in profile.advanced_unlocks.into_iter().enumerate() {
            if unlocked {
                player.book.unlock_advanced(72 + offset);
            }
        }
        player.progression.hagatha_runtime = profile.hagatha_runtime;
        player.refresh_hagatha_skill_effects(&mut self.gameplay_rng);
        self.close_inventory_dye_session(id, None);
        Ok(())
    }

    /// `migratePlayerStarterEquipmentAppearance` also applies to a continued wizard.
    /// Its private offer seed owns the replacement colours; the session RNG is untouched.
    pub fn migrate_participant_starter_appearance(&mut self, id: &str) -> Result<(), String> {
        let participant = self
            .participants
            .get_mut(id)
            .ok_or("game save profile owner is absent from the fresh game")?;
        migrate_starter_equipment_appearance(participant.player);
        Ok(())
    }

    #[must_use]
    pub fn retired_participant_profile(&self, id: &str) -> Option<ParticipantProfile> {
        let participant = self.participants.get(id)?;
        let mut economy = self.retired_wizard_economy(id)?;
        if participant.tutorial_profile_economy.is_some() {
            economy.tutorial_pending = false;
        }
        Some(ParticipantProfile {
            economy,
            hagatha_runtime: participant.player.progression.hagatha_runtime,
            advanced_unlocks: participant.player.book.advanced_unlocks,
        })
    }

    #[must_use]
    pub fn retired_detached_profile(detached: &DetachedSessionParticipant) -> ParticipantProfile {
        let participant = &detached.participant;
        let economy = if let Some(baseline) = &participant.tutorial_profile_economy {
            let mut economy = (**baseline).clone();
            economy.tutorial_pending = false;
            economy
        } else {
            let mut economy = participant.player.economy.clone();
            economy.archive_completed_run(crate::economy::CompletedRunEconomyArchive {
                display_name: &participant.player.selection.name,
                ground_gold: 0,
                ground_items: &[],
                starter_element: participant.player.selection.element,
                transfer_carried_items: true,
            });
            economy
        };
        ParticipantProfile {
            economy,
            hagatha_runtime: participant.player.progression.hagatha_runtime,
            advanced_unlocks: participant.player.book.advanced_unlocks,
        }
    }

    pub(super) fn validate_rejoin(
        &self,
        detached: &DetachedSessionParticipant,
        milestone: Option<&SharedPlayerLevelMilestone>,
    ) -> Result<(), String> {
        let world = self.boneyard.as_ref().ok_or("active-run rejoin requires a live Boneyard")?;
        if self.run.phase != GameRunPhase::Active
            || self.run.run_id.as_deref() != Some(&detached.run_id)
            || world.loaded.run_id != detached.run_id
        {
            return Err("active-run rejoin requires one matching live Boneyard".into());
        }
        if self.participants.contains_key(&detached.participant.id) {
            return Err("active-run rejoin requires a detached matching player".into());
        }
        if let Some(milestone) = milestone {
            milestone.validate()?;
            let previous = detached.participant.player.progression.level;
            if milestone.level < previous
                || milestone.crossed_levels != (previous + 1..=milestone.level).collect::<Vec<_>>()
            {
                return Err("active-run rejoin level milestone is inconsistent".into());
            }
        }
        Ok(())
    }

    fn refresh_rejoin_barrier(&mut self, id: &str, milestone: Option<&SharedPlayerLevelMilestone>) {
        let (barrier, next_id) = self.project_rejoin_barrier(
            &self.participants.get(id).expect("participant exists"),
            milestone,
        );
        self.level_up_barrier = barrier;
        self.next_level_up_barrier_id = next_id;
    }

    pub(super) fn project_rejoin_barrier(
        &self,
        participant: &impl ParticipantRead,
        milestone: Option<&SharedPlayerLevelMilestone>,
    ) -> (Option<PlayerLevelUpBarrier>, u64) {
        let participant = participant.view();
        let mut cohort: Vec<_> = self.participants.keys().cloned().collect();
        if !self.participants.contains_key(participant.id) {
            cohort.push(participant.id.clone());
            cohort.sort();
        }
        let pending: Vec<_> = cohort
            .iter()
            .filter(|id| {
                let player = if *id == participant.id {
                    &participant.player
                } else {
                    &self.participants.get(id).expect("participant exists").player
                };
                player.progression.pending_offer.is_some()
                    || !player.progression.pending_levels.is_empty()
            })
            .cloned()
            .collect();
        if pending.is_empty() {
            return (None, self.next_level_up_barrier_id);
        }
        let progression = &participant.player.progression;
        let experience = milestone.map_or(progression.experience, |milestone| milestone.experience);
        let level = milestone.map_or(progression.level, |milestone| milestone.level);
        if let Some(existing) = &self.level_up_barrier {
            let mut barrier = existing.clone();
            barrier.milestone_experience = barrier.milestone_experience.max(experience);
            barrier.milestone_level = barrier.milestone_level.max(level);
            barrier.participant_ids = cohort;
            barrier.pending_player_ids = pending;
            (Some(barrier), self.next_level_up_barrier_id)
        } else {
            (
                Some(PlayerLevelUpBarrier {
                    barrier_id: self.next_level_up_barrier_id,
                    milestone_experience: experience,
                    milestone_level: level,
                    participant_ids: cohort,
                    pending_player_ids: pending,
                    run_id: self.run.run_id.clone(),
                    source_player_id: participant.id.clone(),
                }),
                self.next_level_up_barrier_id + 1,
            )
        }
    }
}

#[derive(Clone, Debug)]
pub struct SessionPartition {
    pub remaining: GameSession,
    pub selected: GameSession,
}

fn selected_primary_id(player: &PlayerState) -> u32 {
    if player.book.primary_skill_id == 52 {
        player.book.weld_build_id.expect("selected Weld has a build")
    } else {
        player.book.primary_skill_id as u32
    }
}

pub(super) fn import_detached_character(
    participant: &mut SessionParticipant,
    world: &BoneyardWorld,
    order: &mut WorldPainterOrder,
) {
    let had_weapon = participant.lighting.death_weapon_painter_registration.is_some();
    participant.lighting = PlayerLightingState::new(order.register(WorldManagerLane::Actor));
    if had_weapon {
        participant.lighting.death_weapon_painter_registration =
            Some(order.register(WorldManagerLane::Actor));
    }
    participant.wizard = world.spawn_wizard();
    participant.wizard.primary_cast.reset_for_selection(selected_primary_id(&participant.player));
    participant.college_intro = None;
    participant.world_aim = None;
}

pub(super) fn synchronize_detached_player(
    participant: &mut SessionParticipant,
    milestone: &SharedPlayerLevelMilestone,
    rng: &mut crate::NativeRng,
) -> Result<(), String> {
    let previous = participant.player.progression.level;
    participant.player.synchronize_level_milestone(milestone, rng)?;
    if participant.player.progression.level > previous {
        participant.player.combat.current_health = participant.player.combat.maximum_health;
        participant.player.combat.current_mana = participant.player.combat.maximum_mana;
    }
    Ok(())
}

fn migrate_starter_equipment_appearance(player: &mut PlayerState) {
    let primary_tint = match player.selection.element {
        crate::profile::WizardElement::Air => 0x19ffff,
        crate::profile::WizardElement::Earth => 0x00bf00,
        crate::profile::WizardElement::Ether => 0xff19ff,
        crate::profile::WizardElement::Fire => 0xff1919,
        crate::profile::WizardElement::Water => 0x1980ff,
    };
    let is_superseded = |item: &crate::economy::HubInventoryItem, hat| {
        use crate::economy::{EquipmentType, HubItemKind};
        let (equipment_type, records, name, type_id) = if hat {
            (EquipmentType::Hat, [34, 38], "Hat", 7005)
        } else {
            (EquipmentType::Robe, [64, 67], "Robe", 7006)
        };
        item.equipment_type == Some(equipment_type)
            && item.generated_level.is_none()
            && item.icon_records == records
            && item.icon_tints == Some([Some(primary_tint), Some(0xffffff)])
            && item.kind == HubItemKind::Equipment
            && item.mod_content.is_none()
            && item.mod_item_content.is_none()
            && item.mod_affixes.is_none()
            && item.name == name
            && item.native_effects.is_none()
            && item.native_selector.is_none()
            && item.native_subtype.is_none()
            && item.native_type_id == type_id
            && item.quantity == 1
            && item.rarity.is_none()
            && item.recipe_index.is_none()
    };
    if !player.economy.tutorial_pending
        && !player.economy.college_intro_pending
        && player.economy.equipment.hat.as_ref().is_some_and(|item| is_superseded(item, true))
        && player.economy.equipment.robe.as_ref().is_some_and(|item| is_superseded(item, false))
    {
        let appearance = crate::appearance::roll_native_starter_equipment_appearance(
            &mut NativeRng::new(i64::from(player.progression.offer_seed)),
            player.selection.element.into(),
        );
        player.economy.apply_starter_appearance(appearance);
        player.economy.revision += 1;
        player.refresh_economy();
    }
}
