//! `GameSettingsDialog` retains one MobileUiEditor draft and requests one durable save.
use super::{MobileEditorPlayerPreview, MobileUiEditor, Page, SettingsView};
use solomon_sim::hud::quickbar::{
    NativeSkillQuickbarCooldownPresentation, native_skill_quickbar_cooldown_presentation,
};
use solomon_sim::mobile_ui::{MobileUiLayout, MobileUiLayoutState, mobile_ui_editor_page_size};
use solomon_sim::player::PlayerState;
use solomon_sim::secondary::NativeSecondaryPlayerState;
impl SettingsView {
    pub fn set_mobile_layout_state(&mut self, state: &MobileUiLayoutState) -> Result<(), String> {
        self.mobile_state = state.clone();
        if let Some(s) = &mut self.mobile_sharing {
            s.set_admission(self.account_username.is_some(), state.customized)?;
        }
        Ok(())
    }
    pub fn set_mobile_editor_player(&mut self, player: Option<&PlayerState>, in_hub: bool) {
        self.mobile_preview = player.map(|player| {
            let mut p = MobileEditorPlayerPreview::from_player(player, in_hub);
            p.set_cooldowns(&self.mobile_cooldowns);
            p
        });
        self.paint_mobile_preview();
    }
    pub fn set_mobile_editor_secondary(&mut self, secondary: Option<&NativeSecondaryPlayerState>) {
        self.mobile_cooldowns = std::array::from_fn(|id| {
            secondary.map_or_else(NativeSkillQuickbarCooldownPresentation::default, |s| {
                native_skill_quickbar_cooldown_presentation(
                    s.cooldown_ticks_by_skill[id],
                    s.cooldown_maximum_ticks_by_skill[id],
                    s.global_cooldown_ticks,
                )
            })
        });
        if let Some(p) = &mut self.mobile_preview {
            p.set_cooldowns(&self.mobile_cooldowns);
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
        self.mobile_save = Some(editor.saved_layout()?);
        editor.set_busy(true, None)?;
        self.mobile_save_in_flight = true;
        Ok(())
    }
    pub(super) fn clear_mobile_editor(&mut self) -> Result<(), String> {
        self.mobile_editor = None;
        self.mobile_save = None;
        self.mobile_save_in_flight = false;
        self.mobile_fullscreen = false;
        Ok(())
    }
    pub(super) fn build_mobile_editor(&mut self) -> Result<(), String> {
        self.mobile_fullscreen = self.surface.coarse_without_hover()?;
        let page = mobile_ui_editor_page_size(
            f64::from(self.viewport.x),
            f64::from(self.viewport.y),
            self.mobile_fullscreen,
        );
        let mut editor = MobileUiEditor::new(
            self.mobile_art.clone(),
            &self.mobile_state,
            page,
            f64::from(self.settings.ui_scale_percent) / 100.0,
            self.mobile_fullscreen,
        )?;
        editor.set_preview(self.mobile_preview.clone());
        editor.fit_after_attach()?;
        self.mobile_editor = Some(editor);
        Ok(())
    }
    pub(super) fn update_mobile_editor(&mut self) -> Result<(), String> {
        let Some(editor) = &mut self.mobile_editor else { return Ok(()) };
        let page = mobile_ui_editor_page_size(
            f64::from(self.viewport.x),
            f64::from(self.viewport.y),
            self.mobile_fullscreen,
        );
        editor.resize(page, f64::from(self.settings.ui_scale_percent) / 100.0)?;
        editor.update()
    }
}
