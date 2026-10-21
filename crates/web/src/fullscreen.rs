//! Browser-owned fullscreen control from Website GameFullscreenButton/game-fullscreen.
//! Requests run in the original trusted click; only promise completion is asynchronous.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Function, Object, Promise, Reflect};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, Event, HtmlElement, Window};

use crate::ui::{append, element, style};

const CHANGE_EVENTS: [&str; 2] = ["fullscreenchange", "webkitfullscreenchange"];
const ENTER_PATH: &str = "M3 9V3h6M21 9V3h-6M3 15v6h6M21 15v6h-6";
const EXIT_PATH: &str = "M9 3v6H3M15 3v6h6M9 21v-6H3M15 21v-6h6";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FullscreenPlacement {
    Hidden,
    Gameplay,
    Title,
    DarkCloud,
    Create { frame_scale: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ControlMode {
    Fullscreen,
    Install,
    Hidden,
}

struct FullscreenState {
    mode: ControlMode,
    active: bool,
    show_help: bool,
    error: Option<String>,
    button: HtmlElement,
    path: Element,
    help: HtmlElement,
    alert: HtmlElement,
}

impl FullscreenState {
    fn paint(&self) -> Result<(), String> {
        let label = match self.mode {
            ControlMode::Install => "Fullscreen options",
            _ if self.active => "Exit fullscreen",
            _ => "Enter fullscreen",
        };
        self.button.set_attribute("aria-label", label).map_err(|_| "fullscreen label")?;
        self.button.set_attribute("title", label).map_err(|_| "fullscreen title")?;
        self.path
            .set_attribute("d", if self.active { EXIT_PATH } else { ENTER_PATH })
            .map_err(|_| "fullscreen icon")?;
        if self.mode == ControlMode::Install {
            self.button
                .set_attribute("aria-controls", "game-fullscreen-help")
                .map_err(|_| "fullscreen help link")?;
            self.button
                .set_attribute("aria-expanded", if self.show_help { "true" } else { "false" })
                .map_err(|_| "fullscreen help state")?;
        } else {
            self.button
                .set_attribute("aria-pressed", if self.active { "true" } else { "false" })
                .map_err(|_| "fullscreen state")?;
        }
        self.help.set_hidden(!self.show_help);
        self.alert.set_hidden(self.error.is_none());
        self.alert.set_text_content(self.error.as_deref());
        Ok(())
    }
}

pub struct FullscreenControl {
    document: Document,
    root: HtmlElement,
    state: Rc<RefCell<FullscreenState>>,
    click: Closure<dyn FnMut(Event)>,
    change: Closure<dyn FnMut(Event)>,
    placement: FullscreenPlacement,
}

impl FullscreenControl {
    pub fn attach(document: &Document) -> Result<Self, String> {
        let mode = game_fullscreen_control_mode(document)?;
        let root = element(document, "div", "game-fullscreen-control")?;
        let button = element(document, "button", "game-fullscreen-button")?;
        button.set_attribute("type", "button").map_err(|_| "fullscreen button type")?;
        button.set_attribute("data-game-fullscreen", "").map_err(|_| "fullscreen hook")?;
        button
            .set_attribute(
                "data-game-fullscreen-mode",
                if mode == ControlMode::Fullscreen { "fullscreen" } else { "install" },
            )
            .map_err(|_| "fullscreen mode")?;
        let svg = document
            .create_element_ns(Some("http://www.w3.org/2000/svg"), "svg")
            .map_err(|_| "fullscreen SVG")?;
        svg.set_attribute("viewBox", "0 0 24 24").map_err(|_| "fullscreen SVG viewport")?;
        svg.set_attribute("aria-hidden", "true").map_err(|_| "fullscreen SVG semantics")?;
        let path = document
            .create_element_ns(Some("http://www.w3.org/2000/svg"), "path")
            .map_err(|_| "fullscreen SVG path")?;
        svg.append_child(&path).map_err(|_| "fullscreen path parent")?;
        button.append_child(&svg).map_err(|_| "fullscreen SVG parent")?;
        let help = element(document, "div", "game-fullscreen-message")?;
        help.set_id("game-fullscreen-help");
        help.set_attribute("role", "status").map_err(|_| "fullscreen help role")?;
        for (tag, text) in [
            ("strong", "Fullscreen on iPhone or iPad"),
            (
                "span",
                "Tap Share, Add to Home Screen, keep Open as Web App enabled if shown, then launch Solomon Darker from its icon.",
            ),
            ("span", "Otherwise, open the game in a browser that supports fullscreen."),
        ] {
            let node = element(document, tag, "")?;
            node.set_text_content(Some(text));
            append(&help, &node)?;
        }
        let alert = element(document, "div", "game-fullscreen-message")?;
        alert.set_attribute("role", "alert").map_err(|_| "fullscreen error role")?;
        for node in [&button, &help, &alert] {
            append(&root, node)?;
        }
        let state = Rc::new(RefCell::new(FullscreenState {
            mode,
            active: fullscreen_active(document),
            show_help: false,
            error: None,
            button,
            path,
            help,
            alert,
        }));
        state.borrow().paint()?;
        let clicked_state = state.clone();
        let clicked_document = document.clone();
        let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            event.prevent_default();
            let mut state = clicked_state.borrow_mut();
            state.error = None;
            if state.mode == ControlMode::Install {
                state.show_help = !state.show_help;
                report(state.paint());
                return;
            }
            report(state.paint());
            drop(state);
            // Invoke requestFullscreen synchronously, before yielding the click's user activation.
            let request = toggle_fullscreen(&clicked_document);
            let completed = clicked_state.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = match request {
                    Ok(value) => JsFuture::from(Promise::resolve(&value)).await,
                    Err(error) => Err(error),
                };
                if let Err(error) = result {
                    let mut state = completed.borrow_mut();
                    state.error = Some(error_message(error));
                    report(state.paint());
                }
            });
        });
        state
            .borrow()
            .button
            .add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
            .map_err(|_| "fullscreen click listener")?;
        let changed_state = state.clone();
        let changed_document = document.clone();
        let change = Closure::<dyn FnMut(Event)>::new(move |_| {
            let mut state = changed_state.borrow_mut();
            state.active = fullscreen_active(&changed_document);
            state.error = None;
            state.show_help = false;
            report(state.paint());
        });
        for event in CHANGE_EVENTS {
            document
                .add_event_listener_with_callback(event, change.as_ref().unchecked_ref())
                .map_err(|_| "fullscreen change listener")?;
        }
        append(&document.body().ok_or("fullscreen body")?, &root)?;
        root.set_hidden(true);
        Ok(Self {
            document: document.clone(),
            root,
            state,
            click,
            change,
            placement: FullscreenPlacement::Hidden,
        })
    }

    /// Root projects the source scene visibility; Skills, open Chat and pre-loadout
    /// College admission use Hidden. Title includes the Play/Hall title routes.
    pub fn update(&mut self, placement: FullscreenPlacement) -> Result<(), String> {
        if self.placement == placement {
            return Ok(());
        }
        self.placement = placement;
        self.root.set_hidden(
            placement == FullscreenPlacement::Hidden
                || self.state.borrow().mode == ControlMode::Hidden,
        );
        let (id, css) = match placement {
            FullscreenPlacement::Hidden => ("hidden", "".to_owned()),
            FullscreenPlacement::Gameplay => {
                ("gameplay", "top:8px;right:49px;bottom:auto".to_owned())
            }
            FullscreenPlacement::Title => ("title", "top:auto;right:8px;bottom:8px".to_owned()),
            FullscreenPlacement::DarkCloud => {
                ("dark-cloud", "top:8px;right:8px;bottom:auto".to_owned())
            }
            FullscreenPlacement::Create { frame_scale } => (
                "create",
                format!(
                    "top:{}px;right:{}px;bottom:auto",
                    10.0 * frame_scale,
                    8.0 + 80.0 * frame_scale
                ),
            ),
        };
        self.root.set_attribute("data-placement", id).map_err(|_| "fullscreen placement")?;
        style(&self.root, &css)
    }
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

pub(crate) fn game_fullscreen_control_mode(document: &Document) -> Result<ControlMode, String> {
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

pub(crate) fn fullscreen_active(document: &Document) -> bool {
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

pub(crate) fn toggle_fullscreen(document: &Document) -> Result<JsValue, JsValue> {
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

pub(crate) fn error_message(error: JsValue) -> String {
    property(&error, "message")
        .as_string()
        .unwrap_or_else(|| "Fullscreen could not be changed.".to_owned())
}

fn report(result: Result<(), String>) {
    if let Err(error) = result {
        web_sys::console::error_1(&JsValue::from_str(&error));
    }
}

impl Drop for FullscreenControl {
    fn drop(&mut self) {
        let _ = self
            .state
            .borrow()
            .button
            .remove_event_listener_with_callback("click", self.click.as_ref().unchecked_ref());
        for event in CHANGE_EVENTS {
            let _ = self
                .document
                .remove_event_listener_with_callback(event, self.change.as_ref().unchecked_ref());
        }
        self.root.remove();
    }
}
