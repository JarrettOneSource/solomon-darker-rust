//! Hosted PlayerCardDialog admission from Website `MainMenuScene.tsx`.

use super::{GameChatWorldKind, GameSessionKind};

/// The host still validates the command. The source presents Invite to Party in
/// a leader's private College when the resolved wizard is also in a Hub.
#[must_use]
pub fn native_player_card_college_invite_available(
    session_kind: GameSessionKind,
    world_kind: GameChatWorldKind,
    local_player_is_leader: bool,
    resolved_player_activity: GameChatWorldKind,
) -> bool {
    session_kind == GameSessionKind::PrivateCollege
        && world_kind == GameChatWorldKind::Hub
        && local_player_is_leader
        && resolved_player_activity == GameChatWorldKind::Hub
}
