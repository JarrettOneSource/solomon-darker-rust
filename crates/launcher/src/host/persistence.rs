//! One bounded native checkpoint writer. The authority captures a completed tick once;
//! JSON encoding and durable file I/O run here, outside its 100 Hz deadline loop.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread::{self, JoinHandle};

use serde::{Deserialize, Serialize};
use solomon_sim::session::{DetachedSessionParticipant, GameCheckpoint, SessionParticipant};
use solomon_sim::social::{GameChatMessage, PartyMembership};

use super::checkpoint::CheckpointFile;
use crate::transport::PeerId;

const HOST_DOCUMENT_VERSION: u32 = 1;

/// An absent player's saved wizard: in the Hub, detached from an active run, or retired.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SavedParticipant {
    Hub {
        participant: Box<SessionParticipant>,
    },
    Run {
        detached: Box<DetachedSessionParticipant>,
    },
    /// The last materialized actor remains the durable offline roster projection. A future
    /// admission creates its fresh book through the canonical retired-profile command.
    Retired {
        participant: Box<SessionParticipant>,
        economy: Box<solomon_sim::economy::HubEconomyState>,
        hagatha_runtime: solomon_sim::profile::HagathaRuntimeState,
    },
}

/// A player's durable resume credentials and saved wizard, plus the connection now using
/// them.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeSlot {
    pub token: String,
    /// Keep the credential the client presented until this generation ACKs admission.
    /// Both credentials must survive a host restart during the Welcome transfer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_token: Option<String>,
    pub player_reference: String,
    pub saved: Option<SavedParticipant>,
    /// A transport generation is meaningful only in this process, and never authorizes
    /// a reconnect after restart. The durable token and participant do that instead.
    #[serde(skip)]
    pub peer_id: Option<PeerId>,
}

impl ResumeSlot {
    pub fn accepts(&self, token: &str) -> bool {
        super::signing::credentials_equal(&self.token, token)
            || self
                .previous_token
                .as_deref()
                .is_some_and(|previous| super::signing::credentials_equal(previous, token))
    }
}

/// Host state saved beside the game checkpoint: revision, IDs, party, resume slots and chat.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostSaveMetadata {
    pub revision: u64,
    pub next_player_id: u64,
    pub session_id: String,
    pub party: Option<PartyMembership>,
    pub resume_slots: BTreeMap<String, ResumeSlot>,
    pub next_chat_sequence: u64,
    pub chat_messages: Vec<GameChatMessage>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostSaveDocument {
    version: u32,
    host: HostSaveMetadata,
    game_document: String,
}

pub fn decode_host_document(text: &str) -> Result<(HostSaveMetadata, GameCheckpoint), String> {
    let document: HostSaveDocument = serde_json::from_str(text)
        .map_err(|error| format!("Cannot decode the native host checkpoint: {error}"))?;
    if document.version != HOST_DOCUMENT_VERSION {
        return Err(format!("Native host checkpoint version {} is unsupported", document.version));
    }
    let checkpoint = solomon_assets::save_document::decode(&document.game_document)?;
    Ok((document.host, checkpoint))
}

/// The checkpoint writer's report for one save: its revision, tick and result.
pub struct SaveCompletion {
    pub revision: u64,
    pub tick: u64,
    pub result: Result<(), String>,
}

struct SaveRequest {
    checkpoint: Box<GameCheckpoint>,
    metadata: HostSaveMetadata,
}

/// A background thread that encodes and commits one host checkpoint at a time.
pub struct CheckpointWriter {
    requests: Option<SyncSender<SaveRequest>>,
    completions: Receiver<SaveCompletion>,
    worker: Option<JoinHandle<()>>,
    in_flight: bool,
}

impl CheckpointWriter {
    pub fn start(file: CheckpointFile) -> Result<Self, String> {
        let (requests, receiver) = mpsc::sync_channel::<SaveRequest>(1);
        let (completed, completions) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("solomon-checkpoint".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    let revision = request.metadata.revision;
                    let tick = request.checkpoint.ticks;
                    let result = encode_and_commit(&file, request);
                    if completed.send(SaveCompletion { revision, tick, result }).is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| format!("Cannot start the native checkpoint writer: {error}"))?;
        Ok(Self { requests: Some(requests), completions, worker: Some(worker), in_flight: false })
    }

    #[must_use]
    pub fn is_available(&self) -> bool {
        !self.in_flight
    }

    /// At most one owned checkpoint exists in the writer. Later save requests stay as
    /// small revision cursors in the authority and coalesce into its next capture.
    pub fn submit(
        &mut self,
        checkpoint: GameCheckpoint,
        metadata: HostSaveMetadata,
    ) -> Result<(), String> {
        if self.in_flight {
            return Err("The native checkpoint writer already owns a completed tick".into());
        }
        self.requests
            .as_ref()
            .ok_or("The native checkpoint writer is closed")?
            .try_send(SaveRequest { checkpoint: Box::new(checkpoint), metadata })
            .map_err(|_| "The native checkpoint writer stopped unexpectedly")?;
        self.in_flight = true;
        Ok(())
    }

    pub fn poll(&mut self) -> Result<Option<SaveCompletion>, String> {
        match self.completions.try_recv() {
            Ok(completion) => {
                self.in_flight = false;
                Ok(Some(completion))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                Err("The native checkpoint writer stopped unexpectedly".into())
            }
        }
    }

    pub fn finish(&mut self) -> Result<(), String> {
        self.requests.take();
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| "The native checkpoint writer failed")?;
        }
        Ok(())
    }
}

impl Drop for CheckpointWriter {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(worker) = self.worker.take() {
            // Explicit `finish` reports worker failure to the launcher. Drop only joins
            // during an earlier error, so a second failure cannot hide the original one.
            let _ = worker.join();
        }
    }
}

fn encode_and_commit(file: &CheckpointFile, request: SaveRequest) -> Result<(), String> {
    let game_document = solomon_assets::save_document::encode(&request.checkpoint)?;
    let document =
        HostSaveDocument { version: HOST_DOCUMENT_VERSION, host: request.metadata, game_document };
    let text = serde_json::to_string(&document)
        .map_err(|error| format!("Cannot encode native host metadata: {error}"))?;
    file.commit(&text)
}
