//! Retained accepted-chat speech from Website `world-speech-presentation.ts`.
//! This is a client receipt clock; snapshot history never starts a world bubble.

use super::{GameChatChannel, GameChatMessage};

pub const GAME_WORLD_SPEECH_HOLD_MS: f64 = 3_000.0;
pub const GAME_WORLD_SPEECH_FADE_MS: f64 = 2_000.0;
pub const GAME_WORLD_SPEECH_SPEAKER_LIMIT: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub struct GameWorldSpeech {
    pub channel: GameChatChannel,
    pub expires_at_ms: f64,
    pub hold_until_ms: f64,
    pub player_id: String,
    pub sequence: u64,
    pub started_at_ms: f64,
    pub text: String,
}

#[must_use]
pub fn append_game_world_speech(
    speeches: &mut Vec<GameWorldSpeech>,
    message: &GameChatMessage,
    received_at_ms: f64,
) -> bool {
    if speeches.iter().any(|speech| {
        speech.player_id == message.sender.player_id && speech.sequence >= message.sequence
    }) {
        return false;
    }
    speeches.retain(|speech| {
        speech.player_id != message.sender.player_id && received_at_ms < speech.expires_at_ms
    });
    speeches.push(GameWorldSpeech {
        channel: message.channel,
        expires_at_ms: received_at_ms + GAME_WORLD_SPEECH_HOLD_MS + GAME_WORLD_SPEECH_FADE_MS,
        hold_until_ms: received_at_ms + GAME_WORLD_SPEECH_HOLD_MS,
        player_id: message.sender.player_id.clone(),
        sequence: message.sequence,
        started_at_ms: received_at_ms,
        text: message.text.clone(),
    });
    if speeches.len() > GAME_WORLD_SPEECH_SPEAKER_LIMIT {
        speeches.drain(..speeches.len() - GAME_WORLD_SPEECH_SPEAKER_LIMIT);
    }
    true
}

#[must_use]
pub fn sample_game_world_speech_alpha(speech: &GameWorldSpeech, now_ms: f64) -> Option<f64> {
    if now_ms >= speech.expires_at_ms {
        return None;
    }
    Some(if now_ms <= speech.hold_until_ms {
        1.0
    } else {
        (speech.expires_at_ms - now_ms) / (speech.expires_at_ms - speech.hold_until_ms)
    })
}
