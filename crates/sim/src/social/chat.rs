//! Chat text, history, channels and rate limits from `protocol/game-chat.ts`, `game-chat.ts`
//! and `host/game-host.ts`'s `chatRateRetryAfter`. The host supplies the clock and routing.

use super::{GameChatWorldKind, LocalPartyState};

pub const GAME_CHAT_MAX_TEXT_CODE_UNITS: usize = 180;
pub const GAME_CHAT_MAX_TEXT_BYTES: usize = 512;
pub const GAME_CHAT_HISTORY_LIMIT: usize = 80;
pub const GAME_CHAT_INACTIVITY_HOLD_MS: u64 = 5_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum GameChatChannel {
    Boneyard,
    Global,
    Party,
    Whisper,
}

impl GameChatChannel {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Boneyard => "Boneyard",
            Self::Global => "Global",
            Self::Party => "Party",
            Self::Whisper => "Whisper",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum GameChatActivity {
    EnteredCollege,
    SearchingSolomon,
    LeftGame,
}

impl GameChatActivity {
    #[must_use]
    pub fn text(self, display_name: &str) -> String {
        match self {
            Self::EnteredCollege => format!("{display_name} has entered the college."),
            Self::SearchingSolomon => format!("{display_name} is searching for Solomon."),
            Self::LeftGame => format!("{display_name} has left the game."),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct GameChatSender {
    pub display_name: String,
    pub player_id: String,
    pub player_reference: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct GameChatMessage {
    pub activity: Option<GameChatActivity>,
    pub channel: GameChatChannel,
    pub recipient: Option<GameChatSender>,
    pub sender: GameChatSender,
    pub sequence: u64,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameChatTextError {
    Empty,
    TooManyCharacters,
    ControlCharacter,
    TooManyBytes,
}

impl GameChatTextError {
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::Empty => "Chat messages cannot be empty.",
            Self::TooManyCharacters => "Chat messages may contain at most 180 characters.",
            Self::ControlCharacter => "Chat messages cannot contain control characters.",
            Self::TooManyBytes => "Chat messages may contain at most 512 bytes.",
        }
    }
}

pub fn normalize_game_chat_text(value: &str) -> Result<&str, GameChatTextError> {
    // JavaScript trim includes BOM and excludes U+0085. Rust's Unicode trim differs.
    let text = value.trim_matches(javascript_whitespace);
    if text.is_empty() {
        return Err(GameChatTextError::Empty);
    }
    if text.encode_utf16().count() > GAME_CHAT_MAX_TEXT_CODE_UNITS {
        return Err(GameChatTextError::TooManyCharacters);
    }
    if text.chars().any(|character| character <= '\u{1f}' || character == '\u{7f}') {
        return Err(GameChatTextError::ControlCharacter);
    }
    if text.len() > GAME_CHAT_MAX_TEXT_BYTES {
        return Err(GameChatTextError::TooManyBytes);
    }
    Ok(text)
}

pub(crate) fn javascript_whitespace(character: char) -> bool {
    matches!(
        character,
        '\u{0009}'..='\u{000d}'
            | '\u{0020}'
            | '\u{00a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    )
}

/// `availableGameChatChannels` with the actual global service capability. A private
/// direct-join College does not acquire a Global channel merely from its session label.
#[must_use]
pub fn available_game_chat_channels(
    world_kind: GameChatWorldKind,
    party_state: Option<&LocalPartyState>,
    global_chat_available: bool,
    has_whisper_thread: bool,
) -> Vec<GameChatChannel> {
    let grouped = party_state.is_some_and(|state| state.party.member_player_ids.len() > 1);
    let mut channels = match world_kind {
        GameChatWorldKind::Boneyard => vec![GameChatChannel::Boneyard],
        GameChatWorldKind::Hub if !global_chat_available || grouped => vec![GameChatChannel::Party],
        GameChatWorldKind::Hub => Vec::new(),
    };
    if global_chat_available {
        channels.push(GameChatChannel::Global);
    }
    if has_whisper_thread {
        channels.push(GameChatChannel::Whisper);
    }
    channels
}

#[must_use]
pub fn next_game_chat_channel(
    current: GameChatChannel,
    channels: &[GameChatChannel],
) -> GameChatChannel {
    assert!(!channels.is_empty(), "a chat world always supplies a base channel");
    let next = channels.iter().position(|channel| *channel == current).map_or(0, |index| index + 1);
    channels[next % channels.len()]
}

/// Returns whether the accepted message changed the bounded history. The view never uses
/// this to accept its own pending send; only host-authored messages reach this function.
pub fn append_game_chat_message(
    messages: &mut Vec<GameChatMessage>,
    message: GameChatMessage,
) -> bool {
    if message.sequence <= messages.last().map_or(0, |last| last.sequence) {
        return false;
    }
    messages.push(message);
    if messages.len() > GAME_CHAT_HISTORY_LIMIT {
        messages.drain(..messages.len() - GAME_CHAT_HISTORY_LIMIT);
    }
    true
}

#[must_use]
pub fn should_increment_game_chat_unread(
    message: &GameChatMessage,
    local_player_id: &str,
    open: bool,
    current_channel: GameChatChannel,
) -> bool {
    message.sender.player_id != local_player_id && (!open || current_channel != message.channel)
}

#[must_use]
pub fn is_game_chat_faded(open: bool, last_activity_at_ms: f64, now_ms: f64) -> bool {
    !open && now_ms - last_activity_at_ms >= GAME_CHAT_INACTIVITY_HOLD_MS as f64
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum GameChatRejectionReason {
    ChannelUnavailable,
    RateLimited,
    TargetUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct GameChatRejection {
    pub channel: GameChatChannel,
    pub reason: GameChatRejectionReason,
    pub retry_after_ms: u64,
}

impl GameChatRejection {
    #[must_use]
    pub fn text(self) -> String {
        match self.reason {
            GameChatRejectionReason::ChannelUnavailable => {
                format!("{} chat is unavailable here.", self.channel.label())
            }
            GameChatRejectionReason::TargetUnavailable => {
                "That wizard is no longer connected.".to_string()
            }
            GameChatRejectionReason::RateLimited => {
                format!("Slow down. Try again in {}s.", self.retry_after_ms.div_ceil(1000).max(1))
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameChatRateLimiter {
    accepted_at_ms: Vec<u64>,
}

impl GameChatRateLimiter {
    /// Call after channel/recipient admission, once for an attempted accepted send.
    /// Zero admits and records it; a positive value is the source retry delay.
    pub fn record(&mut self, now_ms: u64) -> u64 {
        if now_ms >= 5_000 {
            while self.accepted_at_ms.first().is_some_and(|time| *time <= now_ms - 5_000) {
                self.accepted_at_ms.remove(0);
            }
        }
        if self.accepted_at_ms.len() >= 5 {
            return (self.accepted_at_ms[0] + 5_000).saturating_sub(now_ms).max(1);
        }
        self.accepted_at_ms.push(now_ms);
        0
    }
}
