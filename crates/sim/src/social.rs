//! Host-authored social records from Website `protocol/party-state.ts`,
//! `protocol/game-chat.ts`, `game-chat.ts` and `party-menu-presentation.ts`.
//!
//! The native host owns admission, membership and accepted chat sequences. A view reads
//! this projection and emits commands; it never creates a member or accepts its own chat.
//! Directory fields are absent for a direct native session without a broker.

mod allies;
pub mod broker;
mod card;
mod chat;
mod hub_players;
mod party;
pub mod party_system;
pub mod party_ui;
mod speech;

pub use allies::*;
pub use card::*;
pub(crate) use chat::javascript_whitespace;
pub use chat::*;
pub use hub_players::*;
pub use party::*;
pub use party_system::{PartyIdentity, PartyInvitation, PartyJoinRequest, PartySystem};
pub use speech::*;

use crate::profile::{WizardDiscipline, WizardElement};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum GameSessionKind {
    GlobalHub,
    PrivateCollege,
    Standalone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum GameChatWorldKind {
    Boneyard,
    Hub,
}

/// The Website hello's self-reported social identity. It never grants authority.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerSocialProfile {
    pub account_username: Option<String>,
    pub highest_wave: Option<u64>,
    pub total_playtime_ms: Option<u64>,
}

/// `DEFAULT_GAME_ONLINE_PREFERENCES` from `protocol/game-chat.ts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct GameOnlinePreferences {
    #[cfg_attr(feature = "persistence", serde(alias = "activity_messages"))]
    pub activity_messages: bool,
    #[cfg_attr(feature = "persistence", serde(alias = "global_chat"))]
    pub global_chat: bool,
    #[cfg_attr(feature = "persistence", serde(alias = "submit_runs"))]
    pub submit_runs: bool,
}

impl Default for GameOnlinePreferences {
    fn default() -> Self {
        Self { activity_messages: true, global_chat: true, submit_runs: true }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct GameCollegeInvitation {
    pub expires_at_unix_ms: u64,
    pub id: String,
    pub inviter: GameChatSender,
    pub join_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct GamePlayerCardProfile {
    /// The admitted participant ID links a roster click to this actual profile.
    pub player_id: String,
    pub account_username: Option<String>,
    pub activity: GameChatWorldKind,
    pub discipline: WizardDiscipline,
    pub display_name: String,
    pub element: WizardElement,
    pub gold: u64,
    pub highest_wave: Option<u64>,
    pub player_reference: String,
    pub session_kind: GameSessionKind,
    pub total_playtime_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SocialSnapshot {
    pub local_player_id: String,
    pub session_kind: GameSessionKind,
    pub world_kind: GameChatWorldKind,
    /// The direct native host has no global directory, invitation service or code resolver.
    pub directory_available: bool,
    pub global_chat_available: bool,
    pub party_state: Option<LocalPartyState>,
    pub chat_messages: Vec<GameChatMessage>,
    pub player_cards: Vec<GamePlayerCardProfile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SocialAction {
    SendChat { channel: GameChatChannel, text: String, recipient_player_reference: Option<String> },
    RequestPlayerCard { player_id: String },
    RequestPlayerCardByReference { player_reference: String },
    Kick { player_id: String },
    LeaveParty,
    Invite { player_id: String },
    AcceptInvitation { invitation_id: String },
    DenyInvitation { invitation_id: String },
    DecideJoinRequest { request_id: String, accept: bool },
    SetPartyVisibility { visibility: PartyVisibility },
    RotatePartyCode,
}
