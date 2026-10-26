//! One borrowed network view over the canonical checkpoint schema.

use super::*;
use crate::boneyard::world::BoneyardSnapshot;

#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub struct GameSnapshot<'a> {
    pub inventory_dye_sessions:
        &'a std::collections::BTreeMap<String, crate::economy::NativeInventoryDyeSession>,
    pub screen_flashes: &'a crate::screen_flash::NativeScreenFlashState,
    pub enhanced_effects: &'a bool,
    pub participants: GameSnapshotParticipants<'a>,
    pub participant_order: &'a Vec<String>,
    pub next_entity_id: &'a u64,
    pub npcs: &'a HubNpcState,
    pub ambient: &'a HubAmbientState,
    pub dowser_clock: &'a HubCommonTraderClock,
    pub hub_trader_animation_seed: &'a u32,
    pub hub_memorial: &'a crate::hub::memorial::HubMemorialState,
    pub collision_rng: &'a u32,
    pub gameplay_rng: &'a NativeRng,
    pub combat_rng: &'a NativeRng,
    pub secondary: &'a NativeSecondarySimulationState,
    pub spells: &'a SpellState,
    pub hall_of_fame_clock_started_at_tick: &'a u64,
    pub painter_order: &'a WorldPainterOrder,
    pub run: &'a GameRunLifecycle,
    pub level_up_barrier: &'a Option<PlayerLevelUpBarrier>,
    pub next_level_up_barrier_id: &'a u64,
    pub mod_effects: &'a crate::ComponentEntities<GameSimulationModEffect>,
    pub next_mod_consumable_use_id: &'a u64,
    pub ticks: &'a u64,
    pub boneyard: Option<BoneyardSnapshot<'a>>,
}

impl GameSession {
    #[must_use]
    pub fn snapshot(&self) -> GameSnapshot<'_> {
        GameSnapshot {
            inventory_dye_sessions: &self.inventory_dye_sessions,
            screen_flashes: &self.screen_flashes,
            enhanced_effects: &self.enhanced_effects,
            participants: GameSnapshotParticipants { retained: &self.participants, detached: None },
            participant_order: &self.participant_order,
            next_entity_id: &self.next_entity_id,
            npcs: &self.npcs,
            ambient: &self.ambient,
            dowser_clock: &self.dowser_clock,
            hub_trader_animation_seed: &self.hub_trader_animation_seed,
            hub_memorial: &self.hub_memorial,
            collision_rng: &self.collision_rng,
            gameplay_rng: &self.gameplay_rng,
            combat_rng: &self.combat_rng,
            secondary: &self.secondary,
            spells: &self.spells,
            hall_of_fame_clock_started_at_tick: &self.hall_of_fame_clock_started_at_tick,
            painter_order: &self.painter_order,
            run: &self.run,
            level_up_barrier: &self.level_up_barrier,
            next_level_up_barrier_id: &self.next_level_up_barrier_id,
            mod_effects: &self.mod_effects,
            next_mod_consumable_use_id: &self.next_mod_consumable_use_id,
            ticks: &self.ticks,
            boneyard: self.boneyard.as_ref().map(BoneyardWorld::snapshot),
        }
    }

    /// Apply only completed, validated authority. Same-world updates retain the
    /// scenery collision and compiled schedules; a changed world requires its
    /// exact immutable source. Failure leaves the live session unchanged.
    pub fn apply_checkpoint(
        &mut self,
        mut checkpoint: GameCheckpoint,
        loaded: Option<LoadedBoneyard>,
    ) -> Result<(), String> {
        validate_checkpoint(&checkpoint)?;
        match (&self.boneyard, &checkpoint.boneyard) {
            (Some(world), Some(incoming)) if world.loaded.run_id == incoming.run_id => {
                if let Some(loaded) = &loaded
                    && (loaded.choice_id != incoming.choice_id
                        || loaded.choice_source != incoming.choice_source
                        || loaded.mod_id != incoming.mod_id
                        || loaded.mod_name != incoming.mod_name
                        || loaded.run_id != incoming.run_id
                        || loaded.seed != incoming.seed
                        || loaded.source_sha256 != incoming.source_sha256
                        || loaded.geometry_sha256 != incoming.geometry_sha256)
                {
                    return Err("snapshot immutable source differs from its world".into());
                }
                world.validate_checkpoint(incoming)?;
                let incoming = checkpoint.boneyard.take().unwrap();
                let mut world = self.boneyard.take().unwrap();
                world.apply_checkpoint(*incoming)?;
                *self = session_from_checkpoint(checkpoint, Some(world));
            }
            _ => {
                *self = Self::restore(checkpoint, loaded)?;
            }
        }
        Ok(())
    }
}

/// A normal frame borrows only the retained map. Rejoin staging borrows that
/// same map plus one privately projected participant, without cloning the world.
pub struct GameSnapshotParticipants<'a> {
    pub(super) retained: &'a Participants,
    pub(super) detached: Option<&'a SessionParticipant>,
}

impl<'a> GameSnapshotParticipants<'a> {
    #[must_use]
    pub fn get(&self, id: &str) -> Option<ParticipantRef<'a>> {
        if let Some(participant) = self.detached
            && participant.id == id
        {
            Some(participant.view())
        } else {
            self.retained.get(id)
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.retained.len() + usize::from(self.detached.is_some())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(feature = "persistence")]
impl serde::Serialize for GameSnapshotParticipants<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer
            .serialize_map(Some(self.retained.len() + usize::from(self.detached.is_some())))?;
        for (id, participant) in self.retained.iter() {
            map.serialize_entry(id, &participant)?;
        }
        if let Some(participant) = self.detached {
            map.serialize_entry(&participant.id, participant)?;
        }
        map.end()
    }
}
