//! Native canvas events and a nonvisual IME/clipboard adapter.

use std::cell::RefCell;
use std::rc::Rc;

use egui::{Event as UiEvent, ImeEvent, Key, Modifiers, Pos2, Rect, TouchPhase, pos2, vec2};
use serde::Serialize;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{
    ClipboardEvent, CompositionEvent, Document, Event, EventTarget, HtmlCanvasElement,
    HtmlTextAreaElement, InputEvent, KeyboardEvent, PointerEvent, WheelEvent,
};

use super::{UiCapture, WidgetGeometry};

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
    focused: bool,
    composing: bool,
    capture: UiCapture,
    logical: [f32; 2],
    text_rects: Vec<Rect>,
    primary_touch: Option<i32>,
    gamepad_keys: [bool; 8],
    gamepad_direction: Option<solomon_sim::input::MenuDirection>,
    next_gamepad_repeat_at: f64,
    clipboard_error: Option<String>,
}

struct Listener {
    target: EventTarget,
    name: &'static str,
    callback: Closure<dyn FnMut(Event)>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self
            .target
            .remove_event_listener_with_callback(self.name, self.callback.as_ref().unchecked_ref());
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
    let callback = Closure::<dyn FnMut(Event)>::new(callback);
    target
        .add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())
        .map_err(|error| format!("GPU UI {name} listener: {error:?}"))?;
    listeners.push(Listener { target: target.clone(), name, callback });
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
    pub fn attach(document: &Document, canvas: &HtmlCanvasElement) -> Result<Self, String> {
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
        for (name, pressed) in [("keydown", true), ("keyup", false)] {
            let queue = queue.clone();
            let adapter: EventTarget = textarea.clone().into();
            listen(document.as_ref(), name, &mut listeners, move |event| {
                let Some(key) = event.dyn_ref::<KeyboardEvent>() else { return };
                let mut queue = queue.borrow_mut();
                if key.is_composing() || queue.composing {
                    return;
                }
                let mods =
                    modifiers(key.alt_key(), key.ctrl_key(), key.shift_key(), key.meta_key());
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
                if pressed
                    && mods.command
                    && matches!(key.key().as_str(), "c" | "C" | "x" | "X")
                    && queue.capture.text
                {
                    queue.events.push(if key.key().eq_ignore_ascii_case("c") {
                        UiEvent::Copy
                    } else {
                        UiEvent::Cut
                    });
                    event.prevent_default();
                } else if let Some(mapped) = key_from_browser(&key.key()) {
                    queue.events.push(UiEvent::Key {
                        key: mapped,
                        physical_key: key.code().strip_prefix("Key").and_then(key_from_browser),
                        pressed,
                        repeat: key.repeat(),
                        modifiers: mods,
                    });
                    let native_text = queue.capture.text
                        && key.key().chars().count() == 1
                        && !mods.ctrl
                        && !mods.mac_cmd;
                    if queue.capture.keyboard
                        && !native_text
                        && !(mods.command && key.key().eq_ignore_ascii_case("v"))
                    {
                        event.prevent_default();
                    }
                }
                // A click and the first character can arrive before the next game
                // frame focuses the IME adapter. Queue real desktop key text here;
                // suppress its textarea default so `input` cannot insert it twice.
                // Virtual keyboards without keydown still use the input listener.
                if pressed && key.key().chars().count() == 1 && !mods.ctrl && !mods.mac_cmd {
                    queue.events.push(UiEvent::Text(key.key()));
                    if queue.capture.text || key.target().is_some_and(|target| target == adapter) {
                        event.prevent_default();
                    }
                }
                if queue.capture.keyboard {
                    event.stop_propagation();
                }
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
                        return;
                    }
                }
                queue.events.push(UiEvent::PointerMoved(pos));
                if !matches!(phase, TouchPhase::Move) {
                    let pressed = matches!(phase, TouchPhase::Start);
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
                    queue.events.push(UiEvent::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers: mods,
                    });
                    if pressed && queue.text_rects.iter().any(|rect| rect.contains(pos)) {
                        let _ = textarea.focus();
                    }
                    if touch && !pressed {
                        queue.primary_touch = None;
                        queue.events.push(UiEvent::PointerGone);
                    }
                }
                if queue.capture.pointer {
                    event.prevent_default();
                    event.stop_propagation();
                }
            })?;
        }
        {
            let queue = queue.clone();
            listen(canvas.as_ref(), "pointerleave", &mut listeners, move |_| {
                queue.borrow_mut().events.push(UiEvent::PointerGone)
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
                    queue.primary_touch = None;
                    queue.events.push(UiEvent::PointerGone);
                }
            })?;
        }
        for (name, phase) in
            [("compositionstart", 0), ("compositionupdate", 1), ("compositionend", 2)]
        {
            let queue = queue.clone();
            let textarea = textarea.clone();
            let target: EventTarget = textarea.clone().into();
            listen(&target, name, &mut listeners, move |event| {
                let Some(composition) = event.dyn_ref::<CompositionEvent>() else { return };
                let mut queue = queue.borrow_mut();
                match phase {
                    0 => {
                        queue.composing = true;
                    }
                    1 => queue.events.push(UiEvent::Ime(ImeEvent::Preedit {
                        text: composition.data().unwrap_or_default(),
                        active_range_chars: None,
                    })),
                    _ => {
                        queue.composing = false;
                        queue.events.push(UiEvent::Ime(ImeEvent::Commit(
                            composition.data().unwrap_or_default(),
                        )));
                        textarea.set_value("");
                    }
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
                if queue.composing || input.is_composing() {
                    return;
                }
                let key = match input.input_type().as_str() {
                    "deleteContentBackward" => Some(Key::Backspace),
                    "deleteContentForward" => Some(Key::Delete),
                    "insertParagraph" | "insertLineBreak" => Some(Key::Enter),
                    _ => None,
                };
                if let Some(key) = key {
                    for pressed in [true, false] {
                        queue.events.push(UiEvent::Key {
                            key,
                            physical_key: None,
                            pressed,
                            repeat: false,
                            modifiers: Modifiers::NONE,
                        });
                    }
                    event.prevent_default();
                }
            })?;
        }
        {
            let queue = queue.clone();
            let textarea = textarea.clone();
            let target: EventTarget = textarea.clone().into();
            listen(&target, "input", &mut listeners, move |event| {
                let Some(input) = event.dyn_ref::<InputEvent>() else { return };
                let mut queue = queue.borrow_mut();
                if !queue.composing
                    && !input.is_composing()
                    && input.input_type() != "insertFromComposition"
                {
                    if let Some(text) = input.data().filter(|text| !text.is_empty()) {
                        queue.events.push(UiEvent::Text(text));
                    }
                    textarea.set_value("");
                }
            })?;
        }
        {
            let queue = queue.clone();
            listen(document.as_ref(), "paste", &mut listeners, move |event| {
                let Some(paste) = event.dyn_ref::<ClipboardEvent>() else { return };
                let mut queue = queue.borrow_mut();
                if !queue.capture.text {
                    return;
                }
                if let Some(data) = paste.clipboard_data() {
                    match data.get_data("text/plain") {
                        Ok(text) => {
                            queue.events.push(UiEvent::Paste(text));
                            event.prevent_default();
                        }
                        Err(error) => {
                            queue.clipboard_error = Some(format!("GPU UI paste: {error:?}"))
                        }
                    }
                }
            })?;
        }
        Ok(Self { document: document.clone(), canvas: canvas.clone(), textarea, queue, listeners })
    }

    pub fn take(
        &mut self,
        viewport: [f32; 2],
        pixels_per_point: f32,
        time: f64,
    ) -> Result<egui::RawInput, String> {
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
        if let Some(error) = queue.clipboard_error.take() {
            return Err(error);
        }
        let navigator =
            self.document.default_view().ok_or("no window for gamepad input")?.navigator();
        let gamepads = navigator
            .get_gamepads()
            .map_err(|error| format!("GPU UI gamepad sample: {error:?}"))?;
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
        Ok(raw)
    }

    pub fn set_text_rects(&mut self, widgets: &std::collections::BTreeMap<String, WidgetGeometry>) {
        self.queue.borrow_mut().text_rects = widgets
            .values()
            .filter(|widget| {
                matches!(widget.role.as_str(), "textbox" | "input" | "text-entry") && widget.enabled
            })
            .map(|widget| {
                Rect::from_min_size(
                    pos2(widget.bounds.x, widget.bounds.y),
                    vec2(widget.bounds.width, widget.bounds.height),
                )
            })
            .collect();
    }

    pub fn take_key_codes(&mut self) -> Vec<BrowserKey> {
        std::mem::take(&mut self.queue.borrow_mut().codes)
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
        } else if self.document.active_element().is_some_and(|element| element == **self.textarea) {
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
