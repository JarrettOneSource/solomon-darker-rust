//! The two source College ownership models. Every actor epoch still belongs to the
//! existing `GameSession`; this module selects which actual session owns a participant.

use solomon_sim::boneyard::LoadedBoneyard;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::profile::NewGameSelection;
use solomon_sim::session::{GameSession, GameSessionOptions};
use solomon_sim::shared_worlds::SharedGameWorlds;
use solomon_sim::social::{GameSessionKind, PartyIdentity, PartySystem};

pub enum HostedWorlds {
    Global(SharedGameWorlds),
    Private { state: GameSession, parties: PartySystem },
}

impl HostedWorlds {
    pub fn new(kind: GameSessionKind, seed: u32) -> Result<Self, String> {
        match kind {
            GameSessionKind::GlobalHub => Ok(Self::Global(SharedGameWorlds::new(seed))),
            GameSessionKind::PrivateCollege | GameSessionKind::Standalone => Ok(Self::Private {
                state: GameSession::new_participants(
                    Vec::new(),
                    GameSessionOptions {
                        gameplay_rng_seed: i64::from(seed),
                        hub_trader_animation_seed: seed,
                        ..GameSessionOptions::default()
                    },
                )?,
                parties: PartySystem::default(),
            }),
        }
    }

    pub fn parties(&self) -> &PartySystem {
        match self {
            Self::Global(worlds) => &worlds.parties,
            Self::Private { parties, .. } => parties,
        }
    }

    pub fn parties_mut(&mut self) -> &mut PartySystem {
        match self {
            Self::Global(worlds) => &mut worlds.parties,
            Self::Private { parties, .. } => parties,
        }
    }

    pub fn state_for_player(&self, id: &str) -> Option<&GameSession> {
        match self {
            Self::Global(worlds) => worlds.state_for_player(id),
            Self::Private { state, .. } => state.participant(id).map(|_| state),
        }
    }

    pub fn state_for_player_mut(&mut self, id: &str) -> Option<&mut GameSession> {
        match self {
            Self::Global(worlds) => worlds.state_for_player_mut(id),
            Self::Private { state, .. } => {
                if state.participant(id).is_some() {
                    Some(state)
                } else {
                    None
                }
            }
        }
    }

    pub fn hub(&self) -> &GameSession {
        match self {
            Self::Global(worlds) => &worlds.hub,
            Self::Private { state, .. } => state,
        }
    }

    pub fn participant_count(&self) -> usize {
        match self {
            Self::Global(worlds) => {
                worlds.hub.participants.len()
                    + worlds.runs.iter().map(|run| run.state.participants.len()).sum::<usize>()
            }
            Self::Private { state, .. } => state.participants.len(),
        }
    }

    pub fn add_player(
        &mut self,
        id: String,
        selection: NewGameSelection,
        identity: PartyIdentity,
        maximum_players: usize,
    ) -> Result<(), String> {
        match self {
            Self::Global(worlds) => worlds.add_hub_player(id, selection, identity),
            Self::Private { state, parties } => {
                if state.boneyard.is_some() || state.run.phase != GameRunPhase::Hub {
                    return Err("A new wizard must join the College before its match starts".into());
                }
                let leader_party = parties.parties.first().map(|party| party.id.clone());
                state.add_participant(id.clone(), selection);
                parties.register_player(&id, identity);
                if let Some(party_id) = leader_party {
                    // Reservation admission checked capacity before the actor was created.
                    // Keep both owners atomic if a stale reservation is nevertheless rejected.
                    if let Err(reason) = parties.join(&id, &party_id, maximum_players) {
                        state.remove_participant(&id);
                        parties.remove_player(&id);
                        return Err(reason.text().into());
                    }
                }
            }
        }
        Ok(())
    }

    pub fn restore_player(
        &mut self,
        state: GameSession,
        loaded: Option<LoadedBoneyard>,
        id: &str,
        identity: PartyIdentity,
    ) -> Result<(), String> {
        match self {
            Self::Global(worlds) => worlds.restore_player(state, loaded, id, identity),
            Self::Private { state: current, parties } => {
                if !current.participants.is_empty() || current.boneyard.is_some() {
                    return Err("A save may load only on a fresh host owner.".into());
                }
                *current = state;
                parties.register_player(id, identity);
                Ok(())
            }
        }
    }

    pub fn remove_player(&mut self, id: &str) -> Result<(), String> {
        match self {
            Self::Global(worlds) => worlds.remove_player(id),
            Self::Private { state, parties } => {
                state.remove_participant(id);
                parties.remove_player(id);
                Ok(())
            }
        }
    }

    pub fn loaded_for_player(&self, id: &str) -> Option<&LoadedBoneyard> {
        match self {
            Self::Global(worlds) => worlds.loaded_boneyard_for_player(id),
            Self::Private { state, .. } => state.boneyard.as_ref().map(|world| &world.loaded),
        }
    }

    pub fn loaded_for_run(&self, run_id: &str) -> Option<&LoadedBoneyard> {
        match self {
            Self::Global(worlds) => worlds
                .runs
                .iter()
                .filter_map(|run| run.state.boneyard.as_ref())
                .find(|world| world.loaded.run_id == run_id)
                .map(|world| &world.loaded),
            Self::Private { state, .. } => state
                .boneyard
                .as_ref()
                .filter(|world| world.loaded.run_id == run_id)
                .map(|world| &world.loaded),
        }
    }
}
