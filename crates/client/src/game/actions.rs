//! The menu, player panel and belt actions the UI emits, applied to the game.

use crate::menu::MenuAction;
use crate::player_panels::{PlayerPanel, PlayerPanelAction};
use crate::{set_status, storage};
use solomon_protocol::GameCommand;
use solomon_sim::Vec2;
use solomon_sim::belt::BeltEntry;
use solomon_sim::economy::{HubInventoryAction, HubItemKind};
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::profile::WizardElement;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline};
use solomon_sim::session::{GameSession, GameSessionOptions};

use super::{CommandFeedback, Game, LOCAL_PLAYER_ID};

impl Game {
    pub(super) fn apply_menu_action(&mut self, mut action: MenuAction) -> Result<(), String> {
        if let MenuAction::SettingsChanged(settings) = &mut action
            && !self.request_hosted_settings(settings)?
        {
            return Ok(());
        }
        #[cfg(target_arch = "wasm32")]
        if self.is_editor_test() && action == MenuAction::Leave {
            return self.return_to_editor();
        }
        if self.hosted_menu_action(&action)? {
            return Ok(());
        }
        if self.native_menu_action(&action)? {
            return Ok(());
        }
        match action {
            MenuAction::Start(selection) => {
                if self.has_game {
                    if !self.session.reset_participant(&self.local_player_id, selection) {
                        return Err(
                            "The current shared run must finish before replacing this wizard"
                                .into(),
                        );
                    }
                } else {
                    self.local_player_id = LOCAL_PLAYER_ID.into();
                    self.session = GameSession::new(
                        self.local_player_id.clone(),
                        selection,
                        GameSessionOptions {
                            enhanced_effects: self.menu.settings().enhanced_effects,
                            ..Default::default()
                        },
                    );
                }
                self.reset_world_views()?;
                self.has_game = true;
                self.session.arm_college_intro(&self.local_player_id);
                self.world_load.cancel();
                self.accumulator = 0.0;
                if self.local_save.has_store() {
                    set_status("");
                }
                self.local_save.start_new_game();
                self.request_save();
                self.panels.hide()?;
                self.refresh_hud();
                self.update_combat_audio()?;
                self.menu.complete_create_loadout(true)?;
            }
            MenuAction::ConfirmLoadout(selection) => {
                if !self.session.confirm_loadout(&self.local_player_id, selection) {
                    return Err("The College loadout is not ready".into());
                }
                self.accumulator = 0.0;
                self.request_save();
                self.refresh_hud();
                self.menu.complete_create_loadout(true)?;
            }
            MenuAction::StartTutorial => {
                self.reset_world_views()?;
                self.local_player_id = LOCAL_PLAYER_ID.into();
                self.session = GameSession::new(
                    LOCAL_PLAYER_ID.to_owned(),
                    NewGameSelection {
                        name: "Sirmin".into(),
                        element: WizardElement::Ether,
                        discipline: WizardDiscipline::Arcane,
                    },
                    GameSessionOptions {
                        enhanced_effects: self.menu.settings().enhanced_effects,
                        ..Default::default()
                    },
                );
                self.has_game = true;
                self.queue_world(true)?;
            }
            MenuAction::TutorialDeclined => {
                storage::save_tutorial_declined()?;
            }
            MenuAction::Continue | MenuAction::Resume | MenuAction::CancelCreate => {}
            MenuAction::Cheats => self.pending_cheat_open = true,
            MenuAction::Leave => {
                self.request_save();
                self.menu.show_title(self.has_game)?;
            }
            MenuAction::SettingsChanged(settings) => {
                self.commit_settings(*settings)?;
            }
            MenuAction::SetEnhancedEffects { enabled } => {
                if self.enhanced_effects_authority().is_some_and(|authority| !authority.can_change)
                {
                    return Ok(());
                }
                let mut settings = self.menu.settings().clone();
                settings.enhanced_effects = enabled;
                self.commit_settings(settings)?;
                if self.has_game {
                    if self.native.is_some() {
                        self.send_native_message(&solomon_protocol::ClientMessage::Hosted {
                            command: solomon_protocol::hosted::HostedCommand::SetEnhancedEffects {
                                enabled,
                            },
                        })?;
                    } else {
                        self.session.set_enhanced_effects(enabled);
                        self.request_save();
                    }
                }
            }
            MenuAction::SaveMobileLayout(layout) => {
                let result = match layout {
                    Some(layout) => self.mobile_layout_store.set_layout(&layout),
                    None => self.mobile_layout_store.reset(),
                };
                self.menu.complete_mobile_layout_save(result)?;
                self.update_mobile_layout()?;
            }
        }
        Ok(())
    }

    pub(super) fn apply_panel_action(
        &mut self,
        action: PlayerPanelAction,
        now: f64,
    ) -> Result<(), String> {
        // Surface acknowledgements are sent only while a Tutorial exists.
        // Ordinary Inventory/Skill Book use must not send rejected lesson commands.
        if matches!(action, PlayerPanelAction::Surface(_))
            && self.session.boneyard.as_ref().is_none_or(|world| world.tutorial.is_none())
        {
            return Ok(());
        }
        if matches!(
            action,
            PlayerPanelAction::SelectPrimary(_)
                | PlayerPanelAction::SelectConcentration(_)
                | PlayerPanelAction::SelectConcentrationSlot { .. }
                | PlayerPanelAction::Belt(_)
        ) && !self.can_edit_books()
        {
            return Ok(());
        }
        if self.native.is_some() {
            let command = match &action {
                PlayerPanelAction::Surface(action) => {
                    Some(GameCommand::TutorialAction { action: *action })
                }
                PlayerPanelAction::AcknowledgeCollegeIntroDialogue => {
                    Some(GameCommand::AcknowledgeCollegeIntroDialogue)
                }
                PlayerPanelAction::SelectPrimary(skill_id) => {
                    Some(GameCommand::SelectPrimary { skill_id: *skill_id })
                }
                PlayerPanelAction::SelectConcentration(skill_id) => {
                    Some(GameCommand::SelectConcentration { skill_id: *skill_id })
                }
                PlayerPanelAction::SelectConcentrationSlot { skill_id, slot } => {
                    Some(GameCommand::SelectConcentrationSlot {
                        skill_id: *skill_id,
                        slot: match slot {
                            0 => solomon_sim::profile::PlayerConcentrationSlot::A,
                            1 => solomon_sim::profile::PlayerConcentrationSlot::B,
                            _ => return Err("The concentration slot must be A or B".into()),
                        },
                    })
                }
                PlayerPanelAction::Belt(command) => {
                    Some(GameCommand::BindBelt { command: *command })
                }
                PlayerPanelAction::Inventory(action) => {
                    Some(GameCommand::Inventory { action: action.clone() })
                }
                _ => None,
            };
            if let Some(command) = command {
                let feedback = if let PlayerPanelAction::Inventory(action) = action {
                    CommandFeedback::Inventory(action)
                } else {
                    CommandFeedback::None
                };
                self.send_native_command(command, feedback)?;
                return Ok(());
            }
        }
        match action {
            PlayerPanelAction::OpenInventory
                if !self.menu.is_blocking()
                    && !self.skill_selection_active()
                    && self.can_open_panel(PlayerPanel::Inventory) =>
            {
                self.panels.show(PlayerPanel::Inventory, now)?;
            }
            PlayerPanelAction::OpenSkills
                if !self.menu.is_blocking()
                    && !self.skill_selection_active()
                    && self.can_open_panel(PlayerPanel::Skills) =>
            {
                self.panels.show(PlayerPanel::Skills, now)?;
            }
            PlayerPanelAction::OpenInventory
            | PlayerPanelAction::OpenSkills
            | PlayerPanelAction::Close => {}
            PlayerPanelAction::Surface(action) => {
                if self.session.apply_tutorial_action(action) {
                    self.request_save();
                }
            }
            PlayerPanelAction::AcknowledgeCollegeIntroDialogue => {
                if self.session.acknowledge_college_intro_dialogue(&self.local_player_id) {
                    self.request_save();
                }
            }
            PlayerPanelAction::SelectPrimary(id) => {
                if self.has_game {
                    self.session.select_primary(&self.local_player_id, id);
                }
            }
            PlayerPanelAction::SelectConcentration(id) => {
                if self.has_game {
                    self.session.select_concentration(&self.local_player_id, id);
                }
            }
            PlayerPanelAction::SelectConcentrationSlot { skill_id, slot } => {
                use solomon_sim::profile::runtime::PlayerConcentrationSlot;
                let slot = match slot {
                    0 => PlayerConcentrationSlot::A,
                    1 => PlayerConcentrationSlot::B,
                    _ => return Err("The concentration slot must be A or B".into()),
                };
                self.session.select_concentration_slot(&self.local_player_id, skill_id, slot);
            }
            PlayerPanelAction::Belt(command) => {
                if self.has_game {
                    self.session.bind_belt(&self.local_player_id, command);
                }
            }
            PlayerPanelAction::ActivateBeltSlot { slot, pointer } => {
                let pointer = pointer.and_then(|point| self.surface.point_fraction(point));
                let aim = self.project_input_aim(Some(self.input.secondary_aim(pointer)));
                return self.activate_belt_slot(slot, aim, now);
            }
            PlayerPanelAction::Inventory(action) => {
                if !self.has_game {
                    return Ok(());
                }
                let applied = self.session.apply_inventory(&self.local_player_id, &action);
                self.panels.inventory_action_feedback(
                    &action,
                    &applied.transaction,
                    now,
                    self.mod_content.current(),
                );
            }
        }
        self.refresh_hud();
        self.request_save();
        Ok(())
    }

    pub(super) fn activate_belt_slot(
        &mut self,
        slot: usize,
        aim: Option<Vec2>,
        now: f64,
    ) -> Result<(), String> {
        if !self.can_edit_books()
            || self.tutorial_access().is_some_and(|access| !access.quickbar)
            || !matches!(self.session.run.phase, GameRunPhase::Hub | GameRunPhase::Active)
        {
            return Ok(());
        }
        let player = &self.session.participants[&self.local_player_id].player;
        let Some(entry) = player.belt.entries.get(slot).copied().flatten() else {
            return Ok(());
        };
        let action = if matches!(entry, BeltEntry::Skill { .. }) {
            None
        } else {
            let Some(item) = solomon_sim::belt::belt_entry_item(entry, &player.economy) else {
                return Ok(());
            };
            if item.type_id == 7001 {
                Some(HubInventoryAction::Consume { item_id: item.id })
            } else if item.kind == HubItemKind::Sack {
                Some(HubInventoryAction::EquipSackContents { item_id: item.id })
            } else {
                solomon_sim::belt::belt_equipment_slots(
                    item,
                    player.economy.owned_perk_selectors.contains(&19),
                )
                .first()
                .map(|slot| HubInventoryAction::Equip { item_id: item.id, slot: *slot })
            }
        };
        if self.native.is_some() {
            return self.send_native_command(
                GameCommand::ActivateBeltSlot { slot, aim },
                action.map_or(CommandFeedback::None, CommandFeedback::Inventory),
            );
        }
        let applied = self.session.activate_belt_slot(&self.local_player_id, slot, aim);
        if let Some(action) = action {
            self.panels.inventory_action_feedback(
                &action,
                &applied.transaction,
                now,
                self.mod_content.current(),
            );
        }
        self.refresh_hud();
        self.request_save();
        Ok(())
    }
}
