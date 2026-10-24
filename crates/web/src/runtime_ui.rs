//! Startup handoff and runtime notices. Rust owns both GPU canvases; the browser
//! bridge only carries state between the loader and main WASM modules.

use crate::gpu_ui::UiFrame;
use egui::{Align2, Color32, FontFamily, FontId, Id, Order, pos2};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

mod startup;
pub(crate) use startup::{ACCOUNT, DESKTOP, FIRST_FRAME, RUNTIME, StartupResource};
use startup::{Snapshot, StartupProgress};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "solomonLoader"], js_name = progress, catch)]
    fn loader_progress(
        stage: &str,
        current_item: &str,
        completed: u32,
        total: u32,
    ) -> Result<(), JsValue>;
    #[wasm_bindgen(js_namespace = ["window", "solomonLoader"], js_name = error, catch)]
    fn loader_error(message: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(js_namespace = ["window", "solomonLoader"], js_name = finish, catch)]
    fn loader_finish() -> Result<(), JsValue>;
    #[wasm_bindgen(js_namespace = ["window", "solomonLoader"], js_name = active)]
    fn loader_active() -> bool;
    #[wasm_bindgen(js_namespace = ["window", "solomonLoader"], js_name = state)]
    fn loader_state() -> String;
}

thread_local! {
    static STATUS: RefCell<String> = const { RefCell::new(String::new()) };
    static ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
    static STARTUP: RefCell<Option<StartupProgress>> = const { RefCell::new(None) };
}

pub(crate) fn begin(resources: impl IntoIterator<Item = StartupResource>) -> Result<(), String> {
    let progress = StartupProgress::new(resources)?;
    let snapshot = progress.snapshot();
    STARTUP.with(|state| *state.borrow_mut() = Some(progress));
    publish(snapshot)
}

pub(crate) fn begin_desktop(
    resources: impl IntoIterator<Item = StartupResource>,
) -> Result<(), String> {
    let progress = StartupProgress::for_desktop(resources)?;
    let snapshot = progress.snapshot();
    STARTUP.with(|state| *state.borrow_mut() = Some(progress));
    publish(snapshot)
}

pub(crate) fn enter_phase(stage: &'static str) -> Result<(), String> {
    update(|progress| progress.enter_phase(stage))
}

pub(crate) fn completed(source: &str) -> Result<(), String> {
    update(|progress| progress.complete(source))
}

fn update(change: impl FnOnce(&mut StartupProgress) -> Result<(), String>) -> Result<(), String> {
    let snapshot = STARTUP.with(|state| -> Result<Option<Snapshot>, String> {
        let mut state = state.borrow_mut();
        let Some(progress) = state.as_mut() else {
            return Ok(None);
        };
        change(progress)?;
        Ok(Some(progress.snapshot()))
    })?;
    if let Some(snapshot) = snapshot {
        publish(snapshot)?;
    }
    Ok(())
}

fn publish(snapshot: Snapshot) -> Result<(), String> {
    loader_progress(snapshot.stage, &snapshot.source, snapshot.completed, snapshot.total)
        .map_err(|error| format!("Startup progress: {error:?}"))
}
pub(crate) fn finish() -> Result<(), String> {
    STARTUP.with(|state| state.borrow().as_ref().map_or(Ok(()), StartupProgress::finish))?;
    loader_finish().map_err(|error| format!("Startup handoff: {error:?}"))?;
    STARTUP.with(|progress| progress.borrow_mut().take());
    Ok(())
}
pub(crate) fn loading_snapshot() -> Option<String> {
    loader_active().then(loader_state)
}
pub(crate) fn startup_failed(message: &str) {
    set_error(message);
    if let Err(error) = loader_error(message) {
        web_sys::console::error_1(&error);
    }
}
pub(crate) fn set_status(message: &str) {
    STATUS.with(|status| *status.borrow_mut() = message.to_owned());
}
pub(crate) fn set_error(message: &str) {
    ERROR.with(|error| *error.borrow_mut() = Some(message.to_owned()));
    set_status(message);
}
pub(crate) fn error() -> Option<String> {
    ERROR.with(|error| error.borrow().clone())
}
pub(crate) fn layer_id() -> egui::LayerId {
    egui::LayerId::new(Order::Tooltip, Id::unique("runtime-status"))
}

pub(crate) fn paint(frame: &mut UiFrame<'_>) {
    STATUS.with(|status| {
        let status = status.borrow();
        if status.is_empty() {
            return;
        }
        let painter = frame.ctx.layer_painter(layer_id());
        let screen = frame.ctx.content_rect();
        let position = pos2(screen.left() + 12.0, screen.bottom() - 10.0);
        let color = if error().is_some() {
            Color32::from_rgb(255, 123, 123)
        } else {
            Color32::from_rgb(216, 210, 196)
        };
        let bounds = painter.text(
            position,
            Align2::LEFT_BOTTOM,
            status.as_str(),
            FontId::new(14.0, FontFamily::Name("Arial".into())),
            color,
        );
        frame.record_rect("runtime:status", status.as_str(), "status", bounds, true);
        frame.record_value("runtime:status", status.as_str(), None);
        frame.announce("runtime:status", status.as_str());
    });
}
