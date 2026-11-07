//! Combat sounds and the music scene for the current frame.

use solomon_sim::Vec2;
use solomon_sim::audio::MusicScene;
use solomon_sim::audio::combat::{CombatAudioCursor, CombatAudioFrame, CombatAudioPlayer};

use super::Game;

impl Game {
    pub(super) fn update_combat_audio(&mut self) -> Result<(), String> {
        if !self.has_game {
            return Ok(());
        }
        let session = &self.session;
        let world_keys: Vec<_> = session
            .participant_order
            .iter()
            .map(|id| session.world_key(id).expect("ordered participant"))
            .collect();
        let players: Vec<_> = session
            .participant_order
            .iter()
            .zip(&world_keys)
            .map(|(id, world_key)| {
                let participant = &session.participants[id];
                CombatAudioPlayer {
                    id,
                    world_key,
                    position: participant.wizard.position,
                    element: participant.player.selection.element,
                    selected_primary_skill_id: participant.player.book.primary_skill_id as u32,
                    weld_build_id: participant.player.book.weld_build_id,
                    cast: &participant.wizard.primary_cast,
                    footstep_tick: participant.wizard.footstep_tick,
                }
            })
            .collect();
        let staff_contacts: Vec<_> = session
            .spells
            .transients
            .iter()
            .filter_map(|effect| {
                if let solomon_sim::spells::model::PrimarySpellTransient::Staff(
                    solomon_sim::spells::staff::PlayerStaffTransient::Contact(event),
                ) = effect
                {
                    Some(event.clone())
                } else {
                    None
                }
            })
            .collect();
        let (camera, viewport) = self.scene_view();
        let center = Vec2::new(
            camera.origin.x + viewport.logical_size.x / (2.0 * camera.scale),
            camera.origin.y + viewport.logical_size.y / (2.0 * camera.scale),
        );
        let width = viewport.logical_size.x / camera.scale;
        let alternate = session.participants[&self.local_player_id].player.combat.life_state
            != solomon_sim::combat::PlayerLifeState::Alive;
        let point_gain = |point| {
            solomon_sim::region_feedback::region_point_gain(point, center, width, alternate)
        };
        let hit_gain = |point| {
            solomon_sim::region_feedback::region_hit_point_gain(point, center, width, alternate)
        };
        let world = session.boneyard.as_ref();
        let frame = CombatAudioFrame {
            tick: session.ticks,
            run_id: world.map(|world| world.loaded.run_id.as_str()),
            listener_id: &self.local_player_id,
            players: &players,
            primary: &session.spells,
            secondary: &session.secondary,
            staff_contacts: &staff_contacts,
            enemies: world.map(|world| &world.enemies),
            enemy_events: world.map_or(&[], |world| world.enemy_events.as_slice()),
            loot_events: world.map_or(&[], |world| world.loot_events.as_slice()),
            encounter: world.and_then(|world| world.encounter.as_ref()),
            point_gain: &point_gain,
            hit_gain: &hit_gain,
        };
        let cursor = self.combat_audio.get_or_insert_with(|| CombatAudioCursor::new(&frame));
        let mut output = cursor.update(&frame);
        if self.native.is_some() {
            // Reliable host audio owns one-shots; snapshots project only continuous voices.
            output.events.clear();
        }
        self.audio.apply_combat(output)
    }

    pub(super) fn update_audio(&mut self) -> Result<(), String> {
        self.audio.set_volumes(
            self.menu.settings().sound_volume_percent,
            self.menu.settings().music_volume_percent,
        );
        let world_scene = match &self.session.boneyard {
            None => MusicScene::Hub,
            Some(run) if run.encounter.as_ref().is_some_and(|state| state.combat_enabled()) => {
                MusicScene::Combat
            }
            Some(_) => MusicScene::Boneyard,
        };
        self.audio.set_scene(self.menu.music_scene().unwrap_or(world_scene))?;
        for event in self.menu.take_audio_events() {
            self.audio.apply(event)?;
        }
        for event in self.skill_picker.take_audio_events() {
            self.audio.apply(event)?;
        }
        for event in self.panels.take_audio_events() {
            self.audio.apply(event)?;
        }
        self.audio.update()
    }
}
