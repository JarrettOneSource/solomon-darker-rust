//! The persistent Global College and its independent party runs.
//! Reference: Website `host/shared-game-worlds.ts`. Each world uses the canonical
//! `GameSession`; party transfer preserves source state instead of making fresh players.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use crate::boneyard::LoadedBoneyard;
use crate::boneyard::navigation::NavigationSet;
use crate::enemies::EnemySpawnIntent;
use crate::game_run::GameRunPhase;
use crate::hub::memorial::{
    HubMemorialPlayerProfile, HubMemorialPortrait, archive_hub_memorial_portrait,
};
use crate::modding::extensions::GameSimulationExtensions;
use crate::profile::{NewGameSelection, SharedPlayerLevelMilestone};
use crate::session::{
    DetachedSessionParticipant, GameSession, GameSessionInput, GameSessionOptions,
    GameSessionTickContext, GameSessionTickResult,
};
use crate::social::{PartyActionRejection, PartyIdentity, PartySystem};

#[derive(Clone, Debug)]
pub struct SharedPartyRun {
    pub loaded_boneyard: LoadedBoneyard,
    pub party_id: String,
    pub state: GameSession,
}

#[derive(Clone, Debug)]
pub struct SharedGameWorlds {
    pub hub: GameSession,
    pub parties: PartySystem,
    /// The source steps runs in creation order, independently of opaque party IDs.
    pub runs: Vec<SharedPartyRun>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedWorldActionRejection {
    Party(PartyActionRejection),
    AlreadyRunning,
    RunUnavailable,
}

impl SharedWorldActionRejection {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Party(reason) => reason.id(),
            Self::AlreadyRunning => "already-running",
            Self::RunUnavailable => "run-unavailable",
        }
    }
}

impl From<PartyActionRejection> for SharedWorldActionRejection {
    fn from(reason: PartyActionRejection) -> Self {
        Self::Party(reason)
    }
}

/// Optional host observation of the existing canonical run step. Time, retained
/// diagnostic checkpoints and I/O remain in the native host, outside simulation.
pub trait SharedWorldsTickObserver {
    fn before_run_step(
        &mut self,
        state: &GameSession,
        loaded: &LoadedBoneyard,
        inputs: &BTreeMap<String, GameSessionInput>,
        intents: &[EnemySpawnIntent],
    );
    fn after_run_step(&mut self, state: &GameSession, loaded: &LoadedBoneyard);
}

/// Host-owned holds and immutable navigation are borrowed for this one source epoch.
#[derive(Default)]
pub struct SharedWorldsTickContext<'a> {
    pub paused_party_ids: Option<&'a BTreeSet<String>>,
    pub college_intro_ready_player_ids: Option<&'a BTreeSet<String>>,
    pub navigation_by_party: Option<&'a BTreeMap<String, NavigationSet>>,
    pub enemy_spawn_intents: Option<&'a BTreeMap<String, Vec<EnemySpawnIntent>>>,
    pub memorial_profiles: Option<&'a BTreeMap<String, HubMemorialPlayerProfile>>,
    pub memorial_eligible_player_ids: Option<&'a BTreeSet<String>>,
    pub run_observer: Option<&'a mut dyn SharedWorldsTickObserver>,
}

pub struct SharedWorldsTickResult {
    /// An empty College resets to its source initial state and has no actor epoch.
    pub hub: Option<GameSessionTickResult>,
    pub runs: Vec<(String, GameSessionTickResult)>,
    pub memorial_changed: bool,
}

impl SharedGameWorlds {
    #[must_use]
    pub fn new(hub_seed: u32) -> Self {
        Self { hub: Self::empty_hub(hub_seed), parties: PartySystem::default(), runs: Vec::new() }
    }

    pub fn from_hub(hub: GameSession) -> Result<Self, String> {
        if hub.boneyard.is_some() {
            return Err("shared-game initial state is not a Hub world".into());
        }
        Ok(Self { hub, parties: PartySystem::default(), runs: Vec::new() })
    }

    #[must_use]
    pub fn state_for_player(&self, player_id: &str) -> Option<&GameSession> {
        if self.hub.participant(player_id).is_some() {
            return Some(&self.hub);
        }
        for run in &self.runs {
            if run.state.participant(player_id).is_some() {
                return Some(&run.state);
            }
        }
        None
    }

    pub fn state_for_player_mut(&mut self, player_id: &str) -> Option<&mut GameSession> {
        if self.hub.participant(player_id).is_some() {
            return Some(&mut self.hub);
        }
        for run in &mut self.runs {
            if run.state.participant(player_id).is_some() {
                return Some(&mut run.state);
            }
        }
        None
    }

    pub fn replace_state_for_player(
        &mut self,
        player_id: &str,
        next: GameSession,
    ) -> Result<(), String> {
        if self.hub.participant(player_id).is_some() {
            self.hub = next;
            return Ok(());
        }
        let run = self
            .runs
            .iter_mut()
            .find(|run| run.state.participant(player_id).is_some())
            .ok_or_else(|| format!("shared-game player {player_id} has no world"))?;
        run.state = next;
        Ok(())
    }

    /// Source save projection: an active party already owns its complete world. In the
    /// College only that party is partitioned out; no unrelated visitor enters its save.
    pub fn party_save_state_for_player(
        &self,
        player_id: &str,
    ) -> Result<Option<Cow<'_, GameSession>>, String> {
        let (Some(active), Some(party)) =
            (self.state_for_player(player_id), self.parties.party_for_player(player_id))
        else {
            return Ok(None);
        };
        if active.boneyard.is_some() {
            return Ok(Some(Cow::Borrowed(active)));
        }
        Ok(Some(Cow::Owned(active.partition(&party.member_player_ids)?.selected)))
    }

    #[must_use]
    pub fn loaded_boneyard_for_player(&self, player_id: &str) -> Option<&LoadedBoneyard> {
        self.runs
            .iter()
            .find(|run| run.state.participant(player_id).is_some() && run.state.boneyard.is_some())
            .map(|run| &run.loaded_boneyard)
    }

    pub fn rejoin_party_run_player(
        &mut self,
        detached: DetachedSessionParticipant,
        party_id: &str,
        identity: PartyIdentity,
        maximum_members: usize,
        milestone: Option<&SharedPlayerLevelMilestone>,
    ) -> Result<(), SharedWorldActionRejection> {
        let player_id = &detached.participant.id;
        let existing = self.parties.party_for_player(player_id);
        if self.state_for_player(player_id).is_some()
            || existing.is_some_and(|party| party.id != party_id)
        {
            return Err(PartyActionRejection::AlreadyInParty.into());
        }
        let run = self
            .runs
            .iter_mut()
            .find(|run| {
                run.party_id == party_id
                    && run.loaded_boneyard.run_id == detached.run_id
                    && run.state.run.phase == GameRunPhase::Active
            })
            .ok_or(SharedWorldActionRejection::RunUnavailable)?;
        // Membership is committed only after the canonical rejoin accepts. Its RNG,
        // painter and entity-ID owners all remain with the actual retained run.
        let next_parties = if existing.is_none() {
            let mut next = self.parties.clone();
            next.register_player(player_id, identity);
            next.join(player_id, party_id, maximum_members)?;
            Some(next)
        } else {
            None
        };
        run.state
            .rejoin_participant(detached, milestone)
            .map_err(|_| SharedWorldActionRejection::RunUnavailable)?;
        if let Some(next) = next_parties {
            self.parties = next;
        }
        Ok(())
    }

    pub fn add_hub_player(
        &mut self,
        id: String,
        selection: NewGameSelection,
        identity: PartyIdentity,
    ) {
        if self.state_for_player(&id).is_some() {
            return;
        }
        self.hub.add_participant(id.clone(), selection);
        self.parties.register_player(&id, identity);
    }

    /// A saved continuation owns exactly one unique actor; its world joins the resident
    /// College or becomes that actor's independent party run, as the source specifies.
    pub fn restore_player(
        &mut self,
        restored: GameSession,
        loaded: Option<LoadedBoneyard>,
        player_id: &str,
        identity: PartyIdentity,
    ) -> Result<(), String> {
        if self.state_for_player(player_id).is_some()
            || restored.participant_order.as_slice() != [player_id]
        {
            return Err("shared-game restore requires one unique matching player".into());
        }
        if restored.boneyard.is_none() {
            self.hub.merge_hub(&restored)?;
            self.parties.register_player(player_id, identity);
            return Ok(());
        }
        let loaded =
            loaded.ok_or("shared-game Boneyard restore requires matching loaded content")?;
        if restored.boneyard.as_ref().unwrap().loaded.run_id != loaded.run_id {
            return Err("shared-game Boneyard restore requires matching loaded content".into());
        }
        self.parties.register_player(player_id, identity);
        let party_id = self.parties.party_for_player(player_id).unwrap().id.clone();
        self.runs.push(SharedPartyRun { loaded_boneyard: loaded, party_id, state: restored });
        Ok(())
    }

    pub fn remove_player(&mut self, player_id: &str) -> Result<(), String> {
        self.release_player(player_id, true)
    }

    /// Losing a socket removes its actor, while its actual party membership and leader
    /// role remain available for the host's detached continuation and rejoin admission.
    pub fn detach_player(&mut self, player_id: &str) -> Result<(), String> {
        self.release_player(player_id, false)
    }

    fn release_player(&mut self, player_id: &str, remove_membership: bool) -> Result<(), String> {
        self.hub.remove_participant(player_id);
        let mut retained = Vec::with_capacity(self.runs.len());
        for mut run in self.runs.drain(..) {
            if run.state.participant(player_id).is_none() {
                retained.push(run);
                continue;
            }
            run.state.remove_participant(player_id);
            if run.state.participants.is_empty() {
                continue;
            }
            if run.state.run.phase == GameRunPhase::Hub {
                self.hub.merge_hub(&run.state)?;
            } else {
                retained.push(run);
            }
        }
        self.runs = retained;
        if remove_membership {
            self.parties.remove_player(player_id);
        }
        Ok(())
    }

    pub fn invite(
        &mut self,
        inviter_id: &str,
        invited_id: &str,
        maximum_members: usize,
    ) -> Result<(), SharedWorldActionRejection> {
        self.require_hub_player(inviter_id)?;
        self.require_hub_player(invited_id)?;
        self.parties.invite(inviter_id, invited_id, maximum_members)?;
        Ok(())
    }

    pub fn accept_invitation(
        &mut self,
        player_id: &str,
        invitation_id: &str,
        maximum_members: usize,
    ) -> Result<(), SharedWorldActionRejection> {
        self.require_hub_player(player_id)?;
        self.parties.accept_invitation(player_id, invitation_id, maximum_members)?;
        Ok(())
    }

    pub fn deny_invitation(
        &mut self,
        player_id: &str,
        invitation_id: &str,
    ) -> Result<(), SharedWorldActionRejection> {
        self.require_hub_player(player_id)?;
        self.parties.deny_invitation(player_id, invitation_id)?;
        Ok(())
    }

    pub fn join_party(
        &mut self,
        player_id: &str,
        party_id: &str,
        maximum_members: usize,
    ) -> Result<(), SharedWorldActionRejection> {
        self.require_hub_player(player_id)?;
        self.parties.join(player_id, party_id, maximum_members)?;
        Ok(())
    }

    pub fn leave_party(
        &mut self,
        player_id: &str,
        identity: PartyIdentity,
    ) -> Result<(), SharedWorldActionRejection> {
        self.require_hub_player(player_id)?;
        self.parties.leave(player_id, identity)?;
        Ok(())
    }

    pub fn kick_party_player(
        &mut self,
        leader_id: &str,
        target_id: &str,
        identity: PartyIdentity,
    ) -> Result<(), SharedWorldActionRejection> {
        self.require_hub_player(leader_id)?;
        self.require_hub_player(target_id)?;
        self.parties.kick(leader_id, target_id, identity)?;
        Ok(())
    }

    pub fn start_party_run(
        &mut self,
        leader_id: &str,
        loaded: LoadedBoneyard,
    ) -> Result<(), SharedWorldActionRejection> {
        let party = self
            .parties
            .party_for_player(leader_id)
            .filter(|party| party.leader_player_id == leader_id)
            .ok_or(PartyActionRejection::NotLeader)?;
        if self.runs.iter().any(|run| run.party_id == party.id) {
            return Err(SharedWorldActionRejection::AlreadyRunning);
        }
        if party.member_player_ids.iter().any(|id| self.hub.participant(id).is_none()) {
            return Err(PartyActionRejection::NotInHub.into());
        }
        if party
            .member_player_ids
            .iter()
            .any(|id| self.hub.participants[id].wizard.transition.is_some())
        {
            return Err(SharedWorldActionRejection::RunUnavailable);
        }
        let party_id = party.id.clone();
        let partition = self
            .hub
            .partition(&party.member_player_ids)
            .expect("party membership contains unique existing Hub actors");
        if partition.selected.level_up_barrier.is_some() {
            return Err(SharedWorldActionRejection::RunUnavailable);
        }
        let mut run = partition.selected;
        run.set_enhanced_effects(party.enhanced_effects);
        run.install_boneyard(loaded.clone())
            .expect("an admitted party enters its source Boneyard from the Hub");
        self.hub = partition.remaining;
        self.parties.clear_invitations(&party_id);
        self.runs.push(SharedPartyRun { loaded_boneyard: loaded, party_id, state: run });
        Ok(())
    }

    pub fn confirm_loadout(
        &mut self,
        player_id: &str,
        selection: NewGameSelection,
    ) -> Result<(), SharedWorldActionRejection> {
        if self.hub.confirm_loadout(player_id, selection.clone()) {
            return Ok(());
        }
        let party = self
            .parties
            .party_for_player(player_id)
            .ok_or(SharedWorldActionRejection::RunUnavailable)?;
        let index = self
            .runs
            .iter()
            .position(|run| run.party_id == party.id)
            .ok_or(SharedWorldActionRejection::RunUnavailable)?;
        let run = &mut self.runs[index].state;
        if !run.confirm_loadout(player_id, selection) {
            return Err(SharedWorldActionRejection::RunUnavailable);
        }
        if run.run.phase == GameRunPhase::Hub {
            self.hub.merge_hub(run).expect("returning party actors are absent from the shared Hub");
            self.runs.remove(index);
        }
        Ok(())
    }

    pub fn continue_game_over(
        &mut self,
        player_id: &str,
        run_id: &str,
        event_id: u64,
    ) -> Result<(), SharedWorldActionRejection> {
        let party = self
            .parties
            .party_for_player(player_id)
            .ok_or(SharedWorldActionRejection::RunUnavailable)?;
        let run = self
            .runs
            .iter_mut()
            .find(|run| run.party_id == party.id && run.state.participant(player_id).is_some())
            .ok_or(SharedWorldActionRejection::RunUnavailable)?;
        if !run.state.continue_game_over(run_id, event_id) {
            return Err(SharedWorldActionRejection::RunUnavailable);
        }
        Ok(())
    }

    /// One shared source tick: College first, then each unpaused party in creation order.
    /// Per-run results retain real contact/reward/audio outputs for their host owner.
    pub fn step(
        &mut self,
        inputs: &BTreeMap<String, GameSessionInput>,
        context: SharedWorldsTickContext<'_>,
    ) -> SharedWorldsTickResult {
        self.step_with_extensions(inputs, context, None)
            .expect("stock shared worlds have finite native mutations")
    }

    /// Source `stepSharedGameWorlds` selects the admitted extension owner separately
    /// for each party. The College stays stock; a callback error ends this host epoch.
    pub fn step_with_extensions(
        &mut self,
        inputs: &BTreeMap<String, GameSessionInput>,
        mut context: SharedWorldsTickContext<'_>,
        extensions_by_party: Option<&BTreeMap<String, &dyn GameSimulationExtensions>>,
    ) -> Result<SharedWorldsTickResult, String> {
        assert!(self.hub.boneyard.is_none(), "shared-game Hub owner is not a Hub world");
        let hub = if self.hub.participants.is_empty() {
            if self.hub.ticks != 0 {
                let seed = self.hub.hub_trader_animation_seed;
                let windows = self.hub.npcs.skorcha.fixed_windows();
                let memorial = self.hub.hub_memorial.clone();
                self.hub = Self::empty_hub(seed);
                self.hub.hub_memorial = memorial;
                self.hub.npcs.skorcha =
                    crate::hub::npc::HubSkorchaSchedule::with_windows(seed, windows.0, windows.1);
            }
            None
        } else {
            Some(self.hub.step(
                inputs,
                GameSessionTickContext {
                    college_intro_ready_player_ids: context.college_intro_ready_player_ids,
                    ..GameSessionTickContext::default()
                },
            ))
        };
        let mut results = Vec::new();
        let mut memorial_changed = false;
        for run in &mut self.runs {
            if context.paused_party_ids.is_some_and(|paused| paused.contains(&run.party_id)) {
                continue;
            }
            let navigation =
                context.navigation_by_party.and_then(|navigation| navigation.get(&run.party_id));
            let intents = context
                .enemy_spawn_intents
                .and_then(|intents| intents.get(&run.party_id))
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            if let Some(observer) = context.run_observer.as_deref_mut() {
                observer.before_run_step(&run.state, &run.loaded_boneyard, inputs, intents);
            }
            let previous_run_id =
                run.state.boneyard.as_ref().map(|world| world.loaded.run_id.clone());
            let mut newly_completed = Vec::new();
            for id in &run.state.participant_order {
                if run.state.participants[id].score.elapsed_ticks.is_none() {
                    newly_completed.push(id.clone());
                }
            }
            let extensions =
                extensions_by_party.and_then(|extensions| extensions.get(&run.party_id)).copied();
            let result = run.state.step_with_extensions(
                inputs,
                GameSessionTickContext {
                    navigation,
                    college_intro_ready_player_ids: context.college_intro_ready_player_ids,
                    enemy_spawn_intents: intents,
                },
                extensions,
            )?;
            if let Some(world) = &run.state.boneyard
                && previous_run_id.as_deref() == Some(world.loaded.run_id.as_str())
            {
                // Source Object.keys(...).sort() captures simultaneous completions by
                // player ID, not actor enrollment order. Eligibility precedes the draw.
                newly_completed.sort();
                for id in newly_completed {
                    if context
                        .memorial_eligible_player_ids
                        .is_some_and(|eligible| !eligible.contains(&id))
                    {
                        continue;
                    }
                    let participant = &run.state.participants[&id];
                    let score = &participant.score;
                    let (Some(elapsed_ticks), Some(heading_index), Some(portrait_scale)) =
                        (score.elapsed_ticks, score.portrait_heading_index, score.portrait_scale)
                    else {
                        continue;
                    };
                    let portrait = HubMemorialPortrait {
                        account_username: context
                            .memorial_profiles
                            .and_then(|profiles| profiles.get(&id))
                            .and_then(|profile| profile.account_username.clone()),
                        awesomeness: score.awesomeness,
                        awesomest_kill: score.awesomest_kill.clone(),
                        captured_at_tick: run.state.ticks as f64,
                        config: participant.player.selection.clone(),
                        elapsed_ticks,
                        equipment:
                            crate::appearance::equipment::player_living_native_equipment_appearance(
                                participant.player.selection.element,
                                &participant.player.economy.equipment,
                            ),
                        heading_index,
                        level: participant.player.progression.level,
                        monsters_killed: score.monsters_killed,
                        player_id: id,
                        portrait_scale,
                        run_id: world.loaded.run_id.clone(),
                        wave: world.waves.as_ref().map_or(0, |waves| waves.state.wave_ordinal),
                    };
                    let marker = run.state.gameplay_rng.draw_integer(5) as u32;
                    memorial_changed |= archive_hub_memorial_portrait(
                        &mut self.hub.hub_memorial,
                        &portrait,
                        marker,
                    );
                }
            }
            if let Some(observer) = context.run_observer.as_deref_mut() {
                observer.after_run_step(&run.state, &run.loaded_boneyard);
            }
            results.push((run.party_id.clone(), result));
        }
        Ok(SharedWorldsTickResult { hub, runs: results, memorial_changed })
    }

    fn require_hub_player(&self, id: &str) -> Result<(), SharedWorldActionRejection> {
        if self.hub.participant(id).is_none() {
            return Err(PartyActionRejection::NotInHub.into());
        }
        Ok(())
    }

    fn empty_hub(seed: u32) -> GameSession {
        GameSession::new_participants(
            Vec::new(),
            GameSessionOptions {
                gameplay_rng_seed: i64::from(seed),
                hub_trader_animation_seed: seed,
                ..GameSessionOptions::default()
            },
        )
        .expect("an empty shared Hub has no duplicate identities")
    }
}
