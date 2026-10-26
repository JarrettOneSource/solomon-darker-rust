//! Native direct-join admission and connection-generation ownership. Source counterpart:
//! `host/game-host.ts` hello/resume, input, pause and disconnect branches.

use solomon_protocol::{
    ClientMessage, HostErrorCode, MAX_SAVE_BYTES, PROTOCOL_VERSION, ResumeCredentials,
    ResumeGraceReason, ServerMessage,
};
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::profile::NewGameSelection;
use solomon_sim::session::{GameSession, ParticipantProfile};
use solomon_sim::social::{PartyMembership, PartyVisibility};

use super::audio::ListenerView;
use super::authority::{
    NativeAuthority, PendingSaveAck, close_peer, random_bytes, random_hex, send_control,
};
use super::connection::HostConnection;
use super::input::InputAdmission;
use super::persistence::{ResumeSlot, SavedParticipant};

type AdmissionError = (HostErrorCode, String);

struct HelloRequest {
    version: u32,
    selection: NewGameSelection,
    resume: Option<ResumeCredentials>,
    initial_save: Option<String>,
    begin_college_intro: bool,
    enhanced_effects: bool,
}

impl NativeAuthority {
    pub(super) fn dispatch(
        &mut self,
        connection: &mut HostConnection,
        message: ClientMessage,
        now_ms: f64,
    ) -> Result<(), AdmissionError> {
        if connection.handle_transfer_message(&message).map_err(invalid_owned)? {
            return Ok(());
        }
        if matches!(&message, ClientMessage::Hosted { command } if !matches!(command,solomon_protocol::hosted::HostedCommand::SetEnhancedEffects{..}))
        {
            return Err(invalid("Hosted admission requires the Website session address"));
        }
        if let ClientMessage::InspectSession { resume } = message {
            if connection.player_id.is_some() {
                return Err(invalid("Inspect the native session before joining"));
            }
            let id = resume
                .as_ref()
                .filter(|credentials| {
                    self.metadata
                        .resume_slots
                        .get(&credentials.player_id)
                        .is_some_and(|slot| slot.token == credentials.token)
                })
                .map(|credentials| credentials.player_id.as_str());
            let selection = id
                .and_then(|id| self.retained_participant(id))
                .map(|participant| participant.player.selection.clone());
            let can_resume = id.is_some_and(|id| self.can_resume(id));
            let fresh_join_available = self.fresh_join_available();
            send_control(
                &connection.peer,
                ServerMessage::SessionStatus {
                    version: PROTOCOL_VERSION,
                    can_resume,
                    selection,
                    fresh_join_available,
                },
            )
            .map_err(server_error)?;
            close_peer(&connection.peer, 1000, "native session inspected").map_err(server_error)?;
            return Ok(());
        }
        if let ClientMessage::Hello {
            version,
            selection,
            resume,
            initial_save,
            begin_college_intro,
            enhanced_effects,
        } = message
        {
            if connection.player_id.is_some() {
                return Err(invalid("This connection has already joined"));
            }
            return self.admit(
                connection,
                HelloRequest {
                    version,
                    selection,
                    resume,
                    initial_save,
                    begin_college_intro,
                    enhanced_effects,
                },
                now_ms,
            );
        }
        let Some(id) = connection.player_id.clone() else {
            return Err(invalid("The first native game message must be Hello"));
        };
        if self
            .metadata
            .resume_slots
            .get(&id)
            .is_none_or(|slot| slot.peer_id != Some(connection.peer.id()))
        {
            // A replaced socket can have text already waiting in the transport channel.
            // Only the current generation may apply it to the stable participant owner.
            return Ok(());
        }
        if connection.welcome_revision.is_some() {
            return Err(invalid("Wait for the authoritative Welcome before sending commands"));
        }
        match message {
            ClientMessage::Hosted {
                command: solomon_protocol::hosted::HostedCommand::SetEnhancedEffects { enabled },
            } => {
                if self.metadata.party.as_ref().is_some_and(|party| party.leader_player_id == id) {
                    self.metadata.party.as_mut().unwrap().enhanced_effects = enabled;
                    self.session.as_mut().unwrap().set_enhanced_effects(enabled);
                    self.changed();
                    self.require_checkpoint();
                }
                self.publish_dirty = true;
                self.social_dirty = true;
            }
            ClientMessage::SaveCheckpointChunkAck { sequence, next_offset } => {
                connection
                    .peer
                    .acknowledge_save_checkpoint(sequence, next_offset)
                    .map_err(invalid_owned)?;
            }
            ClientMessage::WelcomeChunkAck { .. } => {
                unreachable!("transfer acknowledgements are consumed before admitted dispatch")
            }
            ClientMessage::Hello { .. }
            | ClientMessage::InspectSession { .. }
            | ClientMessage::Hosted { .. } => {
                unreachable!("title inspection and Hello are admitted above")
            }
            ClientMessage::Input { sequence, target_tick, input } => {
                if self.staged_detached(&id).is_some() {
                    // The source stages only offer/ACK/save controls before rejoin.
                    return Ok(());
                }
                let session = self.session.as_ref().expect("an admitted connection has a session");
                let participant = session
                    .participant(&id)
                    .ok_or_else(|| invalid("This wizard is not materialized"))?;
                let blocked = self.control.blocks_tick()
                    || session.level_up_barrier.is_some()
                    || participant.player.progression.pending_offer.is_some()
                    || (session.boneyard.is_none() && connection.hub_activity.is_some());
                let admission = connection
                    .input
                    .accept(sequence, target_tick, input, session.ticks, blocked)
                    .map_err(|error| invalid(&error.to_string()))?;
                if admission == InputAdmission::Neutralized {
                    self.publish_dirty = true;
                }
            }
            ClientMessage::SnapshotAck { sequence } => {
                let was_blocked = !connection.can_publish_snapshot();
                connection.acknowledge_snapshot(sequence).map_err(invalid)?;
                if was_blocked && connection.can_publish_snapshot() {
                    self.publish_dirty = true;
                }
            }
            ClientMessage::ListenerView { center, world_width } => {
                connection.listener_view =
                    Some(ListenerView::new(center, world_width).map_err(invalid)?);
                self.attach_combat_audio(connection);
            }
            ClientMessage::ReadyBoneyard { run_id } => {
                self.control.renderer_ready(&id, &run_id);
            }
            ClientMessage::ResumeReady { sequence } => {
                let facts = self.control_facts();
                self.control.resume_ready(&id, sequence, &facts.context(), now_ms);
                self.publish_dirty = true;
            }
            ClientMessage::CollegeIntroReady => {
                self.control.college_intro_ready.insert(id);
            }
            ClientMessage::HubActivity { activity } => {
                if self.session.as_ref().is_some_and(|session| session.boneyard.is_none())
                    && connection.hub_activity != activity
                {
                    connection.hub_activity = activity;
                    connection.input.neutralize();
                    self.publish_dirty = true;
                }
            }
            ClientMessage::GameplayPause { paused, source } => {
                let facts = self.control_facts();
                let display_name = self
                    .retained_participant(&id)
                    .expect("the admitted participant is retained")
                    .player
                    .selection
                    .name
                    .clone();
                let changed = if paused {
                    self.control.request_pause(&id, &display_name, source, &facts.context())
                } else {
                    self.control.release_pause(&id, true, &facts.context(), now_ms)
                };
                if changed {
                    connection.input.neutralize();
                    self.neutralize_inputs();
                    self.publish_dirty = true;
                }
            }
            ClientMessage::Command { sequence, command } => {
                self.apply_command(connection, &id, sequence, command, now_ms)?;
            }
            ClientMessage::StartRun { tutorial } => {
                self.start_run(&id, tutorial)?;
                connection.input.neutralize();
            }
            ClientMessage::SaveBeforeLeave { request_id } => {
                connection.input.neutralize();
                if let Some(pending) = self.pending_save_acks.get(&connection.peer.id()) {
                    if pending.request_id != request_id {
                        return Err(invalid("A save-before-leave request is already pending"));
                    }
                    return Ok(());
                }
                let session = self.session.as_ref().expect("an admitted connection has a session");
                if self.metadata.revision <= self.persisted_revision {
                    send_control(
                        &connection.peer,
                        ServerMessage::SaveCommitted { request_id, tick: session.ticks },
                    )
                    .map_err(server_error)?;
                } else {
                    self.pending_save_acks.insert(
                        connection.peer.id(),
                        PendingSaveAck { request_id, revision: self.metadata.revision },
                    );
                    self.require_checkpoint();
                }
            }
            ClientMessage::Ping { nonce } => {
                send_control(&connection.peer, ServerMessage::Pong { nonce })
                    .map_err(server_error)?;
            }
            ClientMessage::Disconnect => {
                connection.input.neutralize();
                close_peer(&connection.peer, 1000, "client disconnect").map_err(server_error)?;
            }
        }
        Ok(())
    }

    fn admit(
        &mut self,
        connection: &mut HostConnection,
        request: HelloRequest,
        now_ms: f64,
    ) -> Result<(), AdmissionError> {
        let HelloRequest {
            version,
            selection,
            resume,
            initial_save,
            begin_college_intro,
            enhanced_effects,
        } = request;
        if version != PROTOCOL_VERSION {
            return Err((
                HostErrorCode::UnsupportedVersion,
                format!("This host requires Rust game protocol {PROTOCOL_VERSION}"),
            ));
        }
        validate_selection(&selection)?;
        if resume.is_some() && initial_save.is_some() {
            return Err(invalid("A reconnect cannot also replace the host with a save"));
        }
        // Obtain fallible OS entropy before changing any canonical player or RNG state.
        let token = random_hex::<32>().map_err(server_error)?;
        let player_reference = if let Some(credentials) = &resume {
            self.metadata
                .resume_slots
                .get(&credentials.player_id)
                .map(|slot| slot.player_reference.clone())
        } else {
            None
        };
        let player_reference = match player_reference {
            Some(reference) => reference,
            None => format!("player-ref-{}", random_hex::<24>().map_err(server_error)?),
        };
        let rejoining = resume.is_some();
        let choose_initial_effects =
            !rejoining && initial_save.is_none() && self.metadata.resume_slots.is_empty();
        let id = if let Some(credentials) = resume {
            self.resume(connection, credentials)?
        } else {
            self.join_new(selection, initial_save)?
        };
        if begin_college_intro {
            self.session
                .as_mut()
                .expect("admission installs the shared session")
                .arm_college_intro(&id);
        }
        let previous_peer = self.metadata.resume_slots.get(&id).and_then(|slot| slot.peer_id);
        let saved =
            if self.session.as_ref().is_some_and(|session| session.participant(&id).is_none()) {
                self.metadata.resume_slots.get_mut(&id).and_then(|slot| slot.saved.take())
            } else {
                None
            };
        self.metadata.resume_slots.insert(
            id.clone(),
            ResumeSlot { token, player_reference, saved, peer_id: Some(connection.peer.id()) },
        );
        if let Some(previous) = previous_peer
            && previous != connection.peer.id()
            && let Some(replaced) = self.connections.get_mut(&previous)
        {
            replaced.player_id = None;
            replaced.welcome_revision = None;
            replaced.input.neutralize();
            close_peer(&replaced.peer, 4002, "wizard resumed in another browser")
                .map_err(server_error)?;
        }
        self.add_party_member(&id);
        if choose_initial_effects {
            self.session.as_mut().unwrap().set_enhanced_effects(enhanced_effects);
            self.metadata.party.as_mut().unwrap().enhanced_effects = enhanced_effects;
        }
        connection.player_id = Some(id.clone());
        self.changed();
        if let Some(session) = &self.session
            && session.boneyard.is_some()
            && session.run.phase == GameRunPhase::Active
        {
            let run_id =
                session.run.run_id.as_deref().expect("an active Boneyard has its run identity");
            let connected = self.connected_ids();
            self.control.require_renderer(&id, run_id);
            self.control.begin_loading_grace(
                &id,
                if rejoining {
                    ResumeGraceReason::GameRejoined
                } else {
                    ResumeGraceReason::GameRestarted
                },
                &connected,
                &[],
            );
            self.neutralize_inputs();
        }
        let facts = self.control_facts();
        self.control.advance(&facts.context(), now_ms);
        connection.welcome_revision = Some(self.metadata.revision);
        self.require_checkpoint();
        self.publish_dirty = true;
        self.social_dirty = true;
        Ok(())
    }

    fn join_new(
        &mut self,
        selection: NewGameSelection,
        initial_save: Option<String>,
    ) -> Result<String, AdmissionError> {
        if self.metadata.resume_slots.len() >= self.maximum_players {
            return Err((HostErrorCode::HostFull, "This native session is full".into()));
        }
        if let Some(text) = initial_save {
            if !self.metadata.resume_slots.is_empty()
                || self.session.as_ref().is_some_and(|session| !session.participants.is_empty())
            {
                return Err(invalid("Only an empty native host can import a standalone save"));
            }
            if text.len() > MAX_SAVE_BYTES {
                return Err(invalid("The initial saved game exceeds the supported size"));
            }
            let checkpoint = solomon_assets::save_document::decode(&text).map_err(invalid_owned)?;
            if checkpoint.participant_order.len() != 1 {
                return Err(invalid(
                    "A browser may seed a host with its one-wizard standalone save",
                ));
            }
            let id = checkpoint.participant_order[0].clone();
            if checkpoint.participants.get(&id).expect("participant exists").player.selection
                != selection
            {
                return Err(invalid("The saved wizard does not match this create/resume request"));
            }
            let loaded =
                solomon_assets::save_document::load_boneyard(&checkpoint).map_err(invalid_owned)?;
            let navigation = loaded
                .as_ref()
                .map(|loaded| self.content.load_navigation(loaded))
                .transpose()
                .map_err(server_error)?;
            let session = GameSession::restore(checkpoint, loaded).map_err(invalid_owned)?;
            self.navigation = navigation;
            self.session = Some(session);
            return Ok(id);
        }
        if self.session.as_ref().is_some_and(|session| {
            session.boneyard.is_some() || session.run.phase != GameRunPhase::Hub
        }) {
            return Err((
                HostErrorCode::RunInProgress,
                "A run is already active; reconnect with its wizard credentials".into(),
            ));
        }
        let id = loop {
            let id = format!("wizard-{}", self.metadata.next_player_id);
            self.metadata.next_player_id += 1;
            if !self.metadata.resume_slots.contains_key(&id) {
                break id;
            }
        };
        let session = self.session.as_mut().expect("the native host starts with an empty College");
        if !session.add_participant(id.clone(), selection) {
            return Err(server_error("The native host could not add its new participant".into()));
        }
        Ok(id)
    }

    fn resume(
        &mut self,
        connection: &HostConnection,
        credentials: ResumeCredentials,
    ) -> Result<String, AdmissionError> {
        let invalid_credentials = || {
            (
                HostErrorCode::InvalidCredentials,
                "These wizard reconnect credentials are no longer current".into(),
            )
        };
        let slot = self
            .metadata
            .resume_slots
            .get(&credentials.player_id)
            .ok_or_else(invalid_credentials)?;
        if slot.token != credentials.token {
            return Err(invalid_credentials());
        }
        if slot.peer_id == Some(connection.peer.id()) {
            return Err(invalid("This connection has already joined"));
        }
        if !self.can_resume(&credentials.player_id) {
            return Err((
                HostErrorCode::RunInProgress,
                "This profile can return when the party reaches the College".into(),
            ));
        }
        // One infrequent participant copy keeps a rejected import from consuming the
        // durable slot. No session/world clone exists per client or per published frame.
        let saved = slot.saved.clone();
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| server_error("The retained wizard has no shared session".into()))?;
        match saved {
            Some(SavedParticipant::Hub { participant }) => {
                session.import_hub_participant(*participant).map_err(invalid_owned)?;
            }
            Some(SavedParticipant::Run { detached }) => {
                let mut detached = detached;
                if let Some(milestone) = session.rejoin_milestone(&detached) {
                    session
                        .synchronize_detached(&mut detached, &milestone)
                        .map_err(invalid_owned)?;
                }
                if detached.participant.player.progression.pending_offer.is_some() {
                    self.metadata
                        .resume_slots
                        .get_mut(&credentials.player_id)
                        .expect("the verified resume slot remains retained")
                        .saved = Some(SavedParticipant::Run { detached });
                } else {
                    session.rejoin_participant(*detached, None).map_err(invalid_owned)?;
                }
            }
            Some(SavedParticipant::Retired { participant, economy, hagatha_runtime }) => {
                if !session.add_participant_from_profile(
                    participant.id,
                    participant.player.selection.clone(),
                    ParticipantProfile {
                        economy: *economy,
                        hagatha_runtime,
                        advanced_unlocks: participant.player.book.advanced_unlocks,
                    },
                ) {
                    return Err(invalid("The retained profile cannot join this active world"));
                }
            }
            None if session.participant(&credentials.player_id).is_none() => {
                return Err(invalid_credentials());
            }
            None => {}
        }
        Ok(credentials.player_id)
    }

    fn start_run(&mut self, id: &str, tutorial: bool) -> Result<(), AdmissionError> {
        let leader = self.metadata.party.as_ref().map(|party| party.leader_player_id.as_str());
        if leader != Some(id) {
            return Err((
                HostErrorCode::CommandRejected,
                "Only the native session leader can start a run".into(),
            ));
        }
        let session = self.session.as_ref().expect("the run request has an admitted session");
        if !session.can_enter_boneyard()
            || self.control.blocks_tick()
            || session.wizard(id).is_none_or(|wizard| wizard.transition.is_some())
        {
            return Err((
                HostErrorCode::CommandRejected,
                "The party is not ready to enter a Boneyard".into(),
            ));
        }
        let (loaded, navigation) = self
            .content
            .prepare_run(random_bytes::<16>().map_err(server_error)?, tutorial)
            .map_err(server_error)?;
        let run_id = loaded.run_id.clone();
        self.session
            .as_mut()
            .expect("the admitted session is retained")
            .install_boneyard(loaded)
            .map_err(invalid_owned)?;
        self.navigation = Some(navigation);
        let connected = self.connected_ids();
        self.control.arm_renderers(&run_id, &connected);
        self.control.begin_loading_grace(id, ResumeGraceReason::GameStarted, &connected, &[]);
        self.neutralize_inputs();
        self.changed();
        self.require_checkpoint();
        self.publish_dirty = true;
        self.social_dirty = true;
        Ok(())
    }

    fn add_party_member(&mut self, id: &str) {
        if let Some(party) = &mut self.metadata.party {
            if !party.member_player_ids.iter().any(|member| member == id) {
                party.member_player_ids.push(id.into());
            }
        } else {
            self.metadata.party = Some(PartyMembership {
                enhanced_effects: self
                    .session
                    .as_ref()
                    .is_none_or(|session| session.enhanced_effects),
                id: self.metadata.session_id.clone(),
                join_code: None,
                listing_id: None,
                leader_player_id: id.into(),
                member_player_ids: vec![id.into()],
                visibility: PartyVisibility::Private,
            });
        }
    }

    fn fresh_join_available(&self) -> bool {
        self.metadata.resume_slots.len() < self.maximum_players
            && self.session.as_ref().is_some_and(|session| {
                session.boneyard.is_none() && session.run.phase == GameRunPhase::Hub
            })
    }

    fn can_resume(&self, id: &str) -> bool {
        let Some(slot) = self.metadata.resume_slots.get(id) else { return false };
        let Some(session) = &self.session else { return false };
        match &slot.saved {
            Some(SavedParticipant::Run { detached }) => {
                session.boneyard.is_some()
                    && session.run.phase == GameRunPhase::Active
                    && session.run.run_id.as_deref() == Some(detached.run_id.as_str())
            }
            Some(SavedParticipant::Hub { .. } | SavedParticipant::Retired { .. }) => {
                session.boneyard.is_none() && session.run.phase == GameRunPhase::Hub
            }
            None => session.participant(id).is_some(),
        }
    }
}

pub(super) fn invalid(reason: &str) -> AdmissionError {
    (HostErrorCode::InvalidMessage, reason.into())
}
pub(super) fn invalid_owned(reason: String) -> AdmissionError {
    (HostErrorCode::InvalidMessage, reason)
}
pub(super) fn server_error(reason: String) -> AdmissionError {
    (HostErrorCode::ServerError, reason)
}

pub(super) fn validate_selection(selection: &NewGameSelection) -> Result<(), AdmissionError> {
    // The source decoder measures JavaScript string length (UTF-16 code units).
    if selection.name.is_empty() || selection.name.encode_utf16().count() > 64 {
        return Err(invalid("A wizard name must contain from one through sixty-four characters"));
    }
    Ok(())
}
