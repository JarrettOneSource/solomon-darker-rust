//! Cross-College broker delivery and source local party/chat commands.

use solomon_protocol::hosted::{HostedServerMessage, PartyAction};
use solomon_protocol::{HostErrorCode, ServerMessage};
use solomon_sim::social::broker::{
    CollegeInvitationRequest, GameSocialDelivery, GameSocialParticipant, GameSocialWorlds,
};
use solomon_sim::social::{
    GameChatChannel, GameChatMessage, GameChatRejection, GameChatRejectionReason,
    GameChatWorldKind, PartyActionRejection, SocialAction, normalize_game_chat_text,
};

use super::{CollegeRecord, HostedSupervisor, unix_ms};
use crate::host::authority::{close_peer, encode, send_control, send_reliable};
use crate::host::connection::HostConnection;
use crate::host::hosted::{HostedGameHost, HostedWorlds};
use crate::host::signing::random_party_identity;

type DispatchError = (HostErrorCode, String);

struct ChatRequest<'a> {
    channel: GameChatChannel,
    text: &'a str,
    recipient_reference: Option<&'a str>,
}

pub(super) struct BrokerWorlds<'a> {
    pub others: &'a [CollegeRecord],
    pub current: Option<&'a HostedGameHost>,
}
impl GameSocialWorlds for BrokerWorlds<'_> {
    fn profile(
        &self,
        participant: &GameSocialParticipant,
    ) -> Option<solomon_sim::social::GamePlayerCardProfile> {
        let host = self.current.filter(|host| host.id == participant.host_id).or_else(|| {
            self.others
                .iter()
                .find(|college| college.host.id == participant.host_id)
                .map(|college| &college.host)
        })?;
        host.player_card(&participant.local_player_id)
    }
    fn can_receive_college_invitation(&self, participant: &GameSocialParticipant) -> bool {
        let host = self.current.filter(|host| host.id == participant.host_id).or_else(|| {
            self.others
                .iter()
                .find(|college| college.host.id == participant.host_id)
                .map(|college| &college.host)
        });
        host.is_some_and(|host| {
            host.chat_world_kind(&participant.local_player_id) == GameChatWorldKind::Hub
        })
    }
}

impl HostedSupervisor {
    pub(super) fn invite_college(
        &mut self,
        host: &HostedGameHost,
        id: &str,
        target: &str,
    ) -> Result<(), DispatchError> {
        let player = host.player(id).ok_or_else(|| {
            server_error("The College invitation sender has no admitted identity".into())
        })?;
        let Some(social) = &player.social_connection else { return Ok(()) };
        let party = host
            .worlds
            .parties()
            .party_for_player(id)
            .ok_or_else(|| server_error("The College invitation sender has no party".into()))?;
        let code = party
            .join_code
            .as_deref()
            .ok_or_else(|| server_error("The College invitation sender has no Party ID".into()))?;
        let invitation_id = format!(
            "college-invitation-{}",
            crate::host::signing::random_token::<18>().map_err(server_error)?
        );
        let result = self.broker.invite_to_college(
            social,
            CollegeInvitationRequest {
                target_reference: target,
                source_party_id: &party.id,
                join_code: code,
                now_unix_ms: unix_ms().map_err(server_error)?,
            },
            &mut || invitation_id.clone(),
            &BrokerWorlds { others: &self.colleges, current: Some(host) },
        );
        // The host exposes the source rejection rather than silently ignoring a stale reference.
        if let Err(reason) = result {
            return Err((HostErrorCode::CommandRejected, reason.text().into()));
        }
        Ok(())
    }

    pub(super) fn deliver_broker(&mut self) -> Result<(), String> {
        for delivery in self.broker.take_deliveries() {
            let target = match &delivery {
                GameSocialDelivery::Chat { connection, .. }
                | GameSocialDelivery::CollegeInvitations { connection, .. } => connection,
            };
            let Some(college) = self.colleges.iter_mut().find(|college| {
                college
                    .host
                    .players
                    .iter()
                    .any(|player| player.social_connection.as_ref() == Some(target))
            }) else {
                continue;
            };
            let player = college
                .host
                .players
                .iter()
                .find(|player| player.social_connection.as_ref() == Some(target))
                .unwrap();
            let Some(connection) = college.host.connections.get(&player.peer_id) else { continue };
            match delivery {
                GameSocialDelivery::Chat { message, .. } => {
                    let sequence = college.host.next_chat_sequence;
                    college.host.next_chat_sequence += 1;
                    let message = GameChatMessage {
                        activity: message.activity,
                        channel: message.channel,
                        recipient: message.recipient,
                        sender: message.sender,
                        sequence,
                        text: message.text,
                    };
                    send_control(&connection.peer, ServerMessage::Chat { message })?;
                }
                GameSocialDelivery::CollegeInvitations { invitations, .. } => {
                    send_control(
                        &connection.peer,
                        ServerMessage::Hosted {
                            message: HostedServerMessage::CollegeInvitations { invitations },
                        },
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn social_command(
        &mut self,
        host: &mut HostedGameHost,
        connection: &mut HostConnection,
        id: &str,
        sequence: u64,
        action: SocialAction,
        now_ms: f64,
    ) -> Result<(), DispatchError> {
        if sequence == 0 {
            return Err((HostErrorCode::InvalidMessage, "Command sequences start at one".into()));
        }
        if sequence <= connection.last_command_sequence {
            if sequence == connection.last_command_sequence
                && let Some(receipt) = &connection.last_command_receipt
            {
                send_reliable(&connection.peer, receipt.clone()).map_err(server_error)?;
            }
            return Ok(());
        }
        let (accepted, reason) = match action {
            SocialAction::SendChat { channel, text, recipient_player_reference } => self
                .send_chat(
                    host,
                    connection,
                    id,
                    ChatRequest {
                        channel,
                        text: &text,
                        recipient_reference: recipient_player_reference.as_deref(),
                    },
                    now_ms,
                )?,
            SocialAction::RequestPlayerCard { player_id } => {
                let profile = host.player_card(&player_id);
                let accepted = profile.is_some();
                send_control(
                    &connection.peer,
                    ServerMessage::Hosted {
                        message: HostedServerMessage::PlayerCard { profile, request_id: sequence },
                    },
                )
                .map_err(server_error)?;
                (accepted, None)
            }
            SocialAction::RequestPlayerCardByReference { player_reference } => {
                let profile = host
                    .players
                    .iter()
                    .find(|player| {
                        player.id == player_reference || player.player_reference == player_reference
                    })
                    .and_then(|player| host.player_card(&player.id))
                    .or_else(|| {
                        self.broker.resolve_player_card(
                            host.player(id)?.social_connection.as_ref()?,
                            &player_reference,
                            &BrokerWorlds { others: &self.colleges, current: Some(host) },
                        )
                    });
                let accepted = profile.is_some();
                send_control(
                    &connection.peer,
                    ServerMessage::Hosted {
                        message: HostedServerMessage::PlayerCard { profile, request_id: sequence },
                    },
                )
                .map_err(server_error)?;
                (accepted, None)
            }
            action => {
                let (kind, result) = self.party_action(host, connection, id, action, now_ms)?;
                let reason = result.err();
                send_control(
                    &connection.peer,
                    ServerMessage::Hosted {
                        message: HostedServerMessage::PartyAction {
                            action: kind,
                            accepted: reason.is_none(),
                            reason,
                        },
                    },
                )
                .map_err(server_error)?;
                (reason.is_none(), reason.map(|reason| reason.text().into()))
            }
        };
        let receipt = encode(&ServerMessage::<()>::CommandResult {
            sequence,
            snapshot_sequence: host.next_snapshot_sequence,
            accepted,
            reason,
            inventory_result: None,
        })
        .map_err(server_error)?;
        connection.last_command_sequence = sequence;
        connection.last_command_receipt = Some(receipt.clone());
        send_reliable(&connection.peer, receipt).map_err(server_error)?;
        if accepted {
            host.social_dirty = true;
            host.publish_dirty = true;
        }
        Ok(())
    }

    fn party_action(
        &mut self,
        host: &mut HostedGameHost,
        connection: &mut HostConnection,
        id: &str,
        action: SocialAction,
        now_ms: f64,
    ) -> Result<(PartyAction, Result<(), PartyActionRejection>), DispatchError> {
        let maximum = host.maximum_players;
        let activity = host
            .runtime_events
            .as_ref()
            .map(|_| (action.clone(), host.capture_party_activity(id, &action)));
        let result = match action {
            SocialAction::Invite { player_id } => {
                let result = match &mut host.worlds {
                    HostedWorlds::Global(worlds) => {
                        worlds.invite(id, &player_id, maximum).map_err(shared_rejection)
                    }
                    HostedWorlds::Private { .. } => Err(PartyActionRejection::SameParty),
                };
                if result.is_ok() {
                    host.schedule_bot_invitation(id, &player_id, now_ms);
                }
                (PartyAction::Invite, result)
            }
            SocialAction::AcceptInvitation { invitation_id } => {
                let result = match &mut host.worlds {
                    HostedWorlds::Global(worlds) => worlds
                        .accept_invitation(id, &invitation_id, maximum)
                        .map_err(shared_rejection),
                    HostedWorlds::Private { .. } => Err(PartyActionRejection::InvitationMissing),
                };
                (PartyAction::AcceptInvitation, result)
            }
            SocialAction::DenyInvitation { invitation_id } => {
                let result = match &mut host.worlds {
                    HostedWorlds::Global(worlds) => {
                        worlds.deny_invitation(id, &invitation_id).map_err(shared_rejection)
                    }
                    HostedWorlds::Private { .. } => Err(PartyActionRejection::InvitationMissing),
                };
                (PartyAction::DenyInvitation, result)
            }
            SocialAction::SetPartyVisibility { visibility } => {
                (PartyAction::Settings, host.worlds.parties_mut().set_visibility(id, visibility))
            }
            SocialAction::RotatePartyCode => {
                let code = random_party_identity().map_err(server_error)?.join_code;
                let party_id = host.party_id_for_player(id).map(str::to_owned);
                let result = host.worlds.parties_mut().rotate_join_code(id, code);
                if result.is_ok()
                    && let Some(social) =
                        host.player(id).and_then(|player| player.social_connection.as_ref())
                    && let Some(party_id) = party_id
                {
                    self.broker.revoke_college_invitations(social, &party_id);
                }
                (PartyAction::RotateCode, result)
            }
            SocialAction::DecideJoinRequest { request_id, accept } => (
                if accept { PartyAction::RequestAccept } else { PartyAction::RequestDeny },
                host.decide_party_join_request(id, &request_id, accept),
            ),
            SocialAction::LeaveParty => {
                let result = match &mut host.worlds {
                    HostedWorlds::Global(worlds) => worlds
                        .leave_party(id, random_party_identity().map_err(server_error)?)
                        .map_err(shared_rejection),
                    HostedWorlds::Private { .. } => {
                        connection.planned_disconnect = Some((
                            "server-invalid-message",
                            "You left the private College.".into(),
                        ));
                        close_peer(&connection.peer, 1008, "You left the private College.")
                            .map_err(server_error)?;
                        Ok(())
                    }
                };
                (PartyAction::Leave, result)
            }
            SocialAction::Kick { player_id } => {
                let result = match &mut host.worlds {
                    HostedWorlds::Global(worlds) => worlds
                        .kick_party_player(
                            id,
                            &player_id,
                            random_party_identity().map_err(server_error)?,
                        )
                        .map_err(shared_rejection),
                    HostedWorlds::Private { .. } => {
                        let party = host.worlds.parties().party_for_player(id);
                        if party.is_none_or(|party| party.leader_player_id != id) {
                            Err(PartyActionRejection::NotLeader)
                        } else if player_id == id
                            || host.player(&player_id).is_none()
                            || !party.unwrap().member_player_ids.contains(&player_id)
                        {
                            Err(PartyActionRejection::PlayerMissing)
                        } else {
                            let peer_id = host.player(&player_id).unwrap().peer_id;
                            let target = host.connections.get_mut(&peer_id).unwrap();
                            target.planned_disconnect = Some((
                                "server-invalid-message",
                                "The College leader removed you.".into(),
                            ));
                            send_control(
                                &target.peer,
                                ServerMessage::Error {
                                    code: HostErrorCode::CommandRejected,
                                    reason: "The College leader removed you.".into(),
                                },
                            )
                            .map_err(server_error)?;
                            close_peer(&target.peer, 1008, "The College leader removed you.")
                                .map_err(server_error)?;
                            Ok(())
                        }
                    }
                };
                (PartyAction::Kick, result)
            }
            SocialAction::SendChat { .. }
            | SocialAction::RequestPlayerCard { .. }
            | SocialAction::RequestPlayerCardByReference { .. } => {
                unreachable!("non-party social actions are handled before membership mutation")
            }
        };
        if result.1.is_ok()
            && let Some((action, before)) = activity
        {
            host.emit_party_action(id, &action, before);
        }
        Ok(result)
    }

    fn send_chat(
        &mut self,
        host: &mut HostedGameHost,
        connection: &mut HostConnection,
        id: &str,
        request: ChatRequest<'_>,
        now_ms: f64,
    ) -> Result<(bool, Option<String>), DispatchError> {
        let ChatRequest { channel, text, recipient_reference } = request;
        let text = match normalize_game_chat_text(text) {
            Ok(text) => text,
            Err(error) => return Ok((false, Some(error.text().into()))),
        };
        let local_target = recipient_reference
            .and_then(|reference| {
                host.players
                    .iter()
                    .find(|player| player.id == reference || player.player_reference == reference)
            })
            .map(|player| player.id.clone());
        let social = host.player(id).and_then(|player| player.social_connection.clone());
        let broker_worlds = BrokerWorlds { others: &self.colleges, current: Some(host) };
        let broker_target = if channel == GameChatChannel::Whisper && local_target.is_none() {
            social.as_ref().zip(recipient_reference).and_then(|(social, reference)| {
                self.broker.resolve_player_card(social, reference, &broker_worlds)
            })
        } else {
            None
        };
        let available = match channel {
            GameChatChannel::Global => {
                host.player(id).is_some_and(|player| player.preferences.global_chat)
                    && social.is_some()
            }
            GameChatChannel::Whisper => {
                local_target.as_ref().is_some_and(|target| target != id) || broker_target.is_some()
            }
            GameChatChannel::Party => host.chat_world_kind(id) == GameChatWorldKind::Hub,
            GameChatChannel::Boneyard => host.chat_world_kind(id) == GameChatWorldKind::Boneyard,
        };
        if !available {
            return rejected_chat(
                connection,
                channel,
                if channel == GameChatChannel::Whisper {
                    GameChatRejectionReason::TargetUnavailable
                } else {
                    GameChatRejectionReason::ChannelUnavailable
                },
                0,
            );
        }
        let retry_after = connection.chat_limit.record(now_ms as u64);
        if retry_after != 0 {
            return rejected_chat(
                connection,
                channel,
                GameChatRejectionReason::RateLimited,
                retry_after,
            );
        }
        if channel == GameChatChannel::Global {
            self.broker.publish_global(social.as_ref().unwrap(), text, &broker_worlds);
            return Ok((true, None));
        }
        if let Some(target) = broker_target {
            self.broker.publish_whisper(
                social.as_ref().unwrap(),
                &target.player_reference,
                text,
                &broker_worlds,
            );
            return Ok((true, None));
        }
        let party = host.party_id_for_player(id).map(str::to_owned);
        let state = host
            .worlds
            .state_for_player(id)
            .ok_or_else(|| server_error("The chat sender has no world".into()))?;
        let recipients: Vec<_> = host
            .players
            .iter()
            .filter(|player| match channel {
                GameChatChannel::Whisper => {
                    player.id == id || local_target.as_ref() == Some(&player.id)
                }
                GameChatChannel::Party => {
                    host.party_id_for_player(&player.id) == party.as_deref()
                        && host.chat_world_kind(&player.id) == GameChatWorldKind::Hub
                }
                GameChatChannel::Boneyard => state.participant(&player.id).is_some(),
                GameChatChannel::Global => false,
            })
            .map(|player| player.peer_id)
            .collect();
        let recipient = local_target
            .as_ref()
            .map(|target| host.chat_sender(target))
            .transpose()
            .map_err(server_error)?;
        let message = GameChatMessage {
            activity: None,
            channel,
            recipient,
            sender: host.chat_sender(id).map_err(server_error)?,
            sequence: host.next_chat_sequence,
            text: text.into(),
        };
        host.next_chat_sequence += 1;
        let payload = encode(&ServerMessage::<()>::Chat { message }).map_err(server_error)?;
        for peer_id in recipients {
            if peer_id == connection.peer.id() {
                send_reliable(&connection.peer, payload.clone()).map_err(server_error)?;
            } else if let Some(other) = host.connections.get(&peer_id) {
                send_reliable(&other.peer, payload.clone()).map_err(server_error)?;
            }
        }
        Ok((true, None))
    }
}

fn shared_rejection(
    error: solomon_sim::shared_worlds::SharedWorldActionRejection,
) -> PartyActionRejection {
    match error {
        solomon_sim::shared_worlds::SharedWorldActionRejection::Party(reason) => reason,
        _ => PartyActionRejection::NotInHub,
    }
}
fn rejected_chat(
    connection: &HostConnection,
    channel: GameChatChannel,
    reason: GameChatRejectionReason,
    retry_after_ms: u64,
) -> Result<(bool, Option<String>), DispatchError> {
    let rejection = GameChatRejection { channel, reason, retry_after_ms };
    let text = rejection.text();
    send_control(&connection.peer, ServerMessage::ChatRejected { rejection })
        .map_err(server_error)?;
    Ok((false, Some(text)))
}
fn server_error(reason: String) -> DispatchError {
    (HostErrorCode::ServerError, reason)
}
