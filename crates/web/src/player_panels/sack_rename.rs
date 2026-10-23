//! Receipt-driven owned-Sack rename, using GPU text editing and the canonical
//! UTF-16 admission contract. Rejection keeps the exact draft available.
use super::chrome::{PanelCanvas, rect};
use super::{PlayerPanel, PlayerPanelAction, PlayerPanelSnapshot, PlayerPanels, Target};
use crate::gpu_ui::{UiFrame, native_text};
use solomon_sim::economy::{
    HubInventoryAction, HubInventoryItem, INVENTORY_SACK_NAME_MAX_LENGTH,
    normalize_inventory_sack_name,
};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec};
use solomon_sim::player_panels::sack_rename::{
    INVENTORY_SACK_RENAME_BUTTON, InventorySackRenameOwner, InventorySackRenameSelection,
    inventory_sack_rename_dialog_layout, inventory_sack_rename_target,
};

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
            c.admitted(!self.inventory_transition_locked()).native_button(
                &self.ui,
                "rename-sack",
                &format!("RENAME SACK: {}", item.name),
                INVENTORY_SACK_RENAME_BUTTON,
            )?;
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
    pub(super) fn close_sack_rename(&mut self) -> Result<(), String> {
        if self.rename_sack.take().is_some() {}
        Ok(())
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
            self.close_sack_rename()?;
        }
        Ok(())
    }
    pub(super) fn paint_sack_rename(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let Some(mut dialog) = self.rename_sack.take() else { return Ok(()) };
        frame.input_scope("sack-rename");
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
        egui::Area::new(egui::Id::unique("sack-rename"))
            .order(egui::Order::Tooltip)
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
                let mut field = layout.field;
                field.x += 90.0;
                field.width -= 90.0;
                native_text(
                    frame,
                    ui.painter(),
                    &NativeUiTextSpec {
                        align: NativeUiTextAlign::Left,
                        tint: 0xd9ba70,
                        ..NativeUiTextSpec::new(
                            NativeUiFontName::Medium,
                            "Sack name",
                            layout.field.x,
                            layout.field.y + 28.0,
                        )
                    },
                );
                let input_id = egui::Id::unique("sack-rename:name");
                let pending = dialog.pending_name.is_some();
                let mut child =
                    crate::player_panels::chrome::child_ui(ui, "hud-admission", ui.max_rect());
                if !(!pending && !self.input_suspended) {
                    child.disable();
                }
                let before = dialog.draft.clone();
                let mut buffer = Utf16Name(&mut dialog.draft);
                if let Err(message) = self.control_panel.paint_frame(
                    frame,
                    ui.painter(),
                    5,
                    rect(field),
                    3.0,
                    egui::Color32::WHITE,
                ) {
                    error = Some(message);
                }
                let response = child.put(
                    rect(field),
                    egui::TextEdit::singleline(&mut buffer)
                        .id(input_id)
                        .font(egui::FontId::new(22.0, egui::FontFamily::Name("Georgia".into())))
                        .desired_width(field.width as f32),
                );
                frame.record("panel:sack-rename:name", "Sack name", "textbox", &response);
                frame.record_value("panel:sack-rename:name", &dialog.draft, None);
                if dialog.focus_next {
                    response.request_focus();
                    let mut state =
                        egui::TextEdit::load_state(frame.ctx, input_id).unwrap_or_default();
                    state.cursor.set_char_range(Some(egui::text::CCursorRange::select_all_str(
                        &dialog.draft,
                    )));
                    state.store(frame.ctx, input_id);
                    dialog.focus_next = false;
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
                if !pending && !self.input_suspended {
                    if frame.ctx.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    }) {
                        close = true;
                    }
                    if valid
                        && frame.ctx.input_mut(|input| {
                            input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                        })
                    {
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

/// HTML maxlength is measured in UTF-16 units, including supplementary characters.
struct Utf16Name<'a>(&'a mut String);
impl egui::TextBuffer for Utf16Name<'_> {
    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<String>()
    }
    fn is_mutable(&self) -> bool {
        true
    }
    fn as_str(&self) -> &str {
        self.0
    }
    fn insert_text(&mut self, text: &str, index: egui::text::CharIndex) -> usize {
        let capacity = INVENTORY_SACK_NAME_MAX_LENGTH.saturating_sub(self.0.encode_utf16().count());
        let mut units = 0;
        let inserted = text
            .chars()
            .take_while(|c| {
                units += c.len_utf16();
                units <= capacity
            })
            .collect::<String>();
        let byte = self.0.char_indices().nth(index.0).map_or(self.0.len(), |(index, _)| index);
        self.0.insert_str(byte, &inserted);
        inserted.chars().count()
    }
    fn delete_char_range(&mut self, range: std::ops::Range<egui::text::CharIndex>) {
        let start =
            self.0.char_indices().nth(range.start.0).map_or(self.0.len(), |(index, _)| index);
        let end = self.0.char_indices().nth(range.end.0).map_or(self.0.len(), |(index, _)| index);
        self.0.replace_range(start..end, "");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::TextBuffer;
    #[test]
    fn supplementary_characters_respect_the_actual_utf16_limit() {
        let mut draft = "a".repeat(29);
        let mut buffer = Utf16Name(&mut draft);
        assert_eq!(buffer.insert_text("😀😀", egui::text::CharIndex(29)), 1);
        assert_eq!(buffer.as_str().encode_utf16().count(), 31);
        assert_eq!(buffer.insert_text("😀", egui::text::CharIndex(30)), 0);
        assert_eq!(buffer.insert_text("z", egui::text::CharIndex(0)), 1);
        assert_eq!(buffer.as_str().encode_utf16().count(), 32);
        buffer.delete_char_range(egui::text::CharIndex(30)..egui::text::CharIndex(31));
        assert_eq!(buffer.as_str(), format!("z{}", "a".repeat(29)));
    }
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
