//! Ordered UI input and editing receipts shared by native and browser transports.

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(test)]
mod cancel_tests;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
pub(super) use browser::BrowserInput as PlatformInput;
#[cfg(not(target_arch = "wasm32"))]
pub(super) use native::NativeInput as PlatformInput;

use super::batches::{DispatchEnd, InputPass, into_passes};
use super::edit::model::{Intent, Receipt};
use super::edit::session::Session;
#[cfg(target_arch = "wasm32")]
use super::edit::session::selection;
use super::edit::{EditorDescriptor, TextSelection};
use super::{HitRegion, UiCapture, WidgetGeometry};
use crate::platform::KeyStroke;
use egui::TouchPhase;
use egui::{Event as UiEvent, Key, Modifiers, Pos2, Rect, pos2, vec2};
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
use web_sys::HtmlTextAreaElement;

pub fn canvas_point(client: [f32; 2], bounds: [f32; 4], logical: [f32; 2]) -> Pos2 {
    pos2(
        (client[0] - bounds[0]) * logical[0] / bounds[2],
        (client[1] - bounds[1]) * logical[1] / bounds[3],
    )
}

pub fn key_from_browser(key: &str) -> Option<Key> {
    match key {
        " " => Some(Key::Space),
        "ArrowDown" => Some(Key::ArrowDown),
        "ArrowUp" => Some(Key::ArrowUp),
        "ArrowLeft" => Some(Key::ArrowLeft),
        "ArrowRight" => Some(Key::ArrowRight),
        "Esc" => Some(Key::Escape),
        _ => Key::from_name(key).or_else(|| Key::from_name(&key.to_ascii_uppercase())),
    }
}

#[derive(Default)]
struct InputQueue {
    events: Vec<UiEvent>,
    codes: Vec<KeyStroke>,
    dispatches: Vec<DispatchEnd>,
    edits: Vec<Receipt>,
    edit_sequence: u64,
    acknowledged_edit_sequence: u64,
    editors: Vec<EditorDescriptor>,
    session: Option<Session>,
    focused: bool,
    composing: bool,
    capture: UiCapture,
    logical: [f32; 2],
    hit_regions: Vec<HitRegion>,
    owned_pointers: std::collections::HashSet<i32>,
    primary_touch: Option<u64>,
    gamepad_keys: [bool; 8],
    gamepad_direction: Option<solomon_sim::input::MenuDirection>,
    next_gamepad_repeat_at: f64,
    scope: Option<String>,
    gamepad_active: bool,
    require_gamepad_neutral: bool,
    clipboard_error: Option<String>,
    #[cfg(target_arch = "wasm32")]
    native_dispatch: Option<Rc<dyn Fn() -> Result<(), String>>>,
}

impl InputQueue {
    fn has_native_input(&self) -> bool {
        !self.events.is_empty()
            || !self.codes.is_empty()
            || !self.edits.is_empty()
            || self.clipboard_error.is_some()
    }

    #[cfg(any(target_arch = "wasm32", test))]
    fn finish_pointer(
        &mut self,
        phase: TouchPhase,
        pos: Pos2,
        button: egui::PointerButton,
        modifiers: Modifiers,
        touch: bool,
    ) {
        if matches!(phase, TouchPhase::Cancel) {
            self.events.push(UiEvent::PointerCancelled);
            self.events.push(UiEvent::PointerGone);
            if touch {
                self.primary_touch = None;
            }
        } else {
            let pressed = matches!(phase, TouchPhase::Start);
            self.events.push(UiEvent::PointerButton { pos, button, pressed, modifiers });
            if touch && !pressed {
                self.primary_touch = None;
                self.events.push(UiEvent::PointerGone);
            }
        }
        self.finish_dispatch();
    }

    fn finish_dispatch(&mut self) {
        let end = DispatchEnd {
            events: self.events.len(),
            codes: self.codes.len(),
            edits: self.edits.len(),
            click_count: None,
            controller: false,
        };
        if !self.dispatches.last().copied().unwrap_or_default().same_endpoint(end) {
            self.dispatches.push(end);
        }
    }

    fn record_click(&mut self, pos: Pos2, count: i32) {
        if count <= 0 {
            return;
        }
        // Browser click follows pointerup synchronously. Retain its native series
        // on that release, before the animation frame drains the queued dispatches.
        for index in (0..self.dispatches.len()).rev() {
            let start = if index == 0 { 0 } else { self.dispatches[index - 1].events };
            let end = self.dispatches[index].events;
            let release = self.events[start..end].iter().rev().find_map(|event| match event {
                UiEvent::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } => Some(*pos),
                _ => None,
            });
            if let Some(release) = release {
                if release == pos && self.dispatches[index].click_count.is_none() {
                    self.dispatches[index].click_count = Some(count as u32);
                }
                return;
            }
        }
    }

    fn edit(&mut self, intent: Intent, caret: Option<Rect>) {
        self.edit_with_selection(intent, caret, None);
    }

    fn edit_with_selection(
        &mut self,
        intent: Intent,
        caret: Option<Rect>,
        selection: Option<TextSelection>,
    ) {
        self.edit_sequence += 1;
        let sequence = self.edit_sequence;
        let receipt = if let Some(session) = &mut self.session {
            session.receipt(sequence, intent, selection, caret)
        } else {
            Receipt::unbound(sequence, intent)
        };
        self.edits.push(receipt);
    }

    #[cfg(target_arch = "wasm32")]
    fn sync_selection(&mut self, textarea: &HtmlTextAreaElement) {
        let selected = selection(textarea);
        if self.session.as_ref().is_some_and(|session| {
            session.model.text() == textarea.value() && session.model.selection() != selected
        }) {
            self.edit(Intent::Select(selected), None);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn sync_editor(&mut self, textarea: &HtmlTextAreaElement) {
        if let Some(session) = &self.session
            && let Err(error) = session.sync(textarea)
        {
            self.clipboard_error = Some(error);
        }
    }

    fn set_scope(&mut self, scope: Option<&str>) {
        if self.scope.as_deref() != scope {
            self.require_gamepad_neutral = scope.is_some();
            self.gamepad_direction = None;
            self.next_gamepad_repeat_at = 0.0;
            self.scope = scope.map(str::to_owned);
        }
    }

    fn scoped_gamepad(
        &mut self,
        current: solomon_sim::input::MenuGamepadState,
    ) -> solomon_sim::input::MenuGamepadState {
        self.gamepad_active = current.active();
        if !self.gamepad_active {
            self.require_gamepad_neutral = false;
        }
        if self.scope.is_none() || self.require_gamepad_neutral {
            solomon_sim::input::MenuGamepadState::default()
        } else {
            current
        }
    }
}

impl InputQueue {
    fn append_gamepad(
        &mut self,
        current: solomon_sim::input::MenuGamepadState,
        raw: &mut egui::RawInput,
        dispatches: &mut Vec<DispatchEnd>,
        codes: usize,
        edits: usize,
        time: f64,
    ) {
        use solomon_sim::input::{MenuDirection, advance_menu_direction_repeat};
        let native_events = raw.events.len();
        let current = self.scoped_gamepad(current);
        let held = [
            current.direction == Some(MenuDirection::Up),
            current.direction == Some(MenuDirection::Down),
            current.direction == Some(MenuDirection::Left),
            current.direction == Some(MenuDirection::Right),
            current.confirm,
            current.back,
            current.previous,
            current.next,
        ];
        let (repeat, next_repeat) = advance_menu_direction_repeat(
            self.gamepad_direction,
            current.direction,
            time * 1000.0,
            self.next_gamepad_repeat_at,
        );
        for (index, key) in [
            Key::ArrowUp,
            Key::ArrowDown,
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::Enter,
            Key::Escape,
            Key::Tab,
            Key::Tab,
        ]
        .into_iter()
        .enumerate()
        {
            if held[index] != self.gamepad_keys[index] || index < 4 && held[index] && repeat {
                raw.events.push(UiEvent::Key {
                    key,
                    physical_key: None,
                    pressed: held[index],
                    repeat: held[index] && self.gamepad_keys[index],
                    modifiers: Modifiers { shift: index == 6, ..Default::default() },
                });
            }
        }
        self.gamepad_keys = held;
        self.gamepad_direction = current.direction;
        self.next_gamepad_repeat_at = next_repeat;
        if raw.events.len() != native_events {
            dispatches.push(DispatchEnd {
                events: raw.events.len(),
                codes,
                edits,
                click_count: None,
                controller: true,
            });
        }
    }
}
