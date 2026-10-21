//! Live prepared content scopes from `host/game-host.ts::ensurePartyModRuntimes`.
//! A scope owns executable engines, never another authoritative Session. Global
//! College visitors remain stock; the actual party receives its admitted content.

use std::collections::BTreeMap;
use std::sync::Arc;

use solomon_modding::host::PreparedModHost;
use solomon_modding::package::MaterializedWebSessionContent;
use solomon_protocol::hosted::HostedServerMessage;
use solomon_protocol::{GameCommand, HostErrorCode, ServerMessage};
use solomon_sim::boneyard::LoadedBoneyard;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::modding::LuaObject;
use solomon_sim::modding::events::{
    GameSimulationEventSnapshot, WebLuaDerivedEvent, derive_web_lua_events,
};

use super::admission::same_mods;
use super::{HostedGameHost, HostedWorlds};
use crate::host::authority::{encode, send_control, send_reliable};
use crate::host::commands::{SessionCommandResult, apply_session_command_with_extensions};
use crate::host::connection::HostConnection;
use crate::host::content::HostContent;
use crate::transport::PeerId;

pub(super) struct PartyModRuntimeScope {
    pub party_id: String,
    pub content: Arc<MaterializedWebSessionContent>,
    pub content_loader: HostContent,
    pub runtime: PreparedModHost,
    pub pending_events: Vec<WebLuaDerivedEvent>,
}

impl HostedGameHost {
    /// Source restores prepared state after the actual save/profile admission, before
    /// the first resumed epoch. An ordinary live rejoin must not rewind its owner.
    pub(super) fn initialize_admitted_mod_world(
        &mut self,
        player_id: &str,
        restore_saved: bool,
    ) -> Result<(), String> {
        let saved = restore_saved.then(|| {
            self.player(player_id).expect("admitted player exists").save_metadata.mod_state.clone()
        });
        let party_id = self
            .party_id_for_player(player_id)
            .ok_or("The admitted mod viewer has no party.")?
            .to_owned();
        match &self.worlds {
            HostedWorlds::Global(worlds)
                if worlds.runs.iter().any(|run| run.party_id == party_id)
                    && !self.party_mod_runtimes.iter().any(|scope| scope.party_id == party_id) =>
            {
                let content = self
                    .content_for_party(&party_id)
                    .ok_or("The restored party has no agreed content.")?;
                let content_loader =
                    self.content_loader.with_mod_entries(content.boneyards.clone())?;
                let HostedWorlds::Global(worlds) = &mut self.worlds else { unreachable!() };
                let run = worlds.runs.iter_mut().find(|run| run.party_id == party_id).unwrap();
                let mut runtime = PreparedModHost::prepare(&content, &mut run.state)?;
                if let Some(saved) = saved.as_ref().filter(|saved| !saved.is_empty()) {
                    runtime.restore_save_state(saved)?;
                }
                self.party_mod_runtimes.push(PartyModRuntimeScope {
                    party_id: party_id.clone(),
                    content,
                    content_loader,
                    runtime,
                    pending_events: Vec::new(),
                });
            }
            _ => {}
        }
        if let (HostedWorlds::Private { .. }, Some(runtime), Some(saved)) =
            (&self.worlds, &mut self.private_mod_host, &saved)
            && !saved.is_empty()
        {
            runtime.restore_save_state(saved)?;
        }
        if self.worlds.state_for_player(player_id).is_some_and(|state| state.boneyard.is_some()) {
            let before = GameSimulationEventSnapshot::capture(
                self.worlds.state_for_player(player_id).unwrap(),
            );
            self.activate_mod_boneyard(&party_id, &before, true)?;
        }
        if saved.is_some() {
            self.player_mut(player_id).unwrap().save_metadata.mod_state = LuaObject::new();
        }
        Ok(())
    }

    /// Source `contentForParty`: durable offline members participate in content
    /// agreement too. A leader cannot launch a different package behind their back.
    fn content_for_party(&self, party_id: &str) -> Option<Arc<MaterializedWebSessionContent>> {
        let party = self.worlds.parties().parties.iter().find(|party| party.id == party_id)?;
        let lineage = self.recovery_lineages.iter().find(|lineage| lineage.party_id == party_id);
        let leader = lineage
            .map(|lineage| &lineage.content)
            .or_else(|| self.player(&party.leader_player_id).map(|player| &player.content))?;
        for id in &party.member_player_ids {
            if self.is_bot(id) {
                if !leader.manifest.mods.is_empty() {
                    return None;
                }
                continue;
            }
            let member = self
                .player(id)
                .map(|player| &player.content)
                .or_else(|| {
                    self.rejoin_slots
                        .iter()
                        .find(|slot| slot.party_id == party_id && slot.player_id == *id)
                        .map(|slot| &slot.content)
                })
                .or_else(|| {
                    lineage
                        .filter(|lineage| {
                            lineage.party_roster.iter().any(|row| row.player_id == *id)
                        })
                        .map(|lineage| &lineage.content)
                })?;
            if !same_mods(&member.manifest.mods, &leader.manifest.mods) {
                return None;
            }
        }
        Some(leader.clone())
    }

    /// Materialize through the retained party catalog before initializing any VM.
    /// An unavailable choice therefore cannot execute `session.started`.
    pub(super) fn prepare_party_match(
        &mut self,
        player_id: &str,
        party_id: &str,
        seed: [u8; 16],
        choice_id: &str,
        run_id: &str,
    ) -> Result<(LoadedBoneyard, NavigationSet), String> {
        let content = self
            .content_for_party(party_id)
            .ok_or("Every party member must enable the same mods before the party can launch.")?;
        if let Some(scope) = self.party_mod_runtimes.iter().find(|scope| {
            scope.party_id == party_id
                && same_mods(&scope.content.manifest.mods, &content.manifest.mods)
        }) {
            return scope
                .content_loader
                .prepare_choice(seed, choice_id, run_id)
                .map_err(|_| "The selected Boneyard is unavailable.".into());
        }
        self.close_party_mod_runtime(party_id);
        let content_loader = self.content_loader.with_mod_entries(content.boneyards.clone())?;
        let loaded = content_loader
            .prepare_choice(seed, choice_id, run_id)
            .map_err(|_| "The selected Boneyard is unavailable.".to_owned())?;
        let HostedWorlds::Global(worlds) = &self.worlds else {
            return Err("A shared prepared scope requires the Global College.".into());
        };
        let mut staged = worlds
            .party_save_state_for_player(player_id)?
            .ok_or("The launching party has no canonical world.")?
            .into_owned();
        let mut runtime = PreparedModHost::prepare(&content, &mut staged)?;
        if let Some(saved) = self
            .worlds
            .parties()
            .party_for_player(player_id)
            .into_iter()
            .flat_map(|party| &party.member_player_ids)
            .filter_map(|id| self.player(id))
            .map(|player| &player.save_metadata.mod_state)
            .find(|state| !state.is_empty())
        {
            runtime.restore_save_state(saved)?;
        }
        if let Some(party) = self.worlds.parties().party_for_player(player_id) {
            let ids = party.member_player_ids.clone();
            for id in ids {
                if let Some(player) = self.player_mut(&id) {
                    player.save_metadata.mod_state = LuaObject::new();
                }
            }
        }
        self.party_mod_runtimes.push(PartyModRuntimeScope {
            party_id: party_id.into(),
            content,
            content_loader,
            runtime,
            pending_events: Vec::new(),
        });
        Ok(loaded)
    }

    pub(super) fn close_party_mod_runtime(&mut self, party_id: &str) {
        if let Some(index) =
            self.party_mod_runtimes.iter().position(|scope| scope.party_id == party_id)
        {
            let mut scope = self.party_mod_runtimes.remove(index);
            scope.runtime.close();
        }
    }

    pub(super) fn activate_mod_boneyard(
        &mut self,
        party_id: &str,
        before: &GameSimulationEventSnapshot,
        restored: bool,
    ) -> Result<(), String> {
        match &mut self.worlds {
            HostedWorlds::Global(worlds) => {
                let run = worlds
                    .runs
                    .iter_mut()
                    .find(|run| run.party_id == party_id)
                    .ok_or("The prepared party lost its actual run.")?;
                let Some(scope) =
                    self.party_mod_runtimes.iter_mut().find(|scope| scope.party_id == party_id)
                else {
                    return Ok(());
                };
                let choice_id = &run.loaded_boneyard.choice_id;
                let content_id = scope
                    .content
                    .boneyards
                    .iter()
                    .find(|entry| entry.choice.id == *choice_id)
                    .and_then(|entry| entry.web_lua_content_id.as_deref());
                scope.runtime.activate_boneyard(content_id, restored, &run.state)?;
                scope.pending_events.extend(derive_web_lua_events(
                    before,
                    &GameSimulationEventSnapshot::capture(&run.state),
                    |_| true,
                ));
            }
            HostedWorlds::Private { state, .. } => {
                if state.boneyard.as_ref().is_some_and(|world| world.tutorial.is_none())
                    && let Some(console) = &mut self.console
                {
                    console.pending_events.extend(derive_web_lua_events(
                        before,
                        &GameSimulationEventSnapshot::capture(state),
                        |name| console.runtime.wants_event(name),
                    ));
                }
                if let Some(runtime) = &mut self.private_mod_host {
                    let choice_id = &state
                        .boneyard
                        .as_ref()
                        .ok_or("The private prepared world has no Boneyard.")?
                        .loaded
                        .choice_id;
                    let content_id = self
                        .content
                        .boneyards
                        .iter()
                        .find(|entry| entry.choice.id == *choice_id)
                        .and_then(|entry| entry.web_lua_content_id.as_deref());
                    runtime.activate_boneyard(content_id, restored, state)?;
                    self.private_mod_events.extend(derive_web_lua_events(
                        before,
                        &GameSimulationEventSnapshot::capture(state),
                        |_| true,
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn mod_host_for_player(&self, id: &str) -> Option<&PreparedModHost> {
        match &self.worlds {
            HostedWorlds::Global(worlds) => {
                let party_id = worlds
                    .runs
                    .iter()
                    .find(|run| run.state.participant(id).is_some())
                    .map(|run| run.party_id.as_str())
                    .or_else(|| {
                        self.rejoin_slots
                            .iter()
                            .find(|slot| slot.player_id == id)
                            .map(|slot| slot.party_id.as_str())
                    })?;
                self.party_mod_runtimes
                    .iter()
                    .find(|scope| scope.party_id == party_id)
                    .map(|scope| &scope.runtime)
            }
            HostedWorlds::Private { .. } => self.private_mod_host.as_ref(),
        }
    }

    pub(super) fn send_prepared_mod_state(
        &self,
        connection: &HostConnection,
        viewer: &str,
    ) -> Result<(), String> {
        self.send_prepared_mod_content(connection, viewer)?;
        self.send_prepared_mod_runtime(connection, viewer)
    }

    pub(super) fn send_prepared_mod_content(
        &self,
        connection: &HostConnection,
        viewer: &str,
    ) -> Result<(), String> {
        if connection.peer.is_closed() {
            return Ok(());
        }
        let Some(runtime) = self.mod_host_for_player(viewer) else { return Ok(()) };
        send_control(
            &connection.peer,
            ServerMessage::Hosted {
                message: HostedServerMessage::ModContent { projection: runtime.project()? },
            },
        )
    }

    pub(in crate::host) fn send_boneyard_mod_content(
        &self,
        connection: &HostConnection,
        viewer: &str,
    ) -> Result<(), String> {
        let Some(runtime) = self.mod_host_for_player(viewer) else { return Ok(()) };
        send_control(
            &connection.peer,
            ServerMessage::Hosted {
                message: HostedServerMessage::ModCatalog {
                    items: runtime.runtime().content.consumables().cloned().collect(),
                },
            },
        )?;
        send_control(
            &connection.peer,
            ServerMessage::Hosted {
                message: HostedServerMessage::ModContent { projection: runtime.project()? },
            },
        )
    }

    pub(super) fn send_prepared_mod_runtime(
        &self,
        connection: &HostConnection,
        viewer: &str,
    ) -> Result<(), String> {
        if connection.peer.is_closed() {
            return Ok(());
        }
        let Some(runtime) = self.mod_host_for_player(viewer) else { return Ok(()) };
        let state = self
            .worlds
            .state_for_player(viewer)
            .or_else(|| {
                let slot = self.rejoin_slots.iter().find(|slot| slot.player_id == viewer)?;
                self.active_run(&slot.party_id, &slot.run_id)
            })
            .ok_or("The mod viewer has no actual world.")?;
        let projected = runtime.runtime_projection(Some(viewer), state)?;
        send_control(
            &connection.peer,
            ServerMessage::Hosted {
                message: HostedServerMessage::ModRuntime {
                    projection: projected.projection,
                    revision: projected.revision,
                },
            },
        )
    }

    pub(super) fn prepared_mod_save_state(&self, id: &str) -> Result<LuaObject, String> {
        self.mod_host_for_player(id)
            .map_or_else(|| Ok(LuaObject::new()), PreparedModHost::save_state)
    }

    pub(super) fn apply_prepared_session_command(
        &mut self,
        id: &str,
        command: GameCommand,
    ) -> Result<SessionCommandResult, (HostErrorCode, String)> {
        let (state, runtime) = match &mut self.worlds {
            HostedWorlds::Global(worlds) => {
                let party_id = worlds.parties.party_for_player(id).map(|party| party.id.clone());
                let state = worlds.state_for_player_mut(id).ok_or_else(|| {
                    (HostErrorCode::InvalidMessage, "The wizard has no actual world.".into())
                })?;
                let runtime = if state.boneyard.is_some() {
                    self.party_mod_runtimes
                        .iter_mut()
                        .find(|scope| Some(&scope.party_id) == party_id.as_ref())
                        .map(|scope| &mut scope.runtime)
                } else {
                    None
                };
                (state, runtime)
            }
            HostedWorlds::Private { state, .. } => (state, self.private_mod_host.as_mut()),
        };
        // Source retains stateBeforeAction only for a consumable transaction. A
        // rejected content.use restores that actual inventory/resource/RNG epoch.
        let before = (runtime.is_some()
            && matches!(
                &command,
                GameCommand::Inventory { .. } | GameCommand::ActivateBeltSlot { .. }
            ))
        .then(|| state.clone());
        let mut result = apply_session_command_with_extensions(
            state,
            id,
            command,
            runtime.as_deref().map(|host| {
                host.runtime() as &dyn solomon_sim::modding::extensions::GameSimulationExtensions
            }),
        )?;
        if let (Some(consumption), Some(runtime)) = (&result.mod_consumption, runtime) {
            match runtime.consume(consumption, state) {
                Ok(consumed) if consumed.accepted => {
                    publish_mod_content(runtime, state, &self.connections, &self.observers)
                        .map_err(|reason| (HostErrorCode::ServerError, reason))?;
                }
                Ok(_) => {
                    *state = before.expect("a mod consumption retained its transaction boundary");
                    result.accepted = false;
                    result.inventory_result = None;
                }
                Err(reason) => {
                    *state = before.expect("a mod consumption retained its transaction boundary");
                    return Err((HostErrorCode::ServerError, reason));
                }
            }
        }
        Ok(result)
    }
}

/// Publish at the source producer boundary using separate borrows of the actual
/// engine, Session and sockets. No checkpoint or mirrored world is constructed.
pub(super) fn publish_mod_content(
    runtime: &PreparedModHost,
    state: &solomon_sim::session::GameSession,
    connections: &BTreeMap<PeerId, HostConnection>,
    observers: &BTreeMap<PeerId, super::HostedObserver>,
) -> Result<(), String> {
    let message = encode(&ServerMessage::<()>::Hosted {
        message: HostedServerMessage::ModContent { projection: runtime.project()? },
    })?;
    for connection in connections.values() {
        if connection.peer.is_closed() {
            continue;
        }
        let accepted =
            connection.player_id.as_ref().is_some_and(|id| state.participants.contains_key(id))
                || observers.get(&connection.peer.id()).is_some_and(|observer| {
                    state.run.run_id.as_deref() == Some(observer.run_id.as_str())
                });
        if accepted {
            send_reliable(&connection.peer, message.clone())?;
        }
    }
    Ok(())
}
