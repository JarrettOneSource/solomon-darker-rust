//! Native browser fullscreen capability with GPU presentation. The same browser
//! adapter backs this row and the edge control.
use super::widgets;
use crate::fullscreen::{
    ControlMode, error_message, fullscreen_active, game_fullscreen_control_mode, toggle_fullscreen,
};
use crate::gpu_ui::{StockAtlas, UiFrame};
use egui::{Ui, Vec2};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen_futures::JsFuture;
use web_sys::Document;
pub(super) struct SettingsFullscreen {
    document: Document,
    mode: ControlMode,
    show_install_help: bool,
    result: Rc<RefCell<Option<Result<(), String>>>>,
    error: Option<String>,
}
impl SettingsFullscreen {
    pub fn new(document: &Document) -> Result<Option<Self>, String> {
        let mode = game_fullscreen_control_mode(document)?;
        Ok((mode != ControlMode::Hidden).then(|| Self {
            document: document.clone(),
            mode,
            show_install_help: false,
            result: Rc::new(RefCell::new(None)),
            error: None,
        }))
    }
    pub fn activate(&mut self) -> Result<(), String> {
        self.error = None;
        if self.mode == ControlMode::Install {
            self.show_install_help = !self.show_install_help;
            return Ok(());
        }
        let request = toggle_fullscreen(&self.document).map_err(error_message)?;
        let completed = self.result.clone();
        wasm_bindgen_futures::spawn_local(async move {
            *completed.borrow_mut() = Some(
                JsFuture::from(js_sys::Promise::resolve(&request))
                    .await
                    .map(|_| ())
                    .map_err(error_message),
            );
        });
        Ok(())
    }
    pub fn paint(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        atlas: &StockAtlas,
        width: f32,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        if let Some(result) = self.result.borrow_mut().take() {
            self.error = result.err();
        }
        let (_, rect) = ui.allocate_space(Vec2::new(width, 44.0));
        atlas.paint_raw(frame, ui.painter(), 3, rect, egui::Color32::WHITE)?;
        let response =
            widgets::interact(frame, ui, "settings-fullscreen", "FULLSCREEN", "button", rect, true);
        widgets::native(
            frame,
            ui,
            "FULLSCREEN",
            NativeUiFontName::ControlPanel,
            rect.min + Vec2::new(10.0, 26.0),
            1.0,
            0xdbc174,
            NativeUiTextAlign::Left,
        );
        if self.mode == ControlMode::Install {
            widgets::native(
                frame,
                ui,
                "OPTIONS",
                NativeUiFontName::ControlPanel,
                egui::pos2(rect.right() - 10.0, rect.top() + 26.0),
                1.0,
                0xd9bd72,
                NativeUiTextAlign::Right,
            );
        } else {
            atlas.paint_centered(
                frame,
                ui.painter(),
                if fullscreen_active(&self.document) { 9 } else { 8 },
                egui::pos2(rect.right() - 32.0, rect.center().y),
                1.0,
                0.0,
                egui::Color32::WHITE,
            )?;
        }
        if response.clicked() {
            actions.push("settings-fullscreen".to_owned());
        }
        if self.show_install_help {
            super::flow(
                ui,
                "Install this page as a web app for fullscreen on iPhone or iPad.",
                width,
                12.0,
                widgets::color(0xa99a70),
            );
        }
        if let Some(error) = &self.error {
            super::flow(ui, error, width, 12.0, widgets::ERROR);
        }
        Ok(())
    }
}
