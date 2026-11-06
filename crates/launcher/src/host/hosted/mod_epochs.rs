//! The order of work in one hosted tick: mod player-control leases and prepared actors, the
//! simulation step, then derived events and mod rules. Every stage borrows the live world.

use std::collections::{BTreeMap, BTreeSet};

use solomon_protocol::ResumeGraceReason;
use solomon_sim::enemies::EnemySpawnIntent;
use solomon_sim::hub::memorial::HubMemorialPlayerProfile;
use solomon_sim::modding::LuaObject;
use solomon_sim::modding::events::{GameSimulationEventSnapshot, derive_web_lua_events};
use solomon_sim::modding::extensions::GameSimulationExtensions;
use solomon_sim::session::{GameSessionInput, GameSessionTickContext};
use solomon_sim::shared_worlds::SharedWorldsTickContext;

use super::bots::HostedBot;
use super::mods::publish_mod_content;
use super::{HostedGameHost, HostedPlayer, HostedRejoinSlot, HostedWorldTick, HostedWorlds};

/// What the prepared mod actors leave for the rest of the tick: each world's events snapshot
/// from before they ran, the enemy spawns they and the Console requested, and each party whose
/// skill picker a player control released, with the viewer who resumes it.
#[derive(Default)]
struct PreparedActors {
    before_runs: BTreeMap<String, GameSimulationEventSnapshot>,
    before_private: Option<GameSimulationEventSnapshot>,
    enemy_spawns: BTreeMap<String, Vec<EnemySpawnIntent>>,
    private_spawns: Vec<EnemySpawnIntent>,
    released_barriers: Vec<(String, String)>,
}

/// The Memorial's account names for every rejoining, connected and bot wizard, and the wizards
/// whose runs it may record.
struct MemorialRoster {
    profiles: BTreeMap<String, HubMemorialPlayerProfile>,
    eligible: BTreeSet<String>,
}

impl HostedGameHost {
    pub(super) fn step_prepared_worlds(
        &mut self,
        now_ms: f64,
        inputs: &mut BTreeMap<String, GameSessionInput>,
        paused: &mut BTreeSet<String>,
    ) -> Result<Vec<HostedWorldTick>, String> {
        self.commit_flame_histories()?;
        self.capture_runtime_activity();
        let shared = matches!(self.worlds, HostedWorlds::Global(_));
        let before_private_console = if shared { None } else { self.console_event_snapshot() };
        let mut console_epoch = if shared { self.begin_console_epoch(None)? } else { None };
        self.commit_flame_histories()?;
        let mut prepared = PreparedActors::default();
        if let Some(epoch) = &mut console_epoch
            && let Some(party) = &epoch.party_id
        {
            prepared
                .enemy_spawns
                .insert(party.clone(), std::mem::take(&mut epoch.enemy_spawn_intents));
        }
        self.run_prepared_actors(now_ms, inputs, paused, &mut prepared)?;
        self.commit_flame_histories()?;
        self.begin_skill_picker_graces(&prepared.released_barriers, now_ms);
        if matches!(self.worlds, HostedWorlds::Private { .. })
            && !prepared.released_barriers.is_empty()
        {
            return Ok(Vec::new());
        }

        if !shared {
            console_epoch = self.begin_console_epoch(before_private_console)?;
            if let Some(epoch) = &mut console_epoch {
                prepared.private_spawns.extend(std::mem::take(&mut epoch.enemy_spawn_intents));
            }
        }
        self.commit_flame_histories()?;
        self.capture_leaderboard_runs();
        let result = self.step_worlds(inputs, paused, &prepared)?;
        self.commit_flame_histories()?;
        // A shared Hub dispatches developer Console events before prepared rules;
        // a private College dispatches them afterwards. Both read the completed world.
        if shared {
            self.finish_console_epoch(console_epoch.take())?;
            self.commit_flame_histories()?;
        }
        self.step_prepared_rules(&prepared.before_runs, prepared.before_private)?;
        self.commit_flame_histories()?;
        if !shared {
            self.finish_console_epoch(console_epoch)?;
            self.commit_flame_histories()?;
        }
        Ok(result)
    }

    /// Applies each world's mod player controls, then ticks its prepared actors. A world whose
    /// skill picker a player control released pauses instead of ticking.
    fn run_prepared_actors(
        &mut self,
        now_ms: f64,
        inputs: &mut BTreeMap<String, GameSessionInput>,
        paused: &mut BTreeSet<String>,
        prepared: &mut PreparedActors,
    ) -> Result<(), String> {
        match &mut self.worlds {
            HostedWorlds::Global(worlds) => {
                for scope in &mut self.party_mod_runtimes {
                    let Some(run) =
                        worlds.runs.iter_mut().find(|run| run.party_id == scope.party_id)
                    else {
                        continue;
                    };
                    // Rule events also report resource changes caused by the prepared
                    // actors, so capture these facts before `runtime.tick`.
                    prepared.before_runs.insert(
                        scope.party_id.clone(),
                        GameSimulationEventSnapshot::capture(&run.state),
                    );
                    if let Some(run_id) = &run.state.run.run_id
                        && scope.runtime.active_scene(run_id)?.is_some()
                    {
                        paused.insert(scope.party_id.clone());
                    }
                    if paused.contains(&scope.party_id) {
                        continue;
                    }
                    let revision = scope.runtime.projection_revision()?;
                    let released =
                        scope.runtime.apply_player_controls(inputs, now_ms, &mut run.state)?;
                    if scope.runtime.projection_revision()? != revision {
                        publish_mod_content(
                            &scope.runtime,
                            &run.state,
                            &self.connections,
                            &self.observers,
                        )?;
                    }
                    if released {
                        if let Some(viewer) = run.state.participant_order.first() {
                            prepared
                                .released_barriers
                                .push((scope.party_id.clone(), viewer.clone()));
                            paused.insert(scope.party_id.clone());
                        }
                        continue;
                    }
                    if scope.runtime.tick(run.state.ticks + 1, &mut run.state)? {
                        publish_mod_content(
                            &scope.runtime,
                            &run.state,
                            &self.connections,
                            &self.observers,
                        )?;
                    }
                    prepared
                        .enemy_spawns
                        .entry(scope.party_id.clone())
                        .or_default()
                        .extend(scope.runtime.drain_enemy_spawns()?);
                }
            }
            HostedWorlds::Private { state, parties } => {
                if let Some(runtime) = &mut self.private_mod_host {
                    prepared.before_private = Some(GameSimulationEventSnapshot::capture(state));
                    let revision = runtime.projection_revision()?;
                    let released = runtime.apply_player_controls(inputs, now_ms, state)?;
                    if runtime.projection_revision()? != revision {
                        publish_mod_content(runtime, state, &self.connections, &self.observers)?;
                    }
                    if released {
                        if let (Some(party), Some(viewer)) =
                            (parties.parties.first(), state.participant_order.first())
                        {
                            prepared.released_barriers.push((party.id.clone(), viewer.clone()));
                        }
                    } else {
                        if runtime.tick(state.ticks + 1, state)? {
                            publish_mod_content(
                                runtime,
                                state,
                                &self.connections,
                                &self.observers,
                            )?;
                        }
                        prepared.private_spawns = runtime.drain_enemy_spawns()?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Each party whose skill picker a player control released has its inputs neutralized
    /// and begins a Skill Picker Closed grace for its viewer.
    fn begin_skill_picker_graces(&mut self, released_barriers: &[(String, String)], now_ms: f64) {
        for (party_id, viewer) in released_barriers {
            self.neutralize_party(party_id);
            let facts = self.control_facts(party_id);
            self.controls.entry(party_id.clone()).or_default().begin_surface_grace(
                viewer,
                ResumeGraceReason::SkillPickerClosed,
                &facts.context(),
                now_ms,
            );
            self.publish_dirty = true;
        }
    }

    /// Steps the worlds one tick: the shared College and each party run with its mod
    /// extensions, or the private College unless its party is paused.
    fn step_worlds(
        &mut self,
        inputs: &BTreeMap<String, GameSessionInput>,
        paused: &BTreeSet<String>,
        prepared: &PreparedActors,
    ) -> Result<Vec<HostedWorldTick>, String> {
        let mut result = Vec::new();
        match &mut self.worlds {
            HostedWorlds::Global(worlds) => {
                let roster = memorial_roster(&self.rejoin_slots, &self.players, &self.bots.rows);
                let mut extensions: BTreeMap<String, &dyn GameSimulationExtensions> =
                    BTreeMap::new();
                for scope in &self.party_mod_runtimes {
                    extensions.insert(scope.party_id.clone(), scope.runtime.runtime());
                }
                let stepped = worlds.step_with_extensions(
                    inputs,
                    SharedWorldsTickContext {
                        paused_party_ids: Some(paused),
                        college_intro_ready_player_ids: Some(&self.college_intro_ready),
                        navigation_by_party: Some(&self.navigation),
                        enemy_spawn_intents: Some(&prepared.enemy_spawns),
                        memorial_profiles: Some(&roster.profiles),
                        memorial_eligible_player_ids: Some(&roster.eligible),
                        run_observer: self.run_archives.as_mut().map(|archives| {
                            archives
                                as &mut dyn solomon_sim::shared_worlds::SharedWorldsTickObserver
                        }),
                    },
                    Some(&extensions),
                )?;
                if stepped.memorial_changed
                    && let Some(file) = &self.memorial_persistence
                {
                    // The Memorial is persisted before post-world rules run, and a disk
                    // failure must cross the host's fatal boundary.
                    file.persist(&worlds.hub.hub_memorial)?;
                }
                if let Some(hub) = stepped.hub {
                    result.push(HostedWorldTick { party_id: None, result: hub });
                }
                for (party_id, tick) in stepped.runs {
                    result.push(HostedWorldTick { party_id: Some(party_id), result: tick });
                }
            }
            HostedWorlds::Private { state, parties } => {
                let party_id = parties.parties.first().map(|party| party.id.clone());
                if !party_id.as_ref().is_some_and(|id| paused.contains(id)) {
                    if let Some(archives) = &mut self.run_archives {
                        archives.before_private_step(state, inputs, &prepared.private_spawns);
                    }
                    let tick = state.step_with_extensions(
                        inputs,
                        GameSessionTickContext {
                            navigation: party_id.as_ref().and_then(|id| self.navigation.get(id)),
                            college_intro_ready_player_ids: Some(&self.college_intro_ready),
                            enemy_spawn_intents: &prepared.private_spawns,
                        },
                        self.private_mod_host
                            .as_ref()
                            .map(|runtime| runtime.runtime() as &dyn GameSimulationExtensions),
                    )?;
                    if let Some(archives) = &mut self.run_archives {
                        archives.after_private_step(state);
                    }
                    result.push(HostedWorldTick { party_id, result: tick });
                }
            }
        }
        Ok(result)
    }

    fn step_prepared_rules(
        &mut self,
        before_runs: &BTreeMap<String, GameSimulationEventSnapshot>,
        before_private: Option<GameSimulationEventSnapshot>,
    ) -> Result<(), String> {
        match &mut self.worlds {
            HostedWorlds::Global(worlds) => {
                for run in &mut worlds.runs {
                    let Some(scope) = self
                        .party_mod_runtimes
                        .iter_mut()
                        .find(|scope| scope.party_id == run.party_id)
                    else {
                        continue;
                    };
                    let Some(before) = before_runs.get(&run.party_id) else { continue };
                    let revision = scope.runtime.projection_revision()?;
                    let mut events = std::mem::take(&mut scope.pending_events);
                    events.extend(derive_web_lua_events(
                        before,
                        &GameSimulationEventSnapshot::capture(&run.state),
                        |_| true,
                    ));
                    let scope_id =
                        run.state.run.run_id.as_deref().unwrap_or(&run.party_id).to_owned();
                    scope.runtime.step(
                        &events,
                        run.state.ticks,
                        &scope_id,
                        &LuaObject::new(),
                        &mut run.state,
                    )?;
                    if scope.runtime.projection_revision()? != revision {
                        publish_mod_content(
                            &scope.runtime,
                            &run.state,
                            &self.connections,
                            &self.observers,
                        )?;
                    }
                    for message in scope.runtime.drain_logs() {
                        eprintln!("mods.rule_failed party={}: {message}", scope.party_id);
                    }
                }
            }
            HostedWorlds::Private { state, .. } => {
                if let (Some(runtime), Some(before)) = (&mut self.private_mod_host, before_private)
                {
                    let revision = runtime.projection_revision()?;
                    let mut events = std::mem::take(&mut self.private_mod_events);
                    events.extend(derive_web_lua_events(
                        &before,
                        &GameSimulationEventSnapshot::capture(state),
                        |_| true,
                    ));
                    let scope_id =
                        state.run.run_id.as_deref().unwrap_or("private-session").to_owned();
                    runtime.step(&events, state.ticks, &scope_id, &LuaObject::new(), state)?;
                    if runtime.projection_revision()? != revision {
                        publish_mod_content(runtime, state, &self.connections, &self.observers)?;
                    }
                    for message in runtime.drain_logs() {
                        eprintln!("mods.rule_failed: {message}");
                    }
                }
            }
        }
        Ok(())
    }
}

fn memorial_roster(
    rejoin_slots: &[HostedRejoinSlot],
    players: &[HostedPlayer],
    bots: &[HostedBot],
) -> MemorialRoster {
    let mut profiles = BTreeMap::new();
    let mut eligible = BTreeSet::new();
    for slot in rejoin_slots {
        profiles.insert(
            slot.player_id.clone(),
            HubMemorialPlayerProfile { account_username: slot.profile.account_username.clone() },
        );
    }
    for player in players {
        profiles.insert(
            player.id.clone(),
            HubMemorialPlayerProfile { account_username: player.profile.account_username.clone() },
        );
        if player.preferences.submit_runs {
            eligible.insert(player.id.clone());
        }
    }
    // Admitted bots join both the Memorial profiles and the eligible set,
    // regardless of any human's Submit Runs preference.
    for bot in bots {
        profiles.insert(
            bot.id.clone(),
            HubMemorialPlayerProfile { account_username: bot.profile.account_username.clone() },
        );
        eligible.insert(bot.id.clone());
    }
    MemorialRoster { profiles, eligible }
}
