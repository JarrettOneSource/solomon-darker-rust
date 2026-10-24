//! Desktop launcher rendered by the same Rust/WASM UI as the game.
//! Source `desktop/launcher.{html,css,mjs}` at c537dc9; Electron retains native IPC.

mod model;
mod view;

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Function, JSON};
use solomon_graphics::Renderer;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlCanvasElement, Window};

use crate::gpu_ui::GpuUi;
use crate::runtime_ui;
use model::{Action, DesktopState, Model};

thread_local! {
    static LAUNCHER: RefCell<Option<Rc<RefCell<Launcher>>>> = const { RefCell::new(None) };
    static PAINTED: RefCell<Option<String>> = const { RefCell::new(None) };
}

struct Launcher {
    window: Window,
    canvas: HtmlCanvasElement,
    renderer: Renderer,
    ui: GpuUi,
    model: Rc<RefCell<Model>>,
    desktop: JsValue,
    _on_state: Closure<dyn FnMut(JsValue)>,
    remove_state: Function,
    first_frame: bool,
}

pub(crate) fn requested(window: &Window) -> Result<bool, String> {
    Ok(window.location().pathname().map_err(super::describe)? == "/launcher"
        && super::desktop(window)?.is_some())
}

pub(crate) fn snapshot() -> Option<String> {
    PAINTED.with(|state| state.borrow().clone())
}

pub(crate) async fn boot(window: Window) -> Result<(), String> {
    runtime_ui::begin_desktop(GpuUi::startup_resources())?;
    runtime_ui::completed(runtime_ui::RUNTIME)?;
    let document = window.document().ok_or("The launcher document is unavailable.")?;
    let canvas: HtmlCanvasElement = document
        .get_element_by_id("game")
        .ok_or("The launcher canvas is unavailable.")?
        .dyn_into()
        .map_err(|_| "The launcher surface is not a canvas.")?;
    let renderer = Renderer::for_canvas(&canvas)?;
    let ui = GpuUi::load(&document, &canvas, &renderer).await?;
    runtime_ui::enter_phase("desktop")?;
    let desktop = super::desktop(&window)?.ok_or("The desktop preload is unavailable.")?;
    let initial = super::settle(super::method(&desktop, "getState")?.call0(&desktop)).await?;
    let model = Rc::new(RefCell::new(Model::new(parse_state(initial)?)));
    let state_owner = Rc::downgrade(&model);
    let on_state = Closure::<dyn FnMut(JsValue)>::new(move |value| {
        if let Some(model) = state_owner.upgrade() {
            match parse_state(value) {
                Ok(state) => model.borrow_mut().receive(state),
                Err(error) => {
                    model.borrow_mut().failed(error.clone());
                    super::report(error);
                }
            }
        }
    });
    let remove_state = super::unsubscribe(
        super::method(&desktop, "onState")?
            .call1(&desktop, on_state.as_ref())
            .map_err(super::describe)?,
        "onState",
    )?;
    runtime_ui::completed(runtime_ui::DESKTOP)?;
    let launcher = Rc::new(RefCell::new(Launcher {
        window: window.clone(),
        canvas,
        renderer,
        ui,
        model,
        desktop,
        _on_state: on_state,
        remove_state,
        first_frame: true,
    }));
    let owner = Rc::downgrade(&launcher);
    let clock = document.timeline();
    launcher.borrow_mut().ui.set_native_dispatch_handler(move || {
        let Some(time) = clock.current_time() else {
            return Ok(());
        };
        let launcher = owner.upgrade().ok_or("The launcher is no longer available for input.")?;
        launcher.borrow_mut().dispatch_native(time)
    });
    LAUNCHER.with(|slot| *slot.borrow_mut() = Some(launcher.clone()));
    let callback: crate::FrameCallback = Rc::new(RefCell::new(None));
    let next = callback.clone();
    let requester = window.clone();
    *callback.borrow_mut() = Some(Closure::new(move |time: f64| {
        if let Err(error) = launcher.borrow_mut().frame(time) {
            runtime_ui::startup_failed(&error);
            super::report(error);
            return;
        }
        if let Some(callback) = next.borrow().as_ref() {
            crate::request_frame(&requester, callback);
        }
    }));
    if let Some(callback) = callback.borrow().as_ref() {
        crate::request_frame(&window, callback);
    }
    Ok(())
}

impl Launcher {
    fn dispatch_native(&mut self, time: f64) -> Result<(), String> {
        let Some(mut run) = self.ui.begin_native_dispatch(time / 1000.0)? else {
            return Ok(());
        };
        let model = self.model.clone();
        let result = (|| {
            while self
                .ui
                .paint_next(&mut run, |frame| view::paint(&mut model.borrow_mut(), frame))?
            {
            }
            self.ui.paint_layout(&mut run, |frame| view::paint(&mut model.borrow_mut(), frame))
        })();
        self.ui.finish_native_dispatch(run, result)?;
        self.start_pending();
        Ok(())
    }

    fn frame(&mut self, time: f64) -> Result<(), String> {
        let viewport = [self.canvas.client_width() as f32, self.canvas.client_height() as f32];
        if viewport.iter().any(|size| *size <= 0.0) {
            return Ok(());
        }
        let ratio = self.window.device_pixel_ratio();
        let width = (f64::from(viewport[0]) * ratio).round().max(1.0) as u32;
        let height = (f64::from(viewport[1]) * ratio).round().max(1.0) as u32;
        if self.canvas.width() != width {
            self.canvas.set_width(width);
        }
        if self.canvas.height() != height {
            self.canvas.set_height(height);
        }
        self.renderer.begin_frame(width as i32, height as i32, viewport);
        let model = self.model.clone();
        self.ui.run(&mut self.renderer, viewport, time / 1000.0, |frame| {
            view::paint(&mut model.borrow_mut(), frame)
        })?;
        self.renderer.end_frame();
        let snapshot = self.ui.snapshot("desktop-launcher", true, None);
        PAINTED.with(|state| *state.borrow_mut() = Some(snapshot.to_string()));
        if self.first_frame {
            runtime_ui::completed(runtime_ui::FIRST_FRAME)?;
            runtime_ui::finish()?;
            self.first_frame = false;
        }
        self.start_pending();
        Ok(())
    }

    fn start_pending(&self) {
        let Some(action) = self.model.borrow_mut().pending.take() else {
            return;
        };
        let desktop = self.desktop.clone();
        let owner = Rc::downgrade(&self.model);
        spawn_local(async move {
            let result = perform(&desktop, &action).await;
            let Some(model) = owner.upgrade() else { return };
            let mut model = model.borrow_mut();
            match result {
                Ok(Some(state)) => {
                    model.status = state.update_error.clone().unwrap_or_else(|| {
                        if state.update.is_some() {
                            "An update is ready to download."
                        } else {
                            "You have the latest available build."
                        }
                        .into()
                    });
                    model.checking = false;
                    model.receive(state);
                }
                Ok(None) => {}
                Err(error) => model.failed(error),
            }
        });
    }
}

impl Drop for Launcher {
    fn drop(&mut self) {
        if let Err(error) = self.remove_state.call0(&JsValue::UNDEFINED) {
            super::report(super::describe(error));
        }
    }
}

fn parse_state(value: JsValue) -> Result<DesktopState, String> {
    let text = JSON::stringify(&value)
        .map_err(super::describe)?
        .as_string()
        .ok_or("The desktop state has no JSON representation.")?;
    serde_json::from_str(&text).map_err(|error| format!("Invalid desktop state: {error}"))
}

async fn perform(desktop: &JsValue, action: &Action) -> Result<Option<DesktopState>, String> {
    let (method, argument) = match action {
        Action::Start(options) => {
            let text = serde_json::to_string(options)
                .map_err(|error| format!("Desktop session options: {error}"))?;
            ("startSession", Some(JSON::parse(&text).map_err(super::describe)?))
        }
        Action::Check => ("checkUpdates", None),
        Action::Download => ("downloadUpdate", None),
        Action::Website => ("openWebsite", None),
    };
    let function = super::method(desktop, method)?;
    let value = super::settle(match argument {
        Some(value) => function.call1(desktop, &value),
        None => function.call0(desktop),
    })
    .await
    .map_err(|error| {
        error
            .strip_prefix("Error invoking remote method '")
            .and_then(|error| error.split_once("': Error: "))
            .map_or(error.clone(), |(_, message)| message.into())
    })?;
    if matches!(action, Action::Check) { Ok(Some(parse_state(value)?)) } else { Ok(None) }
}
