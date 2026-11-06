//! When the hosted player card offers its College invite action.

use super::{GameChatWorldKind, GameSessionKind};

/// Invite to Party appears in a leader's private College when the resolved wizard is also
/// in a Hub. The host still validates the command.
#[must_use]
pub fn player_card_college_invite_available(
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
