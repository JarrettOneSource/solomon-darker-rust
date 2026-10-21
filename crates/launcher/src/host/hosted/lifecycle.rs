//! Per-party holds and actual world epochs. Socket readiness never changes a Session's
//! gameplay counters; it only selects which source world may step at this wall-clock epoch.

use std::collections::{BTreeMap, BTreeSet};

use solomon_protocol::{HostErrorCode, ResumeGraceReason};
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::modding::events::GameSimulationEventSnapshot;
use solomon_sim::session::{GameSessionInput, GameSessionTickResult};

use super::{HostedGameHost, HostedWorlds};
use crate::host::authority::{ControlFacts, random_bytes, random_hex};
use crate::host::control::GameplayControl;

pub struct HostedWorldTick {
    /// None is the persistent College; Some is the actual party's run.
    pub party_id: Option<String>,
    pub result: GameSessionTickResult,
}

impl HostedGameHost {
    /// Source `GameHost.close`: retained diagnostics precede bot/VM retirement,
    /// then actual close callbacks release the remaining player identities.
    pub(in crate::host) fn begin_shutdown(&mut self, failed: bool) -> Result<(), String> {
        self.close_run_archives(super::super::run_archive::RunArchiveEndReason::HostClosed);
        self.remove_all_ml_bots()?;
        self.reset_lua_runtime();
        if let Some(mut runtime) = self.private_mod_host.take() {
            runtime.close();
        }
        for mut scope in self.party_mod_runtimes.drain(..) {
            scope.runtime.close();
        }
        let (code, reason, source, planned_reason) = if failed {
            (1011, "server error", "server-error", "authoritative simulation failure")
        } else {
            (1012, "server shutdown", "server-shutdown", "server shutdown")
        };
        for connection in self.connections.values_mut() {
            connection.planned_disconnect = Some((source, planned_reason.into()));
            super::super::authority::close_peer(&connection.peer, code, reason)?;
        }
        Ok(())
    }

    pub fn party_id_for_player(&self, id: &str) -> Option<&str> {
        self.worlds.parties().party_for_player(id).map(|party| party.id.as_str())
    }

    pub(in crate::host) fn control_facts(&self, party_id: &str) -> ControlFacts {
        let mut materialized = BTreeSet::new();
        let mut connected = BTreeSet::new();
        let mut active_run = false;
        let mut skill_barrier = false;
        if let Some(party) = self.worlds.parties().parties.iter().find(|party| party.id == party_id)
        {
            for id in &party.member_player_ids {
                if self.player(id).is_some() {
                    connected.insert(id.clone());
                }
                if let Some(state) = self.worlds.state_for_player(id) {
                    materialized.insert(id.clone());
                    active_run |=
                        state.boneyard.is_some() && state.run.phase == GameRunPhase::Active;
                    skill_barrier |= state.level_up_barrier.is_some();
                }
            }
        }
        ControlFacts { active_run, skill_barrier, materialized, connected }
    }

    pub fn maintain_controls(&mut self, now_ms: f64) {
        let party_ids: Vec<_> =
            self.worlds.parties().parties.iter().map(|party| party.id.clone()).collect();
        for party_id in &party_ids {
            let facts = self.control_facts(party_id);
            let control = self.controls.entry(party_id.clone()).or_default();
            let previous_grace = control.resume_grace(now_ms);
            let previous_loading = control.loading_run_id().map(str::to_owned);
            control.advance(&facts.context(), now_ms);
            if previous_grace != control.resume_grace(now_ms)
                || previous_loading.as_deref() != control.loading_run_id()
            {
                self.neutralize_party(party_id);
                self.publish_dirty = true;
            }
        }
        self.controls.retain(|id, _| party_ids.contains(id));
        self.reservations.retain(|reservation| reservation.expires_at_ms > now_ms);
    }

    pub(in crate::host) fn neutralize_party(&mut self, party_id: &str) {
        let Some(party) = self.worlds.parties().parties.iter().find(|party| party.id == party_id)
        else {
            return;
        };
        for connection in self.connections.values_mut() {
            if connection.player_id.as_ref().is_some_and(|id| party.member_player_ids.contains(id))
            {
                connection.input.neutralize();
            }
        }
        for bot in &mut self.bots.rows {
            if party.member_player_ids.contains(&bot.id) {
                bot.active_input = GameSessionInput::default();
            }
        }
    }

    /// `client-start-match` / `client-start-tutorial`: geometry and navigation prepare
    /// before the party is partitioned. Failed content admission cannot move its actors.
    pub fn start_match(
        &mut self,
        id: &str,
        boneyard_id: &str,
        tutorial: bool,
    ) -> Result<bool, (HostErrorCode, String)> {
        if self.editor_test {
            if tutorial {
                return Ok(false);
            }
            if !self.boneyard_choices().iter().any(|choice| choice.id == boneyard_id) {
                return Err((
                    HostErrorCode::InvalidMessage,
                    "The selected Boneyard is unavailable.".into(),
                ));
            }
        }
        let Some(party) = self.worlds.parties().party_for_player(id) else { return Ok(false) };
        if party.leader_player_id != id {
            return Ok(false);
        }
        let party_id = party.id.clone();
        let party_size = party.member_player_ids.len();
        let Some(state) = self.worlds.state_for_player(id) else { return Ok(false) };
        if !state.can_enter_boneyard()
            || self.controls.get(&party_id).is_some_and(|control| control.pause().is_some())
            || (!tutorial && state.participants[id].wizard.transition.is_some())
        {
            return Ok(false);
        }
        if tutorial {
            let fresh = state.participants[id].player.economy.tutorial_pending;
            let vanilla = self.content.manifest.mods.is_empty();
            let eligible = self.player(id).is_some_and(|player| match &self.worlds {
                HostedWorlds::Global(_) => player.global_score_eligible && party_size == 1,
                HostedWorlds::Private { .. } => player.tutorial_eligible,
            });
            if !fresh {
                return Err((
                    HostErrorCode::InvalidMessage,
                    "The stock Tutorial is available only to a fresh profile.".into(),
                ));
            }
            if !vanilla || !eligible || self.cheats_enabled {
                return Err((
                    HostErrorCode::InvalidMessage,
                    "The stock Tutorial requires a solo vanilla party.".into(),
                ));
            }
        }
        let before = GameSimulationEventSnapshot::capture(state);
        let activity_before = super::activity::project_game_activity(state);
        let seed = match self.next_lua_run_seed.take() {
            Some(seed) => {
                // Source writes this word big-endian into a zeroed 16-byte seed.
                let mut bytes = [0; 16];
                bytes[..4].copy_from_slice(&seed.to_be_bytes());
                bytes
            }
            None => random_bytes::<16>().map_err(server_error)?,
        };
        let run_id = random_hex::<16>().map_err(server_error)?;
        let (mut loaded, navigation) = if tutorial {
            self.content_loader.prepare_run(seed, true).map_err(server_error)?
        } else if matches!(self.worlds, HostedWorlds::Global(_)) {
            self.prepare_party_match(id, &party_id, seed, boneyard_id, &run_id)
                .map_err(|reason| (HostErrorCode::InvalidMessage, reason))?
        } else {
            self.content_loader.prepare_choice(seed, boneyard_id, &run_id).map_err(|_| {
                (HostErrorCode::InvalidMessage, "The selected Boneyard is unavailable.".into())
            })?
        };
        loaded.run_id = run_id.clone();
        match &mut self.worlds {
            HostedWorlds::Global(worlds) => {
                if worlds.start_party_run(id, loaded).is_err() {
                    return Ok(false);
                }
            }
            HostedWorlds::Private { state, .. } => {
                state.install_boneyard(loaded).map_err(server_error)?
            }
        }
        self.activate_mod_boneyard(&party_id, &before, false).map_err(server_error)?;
        if tutorial {
            self.taint_tutorial_run(&run_id);
        }
        self.taint_ineligible_client_runs();
        self.navigation.insert(party_id.clone(), navigation);
        self.arm_party_rejoins(&party_id).map_err(server_error)?;
        let connected = self.control_facts(&party_id).connected;
        let mut control = GameplayControl::default();
        control.arm_renderers(&run_id, &connected);
        control.begin_loading_grace(id, ResumeGraceReason::GameStarted, &connected, &[]);
        self.controls.insert(party_id.clone(), control);
        self.neutralize_party(&party_id);
        if let Some(state) = self.worlds.state_for_player(id) {
            for connection in self.connections.values() {
                if connection
                    .player_id
                    .as_ref()
                    .is_some_and(|id| state.participants.contains_key(id))
                {
                    self.send_boneyard_mod_content(connection, id).map_err(server_error)?;
                }
            }
        }
        // Source boneyard-entry schedules every client on this host, including Hub
        // visitors. Unchanged documents retain their previous per-player sequence.
        self.pending_player_saves.extend(self.players.iter().map(|player| player.id.clone()));
        self.publish_dirty = true;
        self.social_dirty = true;
        if let Some(state) = self.worlds.state_for_player(id) {
            self.emit_game_activity(
                &activity_before,
                state,
                state
                    .boneyard
                    .as_ref()
                    .map(|world| super::activity::boneyard_activity(&world.loaded)),
                if matches!(self.worlds, HostedWorlds::Global(_)) { Some(&party_id) } else { None },
            );
        }
        self.checkpoint_run_for_player(id);
        Ok(true)
    }

    /// A single source epoch, College first and then retained runs in creation order.
    /// No checkpoint/world clone is constructed for either ticking or publication.
    pub fn step(&mut self, now_ms: f64) -> Result<Vec<HostedWorldTick>, String> {
        self.leaderboard_previous_runs.clear();
        self.maintain_controls(now_ms);
        // The private source host returns before draining inputs during a wall-clock
        // hold. Shared worlds deliberately drain first, then hold only that party run.
        if let HostedWorlds::Private { parties, .. } = &self.worlds
            && parties.parties.first().is_some_and(|party| {
                self.controls.get(&party.id).is_some_and(GameplayControl::blocks_tick)
            })
        {
            return Ok(Vec::new());
        }
        if let (HostedWorlds::Private { state, .. }, Some(runtime)) =
            (&self.worlds, &self.private_mod_host)
            && let Some(run_id) = &state.run.run_id
            && runtime.active_scene(run_id)?.is_some()
        {
            return Ok(Vec::new());
        }
        self.step_ml_bots(now_ms)?;
        self.taint_ineligible_client_runs();
        let mut inputs: BTreeMap<String, GameSessionInput> = BTreeMap::new();
        for (peer_id, connection) in &mut self.connections {
            if self.observers.contains_key(peer_id) {
                continue;
            }
            let Some(id) = &connection.player_id else { continue };
            if let Some(state) = self.worlds.state_for_player(id) {
                inputs.insert(id.clone(), connection.input.for_tick(state.ticks + 1));
            }
        }
        for bot in &self.bots.rows {
            inputs.insert(bot.id.clone(), bot.active_input);
        }
        let mut paused = BTreeSet::new();
        for (id, control) in &self.controls {
            if control.blocks_tick() {
                paused.insert(id.clone());
            }
        }
        self.step_prepared_worlds(now_ms, &mut inputs, &mut paused)
    }
}

fn server_error(reason: String) -> (HostErrorCode, String) {
    (HostErrorCode::ServerError, reason)
}
