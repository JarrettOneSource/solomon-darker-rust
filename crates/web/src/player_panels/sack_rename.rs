//! Receipt-driven owned-Sack rename, using GPU text editing and the canonical
//! UTF-16 admission contract. Rejection keeps the exact draft available.
use super::chrome::{PanelCanvas, rect};
use super::{PlayerPanel, PlayerPanelAction, PlayerPanelSnapshot, PlayerPanels, Target};
use crate::gpu_ui::{UiFrame, native_text};
use solomon_sim::economy::{
    HubInventoryAction, HubInventoryItem, INVENTORY_SACK_NAME_MAX_LENGTH,
    normalize_inventory_sack_name,
};
use solomon_sim::native_ui_text::{
    NativeUiFontName, NativeUiTextAlign, NativeUiTextPlacement, NativeUiTextSpec,
    layout_native_ui_text,
};
use solomon_sim::player_panels::sack_rename::{
    INVENTORY_SACK_RENAME_BUTTON, INVENTORY_SACK_RENAME_BUTTON_SCALE, InventorySackRenameOwner,
    InventorySackRenameSelection, inventory_sack_rename_dialog_layout,
    inventory_sack_rename_target,
};

pub(super) fn layer_id() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("sack-rename"))
}

pub(super) struct SackRenameDialog {
    pub item_id: u64,
    draft: String,
    pending_name: Option<String>,
    error: Option<String>,
    focus_next: bool,
}
impl SackRenameDialog {
    fn new(item: &HubInventoryItem) -> Self {
        Self {
            item_id: item.id,
            draft: item.name.clone(),
            pending_name: None,
            error: None,
            focus_next: true,
        }
    }
    pub fn action_feedback(
        &mut self,
        action: &HubInventoryAction,
        accepted: bool,
    ) -> Result<bool, String> {
        let HubInventoryAction::RenameSack { item_id, name } = action else { return Ok(false) };
        if *item_id != self.item_id || self.pending_name.as_deref() != Some(name.as_str()) {
            return Ok(false);
        }
        if accepted {
            return Ok(true);
        }
        self.pending_name = None;
        self.error = Some("That Sack could not be renamed. Please try again.".into());
        Ok(false)
    }
    pub fn can_cancel(&self) -> bool {
        self.pending_name.is_none()
    }
}
impl PlayerPanels {
    fn sack_rename_candidate<'a>(
        &self,
        s: &'a PlayerPanelSnapshot<'_>,
    ) -> Option<&'a HubInventoryItem> {
        let selection = self.selected.map(|target| match target {
            Target::Item(item_id) => {
                InventorySackRenameSelection { item_id, owner: InventorySackRenameOwner::Backpack }
            }
            Target::Storage(item_id) => {
                InventorySackRenameSelection { item_id, owner: InventorySackRenameOwner::Storage }
            }
            _ => InventorySackRenameSelection {
                item_id: 0,
                owner: InventorySackRenameOwner::Unowned,
            },
        });
        inventory_sack_rename_target(s.economy, selection, &self.sack_path)
    }
    pub(super) fn draw_sack_rename_open(
        &self,
        c: &PanelCanvas<'_, '_>,
        s: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        if !matches!(self.panel, Some(PlayerPanel::Inventory | PlayerPanel::Service(_)))
            || self.notice.is_some()
            || self.dye.is_some()
            || self.rename_sack.is_some()
            || self.sack_transition.is_some()
        {
            return Ok(());
        }
        if let Some(item) = self.sack_rename_candidate(s) {
            let button = c.admitted(!self.inventory_transition_locked()).native_button_art(
                &self.ui,
                "rename-sack",
                &format!("Rename {}", item.name),
                INVENTORY_SACK_RENAME_BUTTON,
            )?;
            button.text(&NativeUiTextSpec {
                tint: 0xd9ba70,
                scale: INVENTORY_SACK_RENAME_BUTTON_SCALE,
                ..NativeUiTextSpec::new(
                    NativeUiFontName::Menu,
                    "RENAME SACK",
                    INVENTORY_SACK_RENAME_BUTTON.width / 2.0,
                    42.5 * INVENTORY_SACK_RENAME_BUTTON_SCALE,
                )
            });
        }
        Ok(())
    }
    pub(super) fn open_sack_rename(&mut self, s: &PlayerPanelSnapshot<'_>) -> Result<(), String> {
        if let Some(item) = self.sack_rename_candidate(s) {
            self.rename_sack = Some(SackRenameDialog::new(item));
            self.press = None;
            self.sound(solomon_sim::audio::AudioCue::Click);
        }
        Ok(())
    }
    pub(super) fn close_sack_rename(&mut self) {
        self.rename_sack = None;
    }
    pub(super) fn update_sack_rename(
        &mut self,
        s: &PlayerPanelSnapshot<'_>,
        _width: i32,
        _height: i32,
    ) -> Result<(), String> {
        if let Some(dialog) = &self.rename_sack
            && solomon_sim::economy::find_inventory_item(&s.economy.backpack, dialog.item_id)
                .or_else(|| {
                    solomon_sim::economy::find_inventory_item(&s.economy.storage, dialog.item_id)
                })
                .is_none()
        {
            self.close_sack_rename();
        }
        Ok(())
    }
    pub(super) fn paint_sack_rename(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let Some(mut dialog) = self.rename_sack.take() else { return Ok(()) };
        frame.input_scope("sack-rename", layer_id());
        let layout = inventory_sack_rename_dialog_layout(
            self.viewport_size.0 as f64,
            self.viewport_size.1 as f64,
            self.ui.record(18)?.logical_width as f64,
            self.ui.record(8)?.logical_height as f64,
        );
        let mut close = false;
        let mut submit = false;
        let mut error = None;
        let bounds = frame.ctx.content_rect();
        egui::Area::new(layer_id().id)
            .fade_in(false)
            .order(layer_id().order)
            .fixed_pos(egui::Pos2::ZERO)
            .constrain(false)
            .show(frame.ctx, |ui| {
                ui.painter().rect_filled(bounds, 0.0, egui::Color32::from_black_alpha(191));
                if let Err(message) = crate::gpu_ui::message_frame_scaled(
                    frame,
                    ui.painter(),
                    &self.ui,
                    rect(layout.body),
                    layout.art_scale as f32,
                ) {
                    error = Some(message);
                }
                frame.record_rect(
                    "panel:sack-rename",
                    "Rename Sack",
                    "dialog",
                    rect(layout.body),
                    false,
                );
                for (label, font, baseline) in [
                    ("Rename Sack".to_owned(), NativeUiFontName::Menu, layout.title_baseline),
                    (
                        format!("Up to {INVENTORY_SACK_NAME_MAX_LENGTH} characters"),
                        NativeUiFontName::Medium,
                        layout.limit_baseline,
                    ),
                ] {
                    native_text(
                        frame,
                        ui.painter(),
                        &NativeUiTextSpec {
                            align: NativeUiTextAlign::Left,
                            ..NativeUiTextSpec::new(font, &label, layout.body.x + 48.0, baseline)
                        },
                    );
                }
                let label = NativeUiTextSpec {
                    align: NativeUiTextAlign::Left,
                    placement: NativeUiTextPlacement::Box,
                    tint: 0xd9ba70,
                    ..NativeUiTextSpec::new(NativeUiFontName::Medium, "Sack name", 0.0, 0.0)
                };
                let label_size = layout_native_ui_text(&label);
                let mut field = layout.field;
                field.x += 20.0 + label_size.width;
                field.width -= 30.0 + label_size.width;
                native_text(
                    frame,
                    ui.painter(),
                    &NativeUiTextSpec {
                        x: layout.field.x + 10.0,
                        y: layout.field.y + (44.0 - label_size.height) / 2.0,
                        ..label
                    },
                );
                let pending = dialog.pending_name.is_some();
                let mut child =
                    crate::player_panels::chrome::child_ui(ui, "hud-admission", ui.max_rect());
                if !(!pending && !self.input_suspended) {
                    child.disable();
                }
                let before = dialog.draft.clone();
                if let Err(message) = self.control_panel.paint_frame(
                    frame,
                    ui.painter(),
                    5,
                    rect(field).shrink2(egui::vec2(0.0, 7.0)),
                    3.0,
                    egui::Color32::WHITE,
                ) {
                    error = Some(message);
                }
                let response = crate::gpu_ui::NativeTextEdit::singleline(
                    &mut dialog.draft,
                    "panel:sack-rename:name",
                    "Sack name",
                )
                .font(egui::FontId::new(16.0, egui::FontFamily::Name("IM FELL English".into())))
                .line_height(16.0 * 1.35)
                .margin(egui::vec2(10.0, 7.0))
                .color(egui::Color32::from_rgb(217, 186, 112))
                .rules(crate::gpu_ui::EditorRules {
                    utf16_limit: Some(INVENTORY_SACK_NAME_MAX_LENGTH),
                    ..Default::default()
                })
                .request_focus(dialog.focus_next)
                .select_all(dialog.focus_next)
                .show(frame, &mut child, rect(field));
                if child.is_visible() && response.has_focus() {
                    dialog.focus_next = false;
                }
                if response.has_focus() {
                    ui.painter().rect_stroke(
                        rect(field).shrink(7.0),
                        0.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(217, 186, 112)),
                        egui::StrokeKind::Inside,
                    );
                }
                if before != dialog.draft {
                    dialog.error = None;
                }
                let valid = normalize_inventory_sack_name(&dialog.draft).is_some();
                let message = dialog.error.as_deref().or(if valid {
                    None
                } else {
                    Some("Enter a name using 1–32 characters.")
                });
                if let Some(message) = message {
                    ui.painter().text(
                        egui::pos2(layout.field.x as f32, (layout.field.y + 52.0) as f32),
                        egui::Align2::LEFT_TOP,
                        message,
                        egui::FontId::new(16.0, egui::FontFamily::Name("Georgia".into())),
                        egui::Color32::from_rgb(241, 211, 126),
                    );
                    frame.record_rect(
                        "panel:sack-rename:error",
                        message,
                        "status",
                        egui::Rect::from_min_size(
                            egui::pos2(layout.field.x as f32, (layout.field.y + 48.0) as f32),
                            egui::vec2(layout.field.width as f32, 25.0),
                        ),
                        false,
                    );
                }
                for (action, label, r, enabled) in [
                    ("save", "SAVE", layout.save, !pending && valid),
                    ("cancel", "CANCEL", layout.cancel, !pending),
                ] {
                    let response = crate::menu::widgets::stock_button(
                        frame,
                        ui,
                        &format!("panel:sack-rename:{action}"),
                        label,
                        rect(r),
                        layout.button_scale as f32,
                        enabled && !self.input_suspended,
                    );
                    if response.clicked() {
                        if action == "save" {
                            submit = true;
                        } else {
                            close = true;
                        }
                    }
                }
                if !pending
                    && !self.input_suspended
                    && frame.ctx.memory(|memory| memory.allows_interaction(layer_id()))
                {
                    if frame.ctx.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    }) {
                        close = true;
                    }
                    if valid && response.submitted {
                        submit = true;
                    }
                }
            });
        if submit
            && dialog.pending_name.is_none()
            && let Some(name) = normalize_inventory_sack_name(&dialog.draft)
        {
            let name = name.to_owned();
            dialog.pending_name = Some(name.clone());
            self.actions.push(PlayerPanelAction::Inventory(HubInventoryAction::RenameSack {
                item_id: dialog.item_id,
                name,
            }));
        }
        if !close {
            self.rename_sack = Some(dialog);
        }
        error.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_receipt_preserves_the_draft_and_unlocks_only_its_own_request() {
        let mut dialog = SackRenameDialog {
            item_id: 7,
            draft: "  My 😀 Sack  ".into(),
            pending_name: Some("My 😀 Sack".into()),
            error: None,
            focus_next: false,
        };
        assert!(
            !dialog
                .action_feedback(
                    &HubInventoryAction::RenameSack { item_id: 8, name: "My 😀 Sack".into() },
                    false
                )
                .unwrap()
        );
        assert!(!dialog.can_cancel());
        assert!(
            !dialog
                .action_feedback(
                    &HubInventoryAction::RenameSack { item_id: 7, name: "My 😀 Sack".into() },
                    false
                )
                .unwrap()
        );
        assert!(dialog.can_cancel());
        assert_eq!(dialog.draft, "  My 😀 Sack  ");
        assert_eq!(
            dialog.error.as_deref(),
            Some("That Sack could not be renamed. Please try again.")
        );
    }
}
