//! Ordinary spectator display selection. Hosted observers retain their explicit
//! participant selection; this owner never changes Game's local player identity.

use solomon_sim::belt::{NativeBeltEntry, PLAYER_BELT_SLOT_COUNT};
use solomon_sim::boneyard::spectator::*;
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::enemies::model::{EnemyBrain, SpiderPhase};
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::session::GameSession;

use super::Game;

fn players(session: &GameSession) -> Vec<BoneyardSpectatorPlayer<'_>> {
    session
        .participant_order
        .iter()
        .map(|id| {
            let participant = &session.participants.get(id).expect("participant exists");
            BoneyardSpectatorPlayer {
                id,
                display_name: &participant.player.selection.name,
                position: participant.wizard.position,
                life_state: participant.player.combat.life_state,
            }
        })
        .collect()
}

fn snapshot<'a>(
    session: &'a GameSession,
    players: &'a [BoneyardSpectatorPlayer<'a>],
) -> BoneyardSpectatorSnapshot<'a> {
    BoneyardSpectatorSnapshot {
        players,
        eligible_player_ids: &session.run.eligible_player_ids,
        phase: session.run.phase,
        run_id: session.run.run_id.as_deref(),
    }
}

pub(super) fn status(
    session: &GameSession,
    local_id: &str,
    state: &BoneyardSpectatorCameraState,
) -> Option<BoneyardSpectatorStatusPresentation> {
    let world = session.boneyard.as_ref()?;
    let players = players(session);
    let snapshot = snapshot(session, &players);
    boneyard_spectator_status(
        &snapshot,
        local_id,
        state,
        world
            .enemies
            .actors
            .iter()
            .filter(|actor| {
                !matches!(&actor.brain,
            EnemyBrain::Spider(brain) if brain.phase == SpiderPhase::Captured)
            })
            .count()
            + world.enemies.maggots.len(),
        world.waves.as_ref().map(|waves| BoneyardSpectatorWaves {
            pending_spawn_budget: waves.state.pending_spawn_budget,
            wave_ordinal: waves.state.wave_ordinal,
            phase: waves.state.phase,
        }),
    )
}

impl Game {
    pub(super) fn configure_spectator_input(&self) {
        let session = self.display_session();
        let player =
            &session.participants.get(&self.local_player_id).expect("participant exists").player;
        let active = session.boneyard.is_some()
            && session.run.phase == GameRunPhase::Active
            && session.run.run_id.is_some()
            && player.combat.life_state == PlayerLifeState::Spectating;
        let item_slots: [bool; PLAYER_BELT_SLOT_COUNT] = player.belt.entries.map(|entry| {
            entry.is_some_and(|entry| !matches!(entry, NativeBeltEntry::Skill { .. }))
        });
        self.input.set_spectator_camera(active, item_slots);
    }

    pub(super) fn update_spectator_camera(&mut self, advances: usize) {
        let session = self.display_session();
        if session.boneyard.is_none() {
            self.spectator_camera = BoneyardSpectatorCameraState::default();
            return;
        }
        let players = players(session);
        let snapshot = snapshot(session, &players);
        let mut state = boneyard_spectator_camera_state(
            &snapshot,
            &self.local_player_id,
            &self.spectator_camera,
            false,
        );
        for _ in 0..advances {
            state = boneyard_spectator_camera_state(&snapshot, &self.local_player_id, &state, true);
        }
        self.spectator_camera = state;
    }

    pub(super) fn boneyard_camera_focus(&self) -> BoneyardCameraFocus {
        let session = self.display_session();
        let world = session.boneyard.as_ref().expect("Boneyard camera owns a loaded scene");
        let players = players(session);
        let snapshot = snapshot(session, &players);
        // Camera queries can occur before the draw's retained-state update.
        // Source cameraFor resolves eligibility against this same display frame.
        let state = boneyard_spectator_camera_state(
            &snapshot,
            &self.local_player_id,
            &self.spectator_camera,
            false,
        );
        boneyard_camera_focus(
            &snapshot,
            &self.local_player_id,
            &state,
            world.loaded.scene.spawn.position,
        )
    }
}
