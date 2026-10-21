//! Source `game-host.ts` authored cast/action admission. Rejected requests consume
//! only their shared authored request cursor and log a warning; they never use the
//! independent Console result lane or invent a successful authored receipt.

use solomon_modding::engines::scenes::ModPortalActivation;
use solomon_modding::host::{PreparedModCastInput, PreparedModHost, PreparedModUiActionInput};
use solomon_protocol::ResumeGraceReason;
use solomon_protocol::hosted::ModAction;
use solomon_sim::modding::{LuaObject, LuaValue};
use solomon_sim::session::GameSession;

use super::mods::publish_mod_content;
use super::{HostedGameHost, HostedWorlds};
use crate::host::connection::HostConnection;

/// A short borrow of the real per-world owners. Nothing here survives the request.
struct PreparedPlayerWorld<'a> {
    runtime: &'a mut PreparedModHost,
    state: &'a mut GameSession,
}

pub(in crate::host) struct HostedModActionRequest<'a> {
    pub action: ModAction,
    pub arguments: LuaObject,
    pub request_id: u64,
    pub target: &'a str,
}

impl HostedGameHost {
    fn prepared_player_world_mut(&mut self, player_id: &str) -> Option<PreparedPlayerWorld<'_>> {
        match &mut self.worlds {
            HostedWorlds::Global(worlds) => {
                let run = worlds
                    .runs
                    .iter_mut()
                    .find(|run| run.state.participant(player_id).is_some())?;
                let scope = self
                    .party_mod_runtimes
                    .iter_mut()
                    .find(|scope| scope.party_id == run.party_id)?;
                Some(PreparedPlayerWorld { runtime: &mut scope.runtime, state: &mut run.state })
            }
            HostedWorlds::Private { state, .. } => {
                state.participant(player_id)?;
                Some(PreparedPlayerWorld { runtime: self.private_mod_host.as_mut()?, state })
            }
        }
    }

    pub(in crate::host) fn apply_mod_cast(
        &mut self,
        connection: &mut HostConnection,
        player_id: &str,
        content_id: &str,
        request_id: u64,
        target_x: f64,
        target_y: f64,
    ) -> Result<(), String> {
        if request_id <= connection.last_mod_request_id {
            return Ok(());
        }
        connection.last_mod_request_id = request_id;
        let Some(world) = self.prepared_player_world_mut(player_id) else { return Ok(()) };
        let mut context = LuaObject::new();
        context.insert("target_x".into(), LuaValue::Number(target_x));
        context.insert("target_y".into(), LuaValue::Number(target_y));
        let result = world
            .runtime
            .cast(PreparedModCastInput { content_id, context, player_id, request_id }, world.state);
        match result {
            Ok(result) if result.accepted => {
                self.publish_mod_action_content(connection, player_id)?;
                self.publish_dirty = true;
            }
            Ok(_) => {}
            Err(reason) => {
                eprintln!("mods.cast_rejected player={player_id} content={content_id}: {reason}")
            }
        }
        Ok(())
    }

    pub(in crate::host) fn apply_mod_action(
        &mut self,
        connection: &mut HostConnection,
        player_id: &str,
        request: HostedModActionRequest<'_>,
        now_ms: f64,
    ) -> Result<(), String> {
        let HostedModActionRequest { action, arguments, request_id, target } = request;
        if request_id <= connection.last_mod_request_id {
            return Ok(());
        }
        connection.last_mod_request_id = request_id;
        let party = self.worlds.parties().party_for_player(player_id).cloned();
        let leader = party.as_ref().is_some_and(|party| party.leader_player_id == player_id);
        let Some(world) = self.prepared_player_world_mut(player_id) else { return Ok(()) };
        let barrier_before = world.state.level_up_barrier.is_some();
        let accepted =
            apply_action(world, player_id, action, &arguments, request_id, target, leader);
        if let Err(reason) = accepted {
            eprintln!("mods.action_rejected action={action:?} player={player_id}: {reason}");
            return Ok(());
        }
        let barrier_closed = action == ModAction::SkillChoose
            && barrier_before
            && self
                .worlds
                .state_for_player(player_id)
                .is_some_and(|state| state.level_up_barrier.is_none());
        if let Some(party) = party {
            if action == ModAction::PortalEnter || barrier_closed {
                connection.input.neutralize();
                self.neutralize_party(&party.id);
            }
            if barrier_closed {
                let facts = self.control_facts(&party.id);
                self.controls.entry(party.id).or_default().begin_surface_grace(
                    player_id,
                    ResumeGraceReason::SkillPickerClosed,
                    &facts.context(),
                    now_ms,
                );
            }
        }
        self.publish_mod_action_content(connection, player_id)?;
        self.publish_dirty = true;
        self.pending_player_saves.insert(player_id.into());
        Ok(())
    }

    fn publish_mod_action_content(
        &self,
        connection: &HostConnection,
        player_id: &str,
    ) -> Result<(), String> {
        if let (Some(runtime), Some(state)) =
            (self.mod_host_for_player(player_id), self.worlds.state_for_player(player_id))
        {
            publish_mod_content(runtime, state, &self.connections, &self.observers)?;
            // The supervisor has temporarily removed this currently dispatched peer
            // from its socket map, so its same-world broadcast is completed here.
            self.send_prepared_mod_content(connection, player_id)?;
        }
        Ok(())
    }
}

fn apply_action(
    world: PreparedPlayerWorld<'_>,
    player_id: &str,
    action: ModAction,
    arguments: &LuaObject,
    request_id: u64,
    target: &str,
    leader: bool,
) -> Result<(), String> {
    match action {
        ModAction::SkillChoose => {
            let eligible =
                world.runtime.runtime().content.skill(target).is_some_and(|definition| {
                    world.state.participants[player_id].player.progression.level
                        >= definition.minimum_level
                });
            if !eligible {
                return Err("mod skill is not eligible".into());
            }
            let mod_sequence =
                integer(arguments, "mod_offer_sequence", "mod skill offer is invalid")?;
            let native_sequence =
                integer(arguments, "native_offer_sequence", "mod skill offer is invalid")?;
            world.runtime.choose_skill(
                player_id,
                target,
                mod_sequence,
                native_sequence,
                world.state,
            )?;
        }
        ModAction::UiAction => {
            let action = arguments
                .get("action")
                .and_then(LuaValue::as_str)
                .ok_or("mod UI action is invalid")?;
            let arguments = arguments
                .get("arguments")
                .and_then(LuaValue::as_object)
                .ok_or("mod UI action is invalid")?
                .clone();
            world.runtime.ui_action(
                PreparedModUiActionInput {
                    action,
                    arguments,
                    content_id: target,
                    player_id,
                    request_id,
                },
                world.state,
            )?;
        }
        ModAction::QuickbarBind => {
            let slot = integer(arguments, "slot", "mod quickbar slot is invalid")?;
            let slot = u32::try_from(slot).map_err(|_| "mod quickbar slot is invalid")?;
            let content = if arguments.get("clear").and_then(LuaValue::as_bool) == Some(true) {
                None
            } else {
                Some(target)
            };
            world.runtime.bind_mod_quickbar(player_id, slot, content)?;
        }
        ModAction::ShopBuy => {
            let row = integer(arguments, "row", "mod shop row is invalid")?;
            world.runtime.purchase_shop(player_id, target, row as usize, world.state)?;
        }
        ModAction::Reforge => {
            let item_id = integer(arguments, "item_id", "mod reforge request is invalid")?;
            let service = integer(arguments, "service", "mod reforge request is invalid")?;
            world.runtime.reforge_shop(
                player_id,
                target,
                service as usize,
                item_id,
                world.state,
            )?;
        }
        ModAction::PortalEnter => {
            let owner = world.state.run.run_id.clone().unwrap_or_else(|| player_id.into());
            let position = world.state.participants[player_id].wizard.position;
            let monument = world.state.boneyard.as_ref().is_some_and(|world| {
                world.loaded.scene.objects.iter().any(|object| {
                    let x = object.position.x - position.x;
                    let y = object.position.y - position.y;
                    object.type_id == 2009 && x * x + y * y < 100.0 * 100.0
                })
            });
            if !monument {
                return Err("no Boneyard monument is in interaction range".into());
            }
            world.runtime.enter_portal(
                ModPortalActivation {
                    actor_kind: "monument",
                    confirmed_by_leader: leader,
                    owner_id: &owner,
                    portal_id: target,
                    scene: "stock.boneyard",
                },
                player_id,
                world.state,
            )?;
        }
        ModAction::SceneRoom => {
            if !leader {
                return Err("only the party leader can change rooms".into());
            }
            let room = integer(arguments, "room", "mod scene room is invalid")?;
            let owner = world.state.run.run_id.clone().unwrap_or_else(|| player_id.into());
            world.runtime.select_scene_room(&owner, room as usize, world.state)?;
        }
        ModAction::SceneReturn => {
            if !leader {
                return Err("only the party leader can return the party".into());
            }
            let owner = world.state.run.run_id.as_deref().unwrap_or(player_id);
            world.runtime.return_scene(owner)?;
        }
    }
    Ok(())
}

fn integer(arguments: &LuaObject, field: &str, reason: &str) -> Result<u64, String> {
    arguments
        .get(field)
        .and_then(LuaValue::as_number)
        .filter(|number| {
            number.is_finite()
                && number.fract() == 0.0
                && *number >= 0.0
                && *number <= 9_007_199_254_740_991.0
        })
        .map(|number| number as u64)
        .ok_or_else(|| reason.into())
}
