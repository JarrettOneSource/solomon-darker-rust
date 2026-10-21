//! Party presentation and direct-session admission rules from `host/party-system.ts`.

use crate::combat::PlayerLifeState;
use crate::profile::WizardElement;

use super::GameSessionKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum PartyVisibility {
    Public,
    InviteOnly,
    Private,
}

impl PartyVisibility {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::InviteOnly => "INVITE ONLY",
            Self::Private => "PRIVATE",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PartyPlayerProfile {
    pub account_username: Option<String>,
    pub highest_wave: Option<u64>,
    pub total_playtime_ms: Option<u64>,
    pub display_name: String,
    pub player_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PartyMembership {
    #[cfg_attr(feature = "persistence", serde(default = "default_enhanced_effects"))]
    pub enhanced_effects: bool,
    pub id: String,
    pub join_code: Option<String>,
    pub leader_player_id: String,
    pub listing_id: Option<String>,
    pub member_player_ids: Vec<String>,
    pub visibility: PartyVisibility,
}

#[cfg(feature = "persistence")]
const fn default_enhanced_effects() -> bool {
    true
}

/// The source transfers leadership to the first retained member, preserving join order,
/// and removes an empty party. Disconnection alone does not remove durable membership.
#[must_use]
pub fn party_after_member_departure(
    party: &PartyMembership,
    player_id: &str,
) -> Option<PartyMembership> {
    let mut next = party.clone();
    next.member_player_ids.retain(|member| member != player_id);
    let first_member = next.member_player_ids.first()?;
    if next.leader_player_id == player_id {
        next.leader_player_id.clone_from(first_member);
    }
    Some(next)
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PartyRosterPlayer {
    pub connected: bool,
    pub current_health: f64,
    pub display_name: String,
    pub element: WizardElement,
    pub life_state: PlayerLifeState,
    pub maximum_health: f64,
    pub player_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PartyInvitationView {
    pub id: String,
    pub inviter: PartyPlayerProfile,
    pub party_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PartyJoinRequester {
    pub account_username: Option<String>,
    pub display_name: String,
    pub requester_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PartyJoinRequestView {
    pub id: String,
    pub requester: PartyJoinRequester,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct LocalPartyState {
    pub hub_players: Vec<PartyPlayerProfile>,
    pub invitations: Vec<PartyInvitationView>,
    pub join_requests: Vec<PartyJoinRequestView>,
    pub party: PartyMembership,
    pub party_roster: Vec<PartyRosterPlayer>,
    pub revision: u64,
}

/// `projectPartyState` requires a durable row for every admitted member. A missing actor
/// can be offline; a missing retained roster entry is an authority/projection error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingPartyRosterPlayer(pub String);

pub fn project_direct_party_state(
    party: &PartyMembership,
    profiles: &[PartyPlayerProfile],
    roster: &[PartyRosterPlayer],
    revision: u64,
) -> Result<LocalPartyState, MissingPartyRosterPlayer> {
    let mut party_roster = Vec::with_capacity(party.member_player_ids.len());
    for player_id in &party.member_player_ids {
        let Some(player) = roster.iter().find(|player| player.player_id == *player_id) else {
            return Err(MissingPartyRosterPlayer(player_id.clone()));
        };
        party_roster.push(player.clone());
    }
    let mut hub_players = profiles.to_vec();
    hub_players.sort_by(|left, right| left.player_id.cmp(&right.player_id));
    Ok(LocalPartyState {
        hub_players,
        invitations: Vec::new(),
        join_requests: Vec::new(),
        party: party.clone(),
        party_roster,
        revision,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum PartyActionRejection {
    AlreadyInParty,
    AlreadyInvited,
    AlreadyRequested,
    InvitationMissing,
    NotLeader,
    NotInHub,
    NotRecipient,
    PartyFull,
    PartyMissing,
    PartyPrivate,
    PlayerMissing,
    RequestMissing,
    SameParty,
    SelfInvite,
    SelfKick,
}

impl PartyActionRejection {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::AlreadyInParty => "already-in-party",
            Self::AlreadyInvited => "already-invited",
            Self::AlreadyRequested => "already-requested",
            Self::InvitationMissing => "invitation-missing",
            Self::NotLeader => "not-leader",
            Self::NotInHub => "not-in-hub",
            Self::NotRecipient => "not-recipient",
            Self::PartyFull => "party-full",
            Self::PartyMissing => "party-missing",
            Self::PartyPrivate => "party-private",
            Self::PlayerMissing => "player-missing",
            Self::RequestMissing => "request-missing",
            Self::SameParty => "same-party",
            Self::SelfInvite => "self-invite",
            Self::SelfKick => "self-kick",
        }
    }

    /// Authored `partyActionErrorMessage` copy from `MainMenuScene.tsx`.
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::AlreadyInParty => "That wizard is already in a party.",
            Self::AlreadyInvited => "That invitation is already pending.",
            Self::AlreadyRequested => "That join request is already pending.",
            Self::InvitationMissing => "That invitation has expired.",
            Self::NotLeader => "Only the party leader can do that.",
            Self::NotInHub => "That wizard is not in the Courtyard.",
            Self::NotRecipient => "That invitation belongs to another wizard.",
            Self::PartyFull => "That party is full.",
            Self::PartyMissing => "That party is no longer available.",
            Self::PartyPrivate => "That party is private.",
            Self::PlayerMissing => "That wizard is no longer available.",
            Self::RequestMissing => "That join request has expired.",
            Self::SameParty => "That wizard is already in your party.",
            Self::SelfInvite | Self::SelfKick => "You cannot target yourself.",
        }
    }
}

/// Admission order is the source `kickPartyPlayer` order. The host performs the accepted
/// departure and transport close; this predicate never edits another gameplay owner.
#[must_use]
pub fn party_kick_rejection(
    party: Option<&PartyMembership>,
    requester_id: &str,
    target_id: &str,
) -> Option<PartyActionRejection> {
    let Some(party) =
        party.filter(|party| party.member_player_ids.iter().any(|member| member == requester_id))
    else {
        return Some(PartyActionRejection::PartyMissing);
    };
    if party.leader_player_id != requester_id {
        return Some(PartyActionRejection::NotLeader);
    }
    if target_id == requester_id {
        return Some(PartyActionRejection::SelfKick);
    }
    if !party.member_player_ids.iter().any(|member| member == target_id) {
        return Some(PartyActionRejection::PlayerMissing);
    }
    None
}

#[must_use]
pub fn party_menu_leave_label(
    member_count: usize,
    session_kind: GameSessionKind,
) -> Option<&'static str> {
    if session_kind == GameSessionKind::PrivateCollege {
        Some("LEAVE COLLEGE")
    } else if member_count > 1 {
        Some("LEAVE PARTY")
    } else {
        None
    }
}

#[must_use]
pub fn party_member_display_name<'a>(state: &'a LocalPartyState, player_id: &'a str) -> &'a str {
    state
        .hub_players
        .iter()
        .find(|player| player.player_id == player_id)
        .map(|player| player.display_name.as_str())
        .or_else(|| {
            state
                .party_roster
                .iter()
                .find(|player| player.player_id == player_id)
                .map(|player| player.display_name.as_str())
        })
        .unwrap_or(player_id)
}
