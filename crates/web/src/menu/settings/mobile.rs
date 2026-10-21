//! Source `GameSettingsDialog` draft/commit boundary for `MobileUiEditor`.
//! The Game owns the device store; this page only edits and requests its one save.

use solomon_sim::hud::quickbar::{
    NativeSkillQuickbarCooldownPresentation, native_skill_quickbar_cooldown_presentation,
};
use solomon_sim::mobile_ui::{MobileUiLayout, MobileUiLayoutState, mobile_ui_editor_page_size};
use solomon_sim::player::PlayerState;
use solomon_sim::secondary::NativeSecondaryPlayerState;

use super::{MobileEditorPlayerPreview, MobileUiEditor, Page, SettingsView, append};

impl SettingsView {
    pub fn set_mobile_layout_state(&mut self, state: &MobileUiLayoutState) -> Result<(), String> {
        // A storage event changes the next editor draft, not the current gesture.
        self.mobile_state = state.clone();
        if let Some(sharing) = &mut self.mobile_sharing {
            sharing.set_admission(self.account_username.is_some(), state.customized)?;
        }
        Ok(())
    }

    pub fn set_mobile_editor_player(&mut self, player: Option<&PlayerState>, in_hub: bool) {
        self.mobile_preview = player.map(|player| {
            let mut preview = MobileEditorPlayerPreview::from_player(player, in_hub);
            preview.set_cooldowns(&self.mobile_cooldowns);
            preview
        });
        self.paint_mobile_preview();
    }

    pub fn set_mobile_editor_secondary(&mut self, secondary: Option<&NativeSecondaryPlayerState>) {
        // Retain only the borrowed presentation rows, not spells or actor ownership.
        self.mobile_cooldowns = std::array::from_fn(|id| {
            secondary.map_or_else(NativeSkillQuickbarCooldownPresentation::default, |state| {
                native_skill_quickbar_cooldown_presentation(
                    state.cooldown_ticks_by_skill[id],
                    state.cooldown_maximum_ticks_by_skill[id],
                    state.global_cooldown_ticks,
                )
            })
        });
        if let Some(preview) = &mut self.mobile_preview {
            preview.set_cooldowns(&self.mobile_cooldowns);
        }
        self.paint_mobile_preview();
    }

    fn paint_mobile_preview(&mut self) {
        if let Some(editor) = &mut self.mobile_editor {
            editor.set_preview(self.mobile_preview.clone());
        }
    }

    pub fn take_mobile_layout_save(&mut self) -> Option<Option<MobileUiLayout>> {
        self.mobile_save.take()
    }

    pub fn complete_mobile_layout_save(
        &mut self,
        result: Result<MobileUiLayoutState, String>,
    ) -> Result<(), String> {
        if !self.mobile_save_in_flight {
            return Ok(());
        }
        self.mobile_save_in_flight = false;
        self.footer.remove_attribute("disabled").map_err(|_| "mobile save availability")?;
        match result {
            Ok(state) => {
                self.mobile_state = state;
                self.page = Page::Root;
                self.build_page()
            }
            Err(error) => {
                if let Some(editor) = &mut self.mobile_editor {
                    editor.set_busy(false, Some(error))?;
                }
                Ok(())
            }
        }
    }

    pub(super) fn request_mobile_save(&mut self) -> Result<(), String> {
        if self.mobile_save_in_flight {
            return Ok(());
        }
        let Some(editor) = &mut self.mobile_editor else { return Ok(()) };
        // Flush native input queued before a Save click in the same browser turn.
        self.mobile_save = Some(editor.saved_layout()?);
        editor.set_busy(true, None)?;
        self.mobile_save_in_flight = true;
        self.footer.set_attribute("disabled", "").map_err(|_| "mobile save availability")?;
        Ok(())
    }

    pub(super) fn clear_mobile_editor(&mut self) -> Result<(), String> {
        self.mobile_editor = None;
        self.mobile_save = None;
        self.mobile_save_in_flight = false;
        self.panel.set_hidden(false);
        self.footer.remove_attribute("disabled").map_err(|_| "mobile save availability")?;
        self.root
            .class_list()
            .remove_1("game-settings-mobile-ui-fullscreen")
            .map_err(|_| "mobile editor presentation")?;
        self.root.set_attribute("role", "presentation").map_err(|_| "settings backdrop role")?;
        self.root.remove_attribute("aria-modal").map_err(|_| "mobile editor modal role")?;
        self.root.remove_attribute("aria-label").map_err(|_| "mobile editor modal label")?;
        Ok(())
    }

    pub(super) fn build_mobile_editor(&mut self) -> Result<(), String> {
        let window = self.document.default_view().ok_or("mobile editor window")?;
        let fullscreen = window
            .match_media("(hover: none) and (pointer: coarse)")
            .map_err(|_| "mobile editor pointer media")?
            .is_some_and(|query| query.matches());
        let bounds = self.root.get_bounding_client_rect();
        let page = mobile_ui_editor_page_size(bounds.width(), bounds.height(), fullscreen);
        let mut editor = MobileUiEditor::new(
            &self.document,
            self.mobile_art.clone(),
            &self.mobile_state,
            page,
            f64::from(self.settings.ui_scale_percent) / 100.0,
            fullscreen,
        )?;
        editor.set_preview(self.mobile_preview.clone());
        if fullscreen {
            self.panel.set_hidden(true);
            self.root
                .class_list()
                .add_1("game-settings-mobile-ui-fullscreen")
                .map_err(|_| "mobile editor presentation")?;
            self.root.set_attribute("role", "dialog").map_err(|_| "mobile editor modal role")?;
            self.root.set_attribute("aria-modal", "true").map_err(|_| "mobile editor modality")?;
            self.root
                .set_attribute("aria-label", "Mobile UI Editor")
                .map_err(|_| "mobile editor label")?;
            append(&self.root, &editor.root)?;
        } else {
            append(&self.content, &editor.root)?;
        }
        editor.fit_after_attach()?;
        self.mobile_editor = Some(editor);
        Ok(())
    }

    pub(super) fn update_mobile_editor(&mut self) -> Result<(), String> {
        let Some(editor) = &mut self.mobile_editor else { return Ok(()) };
        let bounds = self.root.get_bounding_client_rect();
        let fullscreen = self.panel.hidden();
        let page = mobile_ui_editor_page_size(bounds.width(), bounds.height(), fullscreen);
        editor.resize(page, f64::from(self.settings.ui_scale_percent) / 100.0)?;
        editor.update()
    }
}
