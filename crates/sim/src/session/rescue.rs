//! Accepted rescue protection from the normal and Game Over `game-simulation.ts` phases.

use super::{GameSession, ordered_participants_mut};

impl GameSession {
    /// `damageGameSimulationPlayer`: direct authored damage uses the actual
    /// participant's receiver, then publishes its accepted loss/rescue effects once.
    pub fn damage_player(
        &mut self,
        player_id: &str,
        amount: f64,
        tick: u64,
    ) -> super::player_combat::PlayerEntityDamageResult {
        let Some(participant) = self.participants.get_mut(player_id) else {
            return super::player_combat::PlayerEntityDamageResult::default();
        };
        let received = participant.player.damage_with_result(
            amount,
            tick,
            false,
            true,
            1.0,
            participant.lighting.blindness_ticks_remaining,
        );
        let position = participant.wizard.position;
        let light_registration = participant.lighting.light_registration;
        let Some(world) = &mut self.boneyard else { return received };
        let world_key = format!("boneyard:{}", world.loaded.run_id);
        let mut flashes = self.screen_flashes.writer(tick);
        let events = super::player_feedback::materialize_player_rescue_feedback(
            world,
            &mut self.secondary,
            &mut self.painter_order,
            &mut flashes,
            super::player_feedback::PlayerRescueFeedbackInput {
                player_id,
                position,
                light_registration,
                tick,
                world_key: &world_key,
                received,
            },
        );
        if received.cheat_death_triggered {
            self.screen_flashes = flashes.finish();
        }
        if !events.is_empty() {
            super::rewards::retain_boneyard_enemy_events(world, &events, tick);
        }
        received
    }

    pub(super) fn materialize_resource_rescues(
        &mut self,
        results: &[(String, super::player_combat::PlayerCombatResult)],
        tick: u64,
    ) -> Vec<crate::enemies::model::EnemyEvent> {
        let Some(world) = &mut self.boneyard else { return Vec::new() };
        let world_key = format!("boneyard:{}", world.loaded.run_id);
        let mut flashes = self.screen_flashes.writer(tick);
        let mut events = Vec::new();
        // Resource ticks publish every owner's charm loss before any rescue event.
        // Contacts and direct commands instead finish one receiver at a time.
        for cheat_death_phase in [false, true] {
            for (id, result) in results {
                let participant = &self.participants[id];
                events.extend(super::player_feedback::materialize_player_rescue_feedback(
                    world,
                    &mut self.secondary,
                    &mut self.painter_order,
                    &mut flashes,
                    super::player_feedback::PlayerRescueFeedbackInput {
                        player_id: id,
                        position: participant.wizard.position,
                        light_registration: participant.lighting.light_registration,
                        tick,
                        world_key: &world_key,
                        received: super::player_combat::PlayerEntityDamageResult {
                            auto_health_potion_used: result.auto_health_potion_used,
                            cheat_death_triggered: cheat_death_phase
                                && result.cheat_death_triggered,
                            hagatha_charms_lost: !cheat_death_phase && result.hagatha_charms_lost,
                        },
                    },
                ));
            }
        }

        self.screen_flashes = flashes.finish();
        events
    }

    pub(super) fn step_rescue_protection(&mut self) {
        let boneyard_key =
            self.boneyard.as_ref().map(|world| format!("boneyard:{}", world.loaded.run_id));
        for participant in ordered_participants_mut(&mut self.participants, &self.participant_order)
        {
            let world_key = boneyard_key
                .clone()
                .unwrap_or_else(|| format!("hub:{}", participant.wizard.region.id()));
            participant.player.combat.rescue_protection.step(
                participant.wizard.position,
                &world_key,
                &mut self.secondary.rng,
                &mut self.painter_order,
            );
        }
    }
}
