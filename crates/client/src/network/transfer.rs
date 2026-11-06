//! Logical reception from `protocol/game-welcome-transfer.ts` and
//! `protocol/game-save-checkpoint-transfer.ts`. Partial strings never reach the game.

use serde::de::DeserializeOwned;
use solomon_protocol::hosted::{HostedServerMessage, SaveCheckpointReason};
use solomon_protocol::transfer::{GameStringChunk, GameStringReceiver};
use solomon_protocol::{ClientMessage, ServerMessage};

struct ReceivingCheckpoint {
    sequence: u64,
    reason: SaveCheckpointReason,
    total_length: usize,
}

pub(super) struct MessageReceiver {
    welcome: GameStringReceiver,
    welcome_pending: bool,
    welcome_complete: bool,
    welcome_acknowledgement: Option<usize>,
    saves: GameStringReceiver,
    save_sequence: u64,
    save: Option<ReceivingCheckpoint>,
}

pub(super) struct Reception<S> {
    pub message: Option<ServerMessage<S>>,
    pub acknowledgement: Option<ClientMessage>,
}

impl Default for MessageReceiver {
    fn default() -> Self {
        Self {
            welcome: GameStringReceiver::new(super::MAX_FRAME_BYTES, "welcome".into()),
            welcome_pending: false,
            welcome_complete: false,
            welcome_acknowledgement: None,
            saves: GameStringReceiver::new(solomon_protocol::MAX_SAVE_BYTES, "save".into()),
            save_sequence: 0,
            save: None,
        }
    }
}

impl MessageReceiver {
    pub fn receive<S: DeserializeOwned>(
        &mut self,
        message: ServerMessage<S>,
    ) -> Result<Reception<S>, String> {
        if let ServerMessage::WelcomeChunk { data, offset, total_length } = message {
            if self.welcome_complete {
                return Err("The server sent a welcome chunk after its welcome".into());
            }
            let next_offset = offset + data.len();
            let payload = self.welcome.accept(&GameStringChunk { data, offset, total_length })?;
            self.welcome_pending = payload.is_none();
            let Some(payload) = payload else {
                return Ok(Reception {
                    message: None,
                    acknowledgement: Some(ClientMessage::WelcomeChunkAck { next_offset }),
                });
            };
            let text =
                String::from_utf16(&payload).map_err(|_| "The welcome contains invalid Unicode")?;
            let welcome = serde_json::from_str(&text)
                .map_err(|error| format!("The welcome contains an invalid message: {error}"))?;
            if !matches!(welcome, ServerMessage::Welcome { .. }) {
                return Err("The welcome transfer contains another message".into());
            }
            self.welcome_complete = true;
            // Release the host's prefix only after the game validates this welcome.
            self.welcome_acknowledgement = Some(next_offset);
            return Ok(Reception { message: Some(welcome), acknowledgement: None });
        }
        if self.welcome_pending && !matches!(message, ServerMessage::Error { .. }) {
            return Err("The server interrupted its welcome transfer".into());
        }
        if matches!(message, ServerMessage::Welcome { .. }) {
            if self.welcome_complete {
                return Err("The server sent another welcome".into());
            }
            self.welcome_complete = true;
        }
        match message {
            ServerMessage::Hosted {
                message:
                    HostedServerMessage::SaveCheckpointChunk {
                        data,
                        offset,
                        total_length,
                        reason,
                        sequence,
                    },
            } => {
                let next_offset = offset + data.len();
                let save = self.accept_save_chunk(
                    GameStringChunk { data, offset, total_length },
                    reason,
                    sequence,
                )?;
                Ok(Reception {
                    message: save.map(|save| ServerMessage::Hosted {
                        message: HostedServerMessage::SaveCheckpoint { save, reason, sequence },
                    }),
                    acknowledgement: Some(ClientMessage::SaveCheckpointChunkAck {
                        sequence,
                        next_offset,
                    }),
                })
            }
            ServerMessage::Hosted {
                message: HostedServerMessage::SaveCheckpoint { ref sequence, .. },
            } => {
                if *sequence > self.save_sequence {
                    self.save_sequence = *sequence;
                    if self.save.as_ref().is_some_and(|save| save.sequence <= *sequence) {
                        self.saves.close();
                        self.save = None;
                    }
                }
                Ok(Reception { message: Some(message), acknowledgement: None })
            }
            message => Ok(Reception { message: Some(message), acknowledgement: None }),
        }
    }

    fn accept_save_chunk(
        &mut self,
        chunk: GameStringChunk,
        reason: SaveCheckpointReason,
        sequence: u64,
    ) -> Result<Option<String>, String> {
        if sequence <= self.save_sequence
            || self.save.as_ref().is_some_and(|save| sequence < save.sequence)
        {
            return Ok(None);
        }
        if self.save.as_ref().is_none_or(|save| sequence > save.sequence) {
            if chunk.offset != 0 {
                return Err("Checkpoint transfer is missing its first chunk".into());
            }
            self.saves.close();
            self.save =
                Some(ReceivingCheckpoint { sequence, reason, total_length: chunk.total_length });
        }
        let transfer = self.save.as_ref().expect("the first chunk owns its checkpoint");
        if reason != transfer.reason || chunk.total_length != transfer.total_length {
            return Err("Checkpoint chunk does not match its pending transfer".into());
        }
        let Some(payload) = self.saves.accept(&chunk)? else { return Ok(None) };
        self.save = None;
        self.save_sequence = sequence;
        String::from_utf16(&payload)
            .map(Some)
            .map_err(|_| "The save checkpoint contains invalid Unicode".into())
    }

    pub fn take_welcome_acknowledgement(&mut self) -> Option<usize> {
        self.welcome_acknowledgement.take()
    }

    pub fn close(&mut self) {
        self.welcome.close();
        self.welcome_pending = false;
        self.welcome_complete = true;
        self.welcome_acknowledgement = None;
        self.saves.close();
        self.save = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use solomon_protocol::transfer::GameStringSender;

    fn chunk(
        sequence: u64,
        offset: usize,
        total_length: usize,
        text: &str,
    ) -> ServerMessage<Value> {
        ServerMessage::Hosted {
            message: HostedServerMessage::SaveCheckpointChunk {
                data: text.encode_utf16().collect(),
                offset,
                total_length,
                reason: SaveCheckpointReason::Progress,
                sequence,
            },
        }
    }

    #[test]
    fn atomic_checkpoint_cancels_an_incomplete_older_save_and_late_chunks_stay_private() {
        let mut receiver = MessageReceiver::default();
        let partial = receiver.receive(chunk(1, 0, 10, "hello")).unwrap();
        assert!(partial.message.is_none());
        assert!(matches!(
            partial.acknowledgement,
            Some(ClientMessage::SaveCheckpointChunkAck { sequence: 1, next_offset: 5 })
        ));
        let complete = receiver
            .receive::<Value>(ServerMessage::Hosted {
                message: HostedServerMessage::SaveCheckpoint {
                    save: "final".into(),
                    reason: SaveCheckpointReason::GameOver,
                    sequence: 2,
                },
            })
            .unwrap();
        assert!(complete.message.is_some());
        assert!(receiver.receive(chunk(1, 5, 10, "world")).unwrap().message.is_none());
        assert!(receiver.receive(chunk(3, 1, 3, "no")).is_err());
        assert!(receiver.receive(chunk(3, 0, 3, "new")).unwrap().message.is_some());
    }

    #[test]
    fn welcome_acknowledges_progress_and_holds_the_final_ack_for_game_acceptance() {
        let welcome: ServerMessage<Value> = ServerMessage::Welcome {
            version: solomon_protocol::PROTOCOL_VERSION,
            credentials: solomon_protocol::ResumeCredentials {
                player_id: "wizard".into(),
                token: "retained".into(),
            },
            tick_rate: solomon_protocol::GAME_TICK_RATE,
            snapshot_rate: solomon_protocol::SNAPSHOT_RATE,
            frame: solomon_protocol::SnapshotFrame {
                sequence: 1,
                flame_presentation: None,
                snapshot: serde_json::json!({"padding":"🧙".repeat(5000)}),
                input_acknowledgements: Default::default(),
                host: Default::default(),
            },
            hosted: None,
        };
        let mut sender = GameStringSender::new(
            serde_json::to_string(&welcome).unwrap().encode_utf16().collect(),
        )
        .unwrap();
        let chunks = sender.start();
        assert!(chunks.len() > 1);
        let mut receiver = MessageReceiver::default();
        for (index, chunk) in chunks.iter().enumerate() {
            let reception = receiver
                .receive::<Value>(ServerMessage::WelcomeChunk {
                    data: chunk.data.clone(),
                    offset: chunk.offset,
                    total_length: chunk.total_length,
                })
                .unwrap();
            let last = index == chunks.len() - 1;
            assert_eq!(reception.message.is_some(), last);
            assert_eq!(reception.acknowledgement.is_none(), last);
        }
        assert_eq!(receiver.take_welcome_acknowledgement(), Some(chunks[0].total_length));
        assert!(receiver.receive(welcome).is_err());
    }

    #[test]
    fn a_welcome_prefix_rejects_an_interleaved_game_message() {
        let mut receiver = MessageReceiver::default();
        receiver
            .receive::<Value>(ServerMessage::WelcomeChunk {
                data: vec![123],
                offset: 0,
                total_length: 10,
            })
            .unwrap();
        assert!(receiver.receive::<Value>(ServerMessage::Pong { nonce: 1 }).is_err());
    }
}
