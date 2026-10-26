//! `game-host.ts` score completion, run-wide eligibility and once-only receipts.
//! Retain only the previous elapsed-score facts, never a cloned authoritative world.

use std::collections::{BTreeMap, BTreeSet};

use solomon_protocol::ServerMessage;
use solomon_protocol::hosted::HostedServerMessage;
use solomon_sim::hall_of_fame::completed_hall_of_fame_entry;
use solomon_sim::session::GameSession;

use super::{HostedGameHost, HostedWorlds};
use crate::host::authority::send_control;
use crate::host::signing::leaderboard_receipt;
use crate::host::source_utc_timestamp;

pub(super) struct LeaderboardRunSnapshot {
    run_id: String,
    scores: BTreeMap<String, Option<u64>>,
}

impl LeaderboardRunSnapshot {
    fn capture(state: &GameSession) -> Option<Self> {
        let world = state.boneyard.as_ref()?;
        let mut scores = BTreeMap::new();
        for (id, participant) in state.participants.iter() {
            scores.insert(id.clone(), participant.score.elapsed_ticks);
        }
        Some(Self { run_id: world.loaded.run_id.clone(), scores })
    }
}

struct LeaderboardClient<'a> {
    player_id: &'a str,
    user_id: Option<u32>,
    submit_runs: bool,
    socket_open: bool,
}

#[derive(Default)]
pub(super) struct LeaderboardRunState {
    ineligible_run_ids: BTreeSet<String>,
    issued_receipts: BTreeSet<(String, String)>,
}

impl LeaderboardRunState {
    fn taint(&mut self, run_id: &str) {
        self.ineligible_run_ids.insert(run_id.to_owned());
    }

    /// Source builds the Hall entry and reads the wall clock only after this exact
    /// edge is eligible. A missing portrait must not consume the once-only key.
    fn receipt_for(
        &mut self,
        previous: Option<&LeaderboardRunSnapshot>,
        completed: Option<&LeaderboardRunSnapshot>,
        client: LeaderboardClient<'_>,
        secret_enabled: bool,
        produce: impl FnOnce(u32) -> Result<Option<String>, String>,
    ) -> Result<Option<String>, String> {
        let (Some(previous), Some(completed)) = (previous, completed) else { return Ok(None) };
        if !secret_enabled
            || previous.run_id != completed.run_id
            || self.ineligible_run_ids.contains(&completed.run_id)
        {
            return Ok(None);
        }
        let Some(user_id) = client.user_id else { return Ok(None) };
        if !client.submit_runs
            || !client.socket_open
            || previous.scores.get(client.player_id) != Some(&None)
            || !matches!(completed.scores.get(client.player_id), Some(Some(_)))
        {
            return Ok(None);
        }
        let key = (completed.run_id.clone(), client.player_id.to_owned());
        if self.issued_receipts.contains(&key) {
            return Ok(None);
        }
        let Some(receipt) = produce(user_id)? else { return Ok(None) };
        // The Source key is committed before socket.send, so a later I/O failure
        // cannot manufacture a second signed receipt from the same completed edge.
        self.issued_receipts.insert(key);
        Ok(Some(receipt))
    }

    fn prune(&mut self, active_run_ids: &BTreeSet<String>) {
        self.ineligible_run_ids.retain(|id| active_run_ids.contains(id));
        self.issued_receipts.retain(|(id, _)| active_run_ids.contains(id));
    }
}

impl HostedGameHost {
    pub(in crate::host) fn taint_ineligible_client_runs(&mut self) {
        for player in &self.players {
            if player.global_score_eligible || self.staged_rejoin_index(&player.id).is_some() {
                continue;
            }
            if let Some(state) = self.worlds.state_for_player(&player.id)
                && let Some(world) = &state.boneyard
            {
                self.leaderboard_runs.taint(&world.loaded.run_id);
            }
        }
    }

    pub(super) fn taint_tutorial_run(&mut self, run_id: &str) {
        self.leaderboard_runs.taint(run_id);
    }

    /// Called after prepared actors/controls and immediately before the canonical
    /// simulation epoch, at the same previous-state boundary as Source.
    pub(super) fn capture_leaderboard_runs(&mut self) {
        self.leaderboard_previous_runs.clear();
        if self.editor_test {
            return;
        }
        match &self.worlds {
            HostedWorlds::Global(worlds) => {
                for run in &worlds.runs {
                    if let Some(snapshot) = LeaderboardRunSnapshot::capture(&run.state) {
                        self.leaderboard_previous_runs.push(snapshot);
                    }
                }
            }
            HostedWorlds::Private { state, .. } => {
                if let Some(snapshot) = LeaderboardRunSnapshot::capture(state) {
                    self.leaderboard_previous_runs.push(snapshot);
                }
            }
        }
    }

    /// Source publishes after prepared rules and detached milestone synchronization.
    /// Both player order and per-player Submit Runs come from the actual connections.
    pub(in crate::host) fn publish_leaderboard_receipts(&mut self) -> Result<(), String> {
        for previous in std::mem::take(&mut self.leaderboard_previous_runs) {
            let completed = match &self.worlds {
                HostedWorlds::Global(worlds) => worlds.runs.iter().find_map(|run| {
                    run.state
                        .boneyard
                        .as_ref()
                        .filter(|world| world.loaded.run_id == previous.run_id)
                        .map(|_| &run.state)
                }),
                HostedWorlds::Private { state, .. } => state
                    .boneyard
                    .as_ref()
                    .filter(|world| world.loaded.run_id == previous.run_id)
                    .map(|_| state),
            };
            let Some(state) = completed else { continue };
            let completed = LeaderboardRunSnapshot::capture(state);
            for player in &self.players {
                let connection = self.connections.get(&player.peer_id);
                let client = LeaderboardClient {
                    player_id: &player.id,
                    user_id: player.leaderboard_user_id,
                    submit_runs: player.preferences.submit_runs,
                    socket_open: connection.is_some_and(|connection| !connection.peer.is_closed()),
                };
                let receipt = self.leaderboard_runs.receipt_for(
                    Some(&previous),
                    completed.as_ref(),
                    client,
                    !self.recovery_secret.is_empty(),
                    |user_id| {
                        let completed_at = source_utc_timestamp(time::OffsetDateTime::now_utc())?;
                        let Some(participant) = state.participant(&player.id) else {
                            return Ok(None);
                        };
                        let world = state.boneyard.as_ref().expect("the completed run is present");
                        let wave = world.waves.as_ref().map_or(0, |waves| waves.state.wave_ordinal);
                        let Some(entry) = completed_hall_of_fame_entry(
                            &world.loaded.run_id,
                            state.run.phase,
                            participant.score,
                            participant.player,
                            wave,
                            completed_at,
                        ) else {
                            return Ok(None);
                        };
                        leaderboard_receipt(&self.recovery_secret, u64::from(user_id), &entry)
                            .map(Some)
                    },
                )?;
                if let Some(receipt) = receipt {
                    let connection = connection.expect("an eligible receipt has an open socket");
                    send_control(
                        &connection.peer,
                        ServerMessage::Hosted {
                            message: HostedServerMessage::LeaderboardReceipt { receipt },
                        },
                    )?;
                }
            }
        }
        let mut active = BTreeSet::new();
        match &self.worlds {
            HostedWorlds::Global(worlds) => {
                for run in &worlds.runs {
                    if let Some(world) = &run.state.boneyard {
                        active.insert(world.loaded.run_id.clone());
                    }
                }
            }
            HostedWorlds::Private { state, .. } => {
                if let Some(world) = &state.boneyard {
                    active.insert(world.loaded.run_id.clone());
                }
            }
        }
        self.leaderboard_runs.prune(&active);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
