//! Browser fullscreen methods and trusted click ownership.
use super::{ControlMode, FullscreenState};
use crate::platform::Surface;
use egui::pos2;
use js_sys::{Function, Object, Promise, Reflect};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, Event, MouseEvent, Window};

const CHANGE_EVENTS: [&str; 2] = ["fullscreenchange", "webkitfullscreenchange"];

pub(super) struct Binding {
    document: Document,
    click: Closure<dyn FnMut(Event)>,
    change: Closure<dyn FnMut(Event)>,
}

fn document(surface: &Surface) -> Document {
    surface.canvas().owner_document().expect("a game canvas belongs to its document")
}

impl Binding {
    pub(super) fn attach(
        surface: &Surface,
        state: &Rc<RefCell<FullscreenState>>,
    ) -> Result<Self, String> {
        let document = document(surface);
        let clicked = state.clone();
        let clicked_surface = surface.clone();
        let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(mouse) = event.dyn_ref::<MouseEvent>() else { return };
            let point = pos2(mouse.client_x() as f32, mouse.client_y() as f32);
            if !clicked
                .borrow()
                .target
                .as_ref()
                .is_some_and(|target| crate::ui::trusted_gesture::owns_point(target, point))
            {
                return;
            }
            event.prevent_default();
            event.stop_propagation();
            super::activate(&clicked_surface, &clicked);
        });
        document
            .add_event_listener_with_callback_and_bool(
                "click",
                click.as_ref().unchecked_ref(),
                true,
            )
            .map_err(|_| "fullscreen trusted click")?;
        let changed = state.clone();
        let changed_document = document.clone();
        let change = Closure::<dyn FnMut(Event)>::new(move |_| {
            let mut state = changed.borrow_mut();
            state.active = active_document(&changed_document);
            state.error = None;
            state.show_help = false;
        });
        for name in CHANGE_EVENTS {
            document
                .add_event_listener_with_callback(name, change.as_ref().unchecked_ref())
                .map_err(|_| "fullscreen change listener")?;
        }
        Ok(Self { document, click, change })
    }
}

pub(super) fn mode(surface: &Surface) -> Result<ControlMode, String> {
    mode_document(&document(surface))
}

pub(super) fn active(surface: &Surface) -> bool {
    active_document(&document(surface))
}

pub(super) fn toggle(surface: &Surface) -> Result<super::FullscreenRequest, String> {
    let request = request_document(&document(surface)).map_err(error_message)?;
    Ok(Box::pin(async move {
        JsFuture::from(Promise::resolve(&request)).await.map(|_| ()).map_err(error_message)
    }))
}

fn property(target: &JsValue, name: &str) -> JsValue {
    Reflect::get(target, &JsValue::from_str(name)).unwrap_or(JsValue::UNDEFINED)
}

fn method(target: &JsValue, name: &str) -> Option<Function> {
    property(target, name).dyn_into().ok()
}

fn standard_supported(document: &Document, target: &Element) -> bool {
    property(document, "fullscreenEnabled").as_bool() != Some(false)
        && method(target, "requestFullscreen").is_some()
        && method(document, "exitFullscreen").is_some()
}

fn webkit_supported(document: &Document, target: &Element) -> bool {
    property(document, "webkitFullscreenEnabled").as_bool() != Some(false)
        && method(target, "webkitRequestFullscreen").is_some()
        && method(document, "webkitExitFullscreen").is_some()
}

fn mode_document(document: &Document) -> Result<ControlMode, String> {
    let window = document.default_view().ok_or("fullscreen window")?;
    let target = document.document_element().ok_or("fullscreen document root")?;
    if standard_supported(document, &target) || webkit_supported(document, &target) {
        return Ok(ControlMode::Fullscreen);
    }
    Ok(if installed_display_mode(&window)? { ControlMode::Hidden } else { ControlMode::Install })
}

fn current_fullscreen_element(document: &Document) -> JsValue {
    let standard = property(document, "fullscreenElement");
    if standard.is_null_or_undefined() {
        property(document, "webkitFullscreenElement")
    } else {
        standard
    }
}

fn active_document(document: &Document) -> bool {
    document
        .document_element()
        .is_some_and(|target| current_fullscreen_element(document) == JsValue::from(target))
}

fn installed_display_mode(window: &Window) -> Result<bool, String> {
    if property(&window.navigator(), "standalone").as_bool() == Some(true) {
        return Ok(true);
    }
    for query in ["(display-mode: standalone)", "(display-mode: fullscreen)"] {
        if window
            .match_media(query)
            .map_err(|_| "fullscreen display mode query")?
            .is_some_and(|query| query.matches())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn request_document(document: &Document) -> Result<JsValue, JsValue> {
    let target = document
        .document_element()
        .ok_or_else(|| JsValue::from_str("Fullscreen document root is unavailable."))?;
    if !current_fullscreen_element(document).is_null_or_undefined() {
        let standard = property(document, "fullscreenElement");
        let exit = if !standard.is_null_or_undefined() {
            method(document, "exitFullscreen")
        } else {
            method(document, "webkitExitFullscreen")
        };
        let exit = exit
            .or_else(|| method(document, "exitFullscreen"))
            .or_else(|| method(document, "webkitExitFullscreen"));
        return exit
            .ok_or_else(|| JsValue::from_str("Fullscreen is not available in this browser."))?
            .call0(document);
    }
    if standard_supported(document, &target) {
        let options = Object::new();
        Reflect::set(&options, &JsValue::from_str("navigationUI"), &JsValue::from_str("hide"))?;
        return method(&target, "requestFullscreen")
            .expect("supported fullscreen request")
            .call1(&target, &options);
    }
    if webkit_supported(document, &target) {
        return method(&target, "webkitRequestFullscreen")
            .expect("supported WebKit fullscreen request")
            .call0(&target);
    }
    Err(JsValue::from_str("Fullscreen is not available in this browser."))
}

fn error_message(error: JsValue) -> String {
    property(&error, "message")
        .as_string()
        .unwrap_or_else(|| "Fullscreen could not be changed.".to_owned())
}

impl Drop for Binding {
    fn drop(&mut self) {
        if let Err(error) = self.document.remove_event_listener_with_callback_and_bool(
            "click",
            self.click.as_ref().unchecked_ref(),
            true,
        ) {
            web_sys::console::error_1(&error);
        }
        for name in CHANGE_EVENTS {
            if let Err(error) = self
                .document
                .remove_event_listener_with_callback(name, self.change.as_ref().unchecked_ref())
            {
                web_sys::console::error_1(&error);
            }
        }
    }
}
