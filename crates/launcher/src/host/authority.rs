//! One 100 Hz native authority over the canonical shared `GameSession`.
//! Reference timing, input and publication order: `host/game-host.ts:3484..3915`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use solomon_protocol::{
    ClientMessage, GAME_TICK_RATE, HostErrorCode, HostSnapshot, PROTOCOL_VERSION,
    ResumeCredentials, SNAPSHOT_RATE, ServerMessage, SnapshotFrame,
};
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::session::{
    GameSession, GameSessionOptions, GameSessionTickContext, SessionParticipant,
};

use super::checkpoint::CheckpointFile;
use super::connection::HostConnection;
use super::content::HostContent;
use super::control::{ControlContext, GameplayControl};
use super::persistence::{
    CheckpointWriter, HostSaveMetadata, SavedParticipant, decode_host_document,
};
use crate::transport::{NativeServer, PeerId, PeerSendError, PeerSender, TransportEvent};

const MAX_EVENTS_PER_POLL: usize = 64;
const MAX_CONTROL_MESSAGE_BYTES: usize = 64 * 1024;
const AUTOSAVE_INTERVAL_TICKS: u64 = 3_000;
const TICKS_PER_SNAPSHOT: u64 = (GAME_TICK_RATE / SNAPSHOT_RATE) as u64;

pub struct NativeHostConfig {
    pub asset_directory: PathBuf,
    pub checkpoint_path: PathBuf,
    pub maximum_players: usize,
}

impl NativeHostConfig {
    #[must_use]
    pub fn new(asset_directory: PathBuf, checkpoint_path: PathBuf) -> Self {
        Self { asset_directory, checkpoint_path, maximum_players: 16 }
    }
}

pub(super) struct ControlFacts {
    pub(super) active_run: bool,
    pub(super) skill_barrier: bool,
    pub(super) materialized: BTreeSet<String>,
    pub(super) connected: BTreeSet<String>,
}

impl ControlFacts {
    pub(super) fn context(&self) -> ControlContext<'_> {
        ControlContext {
            active_run: self.active_run,
            skill_barrier: self.skill_barrier,
            materialized: &self.materialized,
            connected: &self.connected,
        }
    }
}

pub(super) struct PendingSaveAck {
    pub(super) request_id: u64,
    pub(super) revision: u64,
}

pub struct NativeAuthority {
    pub(super) session: Option<GameSession>,
    pub(super) flame_histories: super::flame::HostFlameHistories,
    pub(super) metadata: HostSaveMetadata,
    pub(super) connections: BTreeMap<PeerId, HostConnection>,
    pub(super) control: GameplayControl,
    pub(super) content: HostContent,
    pub(super) navigation: Option<NavigationSet>,
    pub(super) maximum_players: usize,
    pub(super) publish_dirty: bool,
    pub(super) social_dirty: bool,
    pub(super) next_snapshot_sequence: u64,
    next_publication_ms: f64,
    pub(super) persisted_revision: u64,
    required_save_revision: Option<u64>,
    pub(super) pending_save_acks: BTreeMap<PeerId, PendingSaveAck>,
    checkpoint_writer: CheckpointWriter,
}

impl NativeAuthority {
    pub fn load(config: NativeHostConfig) -> Result<Self, String> {
        if !(1..=16).contains(&config.maximum_players) {
            return Err("The native host supports from one through sixteen players".into());
        }
        let content = HostContent::load(&config.asset_directory)?;
        let file = CheckpointFile::new(config.checkpoint_path);
        let restored = file.load()?;
        let (metadata, session, navigation) = if let Some(text) = restored {
            let (metadata, checkpoint) = decode_host_document(&text)?;
            let loaded = solomon_assets::save_document::load_boneyard(&checkpoint)?;
            let navigation =
                loaded.as_ref().map(|loaded| content.load_navigation(loaded)).transpose()?;
            let session = GameSession::restore(checkpoint, loaded)?;
            (metadata, Some(session), navigation)
        } else {
            let metadata = HostSaveMetadata {
                revision: 0,
                next_player_id: 1,
                session_id: random_hex::<16>()?,
                party: None,
                resume_slots: BTreeMap::new(),
                next_chat_sequence: 1,
                chat_messages: Vec::new(),
            };
            // The source host constructs and ticks an empty College before admission.
            // One OS word seeds its gameplay and trader owners; combat keeps its default.
            let seed = u32::from_le_bytes(random_bytes::<4>()?);
            let session = GameSession::new_participants(
                Vec::new(),
                GameSessionOptions {
                    gameplay_rng_seed: i64::from(seed),
                    hub_trader_animation_seed: seed,
                    ..GameSessionOptions::default()
                },
            )?;
            (metadata, Some(session), None)
        };
        let persisted_revision = metadata.revision;
        let mut authority = Self {
            session,
            flame_histories: super::flame::HostFlameHistories::default(),
            metadata,
            connections: BTreeMap::new(),
            control: GameplayControl::default(),
            content,
            navigation,
            maximum_players: config.maximum_players,
            publish_dirty: false,
            social_dirty: false,
            next_snapshot_sequence: 1,
            next_publication_ms: 0.0,
            persisted_revision,
            required_save_revision: None,
            pending_save_acks: BTreeMap::new(),
            checkpoint_writer: CheckpointWriter::start(file)?,
        };
        authority.validate_restored_membership()?;
        authority.detach_restored_connections()?;
        Ok(authority)
    }

    #[must_use]
    pub fn session(&self) -> Option<&GameSession> {
        self.session.as_ref()
    }

    /// A single thread admits messages and mutates gameplay. Socket workers and the one
    /// checkpoint worker carry owned messages only; neither can step or edit this session.
    pub fn run(mut self, mut transport: NativeServer, stop: Arc<AtomicBool>) -> Result<(), String> {
        let epoch = Instant::now();
        let tick_duration = Duration::from_millis(10);
        let mut next_tick = epoch + tick_duration;
        let result = (|| {
            while !stop.load(Ordering::Acquire) {
                let now = Instant::now();
                let now_ms = now.duration_since(epoch).as_secs_f64() * 1_000.0;
                for _ in 0..MAX_EVENTS_PER_POLL {
                    match transport.try_event() {
                        Ok(event) => self.accept_event(event, now_ms)?,
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            return Err("The native game transport stopped unexpectedly".into());
                        }
                    }
                }
                self.maintain(now_ms)?;
                if self.clock_is_held() {
                    next_tick = now + tick_duration;
                } else {
                    let mut steps = 0;
                    while now >= next_tick && steps < 25 {
                        self.step_once(now_ms)?;
                        next_tick += tick_duration;
                        steps += 1;
                        if self.clock_is_held() {
                            next_tick = now + tick_duration;
                            break;
                        }
                    }
                    // Source drops only excess wall time. It never advances simulation
                    // ticks or RNG words to pretend the skipped interval was simulated.
                    if steps == 25 && now >= next_tick {
                        next_tick = now + tick_duration;
                    }
                }
                self.flush_publications(now_ms)?;
                let wait = next_tick.saturating_duration_since(Instant::now());
                match transport.recv_timeout(wait.min(Duration::from_millis(2))) {
                    Ok(event) => self.accept_event(event, now_ms)?,
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => {
                        return Err("The native game transport stopped unexpectedly".into());
                    }
                }
            }
            self.flush_final_checkpoint(epoch.elapsed().as_secs_f64() * 1_000.0)?;
            Ok(())
        })();
        transport.shutdown();
        let writer_result = self.checkpoint_writer.finish();
        result.and(writer_result)
    }

    pub(super) fn accept_event(
        &mut self,
        event: TransportEvent,
        now_ms: f64,
    ) -> Result<(), String> {
        match event {
            TransportEvent::Connected { peer, .. } => {
                self.connections.insert(peer.id(), HostConnection::new(peer, now_ms));
            }
            TransportEvent::Message { peer_id, payload } => {
                self.accept_message(peer_id, &payload, now_ms)?;
            }
            TransportEvent::Disconnected { peer_id, .. } => {
                if let Some(connection) = self.connections.remove(&peer_id) {
                    self.disconnected(connection, now_ms)?;
                }
            }
            TransportEvent::Http { respond, .. } => {
                let _ = respond.send(crate::transport::HostedHttpResponse::empty(404));
            }
            TransportEvent::Failed { reason } => return Err(reason),
        }
        Ok(())
    }

    fn accept_message(&mut self, peer_id: PeerId, text: &str, now_ms: f64) -> Result<(), String> {
        let Some(mut connection) = self.connections.remove(&peer_id) else { return Ok(()) };
        if let Some(state) = &self.session {
            self.flame_histories.observe("private", state)?;
        }
        let decoded = decode_client_message(text);
        match decoded {
            Ok(message) => {
                if let Err((code, reason)) = self.dispatch(&mut connection, message, now_ms) {
                    if code == HostErrorCode::CommandRejected {
                        send_control(&connection.peer, ServerMessage::Error { code, reason })?;
                    } else {
                        self.reject(&connection.peer, code, &reason)?;
                    }
                }
            }
            Err(reason) => self.reject(&connection.peer, HostErrorCode::InvalidMessage, &reason)?,
        }
        if let Some(state) = &self.session {
            self.flame_histories.observe("private", state)?;
        }
        self.connections.insert(peer_id, connection);
        Ok(())
    }

    pub(super) fn maintain(&mut self, now_ms: f64) -> Result<(), String> {
        let facts = self.control_facts();
        let previous_grace = self.control.resume_grace(now_ms);
        let previous_loading = self.control.loading_run_id().map(str::to_owned);
        self.control.advance(&facts.context(), now_ms);
        if previous_grace != self.control.resume_grace(now_ms)
            || previous_loading.as_deref() != self.control.loading_run_id()
        {
            self.neutralize_inputs();
            self.publish_dirty = true;
        }
        for connection in self.connections.values() {
            if connection.player_id.is_none() && now_ms >= connection.hello_deadline_ms {
                close_peer(&connection.peer, 4000, "game hello timed out")?;
            }
        }
        self.poll_checkpoint(now_ms)?;
        self.submit_checkpoint_if_needed()?;
        Ok(())
    }

    fn clock_is_held(&self) -> bool {
        let Some(session) = &self.session else { return true };
        // An empty College keeps its ambient world clock. An abandoned active run
        // remains frozen so its detached living wizards can resume that exact world.
        (session.boneyard.is_some()
            && !session.participant_order.iter().any(|id| {
                self.metadata.resume_slots.get(id).is_some_and(|slot| slot.peer_id.is_some())
            }))
            || session.level_up_barrier.is_some()
            || self.control.blocks_tick()
    }

    pub(super) fn step_once(&mut self, now_ms: f64) -> Result<(), String> {
        let Some(session) = &mut self.session else { return Ok(()) };
        let next_tick = session.ticks + 1;
        let mut inputs = BTreeMap::new();
        for connection in self.connections.values_mut() {
            let Some(id) = &connection.player_id else { continue };
            if connection.welcome_revision.is_some() || session.participant(id).is_none() {
                continue;
            }
            if self
                .metadata
                .resume_slots
                .get(id)
                .is_some_and(|slot| slot.peer_id == Some(connection.peer.id()))
            {
                inputs.insert(id.clone(), connection.input.for_tick(next_tick));
            }
        }
        let previous_tick = session.ticks;
        let previous_phase = session.run.phase;
        let had_boneyard = session.boneyard.is_some();
        let previous_barrier = session.level_up_barrier.as_ref().map(|barrier| barrier.barrier_id);
        let result = session.step(
            &inputs,
            GameSessionTickContext {
                navigation: self.navigation.as_ref(),
                college_intro_ready_player_ids: Some(&self.control.college_intro_ready),
                enemy_spawn_intents: &[],
            },
        );
        let changed = session.ticks != previous_tick;
        let next_phase = session.run.phase;
        let next_barrier = session.level_up_barrier.as_ref().map(|barrier| barrier.barrier_id);
        let tick = session.ticks;
        let left_boneyard = had_boneyard && session.boneyard.is_none();
        self.flame_histories.observe("private", session)?;
        if changed {
            self.changed();
            self.synchronize_offline_milestones()?;
            if tick % TICKS_PER_SNAPSHOT == 0 {
                self.publish_dirty = true;
            }
            if tick % AUTOSAVE_INTERVAL_TICKS == 0 {
                self.require_checkpoint();
            }
        }
        if previous_barrier != next_barrier || previous_phase != next_phase {
            self.neutralize_inputs();
            self.publish_dirty = true;
            self.social_dirty = true;
            self.require_checkpoint();
        }
        if previous_phase != next_phase && next_phase != GameRunPhase::Active {
            self.retire_offline_run_slots()?;
        }
        if left_boneyard {
            self.navigation = None;
        }
        self.publish_tick_audio(&result.audio_events)?;
        self.maintain(now_ms)
    }

    pub(super) fn flush_publications(&mut self, now_ms: f64) -> Result<(), String> {
        if self.publish_dirty && now_ms >= self.next_publication_ms {
            self.publish_dirty = false;
            self.publish_snapshot(now_ms)?;
            self.next_publication_ms = now_ms + 1_000.0 / f64::from(SNAPSHOT_RATE);
        }
        if self.social_dirty {
            self.social_dirty = false;
            self.publish_social()?;
        }
        Ok(())
    }

    fn publish_snapshot(&mut self, now_ms: f64) -> Result<(), String> {
        if let Some(state) = &self.session {
            self.flame_histories.observe("private", state)?;
        }
        let mut recipients = Vec::new();
        for (&peer_id, connection) in &self.connections {
            if !connection.can_publish_snapshot() {
                continue;
            }
            let id = connection.player_id.as_ref().expect("an active viewer has its identity");
            let staged = self.staged_detached(id).is_some();
            recipients.push((
                peer_id,
                staged.then(|| id.clone()),
                connection.acknowledged_flame_baseline(),
            ));
        }
        if recipients.is_empty() {
            return Ok(());
        }
        let sequence = self.next_snapshot_sequence;
        self.next_snapshot_sequence += 1;
        // Peers sharing a stopped-world cursor share one borrowed serialization.
        // A rejoin still projects only its actual detached participant.
        let mut shared = BTreeMap::new();
        for (peer_id, staged_id, baseline) in recipients {
            let payload = if let Some(id) = staged_id {
                self.encode_completed_frame(Some(&id), sequence, now_ms, None, baseline)?
            } else {
                let key = baseline.map(|row| (row.epoch, row.world_tick));
                if let Some(payload) = shared.get(&key) {
                    Arc::clone(payload)
                } else {
                    let payload =
                        self.encode_completed_frame(None, sequence, now_ms, None, baseline)?;
                    shared.insert(key, Arc::clone(&payload));
                    payload
                }
            };
            let flame_baseline = self.flame_histories.baseline("private")?;
            let connection = self.connections.get_mut(&peer_id).expect("the viewer is retained");
            connection.ensure_boneyard_loaded(
                self.session
                    .as_ref()
                    .and_then(|state| state.boneyard.as_ref())
                    .map(|world| &world.loaded),
            )?;
            match connection.peer.replace_snapshot(payload) {
                Ok(()) => {
                    connection.published_snapshot(sequence);
                    connection.record_flame_baseline(sequence, flame_baseline);
                }
                Err(PeerSendError::Closed) => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        let retained: Vec<_> =
            self.connections.values().flat_map(|peer| peer.retained_flame_baselines()).collect();
        self.flame_histories.discard_sent_history(&retained)?;
        Ok(())
    }

    fn encode_completed_frame(
        &self,
        viewer: Option<&str>,
        sequence: u64,
        now_ms: f64,
        welcome: Option<ResumeCredentials>,
        flame_baseline: Option<solomon_protocol::flame_presentation::FlamePresentationBaseline>,
    ) -> Result<Arc<str>, String> {
        let session = self.session.as_ref().expect("the native host owns its shared session");
        let projected = viewer
            .and_then(|id| self.staged_detached(id))
            .map(|detached| session.project_detached(detached))
            .transpose()?;
        let snapshot = match &projected {
            Some(projected) => session.snapshot_with_detached(projected),
            None => session.snapshot(),
        };
        let frame = SnapshotFrame {
            sequence,
            flame_presentation: Some(self.flame_histories.frame("private", flame_baseline)?),
            input_acknowledgements: self.input_acknowledgements(),
            host: self.host_snapshot(viewer, now_ms),
            snapshot,
        };
        match welcome {
            Some(credentials) => encode(&ServerMessage::Welcome {
                version: PROTOCOL_VERSION,
                credentials,
                tick_rate: GAME_TICK_RATE,
                snapshot_rate: SNAPSHOT_RATE,
                hosted: None,
                frame,
            }),
            None => encode(&ServerMessage::Snapshot { frame }),
        }
    }

    fn input_acknowledgements(&self) -> BTreeMap<String, u64> {
        let mut acknowledgements = BTreeMap::new();
        for connection in self.connections.values() {
            if let Some(id) = &connection.player_id {
                acknowledgements.insert(id.clone(), connection.input.acknowledged_sequence());
            }
        }
        acknowledgements
    }

    fn host_snapshot(&self, viewer: Option<&str>, now_ms: f64) -> HostSnapshot {
        let mut hub_activities = BTreeMap::new();
        for connection in self.connections.values() {
            if let (Some(id), Some(activity)) = (&connection.player_id, connection.hub_activity) {
                hub_activities.insert(id.clone(), activity);
            }
        }
        HostSnapshot {
            authority_player_id: self
                .metadata
                .party
                .as_ref()
                .map(|party| party.leader_player_id.clone()),
            connected_player_ids: self.connected_ids().into_iter().collect(),
            materializing_player_ids: viewer
                .filter(|id| self.staged_detached(id).is_some())
                .map(|id| vec![id.into()])
                .unwrap_or_default(),
            hub_activities,
            pause: self.control.pause().cloned(),
            resume_grace: self.control.resume_grace(now_ms),
            loading_run_id: self.control.loading_run_id().map(str::to_owned),
        }
    }

    pub(super) fn connected_ids(&self) -> BTreeSet<String> {
        self.metadata
            .resume_slots
            .iter()
            .filter(|(_, slot)| slot.peer_id.is_some())
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub(super) fn control_facts(&self) -> ControlFacts {
        let mut materialized = BTreeSet::new();
        let mut active_run = false;
        let mut skill_barrier = false;
        if let Some(session) = &self.session {
            materialized.extend(session.participant_order.iter().cloned());
            active_run = session.boneyard.is_some() && session.run.phase == GameRunPhase::Active;
            skill_barrier = session.level_up_barrier.is_some();
        }
        ControlFacts { active_run, skill_barrier, materialized, connected: self.connected_ids() }
    }

    pub(super) fn retained_participant(&self, id: &str) -> Option<&SessionParticipant> {
        if let Some(participant) = self.session.as_ref().and_then(|session| session.participant(id))
        {
            return Some(participant);
        }
        match self.metadata.resume_slots.get(id)?.saved.as_ref()? {
            SavedParticipant::Hub { participant } => Some(participant),
            SavedParticipant::Run { detached } => Some(&detached.participant),
            SavedParticipant::Retired { participant, .. } => Some(participant),
        }
    }

    pub(super) fn neutralize_inputs(&mut self) {
        for connection in self.connections.values_mut() {
            connection.input.neutralize();
        }
    }

    pub(super) fn changed(&mut self) {
        self.metadata.revision += 1;
    }

    pub(super) fn require_checkpoint(&mut self) {
        self.required_save_revision = Some(self.metadata.revision);
    }

    fn submit_checkpoint_if_needed(&mut self) -> Result<(), String> {
        let Some(required) = self.required_save_revision else { return Ok(()) };
        if required <= self.persisted_revision {
            self.required_save_revision = None;
            return Ok(());
        }
        if !self.checkpoint_writer.is_available() {
            return Ok(());
        }
        let Some(session) = &self.session else { return Ok(()) };
        self.checkpoint_writer.submit(session.checkpoint(), self.metadata.clone())
    }

    fn poll_checkpoint(&mut self, now_ms: f64) -> Result<(), String> {
        let Some(completion) = self.checkpoint_writer.poll()? else { return Ok(()) };
        completion.result?;
        self.persisted_revision = completion.revision;
        if self.required_save_revision.is_some_and(|required| required <= completion.revision) {
            self.required_save_revision = None;
        }
        let acknowledged: Vec<_> = self
            .pending_save_acks
            .iter()
            .filter(|(_, pending)| pending.revision <= completion.revision)
            .map(|(&peer_id, _)| peer_id)
            .collect();
        for peer_id in acknowledged {
            let pending = self
                .pending_save_acks
                .remove(&peer_id)
                .expect("a completed save request is present");
            if let Some(connection) = self.connections.get(&peer_id) {
                send_control(
                    &connection.peer,
                    ServerMessage::SaveCommitted {
                        request_id: pending.request_id,
                        tick: completion.tick,
                    },
                )?;
            }
        }
        self.publish_completed_welcomes(completion.revision, now_ms)
    }

    fn publish_completed_welcomes(&mut self, revision: u64, now_ms: f64) -> Result<(), String> {
        if let Some(state) = &self.session {
            self.flame_histories.observe("private", state)?;
        }
        let ready: Vec<_> = self
            .connections
            .iter()
            .filter(|(_, connection)| {
                connection.welcome_revision.is_some_and(|required| required <= revision)
            })
            .map(|(&peer_id, _)| peer_id)
            .collect();
        for peer_id in ready {
            let connection = &self.connections[&peer_id];
            let id = connection
                .player_id
                .as_ref()
                .expect("a pending welcome has an admitted participant");
            let slot = &self.metadata.resume_slots[id];
            if slot.peer_id != Some(peer_id) {
                continue;
            }
            let sequence = self.next_snapshot_sequence;
            self.next_snapshot_sequence += 1;
            let payload = self.encode_completed_frame(
                Some(id),
                sequence,
                now_ms,
                Some(ResumeCredentials { player_id: id.clone(), token: slot.token.clone() }),
                None,
            )?;
            let id = id.clone();
            let connection =
                self.connections.get_mut(&peer_id).expect("the welcome connection is retained");
            send_welcome(&connection.peer, payload)?;
            connection.ensure_boneyard_loaded(
                self.session
                    .as_ref()
                    .and_then(|state| state.boneyard.as_ref())
                    .map(|world| &world.loaded),
            )?;
            connection.welcomed(id, sequence);
            connection.record_flame_baseline(sequence, self.flame_histories.baseline("private")?);
            self.social_dirty = true;
        }
        Ok(())
    }

    fn flush_final_checkpoint(&mut self, now_ms: f64) -> Result<(), String> {
        if self.session.is_none() {
            return Ok(());
        }
        self.require_checkpoint();
        while self.required_save_revision.is_some() {
            self.poll_checkpoint(now_ms)?;
            self.submit_checkpoint_if_needed()?;
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok(())
    }

    pub(super) fn reject(
        &self,
        peer: &PeerSender,
        code: HostErrorCode,
        reason: &str,
    ) -> Result<(), String> {
        send_control(peer, ServerMessage::Error { code, reason: reason.into() })?;
        close_peer(peer, 1008, "native game admission rejected")
    }
}

pub(super) fn encode<S: Serialize>(message: &ServerMessage<S>) -> Result<Arc<str>, String> {
    serde_json::to_string(message)
        .map(Arc::from)
        .map_err(|error| format!("Cannot encode an authoritative game message: {error}"))
}

pub(super) fn send_control(peer: &PeerSender, message: ServerMessage<()>) -> Result<(), String> {
    if matches!(&message, ServerMessage::Error { .. }) {
        peer.cancel_welcome();
    }
    send_reliable(peer, encode(&message)?)
}

pub(super) fn send_welcome(peer: &PeerSender, payload: Arc<str>) -> Result<(), String> {
    match peer.send_welcome(payload) {
        Ok(()) | Err(PeerSendError::Closed) => Ok(()),
        Err(PeerSendError::ReliableQueueFull) => {
            close_peer(peer, 1013, "game connection is too slow")
        }
        Err(error) => Err(error.to_string()),
    }
}

pub(super) fn send_reliable(peer: &PeerSender, payload: Arc<str>) -> Result<(), String> {
    match peer.send_reliable(payload) {
        Ok(()) | Err(PeerSendError::Closed) => Ok(()),
        Err(PeerSendError::ReliableQueueFull) => {
            close_peer(peer, 1013, "game connection is too slow")
        }
        Err(error) => Err(error.to_string()),
    }
}

pub(super) fn close_peer(peer: &PeerSender, code: u16, reason: &str) -> Result<(), String> {
    match peer.close(code, reason) {
        Ok(()) | Err(PeerSendError::Closed) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

pub(super) fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("Cannot obtain native host entropy: {error}"))?;
    Ok(bytes)
}

pub(super) fn random_hex<const N: usize>() -> Result<String, String> {
    let bytes = random_bytes::<N>()?;
    let mut text = String::with_capacity(N * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(text, "{byte:02x}").expect("writing a credential into a String cannot fail");
    }
    Ok(text)
}

pub(super) fn decode_client_message(text: &str) -> Result<ClientMessage, String> {
    #[derive(Deserialize)]
    struct Header<'a> {
        #[serde(rename = "type", borrow)]
        kind: std::borrow::Cow<'a, str>,
        #[serde(borrow)]
        command: Option<HostedHeader<'a>>,
    }
    #[derive(Deserialize)]
    struct HostedHeader<'a> {
        #[serde(borrow)]
        kind: std::borrow::Cow<'a, str>,
    }
    let header: Header<'_> =
        serde_json::from_str(text).map_err(|error| format!("Invalid game message: {error}"))?;
    // Only Hello carries a complete save. Ordinary typed commands are small; bounding
    // them before owned deserialization prevents a malformed Vec/String from allocating
    // the transport's much larger escaped-save allowance.
    let carries_save = header.kind == "hello"
        || (header.kind == "hosted"
            && header.command.is_some_and(|command| command.kind == "hello"));
    if !carries_save && text.len() > MAX_CONTROL_MESSAGE_BYTES {
        return Err("Native game commands may contain at most 64 KiB".into());
    }
    serde_json::from_str(text).map_err(|error| format!("Invalid game message: {error}"))
}
