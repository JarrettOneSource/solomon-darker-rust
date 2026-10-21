//! The Settings fullscreen row shares the browser adapter with the edge control.
//! Keep the request inside its trusted click and unregister page-owned listeners.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::Promise;
use solomon_sim::native_ui_text::NativeUiTextAlign;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Event, HtmlElement};

use crate::fullscreen::{
    ControlMode, error_message, fullscreen_active, game_fullscreen_control_mode, toggle_fullscreen,
};
use crate::ui::settings::{NativeSettingsArt, settings_text};
use crate::ui::{append, element};

const CHANGE_EVENTS: [&str; 2] = ["fullscreenchange", "webkitfullscreenchange"];

struct State {
    mode: ControlMode,
    active: bool,
    show_install_help: bool,
    error: Option<String>,
    button: HtmlElement,
    indicator: HtmlElement,
    help: HtmlElement,
    alert: HtmlElement,
    art: Rc<NativeSettingsArt>,
    document: Document,
}

impl State {
    fn paint(&self) -> Result<(), String> {
        self.indicator.set_text_content(None);
        if self.mode == ControlMode::Install {
            self.button
                .set_attribute(
                    "aria-expanded",
                    if self.show_install_help { "true" } else { "false" },
                )
                .map_err(|_| "settings fullscreen help state")?;
            append(
                &self.indicator,
                &settings_text(
                    &self.document,
                    "OPTIONS",
                    1.0,
                    0xd9bd72,
                    Some((NativeUiTextAlign::Right, 82.0)),
                )?,
            )?;
        } else {
            self.button
                .set_attribute("aria-pressed", if self.active { "true" } else { "false" })
                .map_err(|_| "settings fullscreen state")?;
            append(&self.indicator, &self.art.toggle(&self.document, self.active)?)?;
        }
        self.help.set_hidden(!self.show_install_help);
        self.alert.set_hidden(self.error.is_none());
        self.alert.set_text_content(self.error.as_deref());
        Ok(())
    }
}

pub(super) struct SettingsFullscreen {
    pub root: HtmlElement,
    state: Rc<RefCell<State>>,
    document: Document,
    click: Closure<dyn FnMut(Event)>,
    change: Closure<dyn FnMut(Event)>,
}

impl SettingsFullscreen {
    pub fn new(document: &Document, art: Rc<NativeSettingsArt>) -> Result<Option<Self>, String> {
        let mode = game_fullscreen_control_mode(document)?;
        if mode == ControlMode::Hidden {
            return Ok(None);
        }
        let root = element(document, "div", "game-settings-fullscreen")?;
        let button = element(document, "button", "game-settings-native-toggle-row")?;
        button.set_attribute("type", "button").map_err(|_| "settings fullscreen type")?;
        button
            .set_attribute(
                "aria-label",
                if mode == ControlMode::Install { "FULLSCREEN, OPTIONS" } else { "FULLSCREEN" },
            )
            .map_err(|_| "settings fullscreen label")?;
        button
            .set_attribute("data-settings-fullscreen", "")
            .map_err(|_| "settings fullscreen hook")?;
        art.row(document, &button)?;
        let label = element(document, "span", "game-settings-native-label")?;
        append(&label, &settings_text(document, "FULLSCREEN", 1.0, 0xdbc174, None)?)?;
        append(&button, &label)?;
        let indicator =
            element(document, if mode == ControlMode::Install { "strong" } else { "span" }, "")?;
        append(&button, &indicator)?;
        append(&root, &button)?;
        let help = element(document, "small", "")?;
        help.set_attribute("role", "status").map_err(|_| "settings fullscreen help role")?;
        help.set_text_content(Some(
            "Install this page as a web app for fullscreen on iPhone or iPad.",
        ));
        append(&root, &help)?;
        let alert = element(document, "small", "")?;
        alert.set_attribute("role", "alert").map_err(|_| "settings fullscreen alert role")?;
        append(&root, &alert)?;
        let state = Rc::new(RefCell::new(State {
            mode,
            active: fullscreen_active(document),
            show_install_help: false,
            error: None,
            button,
            indicator,
            help,
            alert,
            art,
            document: document.clone(),
        }));
        state.borrow().paint()?;
        let clicked = state.clone();
        let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            event.prevent_default();
            let mut state = clicked.borrow_mut();
            state.error = None;
            if state.mode == ControlMode::Install {
                state.show_install_help = !state.show_install_help;
                report(state.paint());
                return;
            }
            report(state.paint());
            let request = toggle_fullscreen(&state.document);
            drop(state);
            let completed = clicked.clone();
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
            .map_err(|_| "settings fullscreen click listener")?;
        let changed = state.clone();
        let change = Closure::<dyn FnMut(Event)>::new(move |_| {
            let mut state = changed.borrow_mut();
            state.active = fullscreen_active(&state.document);
            state.error = None;
            state.show_install_help = false;
            report(state.paint());
        });
        for event in CHANGE_EVENTS {
            document
                .add_event_listener_with_callback(event, change.as_ref().unchecked_ref())
                .map_err(|_| "settings fullscreen change listener")?;
        }
        Ok(Some(Self { root, state, document: document.clone(), click, change }))
    }
}

impl Drop for SettingsFullscreen {
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
    }
}

fn report(result: Result<(), String>) {
    if let Err(error) = result {
        web_sys::console::error_1(&JsValue::from_str(&error));
    }
}
