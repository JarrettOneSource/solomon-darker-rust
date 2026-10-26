//! Source recovery membership uses a retained participant snapshot while its actor is
//! absent. Disconnect never creates another simulation or leaves an offline body behind.

use solomon_assets::website_save::SaveIntegrity;
use solomon_protocol::ResumeGraceReason;
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::session::{ParticipantRead, ParticipantRef};
use solomon_sim::social::{PartyIdentity, broker::GameSocialConnection};

use super::{HostedGameHost, HostedRecoveryLineage, HostedRejoinSlot, HostedWorlds};
use crate::host::connection::HostConnection;
use crate::host::recovery::PartyRecoveryRosterMember;
use crate::host::signing::random_token;

impl HostedGameHost {
    pub(in crate::host) fn retained_participant(&self, id: &str) -> Option<ParticipantRef<'_>> {
        self.worlds.state_for_player(id).and_then(|state| state.participant(id)).or_else(|| {
            self.rejoin_slots.iter().find_map(|slot| {
                (slot.player_id == id)
                    .then_some(slot.detached.as_ref()?)
                    .map(|detached| detached.participant.view())
            })
        })
    }

    pub(in crate::host) fn arm_party_rejoins(&mut self, party_id: &str) -> Result<(), String> {
        if self.editor_test {
            return Ok(());
        }
        let Some(party) = self.worlds.parties().parties.iter().find(|party| party.id == party_id)
        else {
            return Ok(());
        };
        let Some(state) =
            party.member_player_ids.iter().find_map(|id| self.worlds.state_for_player(id))
        else {
            return Ok(());
        };
        if state.run.phase != GameRunPhase::Active
            || state.boneyard.as_ref().is_none_or(|world| world.tutorial.is_some())
        {
            return Ok(());
        }
        let run_id = state.run.run_id.clone().ok_or("An active run has no identity")?;
        let member_ids = state.participant_order.clone();
        let existing = self
            .recovery_lineages
            .iter()
            .find(|lineage| lineage.party_id == party_id && lineage.run_id == run_id);
        let recovery_id = match existing {
            Some(lineage) => lineage.recovery_id.clone(),
            None => {
                let recovery_id = random_token::<32>()?;
                let mut roster = Vec::new();
                for id in &party.member_player_ids {
                    let Some(participant) = self.retained_participant(id) else { continue };
                    let player = &participant.player;
                    roster.push(PartyRecoveryRosterMember {
                        current_health: player.combat.current_health,
                        display_name: player.selection.name.clone(),
                        element: player.selection.element,
                        life_state: player.combat.life_state,
                        maximum_health: player.combat.maximum_health,
                        player_id: id.clone(),
                    });
                }
                self.recovery_lineages.push(HostedRecoveryLineage {
                    content: self
                        .players
                        .iter()
                        .find(|player| member_ids.contains(&player.id))
                        .ok_or("The active run has no admitted content owner")?
                        .content
                        .clone(),
                    party_leader_player_id: party.leader_player_id.clone(),
                    party_id: party_id.into(),
                    party_member_count: party.member_player_ids.len(),
                    party_roster: roster,
                    party_visibility: party.visibility,
                    recovery_id: recovery_id.clone(),
                    run_id: run_id.clone(),
                });
                recovery_id
            }
        };
        for player in &self.players {
            if !member_ids.contains(&player.id) {
                continue;
            }
            if let Some(slot) = self
                .rejoin_slots
                .iter_mut()
                .find(|slot| slot.recovery_id == recovery_id && slot.player_id == player.id)
            {
                slot.connected = true;
                slot.detached = None;
                slot.reservation = None;
                continue;
            }
            self.rejoin_slots.push(HostedRejoinSlot {
                connected: true,
                content: player.content.clone(),
                developer_access: player.developer_access,
                detached: None,
                global_score_eligible: player.global_score_eligible,
                leaderboard_user_id: player.leaderboard_user_id,
                local_only: player.save_metadata.integrity == SaveIntegrity::LocalOnly,
                party_id: party_id.into(),
                party_identity: None,
                player_id: player.id.clone(),
                player_reference: Some(player.player_reference.clone()),
                profile: player.profile.clone(),
                recovery_id: recovery_id.clone(),
                reservation: None,
                run_id: run_id.clone(),
                token: None,
            });
        }
        Ok(())
    }

    pub(in crate::host) fn synchronize_rejoins(&mut self) -> Result<(), String> {
        // Source localeCompare order is observable through shared progression entropy.
        let mut indexes: Vec<_> = self
            .rejoin_slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.detached.is_some())
            .map(|(index, _)| index)
            .collect();
        indexes.sort_by(|&left, &right| {
            self.source_collator
                .compare(&self.rejoin_slots[left].player_id, &self.rejoin_slots[right].player_id)
        });
        for index in indexes {
            let slot = &mut self.rejoin_slots[index];
            let state = match &mut self.worlds {
                HostedWorlds::Global(worlds) => worlds
                    .runs
                    .iter_mut()
                    .find(|run| {
                        run.party_id == slot.party_id
                            && run.state.run.run_id.as_ref() == Some(&slot.run_id)
                    })
                    .map(|run| &mut run.state),
                HostedWorlds::Private { state, .. } => Some(state),
            };
            let Some(state) = state else { continue };
            let detached = slot.detached.as_mut().expect("the selected slot is detached");
            if let Some(milestone) = state.rejoin_milestone(detached) {
                state.synchronize_detached(detached, &milestone)?;
            }
        }
        Ok(())
    }

    pub(in crate::host) fn disconnected(
        &mut self,
        connection: HostConnection,
        now_ms: f64,
        close: crate::transport::TransportClose,
    ) -> Result<Option<GameSocialConnection>, String> {
        let source = connection.planned_disconnect.as_ref().map(|(source, _)| *source).unwrap_or(
            if connection.peer.welcome_timed_out() {
                "welcome-transfer-timeout"
            } else {
                match close.code {
                    Some(1000) => "client-close",
                    Some(1001) => "peer-going-away",
                    Some(1006) | None => "transport-lost",
                    _ => "transport-close",
                }
            },
        );
        if let Some(observer) = self.observers.remove(&connection.peer.id()) {
            self.emit_runtime_event("observer.disconnected", "A developer observer disconnected from an active match.",
                serde_json::json!({"closeCode":close.code,"closeReason":close.reason,
                    "disconnectReason":connection.planned_disconnect.as_ref().map(|(_, reason)| reason.as_str())
                        .filter(|reason| !reason.is_empty()).unwrap_or(if close.reason.is_empty(){"no reason received"}else{&close.reason}),
                    "durationMs":(crate::host::source_wall_clock_ms()-connection.connected_at_utc_ms
                        .unwrap_or_else(crate::host::source_wall_clock_ms)).max(0.0),
                    "disconnectSource":source,"observerId":observer.observer_id,
                    "observerUserId":observer.requested_by_user_id,"observerUsername":observer.requested_by_username,
                    "runId":observer.run_id}));
            return Ok(None);
        }
        let Some(id) = connection.player_id else { return Ok(None) };
        let Some(index) = self
            .players
            .iter()
            .position(|player| player.id == id && player.peer_id == connection.peer.id())
        else {
            // A late close from a replaced socket cannot remove its new generation.
            return Ok(None);
        };
        self.checkpoint_run_for_player(&id);
        let player = self.players.remove(index);
        let server_tick = self.worlds.state_for_player(&id).map_or(0, |state| state.ticks);
        let display_name = self
            .retained_participant(&id)
            .map(|participant| participant.player.selection.name.clone())
            .unwrap_or_else(|| id.clone());
        self.college_intro_ready.remove(&id);
        let party = self.worlds.parties().party_for_player(&id).cloned();
        let detached = self
            .worlds
            .state_for_player(&id)
            .filter(|state| state.run.phase == GameRunPhase::Active && state.boneyard.is_some())
            .map(|state| state.detach_participant(&id))
            .transpose()?;
        let run_id = detached.as_ref().map(|detached| detached.run_id.clone());
        let last_living = self.worlds.state_for_player(&id).is_some_and(|state| {
            state.participant(&id).is_some_and(|participant| {
                participant.player.combat.life_state == PlayerLifeState::Alive
            }) && !state.participant_order.iter().any(|other| {
                other != &id
                    && state
                        .participants
                        .get(other)
                        .expect("participant exists")
                        .player
                        .combat
                        .life_state
                        == PlayerLifeState::Alive
            })
        });
        let slot = self
            .rejoin_slots
            .iter_mut()
            .find(|slot| slot.player_id == id && run_id.as_ref() == Some(&slot.run_id));
        let retained_membership = slot.is_some();
        if let Some(slot) = slot {
            slot.connected = false;
            if detached.is_some() {
                slot.detached = detached;
            }
            slot.reservation = None;
            slot.party_identity = party.as_ref().and_then(|party| {
                Some(PartyIdentity {
                    id: party.id.clone(),
                    join_code: party.join_code.clone()?,
                    listing_id: party.listing_id.clone()?,
                })
            });
        }
        match &mut self.worlds {
            HostedWorlds::Global(worlds) if retained_membership => worlds.detach_player(&id)?,
            HostedWorlds::Global(worlds) => worlds.remove_player(&id)?,
            HostedWorlds::Private { state, parties } => {
                state.remove_participant(&id);
                if !retained_membership {
                    parties.remove_player(&id);
                }
            }
        }
        if let Some(party) = party {
            let facts = self.control_facts(&party.id);
            let control = self.controls.entry(party.id.clone()).or_default();
            if retained_membership && last_living && !facts.connected.is_empty() {
                control.begin_loading_grace(
                    &id,
                    ResumeGraceReason::PartyRejoinWait,
                    &facts.connected,
                    std::slice::from_ref(&id),
                );
            }
            control.disconnected(&id, &facts.context(), now_ms);
            self.neutralize_party(&party.id);
            if retained_membership
                && facts.materialized.is_empty()
                && let Some(run_id) = run_id
            {
                self.retire_recovery_run(&run_id, false);
            }
            if let HostedWorlds::Global(worlds) = &self.worlds
                && !worlds.runs.iter().any(|run| run.party_id == party.id)
            {
                self.close_party_mod_runtime(&party.id);
            }
        }
        if self.console.as_ref().is_some_and(|console| console.owner_player_id == id)
            || self.players.is_empty()
        {
            self.reset_lua_runtime();
        }
        if self.players.is_empty() {
            self.remove_all_ml_bots()?;
        }
        self.retire_empty_private_college_run();
        self.emit_runtime_event("player.disconnected", "A player disconnected from the game host.",
            serde_json::json!({"closeCode":close.code,"closeReason":close.reason,
                "disconnectReason":connection.planned_disconnect.as_ref().map(|(_, reason)| reason.as_str())
                    .filter(|reason| !reason.is_empty()).unwrap_or(if close.reason.is_empty(){"no reason received"}else{&close.reason}),
                "disconnectSource":source,"displayName":display_name,
                "durationMs":(crate::host::source_wall_clock_ms()-connection.connected_at_utc_ms
                    .unwrap_or_else(crate::host::source_wall_clock_ms)).max(0.0),
                "lastInputSequence":connection.input.last_received_sequence(),"playerCount":self.players.len(),
                "playerId":id,"serverTick":server_tick}));
        self.publish_dirty = true;
        self.social_dirty = true;
        Ok(player.social_connection)
    }

    /// `retireEmptyPrivateCollegeRun`: an observer or staged connection can keep
    /// this authority alive after its final actor leaves. Retire its executable
    /// content with the world, so no old rule or pending event runs in the Hub.
    fn retire_empty_private_college_run(&mut self) {
        let HostedWorlds::Private { state, .. } = &mut self.worlds else { return };
        if state.boneyard.is_none() || !state.participants.is_empty() {
            return;
        }
        state.return_to_hub();
        self.navigation.clear();
        if let Some(mut runtime) = self.private_mod_host.take() {
            runtime.close();
        }
        self.private_mod_events.clear();
    }

    pub(in crate::host) fn retire_recovery_run(&mut self, run_id: &str, terminal: bool) {
        let removed: Vec<_> = self
            .recovery_lineages
            .iter()
            .filter(|lineage| lineage.run_id == run_id)
            .map(|lineage| lineage.recovery_id.clone())
            .collect();
        self.recovery_lineages.retain(|lineage| lineage.run_id != run_id);
        for recovery_id in &removed {
            let offline: Vec<_> = self
                .rejoin_slots
                .iter()
                .filter(|slot| &slot.recovery_id == recovery_id && !slot.connected)
                .map(|slot| slot.player_id.clone())
                .collect();
            for id in offline {
                self.worlds.parties_mut().remove_player(&id);
            }
            self.rejoin_slots.retain(|slot| &slot.recovery_id != recovery_id);
            self.retired_recoveries.push((recovery_id.clone(), terminal));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use solomon_modding::package::{
        compile_web_session_content_definitions, materialize_web_session_content,
    };
    use solomon_sim::modding::events::WebLuaDerivedEvent;
    use solomon_sim::modding::{LuaObject, LuaValue};
    use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
    use solomon_sim::social::GameSessionKind;

    use super::*;

    #[test]
    fn empty_private_run_retires_its_real_prepared_owner_and_pending_events() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../sim/tests/fixtures/website/hosted-mod-runtime-goldens.json"
        ))
        .unwrap();
        let payload = serde_json::from_value(fixture["payload"].clone()).unwrap();
        let content = materialize_web_session_content(&payload).unwrap();
        let content = compile_web_session_content_definitions(content).unwrap();
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web");
        let mut host = HostedGameHost::new(
            "retained-private".into(),
            GameSessionKind::PrivateCollege,
            16,
            Arc::new(content),
            &assets,
            "private-retirement-source-test-secret".into(),
            "a".repeat(40),
        )
        .unwrap();

        // An initially empty College still owns its prepared content. Source only
        // retires it when a Boneyard loses its final authoritative participant.
        host.retire_empty_private_college_run();
        assert!(host.private_mod_host.is_some());
        let (loaded, navigation) = host.content_loader.prepare_run([0; 16], false).unwrap();
        let HostedWorlds::Private { state, .. } = &mut host.worlds else { unreachable!() };
        state.add_participant(
            "player-1".into(),
            NewGameSelection {
                name: "Last participant".into(),
                element: WizardElement::Ether,
                discipline: WizardDiscipline::Arcane,
            },
        );
        state.install_boneyard(loaded).unwrap();
        host.navigation.insert("private-party".into(), navigation);
        host.private_mod_events.push(WebLuaDerivedEvent {
            name: "run.started".into(),
            payload: LuaValue::Object(LuaObject::new()),
        });
        host.retire_empty_private_college_run();
        assert!(host.private_mod_host.is_some());
        assert_eq!(host.private_mod_events.len(), 1);

        let HostedWorlds::Private { state, .. } = &mut host.worlds else { unreachable!() };
        state.remove_participant("player-1");
        host.retire_empty_private_college_run();
        let HostedWorlds::Private { state, .. } = &host.worlds else { unreachable!() };
        assert!(state.boneyard.is_none());
        assert_eq!(state.run.phase, GameRunPhase::Hub);
        assert!(host.navigation.is_empty());
        assert!(host.private_mod_host.is_none());
        assert!(host.private_mod_events.is_empty());
        host.step(0.0).unwrap();
        assert!(host.private_mod_host.is_none());
        assert!(host.private_mod_events.is_empty());
    }
}
