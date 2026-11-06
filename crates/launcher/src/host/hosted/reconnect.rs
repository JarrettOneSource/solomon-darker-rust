//! Transport recovery: for a bounded lease, a disconnected player's live wizard and world wait
//! for a reconnect. It never imports a checkpoint or grants new gameplay or account permissions.
use solomon_protocol::hosted::HostedHello;
use solomon_protocol::{ClientMessage, HostErrorCode, PROTOCOL_VERSION, ResumeGraceReason};
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::social::GameSessionKind;
use solomon_sim::social::broker::GameSocialConnection;

use super::{HostedAdmission, HostedGameHost, HostedReconnectAdmission};
use crate::host::authority::close_peer;
use crate::host::connection::HostConnection;
use crate::host::signing::credentials_equal;
use crate::transport::{PeerId, TransportClose};

const RECONNECT_LEASE_MS: f64 = solomon_protocol::hosted::GAME_RECONNECT_LEASE_MS as f64;

/// A live wizard's bounded wait for a replacement connection, and how far that replacement
/// has been admitted.
pub(super) struct ReconnectLease {
    connection: Option<HostConnection>,
    expires_at_ms: f64,
    request_id: Option<String>,
    attempts: u8,
    admitting: bool,
    welcomed: bool,
    renderer_ready: bool,
}

impl HostedGameHost {
    fn begin_reconnect(&mut self, id: &str, now_ms: f64) {
        if self.reconnects.contains_key(id) {
            return;
        }
        let state = self.worlds.state_for_player(id).expect("a retained actor has a world");
        let run_id = state.run.run_id.clone().filter(|_| state.boneyard.is_some());
        let active = run_id.is_some() && state.run.phase == GameRunPhase::Active;
        let party_id = self.party_id_for_player(id).map(str::to_owned);
        self.reconnects.insert(
            id.into(),
            ReconnectLease {
                connection: None,
                expires_at_ms: now_ms + RECONNECT_LEASE_MS,
                request_id: None,
                attempts: 0,
                admitting: false,
                welcomed: false,
                renderer_ready: run_id.is_none(),
            },
        );
        if let Some(party_id) = party_id {
            let facts = self.control_facts(&party_id);
            let control = self.controls.entry(party_id.clone()).or_default();
            if let Some(run_id) = run_id {
                control.require_renderer(id, &run_id);
            }
            if active {
                control.begin_loading_grace(
                    id,
                    ResumeGraceReason::GameRejoined,
                    &facts.connected,
                    &[],
                );
            }
            self.neutralize_party(&party_id);
        }
        self.publish_dirty = true;
        self.emit_runtime_event(
            "connection.awaiting_reconnect",
            "The live wizard is awaiting transport recovery.",
            serde_json::json!({"playerId":id,"leaseMs":RECONNECT_LEASE_MS}),
        );
    }

    pub(in crate::host) fn retain_transport_loss(
        &mut self,
        mut connection: HostConnection,
        now_ms: f64,
    ) {
        let id = connection.player_id.clone().expect("retained connection has an actor");
        connection.input.neutralize();
        self.begin_reconnect(&id, now_ms);
        let lease = self.reconnects.get_mut(&id).expect("a loss has a bounded lease");
        lease.connection = Some(connection);
        if lease.admitting {
            lease.request_id = None;
        }
        lease.admitting = false;
        lease.welcomed = false;
        lease.renderer_ready = self.worlds.loaded_for_player(&id).is_none();
    }

    pub(in crate::host) fn reserve_reconnect(
        &mut self,
        id: &str,
        token: &str,
        account: Option<u32>,
        request_id: &str,
        now_ms: f64,
    ) -> Result<Option<(HostedAdmission, f64)>, String> {
        let Some(player) = self.player(id) else { return Ok(None) };
        if self.editor_test
            || self.kind == GameSessionKind::Standalone
            || player.leaderboard_user_id != account
            || !credentials_equal(token, &player.resume_token)
            || player.reconnect_request_ids.contains(request_id)
            || self.staged_rejoin_index(id).is_some()
        {
            return Ok(None);
        }
        self.begin_reconnect(id, now_ms);
        let lease = self.reconnects.get(id).unwrap();
        if now_ms >= lease.expires_at_ms
            || lease.attempts >= solomon_protocol::hosted::GAME_RECONNECT_MAX_ATTEMPTS
                && lease.request_id.as_deref() != Some(request_id)
        {
            return Ok(None);
        }
        let peer_id = self.player(id).unwrap().peer_id;
        // An authenticated replacement may arrive before the upstream close frame.
        // Removing this generation from routing immediately neutralizes late input.
        if let Some(mut connection) = self.connections.remove(&peer_id) {
            connection.input.neutralize();
            close_peer(&connection.peer, 1001, "transport replaced")?;
            self.reconnects.get_mut(id).unwrap().connection = Some(connection);
        }
        let lease = self.reconnects.get_mut(id).unwrap();
        if lease.request_id.as_deref() != Some(request_id) {
            lease.attempts += 1;
            lease.request_id = Some(request_id.into());
            lease.admitting = false;
            lease.welcomed = false;
        }
        let expires_at_ms = lease.expires_at_ms;
        let player = self.player(id).unwrap();
        Ok(Some((
            HostedAdmission {
                content: player.content.clone(),
                developer_access: player.developer_access,
                leaderboard_user_id: player.leaderboard_user_id,
                party_id: None,
                reservation_id: None,
                party_rejoin_token: None,
                party_recovery_seed: false,
                observer: None,
                reconnect: Some(HostedReconnectAdmission {
                    player_id: id.into(),
                    request_id: request_id.into(),
                    expected_peer_id: peer_id,
                }),
            },
            expires_at_ms,
        )))
    }

    pub(in crate::host) fn admit_reconnect(
        &mut self,
        connection: &mut HostConnection,
        hello: &HostedHello,
        admission: &HostedReconnectAdmission,
        now_ms: f64,
    ) -> Result<String, (HostErrorCode, String)> {
        let invalid = || {
            (
                HostErrorCode::InvalidCredentials,
                "That reconnect admission is no longer current.".into(),
            )
        };
        if hello.version != PROTOCOL_VERSION {
            return Err((
                HostErrorCode::UnsupportedVersion,
                "The game client needs to update.".into(),
            ));
        }
        let id = &admission.player_id;
        let player = self.player(id).ok_or_else(invalid)?;
        let lease = self.reconnects.get(id).ok_or_else(invalid)?;
        if player.peer_id != admission.expected_peer_id
            || now_ms >= lease.expires_at_ms
            || lease.admitting
            || lease.request_id.as_ref() != Some(&admission.request_id)
            || hello.save.is_some()
            || hello.save_intent.is_some()
            || hello.begin_college_intro
            || hello.decline_tutorial
            || hello
                .resume_token
                .as_ref()
                .is_none_or(|token| !credentials_equal(token, &player.resume_token))
        {
            return Err(invalid());
        }
        connection.player_id = Some(id.clone());
        connection.cheats_enabled = self.cheats_enabled;
        let player = self.player_mut(id).unwrap();
        player.peer_id = connection.peer.id();
        player.reconnect_request_ids.insert(admission.request_id.clone());
        let lease = self.reconnects.get_mut(id).unwrap();
        lease.connection = None;
        lease.admitting = true;
        self.publish_dirty = true;
        self.social_dirty = true;
        Ok(id.clone())
    }

    pub(in crate::host) fn note_reconnect_message(
        &mut self,
        id: &str,
        peer_id: PeerId,
        message: &ClientMessage,
    ) {
        if self.player(id).is_none_or(|player| player.peer_id != peer_id) {
            return;
        }
        let run = self.worlds.loaded_for_player(id).map(|loaded| loaded.run_id.clone());
        let Some(lease) = self.reconnects.get_mut(id) else { return };
        if !lease.admitting {
            return;
        }
        match message {
            ClientMessage::Ping { .. } => lease.welcomed = true,
            ClientMessage::ReadyBoneyard { run_id } if run.as_ref() == Some(run_id) => {
                lease.renderer_ready = true
            }
            _ => {}
        }
    }

    pub(in crate::host) fn poll_reconnects(
        &mut self,
        now_ms: f64,
        shutdown: bool,
    ) -> Result<Vec<GameSocialConnection>, String> {
        let ids: Vec<_> = self
            .reconnects
            .iter()
            .filter(|(_, lease)| {
                shutdown
                    || now_ms >= lease.expires_at_ms
                    || lease.admitting && lease.welcomed && lease.renderer_ready
            })
            .map(|(id, _)| id.clone())
            .collect();
        let mut social = Vec::new();
        for id in ids {
            let lease = self.reconnects.remove(&id).unwrap();
            if !shutdown
                && now_ms < lease.expires_at_ms
                && lease.admitting
                && lease.welcomed
                && lease.renderer_ready
            {
                // Invalidate only the comparison cache, never the sequence or durable save owner.
                self.player_mut(&id).unwrap().last_save_document = None;
                self.pending_player_saves.insert(id);
                continue;
            }
            let connection = lease.connection.or_else(|| {
                let peer_id = self.player(&id)?.peer_id;
                self.connections.remove(&peer_id)
            });
            if let Some(mut connection) = connection {
                connection.planned_disconnect =
                    Some(("reconnect-expired", "transport recovery ended".into()));
                close_peer(
                    &connection.peer,
                    if shutdown { 1012 } else { 1000 },
                    "transport recovery ended",
                )?;
                if let Some(owner) = self.disconnected(
                    connection,
                    now_ms,
                    TransportClose {
                        code: Some(if shutdown { 1012 } else { 1000 }),
                        reason: "transport recovery ended".into(),
                    },
                )? {
                    social.push(owner);
                }
            }
        }
        Ok(social)
    }

    pub(in crate::host) fn prepare_reconnects_for_deployment(
        &mut self,
        now_ms: f64,
    ) -> Result<Vec<GameSocialConnection>, String> {
        let live: Vec<_> = self
            .reconnects
            .iter()
            .filter_map(|(id, lease)| {
                let player = self.player(id)?;
                (lease.admitting
                    && self
                        .connections
                        .get(&player.peer_id)
                        .is_some_and(|connection| !connection.peer.is_closed()))
                .then(|| id.clone())
            })
            .collect();
        for id in live {
            // An open replacement owes the normal final checkpoint/storage ACK,
            // including while its independent renderer readiness is pending.
            self.reconnects.remove(&id);
        }
        self.poll_reconnects(now_ms, true)
    }

    pub(in crate::host) fn recovering_player_count(&self) -> usize {
        self.reconnects.len()
    }

    pub(in crate::host) fn connected_human_count(&self) -> usize {
        self.players
            .iter()
            .filter(|player| {
                !self.reconnects.contains_key(&player.id)
                    && self.connections.contains_key(&player.peer_id)
            })
            .count()
    }

    pub(super) fn reconnecting_party_ids(&self) -> std::collections::BTreeSet<String> {
        self.reconnects
            .keys()
            .filter_map(|id| self.party_id_for_player(id).map(str::to_owned))
            .collect()
    }
}
