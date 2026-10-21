//! Developer observation borrows an existing match. An observer has a socket cursor,
//! never a participant, party member, save profile, input queue owner or collision body.
//! Source: `game-host.ts::observationWorld` and observer Hello branch.

use serde::Serialize;
use solomon_protocol::hosted::HostedWelcome;
use solomon_protocol::{
    GAME_TICK_RATE, HostErrorCode, PROTOCOL_VERSION, ResumeCredentials, SNAPSHOT_RATE,
    ServerMessage, SnapshotFrame,
};
use solomon_sim::session::GameSession;

use super::{HostedAdmission, HostedGameHost, HostedWorlds};
use crate::host::authority::{close_peer, encode, send_welcome};
use crate::host::connection::HostConnection;
use crate::host::signing::random_token;
use crate::transport::PeerId;

type AdmissionError = (HostErrorCode, String);

pub struct HostedObserver {
    pub run_id: String,
    pub observer_id: String,
    pub requested_by_user_id: u32,
    pub requested_by_username: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostedObservationTarget {
    pub boneyard_name: String,
    pub party_leader: String,
    pub player_count: usize,
    pub players: Vec<String>,
    pub run_id: String,
    pub visibility: &'static str,
    pub wave_number: u64,
}

impl HostedGameHost {
    pub(in crate::host) fn observation_world(&self, run_id: &str) -> Option<(&GameSession, &str)> {
        let state = match &self.worlds {
            HostedWorlds::Global(worlds) => {
                &worlds.runs.iter().find(|run| run.loaded_boneyard.run_id == run_id)?.state
            }
            HostedWorlds::Private { state, .. } => state,
        };
        if state.boneyard.is_none() || state.run.run_id.as_deref() != Some(run_id) {
            return None;
        }
        let first = state.participant_order.first()?;
        let party = self.worlds.parties().party_for_player(first);
        let view = party
            .map(|party| party.leader_player_id.as_str())
            .filter(|id| state.participant(id).is_some())
            .unwrap_or(first);
        Some((state, view))
    }

    pub(in crate::host) fn observation_targets(&self) -> Vec<HostedObservationTarget> {
        if self.editor_test {
            return Vec::new();
        }
        let mut targets = Vec::new();
        for party in &self.worlds.parties().parties {
            let Some(state) =
                party.member_player_ids.iter().find_map(|id| self.worlds.state_for_player(id))
            else {
                continue;
            };
            let Some(world) = &state.boneyard else { continue };
            let name = |id: &str| {
                self.retained_participant(id)
                    .map(|participant| participant.player.selection.name.clone())
                    .or_else(|| {
                        self.recovery_lineages
                            .iter()
                            .flat_map(|lineage| &lineage.party_roster)
                            .find(|row| row.player_id == id)
                            .map(|row| row.display_name.clone())
                    })
                    .unwrap_or_else(|| id.into())
            };
            targets.push(HostedObservationTarget {
                boneyard_name: world.loaded.name.clone(),
                party_leader: name(&party.leader_player_id),
                player_count: state.participant_order.len(),
                players: state.participant_order.iter().map(|id| name(id)).collect(),
                run_id: world.loaded.run_id.clone(),
                visibility: super::super::supervisor::http::visibility(party.visibility),
                wave_number: world.waves.as_ref().map_or(0, |waves| waves.state.wave_ordinal),
            });
        }
        targets
    }

    pub(in crate::host) fn admit_observer(
        &mut self,
        connection: &mut HostConnection,
        version: u32,
        admission: HostedAdmission,
        now_ms: f64,
    ) -> Result<(), AdmissionError> {
        if version != PROTOCOL_VERSION {
            return Err((
                HostErrorCode::UnsupportedVersion,
                "The game client needs to update.".into(),
            ));
        }
        let target =
            admission.observer.filter(|_| admission.developer_access).ok_or_else(|| {
                (HostErrorCode::InvalidCredentials, "The observer credential is invalid.".into())
            })?;
        if self.observers.len() >= 8 {
            return Err((HostErrorCode::HostFull, "The observer capacity is full.".into()));
        }
        let (_, view) = self.observation_world(&target.run_id).ok_or_else(|| {
            (HostErrorCode::InvalidCredentials, "The observed match has ended.".into())
        })?;
        let view = view.to_owned();
        let observer_id = format!("observer-{}", random_token::<12>().map_err(server_error)?);
        let sequence = self.next_snapshot_sequence;
        self.next_snapshot_sequence += 1;
        self.commit_flame_histories().map_err(server_error)?;
        let state = self.observation_world(&target.run_id).unwrap().0;
        let hosted = HostedWelcome {
            session_kind: self.kind,
            developer_access: false,
            cheats_enabled: self.cheats_enabled,
            observer: true,
            content: admission.content.manifest.clone(),
            mod_assets: admission.content.assets.clone(),
            mod_catalog: self.mod_content.consumables().cloned().collect(),
            boneyards: self.boneyard_choices(),
        };
        let flame_baseline = self.flame_baseline(state).map_err(server_error)?;
        let frame = SnapshotFrame {
            sequence,
            flame_presentation: Some(self.flame_frame(state, None).map_err(server_error)?),
            input_acknowledgements: Default::default(),
            host: self.host_snapshot(&view, state, now_ms),
            snapshot: state.snapshot(),
        };
        let payload = encode(&ServerMessage::Welcome {
            version: PROTOCOL_VERSION,
            credentials: ResumeCredentials { player_id: view.clone(), token: observer_id.clone() },
            tick_rate: GAME_TICK_RATE,
            snapshot_rate: SNAPSHOT_RATE,
            hosted: Some(hosted),
            frame,
        })
        .map_err(server_error)?;
        send_welcome(&connection.peer, payload).map_err(server_error)?;
        connection
            .ensure_boneyard_loaded(self.worlds.loaded_for_run(&target.run_id))
            .map_err(server_error)?;
        self.send_prepared_mod_state(connection, &view).map_err(server_error)?;
        connection.welcomed(view, sequence);
        connection.connected_at_utc_ms = Some(crate::host::source_wall_clock_ms());
        connection.record_flame_baseline(sequence, flame_baseline);
        self.observers.insert(
            connection.peer.id(),
            HostedObserver {
                run_id: target.run_id,
                observer_id,
                requested_by_user_id: target.user_id,
                requested_by_username: target.username,
            },
        );
        let observer = &self.observers[&connection.peer.id()];
        self.emit_runtime_event("observer.connected", "A developer observer connected to an active match.",
            serde_json::json!({"boneyardName":self.worlds.loaded_for_run(&observer.run_id).map(|loaded| &loaded.name),
                "observerId":observer.observer_id,"observerUserId":observer.requested_by_user_id,
                "observerUsername":observer.requested_by_username,"runId":observer.run_id}));
        Ok(())
    }

    pub(in crate::host) fn publish_observers(
        &mut self,
        sequence: u64,
        now_ms: f64,
    ) -> Result<(), String> {
        let peers: Vec<PeerId> = self.observers.keys().copied().collect();
        for peer_id in peers {
            let observer = &self.observers[&peer_id];
            let connection = &self.connections[&peer_id];
            if !connection.can_publish_snapshot() {
                continue;
            }
            let Some((state, preferred)) = self.observation_world(&observer.run_id) else {
                let connection = self.connections.get_mut(&peer_id).unwrap();
                connection.planned_disconnect =
                    Some(("observer-target-ended", "observed match ended".into()));
                close_peer(&connection.peer, 1000, "observed match ended")?;
                continue;
            };
            let current = connection
                .player_id
                .as_deref()
                .filter(|id| state.participant(id).is_some())
                .unwrap_or(preferred)
                .to_owned();
            let payload = self.encode_frame(
                &current,
                sequence,
                now_ms,
                None,
                connection.acknowledged_flame_baseline(),
            )?;
            let flame_baseline = self.flame_baseline(state)?;
            let loaded = self.worlds.loaded_for_run(&observer.run_id);
            let connection = self.connections.get_mut(&peer_id).unwrap();
            connection.ensure_boneyard_loaded(loaded)?;
            match connection.peer.replace_snapshot(payload) {
                Ok(()) => {
                    connection.published_snapshot(sequence);
                    connection.record_flame_baseline(sequence, flame_baseline);
                }
                Err(crate::transport::PeerSendError::Closed) => {}
                Err(error) => return Err(error.to_string()),
            }
            self.send_prepared_mod_runtime(&self.connections[&peer_id], &current)?;
        }
        Ok(())
    }
}
fn server_error(reason: String) -> AdmissionError {
    (HostErrorCode::ServerError, reason)
}
