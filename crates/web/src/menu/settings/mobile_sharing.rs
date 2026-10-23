//! Real saved mobile-layout publication and clipboard receipt, from
//! `MobileUiLayoutSettingsAction.tsx`. Closing invalidates its completion route.
use super::{SettingsView, widgets};
use crate::gpu_ui::UiFrame;
use crate::hosted::SharedMobileUiLayout;
use egui::{Align2, Rect, Ui, Vec2};
use std::cell::RefCell;
use std::rc::Rc;
pub(super) struct SettingsLayoutPublication {
    pending: Option<u64>,
    account_available: bool,
    customized: bool,
    code: Option<String>,
    error: Option<String>,
    copied: bool,
    copy_result: Rc<RefCell<Option<Result<(), String>>>>,
}
impl SettingsView {
    pub fn begin_mobile_layout_publication(&mut self, id: u64) -> Result<bool, String> {
        let Some(v) = &mut self.mobile_sharing else { return Ok(false) };
        if v.pending.is_some() || !v.account_available || !v.customized {
            return Ok(false);
        }
        v.pending = Some(id);
        v.error = None;
        v.code = None;
        v.copied = false;
        Ok(true)
    }
    pub fn mobile_layout_publication_pending(&self, id: u64) -> bool {
        self.mobile_sharing.as_ref().is_some_and(|v| v.pending == Some(id))
    }
    pub fn complete_mobile_layout_publication(
        &mut self,
        id: u64,
        result: Result<SharedMobileUiLayout, String>,
    ) -> Result<bool, String> {
        let Some(v) = &mut self.mobile_sharing else { return Ok(false) };
        if v.pending != Some(id) {
            return Ok(false);
        }
        v.pending = None;
        match result {
            Ok(layout) => v.code = Some(layout.code),
            Err(error) => v.error = Some(error),
        }
        Ok(true)
    }
}
impl SettingsLayoutPublication {
    pub fn new(account_available: bool, customized: bool) -> Self {
        Self {
            pending: None,
            account_available,
            customized,
            code: None,
            error: None,
            copied: false,
            copy_result: Rc::new(RefCell::new(None)),
        }
    }
    pub fn set_admission(
        &mut self,
        account_available: bool,
        customized: bool,
    ) -> Result<(), String> {
        self.account_available = account_available;
        self.customized = customized;
        Ok(())
    }
    pub fn update(&mut self) -> Result<(), String> {
        if let Some(result) = self.copy_result.borrow_mut().take() {
            match result {
                Ok(()) => {
                    self.copied = true;
                    self.error = None;
                }
                Err(error) => self.error = Some(error),
            }
        }
        Ok(())
    }
    pub fn activate(&mut self, action: &str) -> Result<bool, String> {
        if action != "settings-mobile-copy" {
            return Ok(false);
        }
        let Some(code) = &self.code else { return Ok(true) };
        let promise =
            web_sys::window().ok_or("clipboard window")?.navigator().clipboard().write_text(code);
        let result = self.copy_result.clone();
        wasm_bindgen_futures::spawn_local(async move {
            *result.borrow_mut() = Some(
                wasm_bindgen_futures::JsFuture::from(promise)
                    .await
                    .map(|_| ())
                    .map_err(|_| "The code could not be copied.".to_owned()),
            );
        });
        Ok(true)
    }
    pub fn paint(
        &mut self,
        settings: &SettingsView,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        width: f32,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        settings.action_row(
            frame,
            ui,
            if self.pending.is_some() { "SUBMITTING MOBILE UI..." } else { "SUBMIT TO DARK CLOUD" },
            "settings-mobile-publish",
            width,
            self.pending.is_none() && self.account_available && self.customized,
            actions,
        )?;
        let note = if !self.account_available {
            "SIGN IN TO SUBMIT A MOBILE UI LAYOUT."
        } else if !self.customized {
            "CUSTOMIZE AND SAVE A MOBILE UI LAYOUT FIRST."
        } else {
            ""
        };
        if !note.is_empty() {
            super::flow(ui, note, width, 10.0, widgets::color(0x8f856b));
        }
        if let Some(code) = &self.code {
            let (_, r) = ui.allocate_space(Vec2::new(width, 44.0));
            ui.painter().rect_filled(r, 0.0, widgets::color(0x090806));
            widgets::label(
                ui,
                "SHARE CODE",
                r.min + Vec2::new(10.0, 14.0),
                Align2::LEFT_TOP,
                "Cinzel Bold",
                12.0,
                widgets::GOLD,
            );
            widgets::label(
                ui,
                code,
                egui::pos2(r.right() - 102.0, r.center().y),
                Align2::RIGHT_CENTER,
                "JetBrains Mono Bold",
                16.0,
                widgets::color(0xf3e2a9),
            );
            let b = Rect::from_min_size(
                egui::pos2(r.right() - 92.0, r.top() + 5.0),
                Vec2::new(82.0, 34.0),
            );
            if widgets::flat_button(
                frame,
                ui,
                "settings-mobile-copy",
                if self.copied { "COPIED" } else { "COPY" },
                b,
                true,
            )
            .clicked()
            {
                actions.push("settings-mobile-copy".to_owned());
            }
        }
        if let Some(error) = &self.error {
            super::flow(ui, error, width, 12.0, widgets::ERROR);
        }
        Ok(())
    }
}
