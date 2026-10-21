//! Current Website `HotbarControls.tsx` owns one presentation bank across the
//! gameplay HUD and both optional books. All assignments remain player-owned.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use solomon_sim::belt::{NATIVE_BELT_SLOT_COUNT, PLAYER_HOTBAR_COUNT};
use solomon_sim::hud::HudRect;
use solomon_sim::settings::GameSettings;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Element, Event, HtmlElement, KeyboardEvent, Window};

use crate::ui::{append, element};

#[derive(Clone, Copy)]
pub enum HotbarDirection {
    Previous,
    Next,
}

struct Shortcut {
    code: String,
    enabled: bool,
    reverse: bool,
}

struct SelectionState {
    bank: Cell<usize>,
    shortcut: RefCell<Shortcut>,
}

struct SelectionOwner {
    state: Rc<SelectionState>,
    window: Window,
    listener: Closure<dyn FnMut(Event)>,
}

impl Drop for SelectionOwner {
    fn drop(&mut self) {
        if let Err(error) = self.window.remove_event_listener_with_callback_and_bool(
            "keydown",
            self.listener.as_ref().unchecked_ref(),
            true,
        ) {
            web_sys::console::error_1(&error);
        }
    }
}

/// Clone this one owner into the HUD, input and optional books. Selection starts
/// at bank zero and survives world/session changes for the page's lifetime.
#[derive(Clone)]
pub struct HotbarSelection(Rc<SelectionOwner>);

impl HotbarSelection {
    pub fn attach(window: &Window) -> Result<Self, String> {
        let state = Rc::new(SelectionState {
            bank: Cell::new(0),
            shortcut: RefCell::new(Shortcut {
                code: GameSettings::default().controls.cycle_hotbar,
                enabled: false,
                reverse: false,
            }),
        });
        let keyed = state.clone();
        let listener = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(key) = event.dyn_ref::<KeyboardEvent>() else { return };
            let shortcut = keyed.shortcut.borrow();
            if !shortcut.enabled
                || key.code() != shortcut.code
                || key.repeat()
                || event.default_prevented()
                || key.alt_key()
                || key.ctrl_key()
                || key.meta_key()
            {
                return;
            }
            let editing = event
                .target()
                .and_then(|target| target.dyn_into::<HtmlElement>().ok())
                .is_some_and(|target| {
                    target
                        .closest("input,textarea,[contenteditable='true']")
                        .ok()
                        .flatten()
                        .is_some()
                });
            if editing {
                return;
            }
            event.prevent_default();
            event.stop_immediate_propagation();
            let direction = if key.shift_key() != shortcut.reverse {
                HotbarDirection::Previous
            } else {
                HotbarDirection::Next
            };
            cycle_bank(&keyed.bank, direction);
        });
        window
            .add_event_listener_with_callback_and_bool(
                "keydown",
                listener.as_ref().unchecked_ref(),
                true,
            )
            .map_err(|_| "hotbar shortcut listener")?;
        Ok(Self(Rc::new(SelectionOwner { state, window: window.clone(), listener })))
    }

    #[must_use]
    pub fn bank(&self) -> usize {
        self.0.state.bank.get()
    }

    #[must_use]
    pub fn start(&self) -> usize {
        self.bank() * NATIVE_BELT_SLOT_COUNT
    }

    #[must_use]
    pub fn slot(&self, visible_slot: usize) -> usize {
        self.start() + visible_slot
    }

    pub fn cycle(&self, direction: HotbarDirection) {
        cycle_bank(&self.0.state.bank, direction);
    }

    /// Game supplies the Source admission predicate; an owned optional-book pause
    /// allows cycling, while settings/chat/loading/resume barriers do not.
    pub fn set_shortcut(&self, settings: &GameSettings, enabled: bool) {
        let mut shortcut = self.0.state.shortcut.borrow_mut();
        if shortcut.code != settings.controls.cycle_hotbar {
            shortcut.code.clone_from(&settings.controls.cycle_hotbar);
        }
        shortcut.enabled = enabled;
        shortcut.reverse = settings.reverse_hotbar_cycle;
    }
}

fn cycle_bank(bank: &Cell<usize>, direction: HotbarDirection) {
    let step = match direction {
        HotbarDirection::Previous => PLAYER_HOTBAR_COUNT - 1,
        HotbarDirection::Next => 1,
    };
    bank.set((bank.get() + step) % PLAYER_HOTBAR_COUNT);
}

pub(crate) struct HotbarControls {
    pub root: HtmlElement,
    selection: HotbarSelection,
    arrows: [HtmlElement; 2],
    dots: Vec<Element>,
    dots_label: HtmlElement,
    previous_bank: Cell<Option<usize>>,
    previous_disabled: Cell<Option<bool>>,
    click: Closure<dyn FnMut(Event)>,
    pointer: Closure<dyn FnMut(Event)>,
}

impl HotbarControls {
    pub fn new(
        document: &Document,
        selection: HotbarSelection,
        modal_rects: Option<&[HudRect; 8]>,
    ) -> Result<Self, String> {
        let root = element(document, "div", "hotbar-controls")?;
        let modal = modal_rects.is_some();
        if let Some(rects) = modal_rects {
            root.set_attribute("data-modal", "true").map_err(|_| "modal hotbar controls")?;
            for (name, value) in [
                ("--hotbar-top", rects[0].y + 10.0),
                ("--hotbar-left", rects[0].x - 46.0),
                ("--hotbar-right", rects[7].x + rects[7].width + 12.0),
                ("--hotbar-dots-top", rects[0].y - 19.0),
            ] {
                root.style()
                    .set_property(name, &format!("{value}px"))
                    .map_err(|_| "modal hotbar placement")?;
            }
        }
        let previous = arrow(document, -1, modal)?;
        let next = arrow(document, 1, modal)?;
        append(&root, &previous)?;
        let dots_label = element(document, "span", "hotbar-dots")?;
        dots_label.set_attribute("role", "img").map_err(|_| "hotbar dots role")?;
        let mut dots = Vec::new();
        if modal {
            let svg = svg_element(document, "svg")?;
            svg.set_attribute("viewBox", "0 0 70 20").map_err(|_| "hotbar dots view")?;
            for bank in 0..PLAYER_HOTBAR_COUNT {
                let dot = svg_element(document, "g")?;
                for (radius, class, opacity) in [
                    (9, "hotbar-dot-glow", "0.08"),
                    (7, "hotbar-dot-glow", "0.12"),
                    (5, "hotbar-dot-center", "1"),
                ] {
                    let circle = svg_element(document, "circle")?;
                    for (name, value) in [
                        ("cx", (15 + bank * 20).to_string()),
                        ("cy", "5.5".to_owned()),
                        ("r", radius.to_string()),
                        ("class", class.to_owned()),
                        ("fill-opacity", opacity.to_owned()),
                    ] {
                        circle.set_attribute(name, &value).map_err(|_| "modal hotbar dot")?;
                    }
                    dot.append_child(&circle).map_err(|_| "modal hotbar dot art")?;
                }
                svg.append_child(&dot).map_err(|_| "modal hotbar bank")?;
                dots.push(dot);
            }
            dots_label.append_child(&svg).map_err(|_| "modal hotbar dots")?;
        } else {
            for _ in 0..PLAYER_HOTBAR_COUNT {
                let dot = element(document, "span", "")?;
                append(&dots_label, &dot)?;
                dots.push(dot.into());
            }
        }
        append(&root, &dots_label)?;
        append(&root, &next)?;
        let clicked = selection.clone();
        let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(button) = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .and_then(|target| target.closest("[data-hotbar-direction]").ok().flatten())
            else {
                return;
            };
            if button.has_attribute("disabled") {
                return;
            }
            let direction = match button.get_attribute("data-hotbar-direction").as_deref() {
                Some("-1") => HotbarDirection::Previous,
                Some("1") => HotbarDirection::Next,
                _ => return,
            };
            event.stop_propagation();
            clicked.cycle(direction);
        });
        root.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
            .map_err(|_| "hotbar button listener")?;
        let pointer = Closure::<dyn FnMut(Event)>::new(|event: Event| event.stop_propagation());
        root.add_event_listener_with_callback("pointerdown", pointer.as_ref().unchecked_ref())
            .map_err(|_| "hotbar pointer ownership")?;
        Ok(Self {
            root,
            selection,
            arrows: [previous, next],
            dots,
            dots_label,
            previous_bank: Cell::new(None),
            previous_disabled: Cell::new(None),
            click,
            pointer,
        })
    }

    pub fn update(&self, disabled: bool) -> Result<bool, String> {
        let bank = self.selection.bank();
        let changed = self.previous_bank.replace(Some(bank)) != Some(bank);
        if changed {
            self.root
                .set_attribute("data-hotbar-bank", &bank.to_string())
                .map_err(|_| "hotbar bank projection")?;
            self.dots_label
                .set_attribute(
                    "aria-label",
                    &format!("Hotbar {} of {PLAYER_HOTBAR_COUNT}", bank + 1),
                )
                .map_err(|_| "hotbar bank label")?;
            for (index, dot) in self.dots.iter().enumerate() {
                dot.set_attribute("data-active", if index == bank { "true" } else { "false" })
                    .map_err(|_| "hotbar active bank")?;
            }
        }
        if self.previous_disabled.replace(Some(disabled)) != Some(disabled) {
            for arrow in &self.arrows {
                arrow
                    .toggle_attribute_with_force("disabled", disabled)
                    .map_err(|_| "hotbar control availability")?;
            }
        }
        Ok(changed)
    }
}

impl Drop for HotbarControls {
    fn drop(&mut self) {
        for (name, listener) in [("click", &self.click), ("pointerdown", &self.pointer)] {
            if let Err(error) = self
                .root
                .remove_event_listener_with_callback(name, listener.as_ref().unchecked_ref())
            {
                web_sys::console::error_1(&error);
            }
        }
    }
}

fn arrow(document: &Document, direction: i32, modal: bool) -> Result<HtmlElement, String> {
    let previous = direction < 0;
    let button = element(
        document,
        "button",
        if previous { "hotbar-arrow hotbar-previous" } else { "hotbar-arrow hotbar-next" },
    )?;
    for (name, value) in [
        ("type", "button".to_owned()),
        ("aria-label", if previous { "Previous hotbar" } else { "Next hotbar" }.to_owned()),
        ("title", if previous { "Previous hotbar" } else { "Next hotbar" }.to_owned()),
        ("data-hotbar-direction", direction.to_string()),
    ] {
        button.set_attribute(name, &value).map_err(|_| "hotbar arrow semantics")?;
    }
    if !modal {
        let mark = element(document, "span", "")?;
        mark.set_attribute("aria-hidden", "true").map_err(|_| "hotbar arrow decoration")?;
        append(&button, &mark)?;
        return Ok(button);
    }
    // `ModalHotbarControlsView` uses these exact graphics coordinates. Native SVG
    // keeps the stroke/gradient and hit target in the same transformed DOM stage.
    let svg = svg_element(document, "svg")?;
    svg.set_attribute("viewBox", "0 0 34 32").map_err(|_| "modal hotbar arrow view")?;
    let gradient_id = if previous { "hotbar-previous-gradient" } else { "hotbar-next-gradient" };
    let defs = svg_element(document, "defs")?;
    let gradient = svg_element(document, "linearGradient")?;
    for (name, value) in [("id", gradient_id), ("x1", "0"), ("y1", "0"), ("x2", "0"), ("y2", "1")] {
        gradient.set_attribute(name, value).map_err(|_| "hotbar gradient geometry")?;
    }
    for (offset, color) in [("0", "#29261b"), ("1", "#090a08")] {
        let stop = svg_element(document, "stop")?;
        stop.set_attribute("offset", offset).map_err(|_| "hotbar gradient stop")?;
        stop.set_attribute("stop-color", color).map_err(|_| "hotbar gradient color")?;
        gradient.append_child(&stop).map_err(|_| "hotbar gradient art")?;
    }
    defs.append_child(&gradient).map_err(|_| "hotbar gradient")?;
    svg.append_child(&defs).map_err(|_| "hotbar gradient definition")?;
    for (class, x, y, width, height, radius) in [
        ("hotbar-arrow-face", 0.5_f64, 0.5, 33.0, 31.0, 3.0),
        ("hotbar-arrow-inset", 2.0, 2.0, 30.0, 28.0, 2.0),
        ("hotbar-arrow-focus", -4.0, -4.0, 42.0, 40.0, 6.0),
    ] {
        let rect = svg_element(document, "rect")?;
        for (name, value) in [
            ("class", class.to_owned()),
            ("x", x.to_string()),
            ("y", y.to_string()),
            ("width", width.to_string()),
            ("height", height.to_string()),
            ("rx", radius.to_string()),
        ] {
            rect.set_attribute(name, &value).map_err(|_| "modal hotbar rectangle")?;
        }
        if class == "hotbar-arrow-face" {
            rect.set_attribute("fill", &format!("url(#{gradient_id})"))
                .map_err(|_| "hotbar gradient fill")?;
        }
        svg.append_child(&rect).map_err(|_| "modal hotbar rectangle art")?;
    }
    let chevron = svg_element(document, "path")?;
    chevron
        .set_attribute(
            "d",
            &format!(
                "M {} 10 L {} 16 L {} 22",
                17 - direction * 3,
                17 + direction * 3,
                17 - direction * 3
            ),
        )
        .map_err(|_| "modal hotbar chevron geometry")?;
    chevron.set_attribute("class", "hotbar-arrow-chevron").map_err(|_| "hotbar chevron class")?;
    svg.append_child(&chevron).map_err(|_| "hotbar chevron art")?;
    button.append_child(&svg).map_err(|_| "modal hotbar arrow art")?;
    Ok(button)
}

fn svg_element(document: &Document, tag: &str) -> Result<Element, String> {
    document
        .create_element_ns(Some("http://www.w3.org/2000/svg"), tag)
        .map_err(|_| format!("hotbar SVG {tag}"))
}
