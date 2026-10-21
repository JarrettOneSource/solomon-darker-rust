//! The source developer Console's admission, lazy VM owner and world epochs.
//! The VM is temporarily taken while its scoped native callbacks borrow this same
//! authority. No player, Session or executable runtime is cloned for a callback.

use solomon_modding::console::{
    WebLuaBotConfig, WebLuaBotResult, WebLuaExecutionRequest, WebLuaExecutionResult, WebLuaRuntime,
    WebLuaRuntimeBindings,
};
use solomon_protocol::hosted::HostedServerMessage;
use solomon_protocol::{HostErrorCode, ServerMessage};
use solomon_sim::enemies::EnemySpawnIntent;
use solomon_sim::modding::console::{
    WebLuaFrameState, apply_web_lua_commands, create_web_lua_frame_state,
};
use solomon_sim::modding::events::{
    GameSimulationEventSnapshot, WebLuaDerivedEvent, derive_web_lua_events,
};
use solomon_sim::social::GameSessionKind;

use super::{HostedGameHost, HostedWorlds};
use crate::host::authority::send_control;
use crate::host::connection::HostConnection;

pub(super) struct HostedConsole {
    pub owner_player_id: String,
    pub runtime: WebLuaRuntime,
    pub pending_events: Vec<WebLuaDerivedEvent>,
}

pub(super) struct ConsoleWorldEpoch {
    before: GameSimulationEventSnapshot,
    owner_player_id: String,
    pub party_id: Option<String>,
    pub enemy_spawn_intents: Vec<EnemySpawnIntent>,
}

struct ConsoleBindings<'a> {
    host: &'a mut HostedGameHost,
    owner_player_id: &'a str,
}

impl WebLuaRuntimeBindings for ConsoleBindings<'_> {
    fn authority_player_id(&self) -> Option<String> {
        let owner = self.host.player(self.owner_player_id)?;
        if owner.developer_access {
            Some(owner.id.clone())
        } else {
            self.host.players.first().map(|player| player.id.clone())
        }
    }

    fn frame(&self) -> WebLuaFrameState {
        let state = self
            .host
            .worlds
            .state_for_player(self.owner_player_id)
            .unwrap_or_else(|| self.host.worlds.hub());
        create_web_lua_frame_state(state, Some(self.owner_player_id))
    }

    fn summon_bot(&mut self, config: WebLuaBotConfig) -> Result<WebLuaBotResult, String> {
        self.host.queue_ml_bot_summon(self.owner_player_id, config)
    }
}

impl HostedGameHost {
    /// `client-lua-execute`: ordinary rejection is a LuaResult, while a duplicate
    /// currently pending ID is a protocol error. Enabled Cheats is deliberately
    /// absent here: Source separates UI visibility from this wire admission.
    pub(in crate::host) fn enqueue_lua_execution(
        &mut self,
        connection: &mut HostConnection,
        id: &str,
        request_id: u64,
        code: String,
    ) -> Result<(), (HostErrorCode, String)> {
        let developer = self.player(id).is_some_and(|player| player.developer_access);
        let rejection = if self.kind == GameSessionKind::GlobalHub && !developer {
            Some("Authoritative Lua is unavailable on the shared Hub host.")
        } else if !developer && self.players.first().is_none_or(|player| player.id != id) {
            Some("Only the current session host may execute authoritative Lua.")
        } else if self
            .party_id_for_player(id)
            .and_then(|party| self.controls.get(party))
            .is_some_and(|control| control.pause().is_some())
        {
            Some("Lua execution is unavailable while gameplay is paused.")
        } else {
            None
        };
        if let Some(error) = rejection {
            return lua_rejected(connection, request_id, error).map_err(server_error);
        }
        if connection.pending_lua_request_ids.contains(&request_id) {
            return Err((
                HostErrorCode::InvalidMessage,
                "Lua request ID is already pending.".into(),
            ));
        }
        if connection.pending_lua_request_ids.len() >= 8 {
            return lua_rejected(connection, request_id, "Too many Lua executions are pending.")
                .map_err(server_error);
        }
        connection.pending_lua_request_ids.insert(request_id);
        if !developer {
            let player = self.player_mut(id).expect("the admitted Console owner is retained");
            player.global_score_eligible = false;
            player.tutorial_eligible = false;
            player.save_metadata.integrity = solomon_assets::website_save::SaveIntegrity::LocalOnly;
            self.taint_ineligible_client_runs();
        }
        if self.console.as_ref().is_some_and(|console| console.owner_player_id != id) {
            connection.pending_lua_request_ids.remove(&request_id);
            return lua_rejected(
                connection,
                request_id,
                "Another developer connection owns the active Lua console.",
            )
            .map_err(server_error);
        }
        if self.console.is_none() {
            match WebLuaRuntime::new(developer) {
                Ok(runtime) => {
                    self.console = Some(HostedConsole {
                        owner_player_id: id.into(),
                        runtime,
                        pending_events: Vec::new(),
                    });
                }
                Err(error) => {
                    connection.pending_lua_request_ids.remove(&request_id);
                    return lua_rejected(connection, request_id, &error).map_err(server_error);
                }
            }
        }
        let accepted = self
            .console
            .as_mut()
            .expect("the initialized Console is retained")
            .runtime
            .enqueue_execution(WebLuaExecutionRequest { request_id, player_id: id.into(), code });
        if !accepted {
            connection.pending_lua_request_ids.remove(&request_id);
            return lua_rejected(
                connection,
                request_id,
                "The Lua execution queue is full or the code is invalid.",
            )
            .map_err(server_error);
        }
        Ok(())
    }

    pub(super) fn console_event_snapshot(&self) -> Option<GameSimulationEventSnapshot> {
        let console = self.console.as_ref()?;
        self.worlds
            .state_for_player(&console.owner_player_id)
            .map(GameSimulationEventSnapshot::capture)
    }

    pub(super) fn begin_console_epoch(
        &mut self,
        before: Option<GameSimulationEventSnapshot>,
    ) -> Result<Option<ConsoleWorldEpoch>, String> {
        let Some(mut console) = self.console.take() else { return Ok(None) };
        let Some(state) = self.worlds.state_for_player(&console.owner_player_id) else {
            self.console = Some(console);
            return Ok(None);
        };
        let before = before.unwrap_or_else(|| GameSimulationEventSnapshot::capture(state));
        let next_tick = state.ticks + 1;
        let party_id = self.party_id_for_player(&console.owner_player_id).map(str::to_owned);
        let operation: Result<(), String> = {
            let mut bindings =
                ConsoleBindings { host: self, owner_player_id: &console.owner_player_id };
            console.runtime.begin_tick(next_tick, &mut bindings)
        };
        // Restore the actual owner before propagating an execution failure so the
        // fatal host boundary can close that same VM and its pending requests.
        self.console = Some(console);
        operation?;
        let console = self.console.as_mut().expect("the Console was restored after its callback");
        let commands = console.runtime.drain_commands();
        let owner_player_id = console.owner_player_id.clone();
        self.publish_lua_completions()?;
        let applied = match &mut self.worlds {
            HostedWorlds::Global(worlds) => {
                let state = worlds
                    .state_for_player_mut(&owner_player_id)
                    .ok_or("The Console owner left its authoritative world during execution")?;
                let runtime = self
                    .party_mod_runtimes
                    .iter()
                    .find(|scope| Some(&scope.party_id) == party_id.as_ref());
                apply_web_lua_commands(
                    state,
                    &commands,
                    runtime.map(|scope| {
                        scope.runtime.runtime()
                            as &dyn solomon_sim::modding::extensions::GameSimulationExtensions
                    }),
                )
            }
            HostedWorlds::Private { state, .. } => apply_web_lua_commands(
                state,
                &commands,
                self.private_mod_host.as_ref().map(|runtime| {
                    runtime.runtime()
                        as &dyn solomon_sim::modding::extensions::GameSimulationExtensions
                }),
            ),
        };
        if let Some(seed) = applied.next_run_seed {
            self.next_lua_run_seed = Some(seed);
        }
        Ok(Some(ConsoleWorldEpoch {
            before,
            owner_player_id,
            party_id,
            enemy_spawn_intents: applied.enemy_spawn_intents,
        }))
    }

    pub(super) fn finish_console_epoch(
        &mut self,
        epoch: Option<ConsoleWorldEpoch>,
    ) -> Result<(), String> {
        let Some(epoch) = epoch else { return Ok(()) };
        let Some(state) = self.worlds.state_for_player(&epoch.owner_player_id) else {
            return Ok(());
        };
        let after = GameSimulationEventSnapshot::capture(state);
        let Some(mut console) = self.console.take() else { return Ok(()) };
        let mut events = std::mem::take(&mut console.pending_events);
        events.extend(derive_web_lua_events(&epoch.before, &after, |name| {
            console.runtime.wants_event(name)
        }));
        let operation: Result<(), String> = {
            let mut bindings =
                ConsoleBindings { host: self, owner_player_id: &console.owner_player_id };
            (|| {
                for event in events {
                    console.runtime.dispatch(&event.name, &event.payload, None, &mut bindings)?;
                }
                Ok(())
            })()
        };
        self.console = Some(console);
        operation?;
        self.publish_lua_completions()
    }

    fn publish_lua_completions(&mut self) -> Result<(), String> {
        let Some(console) = &mut self.console else { return Ok(()) };
        let connection = self
            .players
            .iter()
            .find(|player| player.id == console.owner_player_id)
            .and_then(|player| self.connections.get_mut(&player.peer_id));
        let completions = console.runtime.drain_completions();
        if let Some(connection) = connection {
            for completion in completions {
                connection.pending_lua_request_ids.remove(&completion.request_id);
                if !connection.peer.is_closed() {
                    lua_result(connection, completion.request_id, completion.result)?;
                }
            }
        }
        for log in console.runtime.drain_logs() {
            eprintln!("{} {}: {}", log.level, log.event, log.detail);
        }
        Ok(())
    }

    pub(super) fn reset_lua_runtime(&mut self) {
        if let Some(mut console) = self.console.take() {
            console.runtime.close();
        }
        self.private_mod_events.clear();
        self.next_lua_run_seed = None;
    }
}

fn lua_rejected(connection: &HostConnection, request_id: u64, error: &str) -> Result<(), String> {
    lua_result(
        connection,
        request_id,
        WebLuaExecutionResult {
            error: Some(error.into()),
            ok: false,
            output: Vec::new(),
            values: Vec::new(),
        },
    )
}

fn lua_result(
    connection: &HostConnection,
    request_id: u64,
    result: WebLuaExecutionResult,
) -> Result<(), String> {
    send_control(
        &connection.peer,
        ServerMessage::Hosted {
            message: HostedServerMessage::LuaResult {
                error: result.error,
                ok: result.ok,
                output: result.output,
                request_id,
                values: result.values,
            },
        },
    )
}

fn server_error(reason: String) -> (HostErrorCode, String) {
    (HostErrorCode::ServerError, reason)
}
