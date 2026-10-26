//! Admitted socket messages use canonical commands and per-party host controls.

use solomon_protocol::{
    ClientMessage, GameCommand, HostErrorCode, ResumeGraceReason, ServerMessage,
};
use solomon_sim::game_run::GameRunPhase;

use super::{HostedGameHost, HostedWorlds};
use crate::host::admission::validate_selection;
use crate::host::audio::{ListenerView, project_listener_audio};
use crate::host::authority::{close_peer, encode, send_control, send_reliable};
use crate::host::commands::surface_allows_command;
use crate::host::connection::HostConnection;
use crate::host::input::InputAdmission;
use crate::host::signing::random_party_identity;

impl HostedGameHost {
    pub(in crate::host) fn dispatch_local_message(
        &mut self,
        connection: &mut HostConnection,
        id: &str,
        message: ClientMessage,
        now_ms: f64,
    ) -> Result<(), (HostErrorCode, String)> {
        let party_id = self
            .party_id_for_player(id)
            .ok_or_else(|| invalid("The admitted wizard has no party."))?
            .to_owned();
        match message {
            ClientMessage::Input { sequence, target_tick, input } => {
                let state = self
                    .worlds
                    .state_for_player(id)
                    .ok_or_else(|| invalid("This wizard is not materialized."))?;
                let participant = &state.participants.get(id).expect("participant exists");
                let blocked =
                    self.controls.get(&party_id).is_some_and(|control| control.blocks_tick())
                        || state.level_up_barrier.is_some()
                        || participant.player.progression.pending_offer.is_some()
                        || (state.boneyard.is_none() && connection.hub_activity.is_some());
                let result = connection
                    .input
                    .accept(sequence, target_tick, input, state.ticks, blocked)
                    .map_err(|error| invalid_owned(error.to_string()))?;
                self.publish_dirty |= result == InputAdmission::Neutralized;
            }
            ClientMessage::SnapshotAck { sequence } => {
                let blocked = !connection.can_publish_snapshot();
                connection.acknowledge_snapshot(sequence).map_err(invalid)?;
                self.publish_dirty |= blocked && connection.can_publish_snapshot();
            }
            ClientMessage::ListenerView { center, world_width } => {
                connection.listener_view =
                    Some(ListenerView::new(center, world_width).map_err(invalid)?);
                let state = self
                    .worlds
                    .state_for_player(id)
                    .ok_or_else(|| invalid("The listener has no world."))?;
                project_listener_audio(state, connection, &[], true);
            }
            ClientMessage::ReadyBoneyard { run_id } => {
                if let Some(control) = self.controls.get_mut(&party_id) {
                    control.renderer_ready(id, &run_id)
                }
            }
            ClientMessage::ResumeReady { sequence } => {
                let facts = self.control_facts(&party_id);
                self.controls.entry(party_id).or_default().resume_ready(
                    id,
                    sequence,
                    &facts.context(),
                    now_ms,
                );
                self.publish_dirty = true;
            }
            ClientMessage::CollegeIntroReady => {
                if self.worlds.state_for_player(id).is_some_and(|state| {
                    state.boneyard.is_none()
                        && state
                            .participants
                            .get(id)
                            .expect("participant exists")
                            .college_intro
                            .is_some()
                        && state
                            .participants
                            .get(id)
                            .expect("participant exists")
                            .player
                            .economy
                            .college_intro_pending
                }) {
                    self.college_intro_ready.insert(id.into());
                }
            }
            ClientMessage::HubActivity { activity } => {
                if self.worlds.state_for_player(id).is_some_and(|state| state.boneyard.is_none())
                    && connection.hub_activity != activity
                {
                    connection.hub_activity = activity;
                    connection.input.neutralize();
                    self.publish_dirty = true;
                }
            }
            ClientMessage::GameplayPause { paused, source } => {
                let facts = self.control_facts(&party_id);
                let name = self
                    .worlds
                    .state_for_player(id)
                    .unwrap()
                    .participants
                    .get(id)
                    .expect("participant exists")
                    .player
                    .selection
                    .name
                    .clone();
                let control = self.controls.entry(party_id.clone()).or_default();
                let changed = if paused {
                    control.request_pause(id, &name, source, &facts.context())
                } else {
                    control.release_pause(id, true, &facts.context(), now_ms)
                };
                if changed {
                    connection.input.neutralize();
                    self.neutralize_party(&party_id);
                    self.publish_dirty = true;
                }
            }
            ClientMessage::Command { sequence, command } => {
                self.apply_local_command(connection, id, sequence, command, now_ms)?;
            }
            ClientMessage::StartRun { tutorial } => {
                if self.start_match(id, "default-random", tutorial)? {
                    self.send_boneyard_mod_content(connection, id).map_err(server_error)?;
                }
                connection.input.neutralize();
            }
            ClientMessage::Ping { nonce } => {
                send_control(&connection.peer, ServerMessage::Pong { nonce })
                    .map_err(server_error)?;
            }
            ClientMessage::Disconnect => {
                connection.planned_disconnect =
                    Some(("client-request", "client requested disconnect".into()));
                close_peer(&connection.peer, 1000, "client disconnect").map_err(server_error)?;
            }
            ClientMessage::Hosted { .. }
            | ClientMessage::WelcomeChunkAck { .. }
            | ClientMessage::SaveCheckpointChunkAck { .. }
            | ClientMessage::Hello { .. }
            | ClientMessage::InspectSession { .. }
            | ClientMessage::SaveBeforeLeave { .. } => {
                unreachable!(
                    "the supervisor handles admission, hosted controls and save boundaries before local dispatch"
                )
            }
        }
        Ok(())
    }

    pub(in crate::host) fn apply_local_command(
        &mut self,
        connection: &mut HostConnection,
        id: &str,
        sequence: u64,
        command: GameCommand,
        now_ms: f64,
    ) -> Result<(), (HostErrorCode, String)> {
        if sequence == 0 {
            return Err(invalid("Command sequences start at one"));
        }
        if sequence <= connection.last_command_sequence {
            if sequence == connection.last_command_sequence
                && let Some(receipt) = &connection.last_command_receipt
            {
                send_reliable(&connection.peer, receipt.clone()).map_err(server_error)?;
            }
            return Ok(());
        }
        if self.staged_rejoin_index(id).is_some() {
            return self.apply_staged_skill_command(connection, id, sequence, command, now_ms);
        }
        let party_id = self
            .party_id_for_player(id)
            .ok_or_else(|| invalid("The admitted wizard has no party."))?
            .to_owned();
        let previous =
            self.worlds.state_for_player(id).ok_or_else(|| invalid("The wizard has no world."))?;
        let previous_run = previous.run.run_id.clone();
        let previous_barrier = previous.level_up_barrier.as_ref().map(|barrier| barrier.barrier_id);
        let mut reason = None;
        let mut inventory_result = None;
        let allowed = self
            .controls
            .get(&party_id)
            .is_none_or(|control| surface_allows_command(control, id, &command, now_ms));
        let accepted = if !allowed {
            false
        } else {
            match command {
                GameCommand::Social { .. } => unreachable!(
                    "the supervisor resolves social commands with its cross-College broker"
                ),
                GameCommand::ResetWizard { selection } => {
                    validate_selection(&selection)?;
                    let members =
                        self.worlds.parties().party_for_player(id).unwrap().member_player_ids.len();
                    let state = self.worlds.state_for_player(id).unwrap();
                    if members > 1
                        && (state.boneyard.is_some() || state.run.phase != GameRunPhase::Hub)
                    {
                        reason =
                            Some("Return to the College before replacing a party wizard".into());
                        false
                    } else {
                        let profile = state.retired_participant_profile(id).unwrap();
                        let identity = random_party_identity().map_err(server_error)?;
                        let accepted = match &mut self.worlds {
                            HostedWorlds::Global(worlds) => {
                                worlds.remove_player(id).map_err(server_error)?;
                                let accepted = worlds.hub.add_participant_from_profile(
                                    id.into(),
                                    selection,
                                    profile,
                                );
                                worlds.parties.register_player(id, identity);
                                accepted
                            }
                            HostedWorlds::Private { state, .. } => {
                                state.reset_participant(id, selection)
                            }
                        };
                        if accepted {
                            self.worlds.state_for_player_mut(id).unwrap().arm_college_intro(id);
                            self.college_intro_ready.remove(id);
                            connection.hub_activity = None;
                            connection.combat_audio = None;
                            if let Some(player) = self.player_mut(id) {
                                player.global_score_eligible = false;
                                player.tutorial_eligible = false;
                            }
                        }
                        accepted
                    }
                }

                GameCommand::ConfirmLoadout { selection } => {
                    validate_selection(&selection)?;
                    match &mut self.worlds {
                        HostedWorlds::Global(worlds) => {
                            worlds.confirm_loadout(id, selection).is_ok()
                        }
                        HostedWorlds::Private { state, .. } => state.confirm_loadout(id, selection),
                    }
                }
                command => {
                    let result = self.apply_prepared_session_command(id, command)?;
                    if result.accepted && result.mod_consumption.is_some() {
                        self.send_prepared_mod_content(connection, id).map_err(server_error)?;
                    }
                    inventory_result = result.inventory_result;
                    result.accepted
                }
            }
        };
        if let Some(transaction) = &inventory_result {
            reason = transaction.reason.map(|reason| reason.as_str().into());
        } else if !accepted && reason.is_none() {
            reason = Some("This action is unavailable right now".into());
        }
        let receipt = encode(&ServerMessage::<()>::CommandResult {
            sequence,
            snapshot_sequence: self.next_snapshot_sequence,
            accepted,
            reason,
            inventory_result,
        })
        .map_err(server_error)?;
        connection.last_command_sequence = sequence;
        connection.last_command_receipt = Some(receipt.clone());
        send_reliable(&connection.peer, receipt).map_err(server_error)?;
        if accepted {
            connection.input.neutralize();
            self.neutralize_party(&party_id);
            let next = self.worlds.state_for_player(id).unwrap();
            let changed_run = previous_run != next.run.run_id;
            let barrier_closed = previous_barrier.is_some() && next.level_up_barrier.is_none();
            if changed_run {
                self.controls.remove(&party_id);
                self.navigation.remove(&party_id);
                if matches!(self.worlds, HostedWorlds::Global(_)) {
                    self.close_party_mod_runtime(&party_id);
                }
            }
            if barrier_closed {
                let facts = self.control_facts(&party_id);
                self.controls.entry(party_id).or_default().begin_surface_grace(
                    id,
                    ResumeGraceReason::SkillPickerClosed,
                    &facts.context(),
                    now_ms,
                );
            }
            self.publish_dirty = true;
            self.social_dirty = true;
            self.pending_player_saves.insert(id.into());
        }
        Ok(())
    }
}
fn invalid(reason: &str) -> (HostErrorCode, String) {
    (HostErrorCode::InvalidMessage, reason.into())
}
fn invalid_owned(reason: String) -> (HostErrorCode, String) {
    (HostErrorCode::InvalidMessage, reason)
}
fn server_error(reason: String) -> (HostErrorCode, String) {
    (HostErrorCode::ServerError, reason)
}
