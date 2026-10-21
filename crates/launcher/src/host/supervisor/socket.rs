//! One-use ticket admission and socket generation routing. Removing the currently
//! dispatched College from the Vec permits borrowed broker queries without aliasing
//! mutable worlds; it is reinserted at the same source creation-order position.

use solomon_protocol::hosted::{HostedCommand, HostedServerMessage};
use solomon_protocol::{ClientMessage, GameCommand, HostErrorCode, ServerMessage};
use solomon_sim::social::broker::GameSocialParticipant;

use super::{HostedSupervisor, social::BrokerWorlds};
use crate::host::authority::{close_peer, decode_client_message, send_control};
use crate::host::connection::HostConnection;
use crate::host::hosted::{HostedGameHost, HostedModActionRequest, PlayerSaveRequest};
use crate::transport::PeerId;

type DispatchError = (HostErrorCode, String);

impl HostedSupervisor {
    pub(super) fn message(
        &mut self,
        peer_id: PeerId,
        payload: &str,
        now_ms: f64,
    ) -> Result<(), String> {
        let Some(index) = self
            .colleges
            .iter()
            .position(|college| college.host.connections.contains_key(&peer_id))
        else {
            return Ok(());
        };
        let mut college = self.colleges.remove(index);
        let mut connection = college
            .host
            .connections
            .remove(&peer_id)
            .expect("the routed peer belongs to this College");
        let message = decode_client_message(payload)
            .map_err(|reason| (HostErrorCode::InvalidMessage, reason));
        let result = (|| {
            if let Ok(message) = &message
                && connection.handle_transfer_message(message).map_err(|reason| invalid(&reason))?
            {
                return Ok(());
            }
            match message {
                Ok(ClientMessage::Hosted {
                    command: HostedCommand::ObserverHello { version, credential },
                }) => {
                    if connection.player_id.is_some() {
                        Err(invalid("This connection already completed admission."))
                    } else {
                        let ticket = college
                            .tickets
                            .remove(&credential)
                            .filter(|ticket| ticket.expires_at_ms > now_ms);
                        match ticket {
                            Some(ticket) => college.host.admit_observer(
                                &mut connection,
                                version,
                                ticket.admission,
                                now_ms,
                            ),
                            None => Err((
                                HostErrorCode::InvalidCredentials,
                                "The observer credential is invalid.".into(),
                            )),
                        }
                    }
                }
                Ok(ClientMessage::Hosted { command: HostedCommand::Hello { hello } }) => {
                    if connection.player_id.is_some() {
                        Err(invalid("This connection already admitted a wizard."))
                    } else {
                        // Consume even an expired ticket. Replaying a failed/old credential
                        // can never become valid after another admission or clock change.
                        let ticket = if self.standalone_credential.is_some() {
                            self.standalone_ticket(&college.host, &hello.credential, now_ms)
                        } else {
                            college
                                .tickets
                                .remove(&hello.credential)
                                .filter(|ticket| ticket.expires_at_ms > now_ms)
                        };
                        match ticket {
                            None => Err((
                                HostErrorCode::InvalidCredentials,
                                "The game admission has expired or is invalid.".into(),
                            )),
                            Some(ticket) => {
                                let activity_selection = hello.selection.clone();
                                let rejoined_party = ticket.admission.party_rejoin_token.is_some()
                                    && !ticket.admission.party_recovery_seed;
                                let replaced_connection =
                                    hello.resume_token.as_ref().is_some_and(|token| {
                                        college
                                            .host
                                            .players
                                            .iter()
                                            .any(|player| &player.resume_token == token)
                                    });
                                let id = college.host.admit_ordinary(
                                    &mut connection,
                                    hello,
                                    ticket.admission,
                                    now_ms,
                                );
                                match id {
                                    Err(error) => Err(error),
                                    Ok(id) => {
                                        college.claimed = true;
                                        let player = college
                                            .host
                                            .player(&id)
                                            .expect("the actual admitted actor has metadata");
                                        let social = if college.host.editor_test
                                            || self.standalone_credential.is_some()
                                        {
                                            None
                                        } else {
                                            Some(
                                                self.broker
                                                    .register(
                                                        GameSocialParticipant {
                                                            host_id: college.host.id.clone(),
                                                            local_player_id: id.clone(),
                                                        },
                                                        player.preferences,
                                                        player.player_reference.clone(),
                                                    )
                                                    .map_err(|reason| {
                                                        server_error(reason.into())
                                                    })?,
                                            )
                                        };
                                        college.host.player_mut(&id).unwrap().social_connection =
                                            social.clone();
                                        connection.connected_at_utc_ms =
                                            Some(crate::host::source_wall_clock_ms());
                                        college.host.emit_player_connected(
                                            &id,
                                            &activity_selection,
                                            rejoined_party,
                                            replaced_connection,
                                        );
                                        let catalog = college
                                            .host
                                            .mod_content
                                            .consumables()
                                            .cloned()
                                            .collect();
                                        college
                                            .host
                                            .send_welcome(&mut connection, &id, now_ms, catalog)
                                            .map_err(server_error)?;
                                        if let Some(social) = &social {
                                            self.broker.activate(social);
                                        }
                                        college.host.pending_player_saves.insert(id);
                                        Ok(())
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(message) => {
                    let id = connection.player_id.clone();
                    match id {
                        None => Err(invalid("Admit a game ticket before sending game commands.")),
                        Some(id) => self.dispatch_admitted(
                            &mut college.host,
                            &mut connection,
                            &id,
                            message,
                            now_ms,
                        ),
                    }
                }
                Err(error) => Err(error),
            }
        })();
        if let Err((code, reason)) = result {
            let source = match code {
                HostErrorCode::UnsupportedVersion => "server-protocol-mismatch",
                HostErrorCode::InvalidCredentials => "server-authentication-failed",
                HostErrorCode::HostFull => "server-server-full",
                _ => "server-invalid-message",
            };
            connection.planned_disconnect = Some((source, reason.clone()));
            send_control(&connection.peer, ServerMessage::Error { code, reason })?;
            close_peer(&connection.peer, 1008, "game message rejected")?;
        }
        college.host.commit_flame_histories()?;
        college.host.connections.insert(peer_id, connection);
        self.colleges.insert(index, college);
        Ok(())
    }

    fn dispatch_admitted(
        &mut self,
        host: &mut HostedGameHost,
        connection: &mut HostConnection,
        id: &str,
        message: ClientMessage,
        now_ms: f64,
    ) -> Result<(), DispatchError> {
        if host.observers.contains_key(&connection.peer.id()) {
            return match message {
                ClientMessage::SnapshotAck { sequence } => connection
                    .acknowledge_snapshot(sequence)
                    .map_err(|reason| (HostErrorCode::InvalidMessage, reason.into())),
                ClientMessage::Ping { nonce } => {
                    send_control(&connection.peer, ServerMessage::Pong { nonce })
                        .map_err(server_error)
                }
                ClientMessage::Disconnect => {
                    connection.planned_disconnect =
                        Some(("observer-request", "observer requested disconnect".into()));
                    close_peer(&connection.peer, 1000, "observer disconnect").map_err(server_error)
                }
                _ => Err(invalid("Observer connections are read-only.")),
            };
        }
        if let ClientMessage::SaveCheckpointChunkAck { sequence, next_offset } = message {
            return connection
                .peer
                .acknowledge_save_checkpoint(sequence, next_offset)
                .map_err(|reason| invalid(&reason));
        }
        if self.deployment.is_some()
            && !matches!(
                message,
                ClientMessage::Hosted { command: HostedCommand::DeploymentReady { .. } }
                    | ClientMessage::Ping { .. }
                    | ClientMessage::Disconnect
            )
        {
            return Ok(());
        }
        if host.staged_rejoin_index(id).is_some()
            && !matches!(
                &message,
                ClientMessage::SnapshotAck { .. }
                    | ClientMessage::ResumeReady { .. }
                    | ClientMessage::Ping { .. }
                    | ClientMessage::SaveBeforeLeave { .. }
                    | ClientMessage::Disconnect
                    | ClientMessage::Hosted { command: HostedCommand::OnlinePreferences { .. } }
                    | ClientMessage::Command {
                        command: GameCommand::ChooseSkill { .. }
                            | GameCommand::RerollSkillOffer { .. }
                            | GameCommand::DeferSkillChoice { .. },
                        ..
                    }
            )
        {
            return Ok(());
        }
        match message {
            ClientMessage::Hosted { command } => match command {
                HostedCommand::OnlinePreferences { preferences } => {
                    if let Some(player) = host.player_mut(id) {
                        player.preferences = preferences;
                        if let Some(social) = &player.social_connection {
                            self.broker.set_online_preferences(social, preferences);
                        }
                    }
                    host.social_dirty = true;
                    Ok(())
                }
                HostedCommand::RequestPlayerCard { request_id, player_reference } => {
                    let local = host
                        .players
                        .iter()
                        .find(|player| {
                            player.player_reference == player_reference
                                || player.id == player_reference
                        })
                        .and_then(|player| host.player_card(&player.id));
                    let profile = local.or_else(|| {
                        let social = host.player(id)?.social_connection.as_ref()?;
                        self.broker.resolve_player_card(
                            social,
                            &player_reference,
                            &BrokerWorlds { others: &self.colleges, current: Some(host) },
                        )
                    });
                    send_control(
                        &connection.peer,
                        ServerMessage::Hosted {
                            message: HostedServerMessage::PlayerCard { profile, request_id },
                        },
                    )
                    .map_err(server_error)
                }
                HostedCommand::InviteCollege { player_reference } => {
                    self.invite_college(host, id, &player_reference)
                }
                HostedCommand::DismissCollegeInvitation { invitation_id } => {
                    if let Some(social) =
                        host.player(id).and_then(|player| player.social_connection.as_ref())
                    {
                        self.broker.dismiss_college_invitation(social, &invitation_id);
                    }
                    Ok(())
                }
                HostedCommand::StartMatch { boneyard_id } => {
                    if host.start_match(id, &boneyard_id, false)? {
                        let party = host.party_id_for_player(id).unwrap().to_owned();
                        host.arm_party_rejoins(&party).map_err(server_error)?;
                        host.send_boneyard_mod_content(connection, id).map_err(server_error)?;
                        connection.input.neutralize();
                    }
                    Ok(())
                }
                HostedCommand::DeploymentReady { checkpoint_sequence, target_revision } => {
                    let Some(restart) = &mut self.deployment else {
                        return Err(invalid("The deployment acknowledgement is not current."));
                    };
                    if restart.target_revision != target_revision
                        || restart.checkpoints.get(&connection.peer.id())
                            != Some(&checkpoint_sequence)
                    {
                        return Err(invalid("The deployment acknowledgement is not current."));
                    }
                    // Source captures savedPlayers before it starts closing sockets.
                    // A late queued ACK cannot change the completed restart's count.
                    if !restart.closing && restart.pending.remove(&connection.peer.id()) {
                        restart.acknowledged.insert(connection.peer.id());
                    }
                    Ok(())
                }
                HostedCommand::SetCheats { enabled } => {
                    if host.kind == solomon_sim::social::GameSessionKind::Standalone {
                        return host.set_standalone_cheats(connection, id, enabled);
                    }
                    let developer = host.player(id).is_some_and(|player| player.developer_access);
                    if host.kind == solomon_sim::social::GameSessionKind::GlobalHub || developer {
                        if enabled {
                            if developer {
                                return Ok(());
                            }
                            return Err(invalid("Cheats require a private College."));
                        }
                        return send_control(
                            &connection.peer,
                            ServerMessage::Hosted {
                                message: HostedServerMessage::CheatMode { enabled: false },
                            },
                        )
                        .map_err(server_error);
                    }
                    let leader = host.players.first().is_some_and(|player| player.id == id);
                    if !leader {
                        return send_control(
                            &connection.peer,
                            ServerMessage::Hosted {
                                message: HostedServerMessage::CheatMode {
                                    enabled: host.cheats_enabled,
                                },
                            },
                        )
                        .map_err(server_error);
                    }
                    host.cheats_enabled = enabled;
                    if enabled {
                        for player in &mut host.players {
                            player.save_metadata.integrity =
                                solomon_assets::website_save::SaveIntegrity::LocalOnly;
                            player.global_score_eligible = false;
                            player.tutorial_eligible = false;
                        }
                    }
                    host.taint_ineligible_client_runs();
                    for player in &host.players {
                        if player.id == id {
                            send_control(
                                &connection.peer,
                                ServerMessage::Hosted {
                                    message: HostedServerMessage::CheatMode {
                                        enabled: host.cheats_enabled,
                                    },
                                },
                            )
                            .map_err(server_error)?;
                        } else if let Some(peer) = host.connections.get(&player.peer_id) {
                            send_control(
                                &peer.peer,
                                ServerMessage::Hosted {
                                    message: HostedServerMessage::CheatMode {
                                        enabled: host.cheats_enabled,
                                    },
                                },
                            )
                            .map_err(server_error)?;
                        }
                    }
                    host.social_dirty = true;
                    Ok(())
                }
                HostedCommand::Hello { .. } | HostedCommand::ObserverHello { .. } => {
                    Err(invalid("This connection has already completed admission."))
                }
                HostedCommand::ModCast { content_id, request_id, target_x, target_y } => host
                    .apply_mod_cast(connection, id, &content_id, request_id, target_x, target_y)
                    .map_err(server_error),
                HostedCommand::ModAction { action, arguments, request_id, target } => host
                    .apply_mod_action(
                        connection,
                        id,
                        HostedModActionRequest { action, arguments, request_id, target: &target },
                        now_ms,
                    )
                    .map_err(server_error),
                HostedCommand::SetEnhancedEffects { enabled } => {
                    host.set_enhanced_effects(id, enabled);
                    Ok(())
                }
                HostedCommand::LuaExecute { request_id, code } => {
                    host.enqueue_lua_execution(connection, id, request_id, code)
                }
            },
            ClientMessage::Command { sequence, command: GameCommand::Social { action } } => {
                self.social_command(host, connection, id, sequence, action, now_ms)
            }
            ClientMessage::SaveBeforeLeave { request_id } => {
                connection.input.neutralize();
                let sequence = host
                    .publish_player_save(
                        connection,
                        id,
                        PlayerSaveRequest {
                            target_revision: None,
                            force: true,
                            include_terminal_profile: true,
                        },
                    )
                    .map_err(server_error)?;
                send_control(
                    &connection.peer,
                    ServerMessage::Hosted {
                        message: HostedServerMessage::SaveBeforeLeave {
                            checkpoint_sequence: sequence,
                            request_id,
                        },
                    },
                )
                .map_err(server_error)
            }
            ClientMessage::Hello { .. } | ClientMessage::InspectSession { .. } => {
                Err(invalid("This address requires a hosted admission."))
            }
            message => host.dispatch_local_message(connection, id, message, now_ms),
        }
    }
}
fn invalid(reason: &str) -> DispatchError {
    (HostErrorCode::InvalidMessage, reason.into())
}
fn server_error(reason: String) -> DispatchError {
    (HostErrorCode::ServerError, reason)
}
