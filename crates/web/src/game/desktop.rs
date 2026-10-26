//! Save-before-close uses the same source checkpoint and durable revision owner
//! as Save and Leave. The OS loop keeps polling it even when drawing is suspended.

#[cfg(not(target_arch = "wasm32"))]
use std::{cell::RefCell, rc::Rc};

#[cfg(not(target_arch = "wasm32"))]
use solomon_graphics::Renderer;

use super::Game;
use crate::menu::MenuAction;
#[cfg(not(target_arch = "wasm32"))]
use crate::{gpu_ui::GpuUi, storage};

pub(super) enum DesktopClose {
    Saving,
    Complete(Result<(), String>),
}

impl Game {
    pub fn cancel_desktop_close(&mut self) {
        self.desktop_close = None;
    }

    pub fn begin_desktop_close(&mut self) -> Result<bool, String> {
        let Some(native) = &self.native else { return Ok(false) };
        if !native.is_playing() && !native.is_leaving() {
            return Ok(false);
        }
        self.desktop_close = Some(DesktopClose::Saving);
        if !native.is_leaving() {
            self.native_menu_action(&MenuAction::Leave)?;
        }
        Ok(true)
    }

    pub(super) fn finish_desktop_close(&mut self, result: Result<(), String>) {
        if matches!(self.desktop_close, Some(DesktopClose::Saving)) {
            self.desktop_close = Some(DesktopClose::Complete(result));
        }
    }

    pub fn poll_desktop_close(&mut self, now: f64) -> Result<bool, String> {
        if matches!(self.desktop_close, Some(DesktopClose::Saving)) {
            self.poll_native(now)?;
            self.poll_hosted(now)?;
        }
        if matches!(self.desktop_close, Some(DesktopClose::Complete(_))) {
            let Some(DesktopClose::Complete(result)) = self.desktop_close.take() else {
                unreachable!("the completed desktop close owns its result")
            };
            return result.map(|()| true);
        }
        Ok(false)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Game {
    pub fn load_native(
        surface: crate::platform::Surface,
        renderer: Renderer,
        endpoint: crate::desktop::DesktopEndpoint,
        controls: crate::desktop::DesktopControls,
    ) -> impl std::future::Future<Output = Result<Rc<RefCell<Self>>, String>> {
        Box::pin(async move {
            let playtime = Some(storage::PlaytimeRecorder::start());
            let mut game = Self::load_resources(surface, renderer, playtime).await?;
            game.initialize_settings()?;
            match storage::read_hall() {
                Ok(entries) => game.hall_entries = entries,
                Err(error) => Self::show_storage_error(&error),
            }
            game.desktop_ui = Some(crate::desktop::DesktopUi::new(controls));
            game.load_hosted(crate::hosted::WebsiteMount::default(), Some(endpoint)).await?;
            crate::runtime_ui::completed(crate::runtime_ui::RUNTIME)?;
            Ok(Rc::new(RefCell::new(game)))
        })
    }

    pub fn native_ui(&mut self) -> &mut GpuUi {
        &mut self.gpu_ui
    }

    pub fn window_event(
        &mut self,
        event: &winit::event::WindowEvent,
        time: f64,
    ) -> Result<(), String> {
        self.dispatch_ui_input(time)?;
        self.gpu_ui.window_event(event)?;
        self.input.window_event(event)
    }

    pub fn accesskit_action(
        &mut self,
        action: egui::accesskit::ActionRequest,
        time: f64,
    ) -> Result<(), String> {
        self.dispatch_ui_input(time)?;
        self.gpu_ui.accesskit_action(action)
    }
}
