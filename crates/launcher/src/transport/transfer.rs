//! Welcome and save-checkpoint senders over the peer's ordered outbox. Payloads above the
//! stream threshold go out as acknowledged UTF-16 chunks. No message is reparsed and no
//! transfer retains simulation state.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use solomon_protocol::ServerMessage;
use solomon_protocol::hosted::{HostedServerMessage, SaveCheckpointReason};
use solomon_protocol::transfer::{
    GAME_STRING_STREAM_THRESHOLD, GAME_WELCOME_PROGRESS_TIMEOUT_MS, GameStringAcknowledgement,
    GameStringChunk, GameStringSender,
};

use super::{PeerSendError, PeerSender};

/// A peer's in-progress Welcome and save-checkpoint transfers.
#[derive(Default)]
pub(super) struct PeerTransfers {
    pub welcome: Option<WelcomeTransfer>,
    completed_welcome_offset: Option<usize>,
    pub save: SaveCheckpointSender,
    welcome_timed_out: bool,
}

/// A chunked Welcome awaiting acknowledgement, with its progress deadline and the reliable
/// payloads deferred behind it.
pub(super) struct WelcomeTransfer {
    sender: GameStringSender,
    total_length: usize,
    deadline: Instant,
    deferred: VecDeque<Arc<str>>,
    deferred_bytes: usize,
}

impl WelcomeTransfer {
    pub(super) fn defer(&mut self, payload: Arc<str>, maximum: usize) -> Result<(), PeerSendError> {
        if payload.len() > maximum.saturating_sub(self.deferred_bytes) {
            return Err(PeerSendError::WelcomeSidebandLimit);
        }
        self.deferred_bytes += payload.len();
        self.deferred.push_back(payload);
        Ok(())
    }
}

impl PeerSender {
    /// Sends a Welcome. A large one is chunked, and later reliable payloads wait behind it
    /// until the client acknowledges all of it.
    pub fn send_welcome(&self, payload: Arc<str>) -> Result<(), PeerSendError> {
        self.send_welcome_at(payload, Instant::now())
    }

    fn send_welcome_at(&self, payload: Arc<str>, now: Instant) -> Result<(), PeerSendError> {
        self.validate_payload(&payload)?;
        let data: Vec<u16> = payload.encode_utf16().collect();
        if data.len() <= GAME_STRING_STREAM_THRESHOLD {
            return self.send_reliable(payload);
        }
        let mut transfers =
            self.0.transfers.lock().expect("transfer producers never panic while locked");
        let total_length = data.len();
        let mut sender = GameStringSender::new(data).expect("an encoded welcome is nonempty");
        let chunks = sender.start();
        transfers.welcome = Some(WelcomeTransfer {
            sender,
            total_length,
            deadline: now + Duration::from_millis(GAME_WELCOME_PROGRESS_TIMEOUT_MS),
            deferred: VecDeque::new(),
            deferred_bytes: 0,
        });
        transfers.completed_welcome_offset = None;
        for chunk in chunks {
            self.queue_reliable(welcome_chunk(chunk))?;
        }
        Ok(())
    }

    #[must_use]
    pub fn welcome_pending(&self) -> bool {
        self.0
            .transfers
            .lock()
            .expect("transfer producers never panic while locked")
            .welcome
            .is_some()
    }

    pub fn acknowledge_welcome(&self, next_offset: usize) -> Result<(), String> {
        self.acknowledge_welcome_at(next_offset, Instant::now())
    }

    fn acknowledge_welcome_at(&self, next_offset: usize, now: Instant) -> Result<(), String> {
        let mut transfers =
            self.0.transfers.lock().expect("transfer producers never panic while locked");
        let Some(welcome) = &mut transfers.welcome else {
            return if transfers.completed_welcome_offset.is_some_and(|offset| next_offset <= offset)
            {
                Ok(())
            } else {
                Err("Welcome acknowledgement has no pending transfer.".into())
            };
        };
        let progress = welcome.sender.acknowledge(next_offset)?;
        match progress.acknowledgement {
            GameStringAcknowledgement::Stale => return Ok(()),
            GameStringAcknowledgement::Advanced => {
                welcome.deadline = now + Duration::from_millis(GAME_WELCOME_PROGRESS_TIMEOUT_MS);
                for chunk in progress.chunks {
                    self.queue_reliable(welcome_chunk(chunk)).map_err(|error| error.to_string())?;
                }
            }
            GameStringAcknowledgement::Complete => {
                let welcome =
                    transfers.welcome.take().expect("the completed transfer remains retained");
                transfers.completed_welcome_offset = Some(welcome.total_length);
                // The deferred payloads were bounded by total bytes, not message count,
                // so they join the outbox whole and in order.
                let mut outbox =
                    self.0.outbox.lock().expect("the peer outbox never panics while locked");
                if self.is_closed() {
                    return Ok(());
                }
                outbox.reliable_bytes += welcome.deferred_bytes;
                outbox.reliable.extend(welcome.deferred);
            }
        }
        Ok(())
    }

    pub fn cancel_welcome(&self) {
        self.0.transfers.lock().expect("transfer producers never panic while locked").welcome =
            None;
    }

    pub(super) fn expire_welcome(&self, now: Instant) -> bool {
        let mut transfers =
            self.0.transfers.lock().expect("transfer producers never panic while locked");
        if transfers.welcome.as_ref().is_none_or(|welcome| now < welcome.deadline) {
            return false;
        }
        transfers.welcome = None;
        transfers.welcome_timed_out = true;
        true
    }

    #[must_use]
    pub fn welcome_timed_out(&self) -> bool {
        self.0
            .transfers
            .lock()
            .expect("transfer producers never panic while locked")
            .welcome_timed_out
    }

    pub fn publish_save_checkpoint(
        &self,
        save: String,
        reason: SaveCheckpointReason,
        sequence: u64,
        atomic: bool,
    ) -> Result<(), String> {
        self.publish_prepared_save_checkpoint(PreparedSaveCheckpoint::new(
            save, reason, sequence, atomic,
        )?)
    }

    /// Admit an already encoded checkpoint through the same sequence and queue bounds.
    pub fn publish_prepared_save_checkpoint(
        &self,
        prepared: PreparedSaveCheckpoint,
    ) -> Result<(), String> {
        let messages = self
            .0
            .transfers
            .lock()
            .expect("transfer producers never panic while locked")
            .save
            .publish(prepared)?;
        self.send_checkpoint_messages(messages)
    }

    pub fn acknowledge_save_checkpoint(
        &self,
        sequence: u64,
        next_offset: usize,
    ) -> Result<(), String> {
        let messages = self
            .0
            .transfers
            .lock()
            .expect("transfer producers never panic while locked")
            .save
            .acknowledge(sequence, next_offset)?;
        self.send_checkpoint_messages(messages)
    }

    fn send_checkpoint_messages(&self, messages: Vec<Arc<str>>) -> Result<(), String> {
        for payload in messages {
            match self.send_reliable(payload) {
                Ok(()) | Err(PeerSendError::Closed) => {}
                Err(PeerSendError::ReliableQueueFull) => {
                    self.close(1013, "game connection is too slow")
                        .map_err(|error| error.to_string())?;
                    return Ok(());
                }
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok(())
    }
}

fn welcome_chunk(chunk: GameStringChunk) -> Arc<str> {
    serde_json::to_string(&ServerMessage::<()>::WelcomeChunk {
        data: chunk.data,
        offset: chunk.offset,
        total_length: chunk.total_length,
    })
    .expect("integer UTF-16 chunks always serialize")
    .into()
}

/// A checkpoint's complete JSON or UTF-16 stream, prepared away from the authority.
/// Preparation does not reserve a sequence or publish any part of the document.
pub struct PreparedSaveCheckpoint {
    sequence: u64,
    payload: PreparedCheckpointPayload,
}

enum PreparedCheckpointPayload {
    Atomic(Arc<str>),
    Stream(PreparedCheckpointStream),
}

struct PreparedCheckpointStream {
    current: SendingCheckpoint,
    initial_messages: Vec<Arc<str>>,
}

impl PreparedSaveCheckpoint {
    pub fn new(
        save: String,
        reason: SaveCheckpointReason,
        sequence: u64,
        atomic: bool,
    ) -> Result<Self, String> {
        if !atomic {
            let data: Vec<u16> = save.encode_utf16().collect();
            if data.len() > GAME_STRING_STREAM_THRESHOLD {
                let mut sender = GameStringSender::new(data)?;
                let chunks = sender.start();
                let current = SendingCheckpoint { sender, reason, sequence };
                let initial_messages = save_chunks(&current, chunks)?;
                return Ok(Self {
                    sequence,
                    payload: PreparedCheckpointPayload::Stream(PreparedCheckpointStream {
                        current,
                        initial_messages,
                    }),
                });
            }
        }
        let message = HostedServerMessage::SaveCheckpoint { save, reason, sequence };
        let payload = encode_checkpoint_message(message)?;
        Ok(Self { sequence, payload: PreparedCheckpointPayload::Atomic(payload) })
    }
}

struct SendingCheckpoint {
    sender: GameStringSender,
    reason: SaveCheckpointReason,
    sequence: u64,
}

/// Streams one peer's save checkpoints one at a time, keeping only the newest waiting one.
#[derive(Default)]
pub(super) struct SaveCheckpointSender {
    sequence: u64,
    current: Option<SendingCheckpoint>,
    pending: Option<PreparedCheckpointStream>,
}

impl SaveCheckpointSender {
    fn publish(&mut self, message: PreparedSaveCheckpoint) -> Result<Vec<Arc<str>>, String> {
        if message.sequence <= self.sequence {
            return Err("Checkpoint publication sequence must advance".into());
        }
        self.sequence = message.sequence;
        match message.payload {
            PreparedCheckpointPayload::Atomic(payload) => {
                self.close();
                Ok(vec![payload])
            }
            PreparedCheckpointPayload::Stream(stream) => {
                if self.current.is_some() {
                    self.pending = Some(stream);
                    Ok(Vec::new())
                } else {
                    Ok(self.start(stream))
                }
            }
        }
    }

    fn start(&mut self, stream: PreparedCheckpointStream) -> Vec<Arc<str>> {
        self.current = Some(stream.current);
        stream.initial_messages
    }

    fn acknowledge(&mut self, sequence: u64, next_offset: usize) -> Result<Vec<Arc<str>>, String> {
        if sequence > self.sequence {
            return Err("Checkpoint acknowledgment refers to an unsent sequence".into());
        }
        let Some(current) = &mut self.current else {
            return Ok(Vec::new());
        };
        if sequence < current.sequence {
            return Ok(Vec::new());
        }
        if sequence > current.sequence {
            return Err("Checkpoint acknowledgment refers to a queued sequence".into());
        }
        let progress = current.sender.acknowledge(next_offset)?;
        if progress.acknowledgement == GameStringAcknowledgement::Complete {
            self.current = None;
            return Ok(self.pending.take().map(|next| self.start(next)).unwrap_or_default());
        }
        save_chunks(current, progress.chunks)
    }

    pub(super) fn close(&mut self) {
        self.current = None;
        self.pending = None;
    }
}

fn save_chunks(
    current: &SendingCheckpoint,
    chunks: Vec<GameStringChunk>,
) -> Result<Vec<Arc<str>>, String> {
    chunks
        .into_iter()
        .map(|chunk| {
            encode_checkpoint_message(HostedServerMessage::SaveCheckpointChunk {
                data: chunk.data,
                offset: chunk.offset,
                total_length: chunk.total_length,
                reason: current.reason,
                sequence: current.sequence,
            })
        })
        .collect()
}

fn encode_checkpoint_message(message: HostedServerMessage) -> Result<Arc<str>, String> {
    serde_json::to_string(&ServerMessage::<()>::Hosted { message })
        .map(Arc::from)
        .map_err(|error| format!("Cannot encode a save transfer: {error}"))
}

#[cfg(test)]
mod tests;
