//! Where the player's aim and pointer land in the world, and the Hub
//! interactions and dialogue they reach.

use crate::courtyard::Camera;
use crate::player_panels::HubDialogueContext;
use solomon_sim::Vec2;
use solomon_sim::hub::interaction::{
    HubInteractionAvailability, HubInteractionId, hub_interaction_at_point,
    hub_interaction_within_range,
};
use solomon_sim::input::{GameplayAim, project_stick_aim, project_world_pointer};

use super::Game;

impl Game {
    pub(super) fn project_input_aim(&self, aim: Option<GameplayAim>) -> Option<Vec2> {
        let (camera, viewport) = self.scene_view();
        let wizard =
            self.display_session().wizard(&self.local_player_id).expect("local participant");
        match aim {
            None => None,
            Some(GameplayAim::Retain) => self.input_aim,
            Some(GameplayAim::Direction(direction)) => {
                project_stick_aim(direction, wizard.position, viewport.logical_size, camera.scale)
            }
            Some(GameplayAim::Facing) => project_stick_aim(
                solomon_sim::heading::heading_vector(wizard.heading_index),
                wizard.position,
                viewport.logical_size,
                camera.scale,
            ),
            Some(GameplayAim::Pointer(point)) => {
                self.pointer_to_world(point, &camera, viewport.logical_size)
            }
        }
    }

    fn pointer_to_world(&self, point: Vec2, camera: &Camera, viewport: Vec2) -> Option<Vec2> {
        let width = f64::from(self.surface.logical_width());
        let height = f64::from(self.surface.logical_height());
        project_world_pointer(
            Vec2::new(point.x * width, point.y * height),
            solomon_sim::hud::HudRect::new(0.0, 0.0, width, height),
            viewport,
            camera.origin,
            camera.scale,
        )
    }

    pub(super) fn interact_at_pointer(&mut self, now: f64) -> Result<(), String> {
        // Always drain this input edge; a click in another world cannot open a College NPC later.
        let point = self.input.take_world_click();
        if !self.has_game
            || self.session.boneyard.is_some()
            || self.panels.is_open()
            || self.cheats.is_open()
            || self.social.input_active()
            || self.menu.is_blocking()
            || self.skill_selection_active()
            || self.native_gameplay_held()
            || self.world_load.is_loading()
            || self.session.participants[&self.local_player_id].wizard.transition.is_some()
        {
            return Ok(());
        }
        let (camera, viewport) = self.scene_view();
        let Some(point) =
            point.and_then(|point| self.pointer_to_world(point, &camera, viewport.logical_size))
        else {
            return Ok(());
        };
        if self.open_hub_player(Some(point)) {
            return Ok(());
        }
        let availability = self.interaction_availability();
        let Some(interaction) = hub_interaction_at_point(
            self.session.participants[&self.local_player_id].wizard.region,
            point,
            availability,
        ) else {
            return Ok(());
        };
        if hub_interaction_within_range(
            interaction,
            self.session.participants[&self.local_player_id].wizard.region,
            self.session.participants[&self.local_player_id].wizard.position,
            availability,
        ) {
            self.open_hub_dialogue(interaction, now)?;
        }
        Ok(())
    }

    pub(super) fn interaction_availability(&self) -> HubInteractionAvailability {
        HubInteractionAvailability {
            skorcha_position: self
                .session
                .npcs
                .skorcha
                .skorcha
                .as_ref()
                .map(|skorcha| skorcha.position),
            skorcha_variant: self
                .session
                .npcs
                .skorcha
                .skorcha
                .as_ref()
                .map(|skorcha| skorcha.variant),
            story_office: self
                .has_game
                .then(|| self.session.participants[&self.local_player_id].player.as_ref())
                .is_some_and(|player| player.economy.college_intro_pending),
        }
    }

    pub(super) fn open_hub_dialogue(
        &mut self,
        interaction: HubInteractionId,
        now: f64,
    ) -> Result<(), String> {
        if !self.has_game {
            return Ok(());
        }
        let player = &self.session.participants[&self.local_player_id].player;
        if self.session.boneyard.is_some()
            || self.session.participants[&self.local_player_id].wizard.transition.is_some()
        {
            return Ok(());
        }
        // Dialogue lines are chosen from UI state, independently of the gameplay RNG.
        let random_index = if interaction == HubInteractionId::Skorcha {
            self.session.npcs.skorcha.skorcha.as_ref().map_or(0, |skorcha| skorcha.dismissal_index)
                as i64
                - 1
        } else {
            (player.economy.revision + player.economy.npc.boast.failure_sequence) as i64
        };
        let context = HubDialogueContext {
            interaction,
            story_office: player.economy.college_intro_pending,
            eulogy_override: None,
            random_index,
        };
        if interaction == HubInteractionId::ArchChancellor
            && self.session.participants[&self.local_player_id].college_intro.is_some_and(|intro| {
                intro.phase == solomon_sim::hub::college_intro::CollegeIntroPhase::ArchDialogue
            })
        {
            self.panels.show_college_intro_dialogue(context, &player.economy.npc, now)
        } else {
            self.panels.show_dialogue(context, &player.economy.npc, now)
        }
    }
}
