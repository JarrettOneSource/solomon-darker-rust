//! The HUD, world overlays and level-up skill picker, brought up to date with
//! the session.

use crate::fullscreen::FullscreenPlacement;
use crate::game_over::GameOverAnchor;
use crate::hud::{HudAction, HudMode};
use crate::player_panels::PlayerPanel;
use crate::skill_picker::SkillPickerAction;
use crate::tutorial_scene::TutorialFrame;
use solomon_protocol::GameCommand;
use solomon_sim::Vec2;
use solomon_sim::belt::{BeltEntry, BeltPotionKind, belt_potion_projection};
use solomon_sim::hud::{HudBeltEntry, HudItemIcon};

use super::{CommandFeedback, Game};

impl Game {
    pub(super) fn update_overlays(&mut self, now: f64) -> Result<(), String> {
        self.update_shared_gameplay(now)?;
        let college_admission = self.has_game
            && !self.menu.covers_world()
            && self.session.boneyard.is_none()
            && self.session.participants[&self.local_player_id]
                .player
                .economy
                .college_intro_pending;
        let panel_hides_fullscreen = matches!(
            self.panels.panel(),
            Some(PlayerPanel::Skills | PlayerPanel::SkillSelector(_))
        ) || (self.session.boneyard.is_none()
            && self.panels.is_open());
        self.fullscreen.update(
            if college_admission || panel_hides_fullscreen || self.social.input_active() {
                FullscreenPlacement::Hidden
            } else {
                self.menu.fullscreen_placement()
            },
        )?;
        let (camera, layout) = self.scene_view();
        let width = self.surface.logical_width();
        let height = self.surface.logical_height();
        self.hud.set_mode(if self.session.boneyard.is_some() {
            HudMode::Run
        } else {
            HudMode::Hub
        })?;
        self.hud.set_tutorial_access(self.tutorial_access().map(|access| {
            crate::hud::HudAccess {
                combat: access.combat,
                inventory: access.inventory,
                quickbar: access.quickbar,
                skills: access.skills,
                spell: access.spell,
            }
        }))?;
        self.hud.set_action_enabled(HudAction::Pause, self.can_pause())?;
        self.hud.set_action_enabled(HudAction::Map, self.can_start_boneyard())?;
        self.hud.set_quickbar_player(
            self.has_game.then(|| self.session.participants[&self.local_player_id].player.as_ref()),
            self.session.secondary.players.get(&self.local_player_id),
            &self.menu.settings().controls,
            self.session.boneyard.is_none(),
        )?;
        self.hud.update(&self.hud_state, width, height)?;
        self.college_overlay.update(
            self.session.participants[&self.local_player_id]
                .college_intro
                .as_ref()
                .filter(|_| !self.menu.covers_world()),
            width,
            height,
        )?;
        let position = self
            .display_session()
            .wizard(&self.local_player_id)
            .expect("local participant")
            .position;
        self.game_over.update(
            (self.has_game && !self.menu.covers_world() && self.battle.is_some())
                .then_some(&self.session.run),
            GameOverAnchor {
                position: Vec2::new(
                    (position.x - camera.origin.x) * camera.scale * layout.display_scale,
                    (position.y - camera.origin.y) * camera.scale * layout.display_scale,
                ),
                zoom: camera.scale * layout.display_scale,
            },
            now,
        )?;
        self.tutorial_scene.update(
            &mut self.tutorial_ui,
            &self.audio,
            self.session.boneyard.as_ref().filter(|_| !self.menu.covers_world()).map(|world| {
                TutorialFrame {
                    world,
                    player: &self.session.participants[&self.local_player_id].player,
                    wizard: self.session.wizard(&self.local_player_id).expect("local participant"),
                    camera: &camera,
                    settings: self.menu.settings(),
                    coarse_pointer: self.surface.touch_capable(),
                }
            }),
            now,
            width,
            height,
        )
    }

    pub(super) fn update_skill_picker(&mut self, now: f64) -> Result<(), String> {
        let was_open = self.skill_picker.is_open();
        // A restored offer waits behind Continue; it must not cover the title screen.
        let player = self
            .has_game
            .then(|| self.session.participants[&self.local_player_id].player.as_ref())
            .filter(|_| !self.menu.covers_world());
        self.skill_picker.update(
            player.and_then(|player| player.progression.pending_offer.as_ref()),
            player.is_some_and(|player| player.progression.sorcerors_charm_available),
            now,
            self.surface.logical_width(),
            self.surface.logical_height(),
        )?;
        if self.skill_picker.is_open() && !was_open {
            self.panels.hide()?;
            self.request_save();
        }
        for action in self.skill_picker.take_actions() {
            if self.native.is_some() {
                let command = match action {
                    SkillPickerAction::Choose(choice) => GameCommand::ChooseSkill { choice },
                    SkillPickerAction::Reroll { offer_sequence } => {
                        GameCommand::RerollSkillOffer { offer_sequence }
                    }
                    SkillPickerAction::Save { offer_sequence } => {
                        GameCommand::DeferSkillChoice { offer_sequence }
                    }
                };
                self.send_native_command(command, CommandFeedback::SkillPicker)?;
                continue;
            }
            let accepted = self.has_game
                && match action {
                    SkillPickerAction::Choose(choice) => {
                        self.session.choose_skill(&self.local_player_id, choice)
                    }
                    SkillPickerAction::Reroll { offer_sequence } => {
                        self.session.reroll_skill_offer(&self.local_player_id, offer_sequence)
                    }
                    SkillPickerAction::Save { offer_sequence } => {
                        self.session.defer_skill_choice(&self.local_player_id, offer_sequence)
                    }
                };
            if accepted {
                self.refresh_hud();
                self.request_save();
            } else {
                self.skill_picker.reject_action(now);
            }
        }
        Ok(())
    }

    pub(super) fn refresh_hud(&mut self) {
        if !self.has_game {
            self.menu.set_mobile_editor_player(None, true);
            self.menu.set_mobile_editor_secondary(None);
            return;
        }
        let player = &self.session.participants[&self.local_player_id].player;
        let secondary = self.session.secondary.players.get(&self.local_player_id);
        self.menu.set_mobile_editor_player(Some(player), self.session.boneyard.is_none());
        self.menu.set_mobile_editor_secondary(secondary);
        let hud = &mut self.hud_state;
        hud.current_health = player.combat.current_health;
        hud.maximum_health = player.combat.maximum_health;
        hud.current_mana = player.combat.current_mana;
        hud.maximum_mana = player.combat.maximum_mana;
        hud.shield_current = secondary.map_or(0.0, |state| state.magic_shield_absorb);
        hud.shield_maximum = secondary.map_or(0.0, |state| state.magic_shield_maximum);
        hud.reserved_mana = secondary.map_or(0.0, |state| state.reserved_mana);
        hud.poisoned = player.combat.poison_ticks_remaining > 0;
        hud.experience_progress = player.progression.experience_progress();
        hud.gold = player.economy.gold.min(u64::from(u32::MAX)) as u32;
        hud.primary_icon_record = player.book.skill_icon_record(player.book.primary_skill_id);
        hud.concentration_icon_records = [
            player.skill_runtime.concentration_skill_id_a,
            player.skill_runtime.concentration_skill_id_b,
        ]
        .map(|id| id.map(|id| player.book.skill_icon_record(id)));
        for (slot, entry) in player
            .belt
            .entries
            .iter()
            .skip(self.hotbar.start())
            .take(solomon_sim::belt::BELT_SLOT_COUNT)
            .enumerate()
        {
            hud.belt[slot] = match entry {
                Some(BeltEntry::Skill { skill_id }) => Some(HudBeltEntry::Skill {
                    record: player.book.skill_icon_record(*skill_id),
                    unavailable: self.session.boneyard.is_none()
                        || !player.combat.can_cast()
                        || secondary.is_some_and(|state| {
                            state.global_cooldown_ticks > 0.0
                                || state.cooldown_ticks_by_skill[*skill_id] > 0.0
                        }),
                }),
                Some(BeltEntry::HealthPotion) => Some(HudBeltEntry::HealthPotion {
                    count: belt_potion_projection(&player.economy.backpack, BeltPotionKind::Health)
                        .count as u32,
                }),
                Some(BeltEntry::ManaPotion) => Some(HudBeltEntry::ManaPotion {
                    count: belt_potion_projection(&player.economy.backpack, BeltPotionKind::Mana)
                        .count as u32,
                }),
                Some(entry @ BeltEntry::Item { .. }) => {
                    solomon_sim::belt::belt_entry_item(*entry, &player.economy).map(|item| {
                        HudBeltEntry::Item {
                            icon: HudItemIcon::from_item(item, player.selection.element),
                            count: item.quantity,
                        }
                    })
                }
                None => None,
            };
        }
    }
}
