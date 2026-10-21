//! Completed source frames and listener one-shots over borrowed canonical worlds.

use std::collections::BTreeMap;
use std::sync::Arc;

use solomon_protocol::hosted::HostedWelcome;
use solomon_protocol::{
    GAME_TICK_RATE, HostSnapshot, PROTOCOL_VERSION, ResumeCredentials, SNAPSHOT_RATE,
    ServerMessage, SnapshotFrame,
};
use solomon_sim::modding::inventory::ModConsumableCatalogEntry;
use solomon_sim::session::GameSession;

use super::{HostedGameHost, HostedWorldTick, HostedWorlds};
use crate::host::audio::project_listener_audio;
use crate::host::authority::{encode, send_control, send_welcome};
use crate::host::connection::HostConnection;
use crate::transport::PeerSendError;

impl HostedGameHost {
    pub(in crate::host) fn send_welcome(
        &mut self,
        connection: &mut HostConnection,
        id: &str,
        now_ms: f64,
        mod_catalog: Vec<ModConsumableCatalogEntry>,
    ) -> Result<(), String> {
        self.commit_flame_histories()?;
        self.checkpoint_run_for_player(id);
        let loaded = self.worlds.loaded_for_player(id).or_else(|| {
            let slot = self.rejoin_slots.iter().find(|slot| slot.player_id == id)?;
            self.worlds.loaded_for_run(&slot.run_id)
        });
        let player = self.player(id).ok_or("The welcome wizard has no admitted identity")?;
        let credentials =
            ResumeCredentials { player_id: id.into(), token: player.resume_token.clone() };
        let hosted = HostedWelcome {
            session_kind: self.kind,
            developer_access: player.developer_access,
            cheats_enabled: if self.kind == solomon_sim::social::GameSessionKind::Standalone {
                connection.cheats_enabled
            } else {
                self.cheats_enabled
            },
            observer: false,
            content: player.content.manifest.clone(),
            mod_assets: player.content.assets.clone(),
            mod_catalog,
            boneyards: self.boneyard_choices(),
        };
        let sequence = self.next_snapshot_sequence;
        self.next_snapshot_sequence += 1;
        let payload = self.encode_frame(id, sequence, now_ms, Some((credentials, hosted)), None)?;
        send_welcome(&connection.peer, payload)?;
        connection.ensure_boneyard_loaded(loaded)?;
        connection.welcomed(id.into(), sequence);
        connection.record_flame_baseline(sequence, self.flame_baseline_for_viewer(id)?);
        self.send_prepared_mod_state(connection, id)?;
        if let Some(state) = self.worlds.state_for_player(id) {
            project_listener_audio(state, connection, &[], true);
        }
        self.social_dirty = true;
        Ok(())
    }

    pub(in crate::host) fn encode_frame(
        &self,
        id: &str,
        sequence: u64,
        now_ms: f64,
        welcome: Option<(ResumeCredentials, HostedWelcome)>,
        flame_baseline: Option<solomon_protocol::flame_presentation::FlamePresentationBaseline>,
    ) -> Result<Arc<str>, String> {
        let staged = self
            .rejoin_slots
            .iter()
            .find(|slot| slot.player_id == id && slot.connected && slot.detached.is_some());
        let state = self
            .worlds
            .state_for_player(id)
            .or_else(|| staged.and_then(|slot| self.active_run(&slot.party_id, &slot.run_id)))
            .ok_or("The accepted viewer has no world")?;
        let projected = staged
            .and_then(|slot| slot.detached.as_ref())
            .map(|detached| state.project_detached(detached))
            .transpose()?;
        let snapshot = match &projected {
            Some(projected) => state.snapshot_with_detached(projected),
            None => state.snapshot(),
        };
        let mut acknowledgements = BTreeMap::new();
        for member in snapshot.participant_order {
            let sequence = self
                .player(member)
                .and_then(|player| self.connections.get(&player.peer_id))
                .map_or(0, |connection| connection.input.acknowledged_sequence());
            acknowledgements.insert(member.clone(), sequence);
        }
        let frame = SnapshotFrame {
            sequence,
            flame_presentation: Some(self.flame_frame(state, flame_baseline)?),
            input_acknowledgements: acknowledgements,
            host: self.host_snapshot(id, state, now_ms),
            snapshot,
        };
        match welcome {
            Some((credentials, hosted)) => encode(&ServerMessage::Welcome {
                version: PROTOCOL_VERSION,
                credentials,
                tick_rate: GAME_TICK_RATE,
                snapshot_rate: SNAPSHOT_RATE,
                hosted: Some(hosted),
                frame,
            }),
            None => encode(&ServerMessage::Snapshot { frame }),
        }
    }

    pub fn publish_snapshot(&mut self, now_ms: f64) -> Result<(), String> {
        self.commit_flame_histories()?;
        let mut groups: BTreeMap<_, Vec<u64>> = BTreeMap::new();
        for (&peer_id, connection) in &self.connections {
            if self.observers.contains_key(&peer_id) {
                continue;
            }
            let Some(id) = connection.player_id.as_deref() else { continue };
            if !connection.can_publish_snapshot() {
                continue;
            }
            if let Some(party_id) = self.party_id_for_player(id) {
                let baseline =
                    connection.acknowledged_flame_baseline().map(|row| (row.epoch, row.world_tick));
                groups.entry((party_id.to_owned(), baseline)).or_default().push(peer_id);
            }
        }
        let sequence = self.next_snapshot_sequence;
        self.next_snapshot_sequence += 1;
        for ((_, baseline), peer_ids) in groups {
            let baseline = baseline.map(|(epoch, world_tick)| {
                solomon_protocol::flame_presentation::FlamePresentationBaseline {
                    epoch,
                    world_tick,
                }
            });
            // Staged rejoin adds a private participant projection. It can never be the
            // ordinary group's representative, or other peers would see that actor early.
            let representative = peer_ids.iter().find_map(|peer_id| {
                let id = self.connections[peer_id].player_id.as_ref().unwrap();
                (!self
                    .rejoin_slots
                    .iter()
                    .any(|slot| slot.player_id == *id && slot.connected && slot.detached.is_some()))
                .then(|| id.clone())
            });
            let shared = representative
                .as_ref()
                .map(|id| self.encode_frame(id, sequence, now_ms, None, baseline))
                .transpose()?;
            for peer_id in peer_ids {
                let connection = &self.connections[&peer_id];
                let id = connection.player_id.as_ref().unwrap().clone();
                let payload =
                    if self.rejoin_slots.iter().any(|slot| {
                        slot.player_id == id && slot.connected && slot.detached.is_some()
                    }) {
                        self.encode_frame(&id, sequence, now_ms, None, baseline)?
                    } else {
                        shared.as_ref().expect("ordinary viewer has an ordinary projection").clone()
                    };
                let loaded = self.worlds.loaded_for_player(&id).or_else(|| {
                    let slot = self.rejoin_slots.iter().find(|slot| slot.player_id == id)?;
                    self.worlds.loaded_for_run(&slot.run_id)
                });
                let flame_baseline = self.flame_baseline_for_viewer(&id)?;
                let connection = self.connections.get_mut(&peer_id).unwrap();
                connection.ensure_boneyard_loaded(loaded)?;
                match connection.peer.replace_snapshot(payload) {
                    Ok(()) => {
                        connection.published_snapshot(sequence);
                        connection.record_flame_baseline(sequence, flame_baseline);
                    }
                    Err(PeerSendError::Closed) => {}
                    Err(error) => return Err(error.to_string()),
                }
                self.send_prepared_mod_runtime(&self.connections[&peer_id], &id)?;
            }
        }
        self.publish_observers(sequence, now_ms)?;
        self.prune_flame_history()?;
        self.publish_dirty = false;
        Ok(())
    }

    pub(in crate::host::hosted) fn host_snapshot(
        &self,
        id: &str,
        state: &GameSession,
        now_ms: f64,
    ) -> HostSnapshot {
        let party = self.worlds.parties().party_for_player(id);
        let control = party.and_then(|party| self.controls.get(&party.id));
        let mut hub_activities = BTreeMap::new();
        if state.boneyard.is_none() {
            for connection in self.connections.values() {
                if let (Some(id), Some(activity)) = (&connection.player_id, connection.hub_activity)
                    && state.participant(id).is_some()
                {
                    hub_activities.insert(id.clone(), activity);
                }
            }
        }
        HostSnapshot {
            materializing_player_ids: if self.staged_rejoin_index(id).is_some() {
                vec![id.into()]
            } else {
                Vec::new()
            },
            authority_player_id: party.map(|party| party.leader_player_id.clone()),
            connected_player_ids: self
                .players
                .iter()
                .filter(|player| state.participant(&player.id).is_some() || player.id == id)
                .map(|player| player.id.clone())
                .collect(),
            hub_activities,
            pause: control.and_then(|control| control.pause().cloned()),
            resume_grace: control.and_then(|control| control.resume_grace(now_ms)),
            loading_run_id: control.and_then(|control| control.loading_run_id().map(str::to_owned)),
        }
    }

    pub(in crate::host) fn active_run(&self, party_id: &str, run_id: &str) -> Option<&GameSession> {
        let state = match &self.worlds {
            HostedWorlds::Global(worlds) => {
                &worlds.runs.iter().find(|run| run.party_id == party_id)?.state
            }
            HostedWorlds::Private { state, .. } => state,
        };
        (state.run.phase == solomon_sim::game_run::GameRunPhase::Active
            && state.boneyard.is_some()
            && state.run.run_id.as_deref() == Some(run_id))
        .then_some(state)
    }

    pub(in crate::host) fn publish_tick_audio(
        &mut self,
        ticks: &[HostedWorldTick],
    ) -> Result<(), String> {
        for connection in self.connections.values_mut() {
            let Some(id) = connection.player_id.as_deref() else { continue };
            let Some(state) = self.worlds.state_for_player(id) else { continue };
            let party_id =
                self.worlds.parties().party_for_player(id).map(|party| party.id.as_str());
            let persistent_hub = matches!(&self.worlds,HostedWorlds::Global(worlds) if worlds.hub.participant(id).is_some());
            let expected_party = if persistent_hub { None } else { party_id };
            let accepted = ticks.iter().find(|tick| tick.party_id.as_deref() == expected_party);
            let Some(accepted) = accepted else { continue };
            let events =
                project_listener_audio(state, connection, &accepted.result.audio_events, false);
            if !events.is_empty() {
                send_control(&connection.peer, ServerMessage::Audio { tick: state.ticks, events })?;
            }
        }
        Ok(())
    }
}
