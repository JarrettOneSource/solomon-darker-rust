//! Native canvas events and a nonvisual IME/clipboard adapter.

#[cfg(test)]
mod cancel_tests;

use std::cell::RefCell;
use std::rc::Rc;

use egui::{Event as UiEvent, Key, Modifiers, Pos2, Rect, TouchPhase, pos2, vec2};
use serde::Serialize;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{
    ClipboardEvent, CompositionEvent, Document, Event, EventTarget, HtmlCanvasElement,
    HtmlTextAreaElement, InputEvent, KeyboardEvent, PointerEvent, WheelEvent,
};

use super::batches::{DispatchEnd, InputPass, into_passes};
use super::edit::model::{EditGroup, Intent, Receipt};
use super::edit::session::{Session, selection};
use super::edit::{EditorDescriptor, TextSelection};
use super::{HitRegion, UiCapture, WidgetGeometry};

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

#[derive(Clone, Debug, Serialize)]
pub struct BrowserKey {
    pub code: String,
    pub key: String,
    pub pressed: bool,
    pub repeat: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub meta: bool,
}

#[derive(Default)]
struct InputQueue {
    events: Vec<UiEvent>,
    codes: Vec<BrowserKey>,
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
    primary_touch: Option<i32>,
    gamepad_keys: [bool; 8],
    gamepad_direction: Option<solomon_sim::input::MenuDirection>,
    next_gamepad_repeat_at: f64,
    scope: Option<String>,
    gamepad_active: bool,
    require_gamepad_neutral: bool,
    clipboard_error: Option<String>,
    native_dispatch: Option<Rc<dyn Fn() -> Result<(), String>>>,
}

impl InputQueue {
    fn has_native_input(&self) -> bool {
        !self.events.is_empty()
            || !self.codes.is_empty()
            || !self.edits.is_empty()
            || self.clipboard_error.is_some()
    }

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
        self.edit_sequence += 1;
        let sequence = self.edit_sequence;
        let receipt = if let Some(session) = &mut self.session {
            session.receipt(sequence, intent, None, caret)
        } else {
            Receipt::unbound(sequence, intent)
        };
        self.edits.push(receipt);
    }

    fn sync_selection(&mut self, textarea: &HtmlTextAreaElement) {
        let selected = selection(textarea);
        if self.session.as_ref().is_some_and(|session| {
            session.model.text() == textarea.value() && session.model.selection() != selected
        }) {
            self.edit(Intent::Select(selected), None);
        }
    }

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

struct Listener {
    target: EventTarget,
    name: &'static str,
    callback: Closure<dyn FnMut(Event)>,
    capture: bool,
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.target.remove_event_listener_with_callback_and_bool(
            self.name,
            self.callback.as_ref().unchecked_ref(),
            self.capture,
        );
    }
}

pub(super) struct BrowserInput {
    document: Document,
    canvas: HtmlCanvasElement,
    textarea: HtmlTextAreaElement,
    queue: Rc<RefCell<InputQueue>>,
    listeners: Vec<Listener>,
}

fn modifiers(alt: bool, ctrl: bool, shift: bool, meta: bool) -> Modifiers {
    let is_mac = web_sys::window()
        .is_some_and(|window| window.navigator().platform().unwrap_or_default().contains("Mac"));
    Modifiers { alt, ctrl, shift, mac_cmd: meta, command: if is_mac { meta } else { ctrl } }
}

fn listen(
    target: &EventTarget,
    name: &'static str,
    listeners: &mut Vec<Listener>,
    callback: impl FnMut(Event) + 'static,
) -> Result<(), String> {
    listen_phase(target, name, listeners, false, callback)
}

fn listen_phase(
    target: &EventTarget,
    name: &'static str,
    listeners: &mut Vec<Listener>,
    capture: bool,
    callback: impl FnMut(Event) + 'static,
) -> Result<(), String> {
    let callback = Closure::<dyn FnMut(Event)>::new(callback);
    target
        .add_event_listener_with_callback_and_bool(name, callback.as_ref().unchecked_ref(), capture)
        .map_err(|error| format!("GPU UI {name} listener: {error:?}"))?;
    listeners.push(Listener { target: target.clone(), name, callback, capture });
    Ok(())
}

fn point(canvas: &HtmlCanvasElement, queue: &InputQueue, x: i32, y: i32) -> Pos2 {
    let rect = canvas.get_bounding_client_rect();
    canvas_point(
        [x as f32, y as f32],
        [rect.x() as f32, rect.y() as f32, rect.width() as f32, rect.height() as f32],
        queue.logical,
    )
}

impl BrowserInput {
    pub fn attach(
        document: &Document,
        canvas: &HtmlCanvasElement,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        let textarea: HtmlTextAreaElement = document
            .create_element("textarea")
            .map_err(|error| format!("GPU UI composition adapter: {error:?}"))?
            .dyn_into()
            .map_err(|_| "composition adapter is not a textarea")?;
        for (name, value) in [
            ("data-gpu-text-input", ""),
            ("aria-hidden", "true"),
            ("tabindex", "-1"),
            ("autocomplete", "off"),
            ("autocapitalize", "off"),
            ("spellcheck", "false"),
            ("wrap", "off"),
            (
                "style",
                "position:fixed;left:0;top:0;width:1px;height:1px;opacity:0;pointer-events:none;border:0;padding:0;resize:none",
            ),
        ] {
            textarea
                .set_attribute(name, value)
                .map_err(|error| format!("GPU UI composition attribute: {error:?}"))?;
        }
        document
            .body()
            .ok_or("no body for composition adapter")?
            .append_child(&textarea)
            .map_err(|error| format!("GPU UI composition attachment: {error:?}"))?;
        canvas.set_tab_index(0);
        canvas
            .set_attribute("role", "application")
            .map_err(|error| format!("GPU canvas accessibility role: {error:?}"))?;
        canvas
            .set_attribute("aria-label", "Solomon Dark")
            .map_err(|error| format!("GPU canvas accessibility label: {error:?}"))?;
        let queue = Rc::new(RefCell::new(InputQueue {
            focused: document.has_focus().unwrap_or(true),
            logical: [canvas.client_width() as f32, canvas.client_height() as f32],
            ..Default::default()
        }));
        let mut listeners = Vec::new();
        // A prior UI action must commit actual focus before the next native
        // editor default/clipboard event. Keep the queue unborrowed while Game
        // paints its genuine controls; selection notifications may reenter it.
        let window = document.default_view().ok_or("no window for native UI dispatch")?;
        for name in [
            "keydown",
            "keyup",
            "copy",
            "cut",
            "paste",
            "beforeinput",
            "compositionstart",
            "pointerdown",
        ] {
            let queue = queue.clone();
            listen_phase(window.as_ref(), name, &mut listeners, true, move |event| {
                let handler = {
                    let queue = queue.borrow();
                    queue.has_native_input().then(|| queue.native_dispatch.clone()).flatten()
                };
                if let Some(handler) = handler
                    && let Err(error) = handler()
                {
                    queue.borrow_mut().clipboard_error = Some(error);
                    event.prevent_default();
                    event.stop_immediate_propagation();
                }
            })?;
        }
        for (name, pressed) in [("keydown", true), ("keyup", false)] {
            let queue = queue.clone();
            let textarea = textarea.clone();
            let adapter: EventTarget = textarea.clone().into();
            listen(document.as_ref(), name, &mut listeners, move |event| {
                let Some(key) = event.dyn_ref::<KeyboardEvent>() else { return };
                // A native capture owner (bindings, authored shortcuts, consent)
                // has already handled this dispatch and its default editing.
                if key.default_prevented() {
                    return;
                }
                let mut queue = queue.borrow_mut();
                if key.is_composing() || queue.composing {
                    return;
                }
                let mods =
                    modifiers(key.alt_key(), key.ctrl_key(), key.shift_key(), key.meta_key());
                let text_modifier =
                    !mods.ctrl && !mods.mac_cmd || key.get_modifier_state("AltGraph");
                queue.codes.push(BrowserKey {
                    code: key.code(),
                    key: key.key(),
                    pressed,
                    repeat: key.repeat(),
                    alt: key.alt_key(),
                    ctrl: key.ctrl_key(),
                    shift: key.shift_key(),
                    meta: key.meta_key(),
                });
                queue.events.push(UiEvent::ModifiersChanged(mods));
                let mapped = key_from_browser(&key.key());
                let active = queue.session.is_some();
                let native_caret_arrow = active
                    && matches!(
                        mapped,
                        Some(Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown)
                    )
                    && !queue.session.as_ref().is_some_and(|session| {
                        session.descriptor.code_editor
                            && mods.alt
                            && matches!(mapped, Some(Key::ArrowUp | Key::ArrowDown))
                    });
                // Physical editor arrows own native caret receipts. Controller
                // arrows are queued separately and retain Source spatial focus;
                // Console Alt+Up/Down still reaches its actual history owner.
                if let Some(mapped) = mapped
                    && !native_caret_arrow
                {
                    queue.events.push(UiEvent::Key {
                        key: mapped,
                        physical_key: key.code().strip_prefix("Key").and_then(key_from_browser),
                        pressed,
                        repeat: key.repeat(),
                        modifiers: mods,
                    });
                }
                if active {
                    queue.sync_selection(&textarea);
                }
                let mut handled = false;
                if pressed && active {
                    let shortcut = if mods.command {
                        match key.key().to_ascii_lowercase().as_str() {
                            "a" => Some(Intent::SelectAll),
                            "z" if mods.shift => Some(Intent::Redo),
                            "z" => Some(Intent::Undo),
                            "y" => Some(Intent::Redo),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if let Some(intent) = shortcut {
                        queue.edit(intent, None);
                        handled = true;
                    } else if mapped == Some(Key::Enter) {
                        let single = queue
                            .session
                            .as_ref()
                            .is_some_and(|session| !session.descriptor.multiline);
                        if single {
                            queue.edit(Intent::Submit, None);
                            handled = true;
                        } else if mods.command || mods.ctrl {
                            handled = true;
                        }
                    } else if let Some(mapped) = mapped {
                        match queue
                            .session
                            .as_mut()
                            .expect("active editor")
                            .navigation(mapped, mods)
                        {
                            Ok(Some(intent)) => {
                                queue.edit(intent, None);
                                handled = true;
                            }
                            Ok(None) => {}
                            Err(error) => queue.clipboard_error = Some(error),
                        }
                    }
                }
                // Native input owns normal typing in the sole textarea. A real
                // character before a newly opened field's first RAF is retained
                // as an unbound intent and admitted only by actual focused paint.
                if pressed
                    && key.key().chars().count() == 1
                    && text_modifier
                    && (!active || !key.target().is_some_and(|target| target == adapter))
                {
                    let range = queue
                        .session
                        .as_ref()
                        .map_or(TextSelection::caret(0), |session| session.model.selection());
                    queue.edit(
                        Intent::Replace { range, text: key.key(), group: Some(EditGroup::Typing) },
                        None,
                    );
                    handled = true;
                }
                if handled
                    || queue.capture.keyboard && !active
                    || active && mapped == Some(Key::Tab)
                {
                    event.prevent_default();
                }
                if handled {
                    queue.sync_editor(&textarea);
                }
                if queue.capture.keyboard || active {
                    event.stop_propagation();
                }
                queue.finish_dispatch();
            })?;
        }
        for (name, phase) in [
            ("pointerdown", TouchPhase::Start),
            ("pointermove", TouchPhase::Move),
            ("pointerup", TouchPhase::End),
            ("pointercancel", TouchPhase::Cancel),
        ] {
            let queue = queue.clone();
            let canvas = canvas.clone();
            let textarea = textarea.clone();
            let ctx = ctx.clone();
            let target: EventTarget = if matches!(phase, TouchPhase::Start) {
                canvas.clone().into()
            } else {
                document.clone().into()
            };
            listen(&target, name, &mut listeners, move |event| {
                let Some(pointer) = event.dyn_ref::<PointerEvent>() else { return };
                let mut queue = queue.borrow_mut();
                let pos = point(&canvas, &queue, pointer.client_x(), pointer.client_y());
                let touch = pointer.pointer_type() != "mouse";
                let owned = if matches!(phase, TouchPhase::Start) {
                    let owned = queue.scope.is_some()
                        || queue.hit_regions.iter().any(|region| region.contains(pos));
                    if owned {
                        queue.owned_pointers.insert(pointer.pointer_id());
                    }
                    owned
                } else {
                    queue.owned_pointers.contains(&pointer.pointer_id())
                };
                if matches!(phase, TouchPhase::End | TouchPhase::Cancel) {
                    queue.owned_pointers.remove(&pointer.pointer_id());
                }
                if owned {
                    event.prevent_default();
                    event.stop_immediate_propagation();
                }
                if touch && !owned {
                    return;
                }

                if touch {
                    queue.events.push(UiEvent::Touch {
                        device_id: egui::TouchDeviceId(0),
                        id: egui::TouchId(pointer.pointer_id() as u64),
                        phase,
                        pos,
                        force: Some(pointer.pressure()),
                    });
                    if matches!(phase, TouchPhase::Start) && queue.primary_touch.is_none() {
                        queue.primary_touch = Some(pointer.pointer_id());
                    }
                    if queue.primary_touch != Some(pointer.pointer_id()) {
                        if !matches!(phase, TouchPhase::Move) {
                            queue.finish_dispatch();
                        }
                        return;
                    }
                }
                queue.events.push(UiEvent::PointerMoved(pos));
                if matches!(phase, TouchPhase::Start) && pointer.button() == 0 {
                    let target = ctx.pointer_widget_at(pos);
                    let descriptor = queue
                        .editors
                        .iter()
                        .rev()
                        .find(|editor| {
                            editor.enabled
                                && editor.bounds.contains(pos)
                                && target == Some(editor.owner.id)
                                && ctx.memory(|memory| memory.allows_interaction(editor.layer_id))
                        })
                        .cloned();
                    if let Some(descriptor) = descriptor {
                        if queue
                            .session
                            .as_ref()
                            .is_none_or(|session| session.descriptor.owner != descriptor.owner)
                        {
                            queue.session = Some(Session::new(descriptor));
                        }
                        let session = queue.session.as_mut().expect("actual editor hit");
                        session.pointer = Some(pointer.pointer_id());
                        match session.pointer(pos, pointer.shift_key()) {
                            Ok((intent, caret)) => queue.edit(intent, Some(caret)),
                            Err(error) => queue.clipboard_error = Some(error),
                        }
                        queue.sync_editor(&textarea);
                        if let Err(error) = textarea.focus() {
                            queue.clipboard_error = Some(format!("GPU editor focus: {error:?}"));
                        }
                    } else {
                        queue.session = None;
                        if let Err(error) = textarea.blur() {
                            queue.clipboard_error = Some(format!("GPU editor blur: {error:?}"));
                        }
                    }
                } else if matches!(phase, TouchPhase::Move)
                    && queue
                        .session
                        .as_ref()
                        .is_some_and(|session| session.pointer == Some(pointer.pointer_id()))
                {
                    match queue
                        .session
                        .as_mut()
                        .expect("owned selection pointer")
                        .pointer(pos, true)
                    {
                        Ok((intent, caret)) => queue.edit(intent, Some(caret)),
                        Err(error) => queue.clipboard_error = Some(error),
                    }
                    queue.sync_editor(&textarea);
                    queue.finish_dispatch();
                } else if matches!(phase, TouchPhase::End | TouchPhase::Cancel)
                    && let Some(session) = &mut queue.session
                {
                    session.pointer = None;
                }
                if !matches!(phase, TouchPhase::Move) {
                    let button = match pointer.button() {
                        1 => egui::PointerButton::Middle,
                        2 => egui::PointerButton::Secondary,
                        _ => egui::PointerButton::Primary,
                    };
                    let mods = modifiers(
                        pointer.alt_key(),
                        pointer.ctrl_key(),
                        pointer.shift_key(),
                        pointer.meta_key(),
                    );
                    queue.finish_pointer(phase, pos, button, mods, touch);
                }
            })?;
        }
        {
            let queue = queue.clone();
            let textarea = textarea.clone();
            let canvas = canvas.clone();
            listen(canvas.clone().as_ref(), "click", &mut listeners, move |event| {
                let Some(click) = event.dyn_ref::<web_sys::MouseEvent>() else { return };
                if click.button() != 0 {
                    return;
                }
                let mut queue = queue.borrow_mut();
                let pos = point(&canvas, &queue, click.client_x(), click.client_y());
                queue.record_click(pos, click.detail());
                if click.detail() < 2 {
                    return;
                }
                let Some(session) = queue
                    .session
                    .as_ref()
                    .filter(|session| session.descriptor.bounds.contains(pos))
                else {
                    return;
                };
                match session.click_selection(pos, click.detail()) {
                    Ok(intent) => queue.edit(intent, None),
                    Err(error) => queue.clipboard_error = Some(error),
                }
                queue.sync_editor(&textarea);
                queue.finish_dispatch();
                event.prevent_default();
            })?;
        }
        {
            let queue = queue.clone();
            listen(canvas.as_ref(), "pointerleave", &mut listeners, move |_| {
                let mut queue = queue.borrow_mut();
                queue.events.push(UiEvent::PointerGone);
                queue.finish_dispatch();
            })?;
        }
        {
            let queue = queue.clone();
            listen(canvas.as_ref(), "wheel", &mut listeners, move |event| {
                let Some(wheel) = event.dyn_ref::<WheelEvent>() else { return };
                let mut queue = queue.borrow_mut();
                queue.events.push(UiEvent::MouseWheel {
                    unit: match wheel.delta_mode() {
                        1 => egui::MouseWheelUnit::Line,
                        2 => egui::MouseWheelUnit::Page,
                        _ => egui::MouseWheelUnit::Point,
                    },
                    delta: vec2(-wheel.delta_x() as f32, -wheel.delta_y() as f32),
                    phase: TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: modifiers(
                        wheel.alt_key(),
                        wheel.ctrl_key(),
                        wheel.shift_key(),
                        wheel.meta_key(),
                    ),
                });
                queue.finish_dispatch();
                if queue.capture.pointer {
                    event.prevent_default();
                }
            })?;
        }
        for (name, focused) in [("focus", true), ("blur", false)] {
            let queue = queue.clone();
            let window = document.default_view().ok_or("no window for GPU UI focus")?;
            listen(window.as_ref(), name, &mut listeners, move |_| {
                let mut queue = queue.borrow_mut();
                queue.focused = focused;
                queue.events.push(UiEvent::WindowFocused(focused));
                if !focused {
                    if let Some(session) = &mut queue.session {
                        session.pointer = None;
                    }
                    queue.primary_touch = None;
                    queue.owned_pointers.clear();
                    queue.events.push(UiEvent::PointerGone);
                }
                queue.finish_dispatch();
            })?;
        }
        for (name, start) in [("compositionstart", true), ("compositionend", false)] {
            let queue = queue.clone();
            let textarea = textarea.clone();
            let target: EventTarget = textarea.clone().into();
            listen(&target, name, &mut listeners, move |event| {
                if event.dyn_ref::<CompositionEvent>().is_none() {
                    return;
                }
                let mut queue = queue.borrow_mut();
                queue.composing = start;
                if queue.session.is_some() {
                    queue.edit(
                        if start { Intent::CompositionStart } else { Intent::CompositionEnd },
                        None,
                    );
                    queue.finish_dispatch();
                }
            })?;
        }
        {
            let queue = queue.clone();
            let textarea = textarea.clone();
            let target: EventTarget = textarea.clone().into();
            listen(&target, "beforeinput", &mut listeners, move |event| {
                let Some(input) = event.dyn_ref::<InputEvent>() else { return };
                let mut queue = queue.borrow_mut();
                queue.sync_selection(&textarea);
                let Some(session) = &queue.session else { return };
                let intent = match input.input_type().as_str() {
                    "historyUndo" => Some(Intent::Undo),
                    "historyRedo" => Some(Intent::Redo),
                    "insertParagraph" | "insertLineBreak" if !session.descriptor.multiline => {
                        Some(Intent::Submit)
                    }
                    _ => None,
                };
                if let Some(intent) = intent {
                    event.prevent_default();
                    queue.edit(intent, None);
                    queue.sync_editor(&textarea);
                }
                queue.finish_dispatch();
            })?;
        }
        {
            let queue = queue.clone();
            let textarea = textarea.clone();
            let target: EventTarget = textarea.clone().into();
            listen(&target, "input", &mut listeners, move |event| {
                if event.dyn_ref::<InputEvent>().is_none() {
                    return;
                }
                let mut queue = queue.borrow_mut();
                if queue.session.is_some() {
                    queue.edit_sequence += 1;
                    let sequence = queue.edit_sequence;
                    let input = event.dyn_ref::<InputEvent>().expect("native input event");
                    let receipt = queue.session.as_mut().expect("active editor").input(
                        sequence,
                        &input.input_type(),
                        &textarea,
                    );
                    queue.edits.push(receipt);
                    // Restore rejected edits immediately, before a later native
                    // dispatch can build another edit against an invalid value.
                    queue.sync_editor(&textarea);
                    queue.finish_dispatch();
                }
            })?;
        }
        for (target, name) in [
            (EventTarget::from(textarea.clone()), "select"),
            (EventTarget::from(textarea.clone()), "selectionchange"),
            (EventTarget::from(document.clone()), "selectionchange"),
        ] {
            let queue = queue.clone();
            let textarea = textarea.clone();
            let document = document.clone();
            listen(&target, name, &mut listeners, move |_| {
                if !document.active_element().is_some_and(|element| element == **textarea) {
                    return;
                }
                let mut queue = queue.borrow_mut();
                let selected = selection(&textarea);
                if queue
                    .session
                    .as_ref()
                    .is_some_and(|session| session.model.selection() != selected)
                {
                    queue.edit(Intent::Select(selected), None);
                    queue.finish_dispatch();
                }
            })?;
        }
        for (name, intent) in [("copy", Intent::Copy), ("cut", Intent::Cut)] {
            let queue = queue.clone();
            let textarea = textarea.clone();
            listen(document.as_ref(), name, &mut listeners, move |event| {
                let Some(clipboard) = event.dyn_ref::<ClipboardEvent>() else { return };
                let mut queue = queue.borrow_mut();
                if queue.session.is_none() {
                    return;
                }
                queue.sync_selection(&textarea);
                let session = queue.session.as_ref().expect("active editor");
                let range = session.model.selection().range();
                if range.is_empty() {
                    return;
                }
                let copied = super::edit::model::slice_utf16(session.model.text(), range);
                let result = clipboard
                    .clipboard_data()
                    .ok_or_else(|| "native copy has no clipboard data".to_owned())
                    .and_then(|data| {
                        data.set_data("text/plain", copied)
                            .map_err(|error| format!("GPU editor native copy: {error:?}"))
                    });
                if let Err(error) = result {
                    queue.clipboard_error = Some(error);
                }
                event.prevent_default();
                queue.edit(intent.clone(), None);
                if let Some(receipt) = queue.edits.last_mut() {
                    receipt.clipboard_handled = true;
                }
                queue.sync_editor(&textarea);
                queue.finish_dispatch();
            })?;
        }
        {
            let queue = queue.clone();
            let textarea = textarea.clone();
            listen(document.as_ref(), "paste", &mut listeners, move |event| {
                let Some(paste) = event.dyn_ref::<ClipboardEvent>() else { return };
                let mut queue = queue.borrow_mut();
                if queue.session.is_none() {
                    return;
                }
                queue.sync_selection(&textarea);
                if let Some(data) = paste.clipboard_data() {
                    match data.get_data("text/plain") {
                        Ok(text) if !text.is_empty() => {
                            let range =
                                queue.session.as_ref().expect("active editor").model.selection();
                            queue.edit(Intent::Replace { range, text, group: None }, None);
                            queue.sync_editor(&textarea);
                            event.prevent_default();
                        }
                        Ok(_) => {}
                        Err(error) => {
                            queue.clipboard_error = Some(format!("GPU UI paste: {error:?}"))
                        }
                    }
                }
                queue.finish_dispatch();
            })?;
        }
        Ok(Self { document: document.clone(), canvas: canvas.clone(), textarea, queue, listeners })
    }

    pub fn take(
        &mut self,
        viewport: [f32; 2],
        pixels_per_point: f32,
        time: f64,
    ) -> Result<Vec<InputPass>, String> {
        self.take_input(viewport, pixels_per_point, time, true)
    }

    pub fn take_native(
        &mut self,
        viewport: [f32; 2],
        pixels_per_point: f32,
        time: f64,
    ) -> Result<Option<Vec<InputPass>>, String> {
        let pending = self.queue.borrow().has_native_input();
        if pending {
            self.take_input(viewport, pixels_per_point, time, false).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn set_native_dispatch_handler(
        &mut self,
        handler: impl Fn() -> Result<(), String> + 'static,
    ) {
        self.queue.borrow_mut().native_dispatch = Some(Rc::new(handler));
    }

    fn take_input(
        &mut self,
        viewport: [f32; 2],
        pixels_per_point: f32,
        time: f64,
        sample_gamepad: bool,
    ) -> Result<Vec<InputPass>, String> {
        let mut queue = self.queue.borrow_mut();
        queue.logical = viewport;
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(viewport[0], viewport[1]))),
            time: Some(time),
            events: std::mem::take(&mut queue.events),
            focused: queue.focused,
            ..Default::default()
        };
        raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point =
            Some(pixels_per_point);
        let mut dispatches = std::mem::take(&mut queue.dispatches);
        let codes = std::mem::take(&mut queue.codes);
        let edits = std::mem::take(&mut queue.edits);
        if let Some(error) = queue.clipboard_error.take() {
            return Err(error);
        }
        if !sample_gamepad {
            return Ok(into_passes(raw, codes, edits, dispatches));
        }
        let native_events = raw.events.len();
        let gamepads = if queue.scope.is_some() {
            self.document
                .default_view()
                .ok_or("no window for gamepad input")?
                .navigator()
                .get_gamepads()
                .map_err(|error| format!("GPU UI gamepad sample: {error:?}"))?
        } else {
            js_sys::Array::new()
        };
        use solomon_sim::input::{
            MenuDirection, MenuGamepadButtons, MenuGamepadState, advance_menu_direction_repeat,
            read_menu_gamepad,
        };
        let mut current = MenuGamepadState::default();
        for value in gamepads.iter().filter(|value| !value.is_null()) {
            let Ok(gamepad) = value.dyn_into::<web_sys::Gamepad>() else { continue };
            if !gamepad.connected() || gamepad.mapping() != web_sys::GamepadMappingType::Standard {
                continue;
            }
            let buttons = gamepad.buttons();
            let pressed = |index| {
                buttons
                    .get(index)
                    .dyn_into::<web_sys::GamepadButton>()
                    .is_ok_and(|button| button.pressed() || button.value() > 0.5)
            };
            let axes = gamepad.axes();
            let axis = |index| axes.get(index).as_f64().unwrap_or(0.0);
            let state = read_menu_gamepad(
                solomon_sim::Vec2::new(axis(0), axis(1)),
                MenuGamepadButtons {
                    up: pressed(12),
                    down: pressed(13),
                    left: pressed(14),
                    right: pressed(15),
                    confirm: pressed(0),
                    back: pressed(1),
                    previous: pressed(4),
                    next: pressed(5),
                },
            );
            if state.active() {
                current = state;
                break;
            }
        }
        let current = queue.scoped_gamepad(current);
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
            queue.gamepad_direction,
            current.direction,
            time * 1000.0,
            queue.next_gamepad_repeat_at,
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
            if held[index] != queue.gamepad_keys[index] || index < 4 && held[index] && repeat {
                raw.events.push(UiEvent::Key {
                    key,
                    physical_key: None,
                    pressed: held[index],
                    repeat: held[index] && queue.gamepad_keys[index],
                    modifiers: Modifiers { shift: index == 6, ..Default::default() },
                });
            }
        }
        queue.gamepad_keys = held;
        queue.gamepad_direction = current.direction;
        queue.next_gamepad_repeat_at = next_repeat;
        if raw.events.len() != native_events {
            dispatches.push(DispatchEnd {
                events: raw.events.len(),
                codes: codes.len(),
                edits: edits.len(),
                click_count: None,
            });
        }
        Ok(into_passes(raw, codes, edits, dispatches))
    }

    pub fn set_editors(
        &mut self,
        editors: Vec<EditorDescriptor>,
        acknowledged: Option<u64>,
    ) -> Result<(), String> {
        let mut queue = self.queue.borrow_mut();
        if let Some(sequence) = acknowledged {
            queue.acknowledged_edit_sequence = queue.acknowledged_edit_sequence.max(sequence);
        }
        let focused = editors.iter().find(|editor| editor.focused && editor.enabled).cloned();
        let acknowledged = queue.acknowledged_edit_sequence;
        if super::edit::session::reconcile_session(&mut queue.session, focused, acknowledged)
            && let Some(session) = &queue.session
        {
            session.sync(&self.textarea)?;
        }
        queue.editors = editors;
        Ok(())
    }

    pub fn set_widgets(&mut self, widgets: &std::collections::BTreeMap<String, WidgetGeometry>) {
        self.queue.borrow_mut().hit_regions = widgets
            .values()
            .filter(|widget| widget.is_interactive())
            .map(WidgetGeometry::hit_region)
            .collect();
    }

    pub fn owns_pointer(&self) -> bool {
        !self.queue.borrow().owned_pointers.is_empty()
    }

    pub fn set_scope(&mut self, scope: Option<&str>) {
        self.queue.borrow_mut().set_scope(scope);
    }

    pub fn apply_output(
        &mut self,
        output: &egui::PlatformOutput,
        capture: UiCapture,
    ) -> Result<(), String> {
        self.queue.borrow_mut().capture = capture;
        self.canvas
            .style()
            .set_property("cursor", cursor_css(output.cursor_icon))
            .map_err(|error| format!("GPU UI cursor: {error:?}"))?;
        if let Some(ime) = &output.ime {
            let bounds = self.canvas.get_bounding_client_rect();
            self.textarea
                .style()
                .set_property(
                    "left",
                    &format!("{}px", bounds.left() + f64::from(ime.cursor_rect.left())),
                )
                .map_err(|error| format!("IME horizontal placement: {error:?}"))?;
            self.textarea
                .style()
                .set_property(
                    "top",
                    &format!("{}px", bounds.top() + f64::from(ime.cursor_rect.bottom())),
                )
                .map_err(|error| format!("IME vertical placement: {error:?}"))?;
            self.textarea.focus().map_err(|error| format!("GPU UI text focus: {error:?}"))?;
        } else if self.document.active_element().is_some_and(|element| element == **self.textarea)
            && !self.queue.borrow().session.as_ref().is_some_and(|session| {
                session.model.sequence > self.queue.borrow().acknowledged_edit_sequence
            })
        {
            self.textarea.blur().map_err(|error| format!("GPU UI text blur: {error:?}"))?;
        }
        for command in &output.commands {
            match command {
                egui::OutputCommand::CopyText(text) => {
                    let clipboard = self
                        .document
                        .default_view()
                        .ok_or("no clipboard window")?
                        .navigator()
                        .clipboard();
                    let promise = clipboard.write_text(text);
                    let queue = self.queue.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Err(error) = wasm_bindgen_futures::JsFuture::from(promise).await {
                            queue.borrow_mut().clipboard_error =
                                Some(format!("GPU UI clipboard write: {error:?}"));
                        }
                    });
                }
                egui::OutputCommand::OpenUrl(url) => {
                    self.document
                        .default_view()
                        .ok_or("no window for link")?
                        .open_with_url_and_target(
                            &url.url,
                            if url.new_tab { "_blank" } else { "_self" },
                        )
                        .map_err(|error| format!("GPU UI open link: {error:?}"))?;
                }
                egui::OutputCommand::TextSelectionSettled(_) => {}
                egui::OutputCommand::CopyImage(_) => {
                    return Err("the game does not expose image clipboard commands".to_owned());
                }
            }
        }
        Ok(())
    }
}

fn cursor_css(cursor: egui::CursorIcon) -> &'static str {
    use egui::CursorIcon::*;
    match cursor {
        Default => "default",
        None => "none",
        ContextMenu => "context-menu",
        Help => "help",
        PointingHand => "pointer",
        Progress => "progress",
        Wait => "wait",
        Cell => "cell",
        Crosshair => "crosshair",
        Text => "text",
        VerticalText => "vertical-text",
        Alias => "alias",
        Copy => "copy",
        Move => "move",
        NoDrop => "no-drop",
        NotAllowed => "not-allowed",
        Grab => "grab",
        Grabbing => "grabbing",
        AllScroll => "all-scroll",
        ResizeHorizontal => "ew-resize",
        ResizeNeSw => "nesw-resize",
        ResizeNwSe => "nwse-resize",
        ResizeVertical => "ns-resize",
        ResizeEast => "e-resize",
        ResizeSouthEast => "se-resize",
        ResizeSouth => "s-resize",
        ResizeSouthWest => "sw-resize",
        ResizeWest => "w-resize",
        ResizeNorthWest => "nw-resize",
        ResizeNorth => "n-resize",
        ResizeNorthEast => "ne-resize",
        ResizeColumn => "col-resize",
        ResizeRow => "row-resize",
        ZoomIn => "zoom-in",
        ZoomOut => "zoom-out",
    }
}

impl Drop for BrowserInput {
    fn drop(&mut self) {
        self.listeners.clear();
        self.textarea.remove();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solomon_sim::input::{MenuDirection, MenuGamepadState};

    #[test]
    fn browser_click_series_stays_on_its_real_release_and_never_replays_in_layout() {
        let mut queue = InputQueue::default();
        for (position, count) in
            [(pos2(20.0, 20.0), 1), (pos2(90.0, 90.0), 1), (pos2(90.0, 90.0), 2)]
        {
            for pressed in [true, false] {
                queue.events.push(UiEvent::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                });
                queue.finish_dispatch();
            }
            queue.record_click(position, count);
            queue.finish_dispatch();
        }
        assert_eq!(queue.dispatches.len(), 6, "click metadata does not duplicate the release");
        queue.record_click(pos2(20.0, 20.0), 2);
        let passes = into_passes(
            egui::RawInput { events: queue.events, ..Default::default() },
            Vec::new(),
            Vec::new(),
            queue.dispatches,
        );
        assert_eq!(passes.len(), 12);
        assert_eq!(
            passes.iter().filter_map(|pass| pass.click_count).collect::<Vec<_>>(),
            [1, 1, 2]
        );
        assert!(
            passes
                .iter()
                .filter(|pass| pass.raw.events.is_empty())
                .all(|pass| pass.click_count.is_none())
        );
        assert!(passes.iter().filter(|pass| pass.click_count.is_some()).all(|pass| {
            matches!(pass.raw.events.last(), Some(UiEvent::PointerButton { pressed: false, .. }))
        }));
    }

    #[test]
    fn foreground_admission_requires_neutral_without_polling_the_gameplay_controller() {
        let mut queue = InputQueue::default();
        queue.set_scope(Some("pause"));
        let held = MenuGamepadState { back: true, ..Default::default() };
        assert!(!queue.scoped_gamepad(held).active());
        queue.scoped_gamepad(MenuGamepadState::default());
        assert_eq!(queue.scoped_gamepad(held), held);
    }

    #[test]
    fn gameplay_keeps_the_controller_and_foreground_scope_changes_require_a_neutral_handoff() {
        let mut queue = InputQueue::default();
        let held = MenuGamepadState {
            confirm: true,
            direction: Some(MenuDirection::Down),
            ..Default::default()
        };
        assert!(!queue.scoped_gamepad(held).active());
        queue.set_scope(Some("title"));
        assert!(!queue.scoped_gamepad(held).active());
        assert!(!queue.scoped_gamepad(MenuGamepadState::default()).active());
        assert_eq!(queue.scoped_gamepad(held), held);
        queue.set_scope(Some("create"));
        assert!(!queue.scoped_gamepad(held).active());
        queue.scoped_gamepad(MenuGamepadState::default());
        assert_eq!(queue.scoped_gamepad(held), held);
        queue.set_scope(None);
        assert!(!queue.scoped_gamepad(held).active());
    }
}
