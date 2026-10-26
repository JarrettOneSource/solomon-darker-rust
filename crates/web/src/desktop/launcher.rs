//! Native launcher rendered by the same GPU UI and stock fonts as the game.

mod model;
mod view;

use crate::{gpu_ui::GpuUi, platform::Surface, runtime_ui};
use model::Model;
pub use model::{Action, Build, DesktopState, SessionOptions, Update};
use solomon_graphics::Renderer;

pub struct Launcher {
    surface: Surface,
    renderer: Renderer,
    ui: GpuUi,
    model: Model,
    first_frame: bool,
}

impl Launcher {
    pub async fn load(
        surface: Surface,
        renderer: Renderer,
        state: DesktopState,
    ) -> Result<Self, String> {
        runtime_ui::begin_desktop(GpuUi::startup_resources())?;
        runtime_ui::completed(runtime_ui::RUNTIME)?;
        let ui = GpuUi::load(&surface, &renderer).await?;
        runtime_ui::enter_phase("desktop")?;
        runtime_ui::completed(runtime_ui::DESKTOP)?;
        Ok(Self { surface, renderer, ui, model: Model::new(state), first_frame: true })
    }

    pub fn receive(&mut self, state: DesktopState) {
        self.model.receive(state);
    }
    pub fn failed(&mut self, error: String) {
        self.model.failed(error);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn show_previous_app_data(&mut self, present: bool) {
        self.model.previous_app_data = present;
    }
    pub fn take_action(&mut self) -> Option<Action> {
        self.model.pending.take()
    }

    pub fn update_checked(&mut self, state: DesktopState) {
        self.model.status = state.update_error.clone().unwrap_or_else(|| {
            if state.update.is_some() {
                "An update is ready to download."
            } else {
                "You have the latest available build."
            }
            .into()
        });
        self.model.checking = false;
        self.model.receive(state);
    }

    pub fn snapshot(&self) -> serde_json::Value {
        self.ui.snapshot("desktop-launcher", true, None)
    }

    pub fn frame(&mut self, time: f64) -> Result<(), String> {
        let viewport = self.surface.logical_size().map(|size| size as f32);
        if viewport.iter().any(|size| *size <= 0.0) {
            return Ok(());
        }
        let (width, height) = self.surface.fit_buffer();
        self.renderer.begin_frame(width, height, viewport);
        let model = &mut self.model;
        self.ui
            .run(&mut self.renderer, viewport, time / 1000.0, |frame| view::paint(model, frame))?;
        self.renderer.end_frame();
        if self.first_frame {
            runtime_ui::completed(runtime_ui::FIRST_FRAME)?;
            runtime_ui::finish()?;
            self.first_frame = false;
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn native_ui(&mut self) -> &mut GpuUi {
        &mut self.ui
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn dispatch_native(&mut self, time: f64) -> Result<(), String> {
        let Some(mut run) = self.ui.begin_native_dispatch(time / 1000.0)? else { return Ok(()) };
        let model = &mut self.model;
        let result = (|| {
            while self.ui.paint_next(&mut run, |frame| view::paint(model, frame))? {}
            self.ui.paint_layout(&mut run, |frame| view::paint(model, frame))
        })();
        self.ui.finish_native_dispatch(run, result)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn window_event(
        &mut self,
        event: &winit::event::WindowEvent,
        time: f64,
    ) -> Result<(), String> {
        self.dispatch_native(time)?;
        self.ui.window_event(event)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn accesskit_action(
        &mut self,
        action: egui::accesskit::ActionRequest,
        time: f64,
    ) -> Result<(), String> {
        self.dispatch_native(time)?;
        self.ui.accesskit_action(action)
    }
}
