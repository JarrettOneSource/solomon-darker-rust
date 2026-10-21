//! Cross-College social routing from Website `host/game-social-broker.ts`.
//!
//! The native supervisor supplies cryptographic references/IDs and wall time. Profiles
//! and invitation availability are queried from its actual current worlds. This broker
//! retains connection generations and social deliveries, never another player state.

use super::{
    GameChatActivity, GameChatChannel, GameChatSender, GameCollegeInvitation,
    GameOnlinePreferences, GamePlayerCardProfile, PartyActionRejection,
};

pub const GAME_SOCIAL_INVITATION_TIMEOUT_MS: u64 = 10 * 60 * 1_000;
pub const GAME_SOCIAL_MAX_INVITATIONS_PER_PLAYER: usize = 8;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSocialParticipant {
    pub host_id: String,
    pub local_player_id: String,
}

/// Borrowed query boundary for the source participant callbacks. The supervisor reads
/// Session/economy/connection owners here; no social copy can decide gameplay admission.
pub trait GameSocialWorlds {
    fn profile(&self, participant: &GameSocialParticipant) -> Option<GamePlayerCardProfile>;
    fn can_receive_college_invitation(&self, participant: &GameSocialParticipant) -> bool;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSocialConnection {
    pub player_reference: String,
    generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSocialChatDelivery {
    pub activity: Option<GameChatActivity>,
    pub channel: GameChatChannel,
    pub delivery_id: u64,
    pub recipient: Option<GameChatSender>,
    pub sender: GameChatSender,
    pub text: String,
}

pub struct CollegeInvitationRequest<'a> {
    pub target_reference: &'a str,
    pub source_party_id: &'a str,
    pub join_code: &'a str,
    pub now_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameSocialDelivery {
    Chat { connection: GameSocialConnection, message: GameSocialChatDelivery },
    CollegeInvitations { connection: GameSocialConnection, invitations: Vec<GameCollegeInvitation> },
}

struct BrokerMember {
    active: bool,
    participant: GameSocialParticipant,
    connection: GameSocialConnection,
    preferences: GameOnlinePreferences,
}

struct BrokerInvitation {
    invitation: GameCollegeInvitation,
    source_party_id: String,
    source_player_reference: String,
    target_player_reference: String,
}

pub struct GameSocialBroker {
    // JavaScript Map insertion order also defines Global delivery order. Replacing a
    // member keeps its original position, just as Map.set(existingKey, value) does.
    members: Vec<BrokerMember>,
    invitations: Vec<BrokerInvitation>,
    closed: bool,
    next_generation: u64,
    next_chat_delivery_id: u64,
    deliveries: Vec<GameSocialDelivery>,
}

impl Default for GameSocialBroker {
    fn default() -> Self {
        Self {
            members: Vec::new(),
            invitations: Vec::new(),
            closed: false,
            next_generation: 1,
            next_chat_delivery_id: 1,
            deliveries: Vec::new(),
        }
    }
}

impl GameSocialBroker {
    pub fn register(
        &mut self,
        participant: GameSocialParticipant,
        preferences: GameOnlinePreferences,
        player_reference: String,
    ) -> Result<GameSocialConnection, &'static str> {
        if self.closed {
            return Err("The game social broker is closed");
        }
        if !is_player_reference(&player_reference) {
            return Err("The game social participant reference is invalid");
        }
        let index = self
            .members
            .iter()
            .position(|member| member.connection.player_reference == player_reference);
        if index.is_some_and(|index| self.members[index].participant != participant) {
            return Err("The game social participant reference is already owned");
        }
        let connection =
            GameSocialConnection { player_reference, generation: self.next_generation };
        self.next_generation += 1;
        let member = BrokerMember {
            active: false,
            participant,
            connection: connection.clone(),
            preferences,
        };
        match index {
            Some(index) => self.members[index] = member,
            None => self.members.push(member),
        }
        Ok(connection)
    }

    pub fn activate(&mut self, connection: &GameSocialConnection) {
        let Some(index) = self.current_member(connection) else { return };
        if self.members[index].active {
            return;
        }
        self.members[index].active = true;
        self.deliver_invitations(&connection.player_reference);
    }

    pub fn close_connection(&mut self, connection: &GameSocialConnection) {
        let Some(index) = self.current_member(connection) else { return };
        self.members.remove(index);
        self.remove_invitations(|invitation| {
            invitation.source_player_reference == connection.player_reference
                || invitation.target_player_reference == connection.player_reference
        });
    }

    pub fn dismiss_college_invitation(&mut self, connection: &GameSocialConnection, id: &str) {
        if self.current_member(connection).is_none() {
            return;
        }
        let Some(index) = self.invitations.iter().position(|pending| {
            pending.invitation.id == id
                && pending.target_player_reference == connection.player_reference
        }) else {
            return;
        };
        self.invitations.remove(index);
        self.deliver_invitations(&connection.player_reference);
    }

    pub fn invite_to_college(
        &mut self,
        connection: &GameSocialConnection,
        request: CollegeInvitationRequest<'_>,
        create_invitation_id: &mut dyn FnMut() -> String,
        worlds: &impl GameSocialWorlds,
    ) -> Result<(), PartyActionRejection> {
        let CollegeInvitationRequest { target_reference, source_party_id, join_code, now_unix_ms } =
            request;
        let Some(source) = self.current_member(connection).filter(|&index| {
            self.members[index].active && self.members[index].preferences.global_chat
        }) else {
            return Err(PartyActionRejection::PlayerMissing);
        };
        if target_reference == connection.player_reference {
            return Err(PartyActionRejection::SelfInvite);
        }
        let target =
            self.available_member(target_reference).ok_or(PartyActionRejection::PlayerMissing)?;
        if !worlds.can_receive_college_invitation(&self.members[target].participant) {
            return Err(PartyActionRejection::NotInHub);
        }
        self.prune(now_unix_ms);
        if self.invitations.iter().any(|pending| {
            pending.source_party_id == source_party_id
                && pending.target_player_reference == target_reference
        }) || self.invitations_for(target_reference).len()
            >= GAME_SOCIAL_MAX_INVITATIONS_PER_PLAYER
        {
            return Err(PartyActionRejection::AlreadyInvited);
        }
        let invitation = GameCollegeInvitation {
            expires_at_unix_ms: now_unix_ms + GAME_SOCIAL_INVITATION_TIMEOUT_MS,
            id: create_invitation_id(),
            inviter: identity_for(
                &self.members[source],
                &self.members[target].participant.host_id,
                worlds,
            ),
            join_code: join_code.into(),
        };
        self.invitations.push(BrokerInvitation {
            invitation,
            source_party_id: source_party_id.into(),
            source_player_reference: connection.player_reference.clone(),
            target_player_reference: target_reference.into(),
        });
        self.deliver_invitations(target_reference);
        Ok(())
    }

    pub fn publish_activity(
        &mut self,
        connection: &GameSocialConnection,
        activity: GameChatActivity,
        worlds: &impl GameSocialWorlds,
    ) {
        let Some(index) = self.current_member(connection) else { return };
        let sender = &self.members[index];
        if !sender.active
            || !sender.preferences.activity_messages
            || !sender.preferences.global_chat
        {
            return;
        }
        let delivery_id = self.consume_chat_delivery_id();
        let sender = &self.members[index];
        for recipient in &self.members {
            if recipient.connection == *connection
                || !recipient.active
                || !recipient.preferences.activity_messages
                || !recipient.preferences.global_chat
            {
                continue;
            }
            let identity = identity_for(sender, &recipient.participant.host_id, worlds);
            let text = activity.text(
                &worlds
                    .profile(&sender.participant)
                    .map_or_else(|| identity.display_name.clone(), |profile| profile.display_name),
            );
            self.deliveries.push(GameSocialDelivery::Chat {
                connection: recipient.connection.clone(),
                message: GameSocialChatDelivery {
                    activity: Some(activity),
                    channel: GameChatChannel::Global,
                    delivery_id,
                    recipient: None,
                    sender: identity,
                    text,
                },
            });
        }
    }

    pub fn publish_global(
        &mut self,
        connection: &GameSocialConnection,
        text: &str,
        worlds: &impl GameSocialWorlds,
    ) -> bool {
        let Some(index) = self.current_member(connection) else { return false };
        if !self.members[index].active || !self.members[index].preferences.global_chat {
            return false;
        }
        let delivery_id = self.consume_chat_delivery_id();
        let sender = &self.members[index];
        for recipient in &self.members {
            if !recipient.active || !recipient.preferences.global_chat {
                continue;
            }
            self.deliveries.push(GameSocialDelivery::Chat {
                connection: recipient.connection.clone(),
                message: GameSocialChatDelivery {
                    activity: None,
                    channel: GameChatChannel::Global,
                    delivery_id,
                    recipient: None,
                    sender: identity_for(sender, &recipient.participant.host_id, worlds),
                    text: text.into(),
                },
            });
        }
        true
    }

    pub fn publish_whisper(
        &mut self,
        connection: &GameSocialConnection,
        target_reference: &str,
        text: &str,
        worlds: &impl GameSocialWorlds,
    ) -> bool {
        let Some(sender_index) = self.current_member(connection) else { return false };
        let Some(target_index) = self.available_member(target_reference) else { return false };
        if !self.members[sender_index].active
            || !self.members[sender_index].preferences.global_chat
            || sender_index == target_index
        {
            return false;
        }
        let delivery_id = self.consume_chat_delivery_id();
        let sender = &self.members[sender_index];
        let target = &self.members[target_index];
        for index in [sender_index, target_index] {
            let recipient = &self.members[index];
            self.deliveries.push(GameSocialDelivery::Chat {
                connection: recipient.connection.clone(),
                message: GameSocialChatDelivery {
                    activity: None,
                    channel: GameChatChannel::Whisper,
                    delivery_id,
                    recipient: Some(identity_for(target, &recipient.participant.host_id, worlds)),
                    sender: identity_for(sender, &recipient.participant.host_id, worlds),
                    text: text.into(),
                },
            });
        }
        true
    }

    pub fn refresh_college_invitation_availability(
        &mut self,
        connection: &GameSocialConnection,
        worlds: &impl GameSocialWorlds,
    ) {
        let Some(index) = self.current_member(connection) else { return };
        if !worlds.can_receive_college_invitation(&self.members[index].participant) {
            self.remove_invitations(|pending| {
                pending.target_player_reference == connection.player_reference
            });
        } else {
            self.deliver_invitations(&connection.player_reference);
        }
    }

    #[must_use]
    pub fn resolve_player_card(
        &self,
        connection: &GameSocialConnection,
        target_reference: &str,
        worlds: &impl GameSocialWorlds,
    ) -> Option<GamePlayerCardProfile> {
        let requester = self.current_member(connection)?;
        let target = self.available_member(target_reference)?;
        if !self.members[requester].active || !self.members[requester].preferences.global_chat {
            return None;
        }
        let profile = worlds.profile(&self.members[target].participant)?;
        (profile.player_reference == target_reference).then_some(profile)
    }

    pub fn revoke_college_invitations(
        &mut self,
        connection: &GameSocialConnection,
        source_party_id: &str,
    ) {
        if self.current_member(connection).is_none() {
            return;
        }
        self.remove_invitations(|pending| pending.source_party_id == source_party_id);
    }

    pub fn set_online_preferences(
        &mut self,
        connection: &GameSocialConnection,
        preferences: GameOnlinePreferences,
    ) {
        let Some(index) = self.current_member(connection) else { return };
        let was_available = self.members[index].preferences.global_chat;
        self.members[index].preferences = preferences;
        if was_available && !preferences.global_chat {
            self.remove_invitations(|pending| {
                pending.source_player_reference == connection.player_reference
                    || pending.target_player_reference == connection.player_reference
            });
        } else if !was_available && preferences.global_chat {
            self.deliver_invitations(&connection.player_reference);
        }
    }

    pub fn prune(&mut self, now_unix_ms: u64) {
        if self.closed {
            return;
        }
        self.remove_invitations(|pending| pending.invitation.expires_at_unix_ms <= now_unix_ms);
    }

    pub fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.members.clear();
        self.invitations.clear();
    }

    pub fn take_deliveries(&mut self) -> Vec<GameSocialDelivery> {
        std::mem::take(&mut self.deliveries)
    }

    fn current_member(&self, connection: &GameSocialConnection) -> Option<usize> {
        self.members.iter().position(|member| member.connection == *connection)
    }

    fn available_member(&self, reference: &str) -> Option<usize> {
        self.members.iter().position(|member| {
            member.connection.player_reference == reference
                && member.active
                && member.preferences.global_chat
        })
    }

    fn consume_chat_delivery_id(&mut self) -> u64 {
        let id = self.next_chat_delivery_id;
        self.next_chat_delivery_id = if id == MAX_SAFE_INTEGER { 1 } else { id + 1 };
        id
    }

    fn invitations_for(&self, reference: &str) -> Vec<GameCollegeInvitation> {
        let mut invitations = Vec::new();
        for pending in &self.invitations {
            if pending.target_player_reference == reference {
                invitations.push(pending.invitation.clone());
            }
        }
        invitations.sort_by_key(|invitation| invitation.expires_at_unix_ms);
        invitations
    }

    fn deliver_invitations(&mut self, reference: &str) {
        let Some(member) = self
            .members
            .iter()
            .find(|member| member.connection.player_reference == reference && member.active)
        else {
            return;
        };
        let invitations = if member.preferences.global_chat {
            self.invitations_for(reference)
        } else {
            Vec::new()
        };
        self.deliveries.push(GameSocialDelivery::CollegeInvitations {
            connection: member.connection.clone(),
            invitations,
        });
    }

    fn remove_invitations(&mut self, predicate: impl Fn(&BrokerInvitation) -> bool) {
        let mut affected = Vec::new();
        self.invitations.retain(|pending| {
            if !predicate(pending) {
                return true;
            }
            if !affected.contains(&pending.target_player_reference) {
                affected.push(pending.target_player_reference.clone());
            }
            false
        });
        for reference in affected {
            self.deliver_invitations(&reference);
        }
    }
}

fn is_player_reference(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("player-ref-") else { return false };
    suffix.len() == 32
        && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn identity_for(
    member: &BrokerMember,
    recipient_host: &str,
    worlds: &impl GameSocialWorlds,
) -> GameChatSender {
    GameChatSender {
        display_name: worlds.profile(&member.participant).map_or_else(
            || member.participant.local_player_id.clone(),
            |profile| profile.display_name,
        ),
        player_id: if member.participant.host_id == recipient_host {
            member.participant.local_player_id.clone()
        } else {
            member.connection.player_reference.clone()
        },
        player_reference: member.connection.player_reference.clone(),
    }
}
