//! Direct-session projection and routing. Pure party/chat rules live in `sim::social`;
//! this owner supplies admitted membership, actual profiles, recipient sockets and time.
//! Reference: `host/game-host.ts::chatRecipients`, `localClientByReference` and chat branch.

use std::collections::BTreeSet;

use solomon_protocol::{HostErrorCode, ServerMessage};
use solomon_sim::session::{GameSession, ParticipantProfile};
use solomon_sim::social::{
    GameChatChannel, GameChatMessage, GameChatRejection, GameChatRejectionReason, GameChatSender,
    GameChatWorldKind, GamePlayerCardProfile, GameSessionKind, PartyPlayerProfile,
    PartyRosterPlayer, SocialAction, SocialSnapshot, append_game_chat_message,
    normalize_game_chat_text, party_after_member_departure, party_kick_rejection,
    project_direct_party_state,
};

use super::authority::{NativeAuthority, close_peer, encode, send_control, send_reliable};
use super::connection::HostConnection;
use super::persistence::SavedParticipant;

type SocialResult = Result<(bool, Option<String>), (HostErrorCode, String)>;

impl NativeAuthority {
    pub(super) fn publish_social(&self) -> Result<(), String> {
        for connection in self.connections.values() {
            let Some(id) = &connection.player_id else { continue };
            if connection.welcome_revision.is_none() {
                send_control(
                    &connection.peer,
                    ServerMessage::Social { snapshot: self.social_snapshot(id)? },
                )?;
            }
        }
        Ok(())
    }

    fn social_snapshot(&self, id: &str) -> Result<SocialSnapshot, String> {
        let mut profiles = Vec::new();
        let mut roster = Vec::new();
        let mut player_cards = Vec::new();
        if let Some(party) = &self.metadata.party {
            for member_id in &party.member_player_ids {
                let participant = self.retained_participant(member_id).ok_or_else(|| {
                    format!("The native roster has no retained wizard {member_id}")
                })?;
                let player = &participant.player;
                let connected = self.metadata.resume_slots[member_id].peer_id.is_some();
                profiles.push(PartyPlayerProfile {
                    account_username: None,
                    highest_wave: None,
                    total_playtime_ms: None,
                    display_name: player.selection.name.clone(),
                    player_id: member_id.clone(),
                });
                roster.push(PartyRosterPlayer {
                    connected,
                    current_health: player.combat.current_health,
                    display_name: player.selection.name.clone(),
                    element: player.selection.element,
                    life_state: player.combat.life_state,
                    maximum_health: player.combat.maximum_health,
                    player_id: member_id.clone(),
                });
                if connected {
                    player_cards.push(self.player_card(member_id)?);
                }
            }
        }
        let party_state = self
            .metadata
            .party
            .as_ref()
            .map(|party| {
                project_direct_party_state(party, &profiles, &roster, self.metadata.revision)
                    .map_err(|missing| {
                        format!("The native party has no durable roster for {}", missing.0)
                    })
            })
            .transpose()?;
        let chat_messages = self
            .metadata
            .chat_messages
            .iter()
            .filter(|message| {
                message.channel != GameChatChannel::Whisper
                    || message.sender.player_id == id
                    || message.recipient.as_ref().is_some_and(|recipient| recipient.player_id == id)
            })
            .cloned()
            .collect();
        Ok(SocialSnapshot {
            local_player_id: id.into(),
            session_kind: GameSessionKind::Standalone,
            world_kind: self.chat_world_kind(),
            directory_available: false,
            global_chat_available: false,
            party_state,
            chat_messages,
            player_cards,
        })
    }

    fn player_card(&self, id: &str) -> Result<GamePlayerCardProfile, String> {
        let participant = self.retained_participant(id).ok_or("The requested wizard is absent")?;
        let slot =
            self.metadata.resume_slots.get(id).ok_or("The requested wizard has no identity")?;
        let selection = &participant.player.selection;
        Ok(GamePlayerCardProfile {
            player_id: id.into(),
            account_username: None,
            activity: self.chat_world_kind(),
            discipline: selection.discipline,
            display_name: selection.name.clone(),
            element: selection.element,
            gold: participant.player.economy.gold,
            highest_wave: None,
            player_reference: slot.player_reference.clone(),
            session_kind: GameSessionKind::Standalone,
            total_playtime_ms: None,
        })
    }

    pub(super) fn apply_social_action(
        &mut self,
        connection: &mut HostConnection,
        id: &str,
        action: SocialAction,
        now_ms: f64,
    ) -> SocialResult {
        match action {
            SocialAction::SendChat { channel, text, recipient_player_reference } => self.send_chat(
                connection,
                id,
                channel,
                &text,
                recipient_player_reference.as_deref(),
                now_ms,
            ),
            SocialAction::RequestPlayerCard { player_id } => {
                self.send_player_card(connection, &player_id)
            }
            SocialAction::RequestPlayerCardByReference { player_reference } => {
                let target = self.connected_id_by_reference(&player_reference, id);
                match target {
                    Some(target) => self.send_player_card(connection, &target),
                    None => Ok((false, Some("That wizard is no longer connected".into()))),
                }
            }
            SocialAction::Kick { player_id } => {
                if let Some(rejection) =
                    party_kick_rejection(self.metadata.party.as_ref(), id, &player_id)
                {
                    return Ok((false, Some(rejection.text().into())));
                }
                self.depart_party_member(
                    &player_id,
                    "Removed from the native party; Continue is available in the College",
                    now_ms,
                )
                .map_err(server_error)?;
                Ok((true, None))
            }
            SocialAction::LeaveParty => {
                self.depart_party_member(
                    id,
                    "Left the native party; Continue is available in the College",
                    now_ms,
                )
                .map_err(server_error)?;
                // This connection is temporarily borrowed out of the map during dispatch.
                connection.player_id = None;
                connection.input.neutralize();
                close_peer(&connection.peer, 1000, "left native party").map_err(server_error)?;
                Ok((true, None))
            }
            SocialAction::Invite { .. }
            | SocialAction::AcceptInvitation { .. }
            | SocialAction::DenyInvitation { .. }
            | SocialAction::DecideJoinRequest { .. }
            | SocialAction::SetPartyVisibility { .. }
            | SocialAction::RotatePartyCode => {
                Ok((false, Some("This native session has no party directory".into())))
            }
        }
    }

    fn send_player_card(&self, connection: &HostConnection, target: &str) -> SocialResult {
        if self.metadata.resume_slots.get(target).is_none_or(|slot| slot.peer_id.is_none()) {
            return Ok((false, Some("That wizard is no longer connected".into())));
        }
        let id = connection.player_id.as_deref().expect("an admitted connection requests a card");
        let mut snapshot = self.social_snapshot(id).map_err(server_error)?;
        snapshot.player_cards = vec![self.player_card(target).map_err(server_error)?];
        send_control(&connection.peer, ServerMessage::Social { snapshot }).map_err(server_error)?;
        Ok((true, None))
    }

    fn send_chat(
        &mut self,
        connection: &mut HostConnection,
        id: &str,
        channel: GameChatChannel,
        text: &str,
        recipient_reference: Option<&str>,
        now_ms: f64,
    ) -> SocialResult {
        let text = match normalize_game_chat_text(text) {
            Ok(text) => text,
            Err(error) => return Ok((false, Some(error.text().into()))),
        };
        let mut recipients = BTreeSet::new();
        let recipient = if channel == GameChatChannel::Whisper {
            let target = recipient_reference
                .and_then(|reference| self.connected_id_by_reference(reference, id));
            let Some(target) = target.filter(|target| target != id) else {
                return reject_chat(
                    connection,
                    channel,
                    GameChatRejectionReason::TargetUnavailable,
                    0,
                );
            };
            recipients.insert(target.clone());
            recipients.insert(id.into());
            Some(self.chat_sender(&target).map_err(server_error)?)
        } else {
            let available = match channel {
                GameChatChannel::Boneyard => self.chat_world_kind() == GameChatWorldKind::Boneyard,
                GameChatChannel::Party => self.chat_world_kind() == GameChatWorldKind::Hub,
                GameChatChannel::Global | GameChatChannel::Whisper => false,
            };
            if !available {
                return reject_chat(
                    connection,
                    channel,
                    GameChatRejectionReason::ChannelUnavailable,
                    0,
                );
            }
            recipients = self.connected_ids();
            None
        };
        // The source counts only an admitted route. An unavailable recipient or channel
        // cannot consume a player's five accepted sends in the 5,000 ms window.
        let retry_after = connection.chat_limit.record(now_ms as u64);
        if retry_after != 0 {
            return reject_chat(
                connection,
                channel,
                GameChatRejectionReason::RateLimited,
                retry_after,
            );
        }
        let message = GameChatMessage {
            activity: None,
            channel,
            recipient,
            sender: self.chat_sender(id).map_err(server_error)?,
            sequence: self.metadata.next_chat_sequence,
            text: text.into(),
        };
        self.metadata.next_chat_sequence += 1;
        append_game_chat_message(&mut self.metadata.chat_messages, message.clone());
        let payload = encode(&ServerMessage::<()>::Chat { message }).map_err(server_error)?;
        if recipients.contains(id) {
            send_reliable(&connection.peer, payload.clone()).map_err(server_error)?;
        }
        for other in self.connections.values() {
            if other.welcome_revision.is_none()
                && other.player_id.as_ref().is_some_and(|id| recipients.contains(id))
            {
                send_reliable(&other.peer, payload.clone()).map_err(server_error)?;
            }
        }
        self.changed();
        self.require_checkpoint();
        Ok((true, None))
    }

    fn connected_id_by_reference(&self, reference: &str, current_id: &str) -> Option<String> {
        for (id, slot) in &self.metadata.resume_slots {
            if slot.peer_id.is_none() || (id != reference && slot.player_reference != reference) {
                continue;
            }
            let ready = id == current_id
                || slot
                    .peer_id
                    .and_then(|peer| self.connections.get(&peer))
                    .is_some_and(|connection| connection.welcome_revision.is_none());
            if ready
                && self.session.as_ref().is_some_and(|session| session.participant(id).is_some())
            {
                return Some(id.clone());
            }
        }
        None
    }

    fn chat_sender(&self, id: &str) -> Result<GameChatSender, String> {
        let player = self.retained_participant(id).ok_or("The chat sender is absent")?;
        Ok(GameChatSender {
            display_name: player.player.selection.name.clone(),
            player_id: id.into(),
            player_reference: self.metadata.resume_slots[id].player_reference.clone(),
        })
    }

    fn chat_world_kind(&self) -> GameChatWorldKind {
        if self.session.as_ref().is_some_and(|session| session.boneyard.is_some()) {
            GameChatWorldKind::Boneyard
        } else {
            GameChatWorldKind::Hub
        }
    }

    fn depart_party_member(&mut self, id: &str, reason: &str, now_ms: f64) -> Result<(), String> {
        let participant =
            self.retained_participant(id).ok_or("The departing wizard is absent")?.snapshot();
        let session = self.session.as_mut().expect("the native party has a shared session");
        let profile = if let Some(profile) = session.retired_participant_profile(id) {
            profile
        } else {
            match self.metadata.resume_slots[id].saved.as_ref() {
                Some(SavedParticipant::Run { detached }) => {
                    GameSession::retired_detached_profile(detached)
                }
                Some(SavedParticipant::Retired { economy, hagatha_runtime, .. }) => {
                    ParticipantProfile {
                        economy: (**economy).clone(),
                        hagatha_runtime: *hagatha_runtime,
                        advanced_unlocks: participant.player.book.advanced_unlocks,
                    }
                }
                Some(SavedParticipant::Hub { participant }) => ParticipantProfile {
                    economy: participant.player.economy.clone(),
                    hagatha_runtime: participant.player.progression.hagatha_runtime,
                    advanced_unlocks: participant.player.book.advanced_unlocks,
                },
                None => return Err("The departing wizard has no durable profile".into()),
            }
        };
        session.remove_participant(id);
        let slot =
            self.metadata.resume_slots.get_mut(id).expect("a party member owns its resume slot");
        let previous_peer = slot.peer_id.take();
        slot.saved = Some(SavedParticipant::Retired {
            participant: Box::new(participant),
            economy: Box::new(profile.economy),
            hagatha_runtime: profile.hagatha_runtime,
        });
        self.metadata.party =
            self.metadata.party.as_ref().and_then(|party| party_after_member_departure(party, id));
        if let Some(peer_id) = previous_peer
            && let Some(connection) = self.connections.get_mut(&peer_id)
        {
            connection.player_id = None;
            connection.input.neutralize();
            send_control(
                &connection.peer,
                ServerMessage::Error {
                    code: HostErrorCode::CommandRejected,
                    reason: reason.into(),
                },
            )?;
            close_peer(&connection.peer, 1000, "native party membership ended")?;
        }
        let facts = self.control_facts();
        self.control.disconnected(id, &facts.context(), now_ms);
        self.retire_empty_world()?;
        self.changed();
        self.require_checkpoint();
        self.publish_dirty = true;
        self.social_dirty = true;
        Ok(())
    }
}

fn reject_chat(
    connection: &HostConnection,
    channel: GameChatChannel,
    reason: GameChatRejectionReason,
    retry_after_ms: u64,
) -> SocialResult {
    let rejection = GameChatRejection { channel, reason, retry_after_ms };
    send_control(&connection.peer, ServerMessage::ChatRejected { rejection })
        .map_err(server_error)?;
    Ok((false, Some(rejection.text())))
}

fn server_error(reason: String) -> (HostErrorCode, String) {
    (HostErrorCode::ServerError, reason)
}
