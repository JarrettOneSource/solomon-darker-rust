//! Admitted learned bots from `game-host.ts::queueMlBotSummon` through
//! `drainBotIntents`. Controllers own policy memory; actual actors live in Session.

mod worker;

use std::collections::BTreeMap;
use std::sync::Arc;

use solomon_modding::console::{WebLuaBotConfig, WebLuaBotResult};
use solomon_protocol::GameCommand;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::ml_bot::{
    MlBotEntranceNavigationError, MlBotHostController, MlBotHostControllerContext,
    MlBotHostControllerError, MlBotHostIntent, MlBotPolicyControllerKind,
    MlBotPolicyObservationContext, MlBotPolicyRuntime,
};
use solomon_sim::modding::{LuaObject, LuaValue};
use solomon_sim::profile::{NewGameSelection, PlayerSkillChoice};
use solomon_sim::session::GameSessionInput;
use solomon_sim::social::{GameSessionKind, PlayerSocialProfile};

use super::{HostedGameHost, HostedWorlds};
use crate::host::signing::{random_party_identity, random_token};
use worker::{BotInferenceWorker, InferenceJob};

pub(super) struct HostedBot {
    pub id: String,
    pub character: NewGameSelection,
    pub profile: PlayerSocialProfile,
    pub active_input: GameSessionInput,
    controller: MlBotHostController,
    queued_intents: Vec<MlBotHostIntent>,
}

struct PendingBotSummon {
    id: String,
    character: NewGameSelection,
}

struct PendingBotInvitation {
    accept_at_ms: f64,
    invitation_id: String,
    player_id: String,
}

#[derive(Default)]
pub(super) struct HostedBots {
    pub rows: Vec<HostedBot>,
    worker: Option<BotInferenceWorker>,
    pending_summons: Vec<PendingBotSummon>,
    pending_invitations: Vec<PendingBotInvitation>,
    failed: Vec<(String, String)>,
    next_ordinal: u64,
}

impl HostedGameHost {
    pub(in crate::host) fn install_ml_bot_policy(
        &mut self,
        policy: Arc<MlBotPolicyRuntime>,
    ) -> Result<(), String> {
        self.bots.worker = Some(BotInferenceWorker::new(policy)?);
        Ok(())
    }

    pub(in crate::host) fn bot_count(&self) -> usize {
        self.bots.rows.len()
    }

    pub(in crate::host) fn is_bot(&self, id: &str) -> bool {
        self.bots.rows.iter().any(|bot| bot.id == id)
    }

    pub(in crate::host) fn bot_players(
        &self,
    ) -> impl Iterator<Item = (&str, &str, &PlayerSocialProfile)> {
        self.bots
            .rows
            .iter()
            .map(|bot| (bot.id.as_str(), bot.character.name.as_str(), &bot.profile))
    }

    pub(super) fn queue_ml_bot_summon(
        &mut self,
        summoner_id: &str,
        config: WebLuaBotConfig,
    ) -> Result<WebLuaBotResult, String> {
        if self.kind != GameSessionKind::GlobalHub {
            return Err("sd.bots.summon requires the shared Hub".into());
        }
        if self.bots.worker.is_none() {
            return Err("The ML bot policy is unavailable".into());
        }
        if !self.player(summoner_id).is_some_and(|player| player.developer_access) {
            return Err("sd.bots.summon requires developer access".into());
        }
        if !self.worlds.state_for_player(summoner_id).is_some_and(|state| state.boneyard.is_none())
        {
            return Err("sd.bots.summon may only be called in the Hub".into());
        }
        if self.players.len() + self.bots.rows.len() + self.bots.pending_summons.len()
            >= self.maximum_players
        {
            return Err("The shared Hub is full".into());
        }
        self.bots.next_ordinal += 1;
        let id = format!("bot-{}", random_token::<12>()?);
        let character = NewGameSelection {
            name: format!("Policy Bot {}", self.bots.next_ordinal),
            element: config.element,
            discipline: config.discipline,
        };
        let result =
            WebLuaBotResult { display_name: character.name.clone(), player_id: id.clone() };
        self.bots.pending_summons.push(PendingBotSummon { id, character });
        Ok(result)
    }

    pub(in crate::host) fn schedule_bot_invitation(
        &mut self,
        inviter_id: &str,
        invited_id: &str,
        now_ms: f64,
    ) {
        if !self.is_bot(invited_id) {
            return;
        }
        let invitation = self.worlds.parties().invitations.iter().find(|invitation| {
            invitation.inviter_player_id == inviter_id && invitation.invited_player_id == invited_id
        });
        if let Some(invitation) = invitation {
            self.bots.pending_invitations.push(PendingBotInvitation {
                accept_at_ms: now_ms + 3_000.0,
                invitation_id: invitation.id.clone(),
                player_id: invited_id.into(),
            });
        }
    }

    /// Source processes asynchronous completions between epochs, then failures,
    /// actual admission, delayed invitations, controller polls and queued intents.
    pub(super) fn step_ml_bots(&mut self, now_ms: f64) -> Result<(), String> {
        let completions = self.bots.worker.as_ref().map_or_else(Vec::new, |worker| worker.drain());
        for completed in completions {
            let Some(bot) = self.bots.rows.iter_mut().find(|bot| bot.id == completed.player_id)
            else {
                continue;
            };
            match bot.controller.complete(
                completed.request_id,
                self.worlds.state_for_player(&bot.id),
                completed.result,
            ) {
                Ok(Some(intent)) if bot.queued_intents.len() < 8 => bot.queued_intents.push(intent),
                Ok(_) => {}
                Err(error) => self.bots.failed.push((bot.id.clone(), bot_error(error))),
            }
        }
        for (id, error) in std::mem::take(&mut self.bots.failed) {
            eprintln!("ml_bot.controller_failed player={id}: {error}");
            self.remove_ml_bot(&id)?;
        }
        self.process_pending_bot_summons()?;
        self.process_pending_bot_invitations(now_ms)?;
        for index in 0..self.bots.rows.len() {
            let id = &self.bots.rows[index].id;
            let state = self.worlds.state_for_player(id);
            if state.is_none_or(|state| {
                state.boneyard.is_none() || state.run.phase != GameRunPhase::Active
            }) {
                self.bots.rows[index].active_input = GameSessionInput::default();
            }
            let id = &self.bots.rows[index].id;
            let state = self.worlds.state_for_player(id);
            let mut active_inputs = BTreeMap::new();
            let mut controllers = BTreeMap::new();
            if let Some(state) = state {
                for connection in self.connections.values() {
                    if let Some(player_id) = &connection.player_id
                        && state.participants.contains_key(player_id)
                        && !self.observers.contains_key(&connection.peer.id())
                    {
                        active_inputs.insert(player_id.clone(), connection.input.active_input());
                        controllers.insert(player_id.clone(), MlBotPolicyControllerKind::Human);
                    }
                }
                for bot in &self.bots.rows {
                    if state.participants.contains_key(&bot.id) {
                        active_inputs.insert(bot.id.clone(), bot.active_input);
                        controllers.insert(bot.id.clone(), MlBotPolicyControllerKind::Bot);
                    }
                }
            }
            let compare = |left: &str, right: &str| self.source_collator.compare(left, right);
            let context = state.map(|session| MlBotHostControllerContext {
                session,
                observation: MlBotPolicyObservationContext {
                    active_inputs: &active_inputs,
                    controllers: &controllers,
                    has_consumable: None,
                    compare_player_ids: &compare,
                },
            });
            let bot = &mut self.bots.rows[index];
            match bot.controller.poll(context) {
                Ok(output) => {
                    if let Some(intent) = output.intent
                        && bot.queued_intents.len() < 8
                    {
                        bot.queued_intents.push(intent);
                    }
                    if let Some(request) = output.request
                        && let Err(error) = self
                            .bots
                            .worker
                            .as_ref()
                            .expect("an admitted bot retains its policy worker")
                            .submit(InferenceJob { player_id: bot.id.clone(), request })
                    {
                        self.bots.failed.push((bot.id.clone(), error));
                    }
                }
                Err(error) => self.bots.failed.push((bot.id.clone(), bot_error(error))),
            }
        }
        self.drain_bot_intents(now_ms)
    }

    fn process_pending_bot_summons(&mut self) -> Result<(), String> {
        let Some(worker) = &self.bots.worker else { return Ok(()) };
        let choice_mode = worker.choice_mode;
        for pending in std::mem::take(&mut self.bots.pending_summons) {
            if self.players.len() + self.bots.rows.len() >= self.maximum_players
                || self.is_bot(&pending.id)
            {
                continue;
            }
            let HostedWorlds::Global(worlds) = &mut self.worlds else { continue };
            let controller = MlBotHostController::new(
                pending.id.clone(),
                pending.character.clone(),
                choice_mode,
            )?;
            worlds.add_hub_player(
                pending.id.clone(),
                pending.character.clone(),
                random_party_identity()?,
            );
            self.bots.rows.push(HostedBot {
                id: pending.id,
                character: pending.character,
                profile: PlayerSocialProfile {
                    account_username: None,
                    highest_wave: None,
                    total_playtime_ms: None,
                },
                active_input: GameSessionInput::default(),
                controller,
                queued_intents: Vec::new(),
            });
            self.publish_dirty = true;
            self.social_dirty = true;
        }
        Ok(())
    }

    fn process_pending_bot_invitations(&mut self, now_ms: f64) -> Result<(), String> {
        for index in (0..self.bots.pending_invitations.len()).rev() {
            if self.bots.pending_invitations[index].accept_at_ms > now_ms {
                continue;
            }
            let pending = self.bots.pending_invitations.remove(index);
            if !self.is_bot(&pending.player_id) {
                continue;
            }
            let HostedWorlds::Global(worlds) = &mut self.worlds else { continue };
            let invitation = worlds
                .parties
                .invitations
                .iter()
                .find(|invitation| invitation.id == pending.invitation_id)
                .filter(|invitation| invitation.invited_player_id == pending.player_id);
            let Some(invitation) = invitation else { continue };
            let destination = invitation.party_id.clone();
            let source =
                worlds.parties.party_for_player(&pending.player_id).map(|party| party.id.clone());
            if worlds
                .accept_invitation(&pending.player_id, &pending.invitation_id, self.maximum_players)
                .is_ok()
            {
                if let Some(source) = source {
                    self.close_party_mod_runtime(&source);
                }
                self.close_party_mod_runtime(&destination);
                self.publish_dirty = true;
                self.social_dirty = true;
            }
        }
        Ok(())
    }

    fn drain_bot_intents(&mut self, now_ms: f64) -> Result<(), String> {
        for index in 0..self.bots.rows.len() {
            let id = self.bots.rows[index].id.clone();
            for intent in std::mem::take(&mut self.bots.rows[index].queued_intents) {
                let Some(state) = self.worlds.state_for_player_mut(&id) else { continue };
                match intent {
                    MlBotHostIntent::Input(input) | MlBotHostIntent::ScriptedInput(input) => {
                        self.bots.rows[index].active_input = if state.boneyard.is_some()
                            && state.run.phase == GameRunPhase::Active
                        {
                            input
                        } else {
                            GameSessionInput::default()
                        };
                    }
                    MlBotHostIntent::HubAction(action) => {
                        self.bots.rows[index].active_input = GameSessionInput::default();
                        state.apply_inventory(&id, &action);
                    }
                    MlBotHostIntent::SelectSkill { choice_index, offer_sequence, skill_id } => {
                        self.bots.rows[index].active_input = GameSessionInput::default();
                        let before_barrier = state.level_up_barrier.is_some();
                        if self
                            .apply_prepared_session_command(
                                &id,
                                GameCommand::ChooseSkill {
                                    choice: PlayerSkillChoice {
                                        choice_index,
                                        offer_sequence,
                                        skill_id,
                                    },
                                },
                            )
                            .map_err(|(_, reason)| reason)?
                            .accepted
                        {
                            if before_barrier
                                && self
                                    .worlds
                                    .state_for_player(&id)
                                    .is_some_and(|state| state.level_up_barrier.is_none())
                            {
                                let party = self.party_id_for_player(&id).map(str::to_owned);
                                if let Some(party) = party {
                                    self.neutralize_party(&party);
                                    let facts = self.control_facts(&party);
                                    self.controls.entry(party).or_default().begin_surface_grace(
                                        &id,
                                        solomon_protocol::ResumeGraceReason::SkillPickerClosed,
                                        &facts.context(),
                                        now_ms,
                                    );
                                }
                            }
                            self.publish_dirty = true;
                            self.social_dirty = true;
                        }
                    }
                    MlBotHostIntent::ConfirmLoadout(character) => {
                        self.bots.rows[index].active_input = GameSessionInput::default();
                        if let HostedWorlds::Global(worlds) = &mut self.worlds
                            && worlds.confirm_loadout(&id, character).is_ok()
                        {
                            self.publish_dirty = true;
                            self.social_dirty = true;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn remove_ml_bot(&mut self, id: &str) -> Result<(), String> {
        let Some(index) = self.bots.rows.iter().position(|bot| bot.id == id) else { return Ok(()) };
        self.bots.rows.remove(index);
        self.bots.failed.retain(|(player, _)| player != id);
        self.bots.pending_invitations.retain(|invitation| invitation.player_id != id);
        let party = self.party_id_for_player(id).map(str::to_owned);
        self.worlds.remove_player(id)?;
        if let Some(party) = party
            && !self.worlds.parties().parties.iter().any(|retained| retained.id == party)
        {
            self.close_party_mod_runtime(&party);
        }
        self.publish_dirty = true;
        self.social_dirty = true;
        Ok(())
    }

    pub(super) fn remove_all_ml_bots(&mut self) -> Result<(), String> {
        self.bots.pending_summons.clear();
        self.bots.pending_invitations.clear();
        let ids: Vec<_> = self.bots.rows.iter().map(|bot| bot.id.clone()).collect();
        for id in ids {
            self.remove_ml_bot(&id)?;
        }
        Ok(())
    }
}

fn bot_error(error: MlBotHostControllerError) -> String {
    match error {
        MlBotHostControllerError::Policy(error) => error,
        MlBotHostControllerError::Entrance(MlBotEntranceNavigationError::NoPlayer(id)) => {
            format!("Boneyard entrance navigation has no player {id}")
        }
        MlBotHostControllerError::Entrance(MlBotEntranceNavigationError::NoRoute {
            start,
            target,
        }) => {
            let point = |position: solomon_sim::Vec2| {
                let mut row = LuaObject::new();
                row.insert("x".into(), LuaValue::Number(position.x));
                row.insert("y".into(), LuaValue::Number(position.y));
                solomon_assets::json_value::stringify(&LuaValue::Object(row))
                    .expect("source route points are finite")
            };
            let target = target.map_or_else(|| "Solomon".into(), point);
            format!("ML bot found no collision-safe route to {target} from {}", point(start))
        }
    }
}
