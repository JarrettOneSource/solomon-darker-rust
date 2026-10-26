//! Accepted host social state projected into the browser surfaces. The source
//! `MainMenuScene` owns admission; `AllyHud` reads live and retained party health.

use super::*;
use crate::social::SocialUiContext;
use solomon_sim::secondary::NativeSecondaryActorKind;
use solomon_sim::secondary::SecondaryActorRead;
use solomon_sim::social::{
    AllyGolemProjection, HubPlayerSelection, PartyRosterPlayer, derive_golem_ally_hud_rows,
    derive_player_ally_hud_rows, nearest_hub_player, select_hub_player_at_point,
};

impl Game {
    pub(super) fn update_social(&mut self, now: f64) -> Result<(), String> {
        self.update_hosted_party_mods();
        if let Some(native) = &mut self.native {
            for rejection in std::mem::take(&mut native.chat_rejections) {
                self.social.chat_rejected(rejection);
            }
        }
        let host = self.native.as_ref().and_then(NativeGame::host_snapshot);
        let snapshot = self.native.as_ref().and_then(|native| native.social.as_ref());
        let players: Vec<_> = self
            .session
            .participant_order
            .iter()
            .map(|id| {
                let player = &self.session.participants[id].player;
                PartyRosterPlayer {
                    connected: host.is_some_and(|host| host.connected_player_ids.contains(id)),
                    current_health: player.combat.current_health,
                    display_name: player.selection.name.clone(),
                    element: player.selection.element,
                    life_state: player.combat.life_state,
                    maximum_health: player.combat.maximum_health,
                    player_id: id.clone(),
                }
            })
            .collect();
        let mut allies = derive_player_ally_hud_rows(
            &players,
            &self.local_player_id,
            snapshot.and_then(|snapshot| {
                snapshot.party_state.as_ref().map(|party| party.party_roster.as_slice())
            }),
        );
        let golems: Vec<_> = self
            .session
            .secondary
            .actors
            .iter()
            .filter(|actor| actor.kind == NativeSecondaryActorKind::Golem)
            .filter_map(|actor| {
                actor.golem().map(|golem| AllyGolemProjection {
                    id: actor.id,
                    world_key: &actor.common.world_key,
                    current_health: golem.current_health,
                    maximum_health: golem.maximum_health,
                })
            })
            .collect();
        let world_key = self.session.world_key(&self.local_player_id).expect("local participant");
        allies.extend(derive_golem_ally_hud_rows(&golems, &world_key));
        let tutorial = self.session.boneyard.as_ref().is_some_and(|world| world.tutorial.is_some());
        let college_admission = self.session.boneyard.is_none()
            && self.session.participants[&self.local_player_id]
                .player
                .economy
                .college_intro_pending;
        let display = self
            .native
            .as_ref()
            .and_then(|native| native.display.as_ref())
            .map_or(&self.session, |display| display.session.as_ref());
        self.social.set_hub_players(
            self.has_game.then_some(display),
            &self.local_player_id,
            host.map(|host| &host.hub_activities),
        );
        self.social.update(
            snapshot,
            &allies,
            SocialUiContext {
                visible: self.has_game
                    && !self.is_hosted_observer()
                    && !self.menu.covers_world()
                    && !college_admission
                    && !tutorial,
                chat_enabled: self.session.run.phase != GameRunPhase::GameOver
                    && !self.cheats.is_open(),
                input_suspended: self.world_loading
                    || self.cheats.is_open()
                    || self.menu.settings_open()
                    || host.is_some_and(|host| host.resume_grace.is_some()),
                gold_clear_left: 0.0,
            },
            now,
            self.canvas.client_width(),
            self.canvas.client_height(),
        )?;
        self.menu.set_input_suspended(self.social.input_active())?;
        self.panels.set_input_suspended(self.social.input_active())?;
        for action in self.social.take_actions() {
            self.send_native_command(GameCommand::Social { action }, CommandFeedback::Social)?;
        }
        Ok(())
    }

    /// `HubScene` gives the displayed player actor precedence over an NPC. The
    /// controller selects the nearest actor; pointer selection uses feet depth.
    pub(super) fn open_hub_player(&mut self, point: Option<Vec2>) -> bool {
        let session = self.display_session();
        if !self.has_game || session.boneyard.is_some() {
            return false;
        }
        let local = &session.participants[&self.local_player_id];
        if local.wizard.transition.is_some() || local.player.economy.college_intro_pending {
            return false;
        }
        let players: Vec<_> = session
            .participant_order
            .iter()
            .map(|id| {
                let wizard = &session.participants[id].wizard;
                HubPlayerSelection {
                    player_id: id,
                    position: wizard.position,
                    region: Some(wizard.region),
                }
            })
            .collect();
        let selected = match point {
            Some(point) => select_hub_player_at_point(&players, &self.local_player_id, point),
            None => nearest_hub_player(&players, &self.local_player_id, 120.0),
        }
        .map(str::to_owned);
        selected.is_some_and(|id| self.social.open_hub_player(&id))
    }
}
