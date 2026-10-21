//! NativeUiPartyMenu's browser gestures and trusted clipboard boundary. Scroll and
//! copy state are local presentation; membership actions still leave through SocialUi.

use std::cell::RefCell;
use std::rc::Rc;

use solomon_sim::hud::HudRect;
use solomon_sim::social::party_ui::{
    NATIVE_PARTY_MENU_FRAME, NATIVE_PARTY_MENU_VIEWPORT, NATIVE_PARTY_MENU_WHEEL_STEP,
    NATIVE_PARTY_VISIBILITIES, NativePartyMenuPlan, NativePartyMenuTab,
    clamp_native_party_menu_scroll, visibility_id,
};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Element, Event, HtmlElement, KeyboardEvent, PointerEvent, WheelEvent};

use super::super::{Intent, Listener, listen};

struct Drag {
    pointer_id: i32,
    started_y: f64,
    pointer_y: f64,
    scroll_y: f64,
    moved: bool,
}

pub(in crate::social) struct PartyMenuInteraction {
    pub scroll_y: f64,
    pub copied: bool,
    pub focus_action: Option<String>,
    copied_until: Option<f64>,
    active: bool,
    tab: NativePartyMenuTab,
    origin: [f64; 2],
    scale: f64,
    content_height: f64,
    visible_scroll: f64,
    rows: Vec<(String, HudRect)>,
    drag: Option<Drag>,
    suppress_click: bool,
}

impl Default for PartyMenuInteraction {
    fn default() -> Self {
        Self {
            scroll_y: 0.0,
            copied: false,
            focus_action: None,
            copied_until: None,
            active: false,
            tab: NativePartyMenuTab::Members,
            origin: [0.0; 2],
            scale: 1.0,
            content_height: 0.0,
            visible_scroll: 0.0,
            rows: Vec::new(),
            drag: None,
            suppress_click: false,
        }
    }
}

impl PartyMenuInteraction {
    pub fn select_tab(&mut self, tab: NativePartyMenuTab, opening: bool) {
        self.tab = tab;
        if opening {
            self.scroll_y = 0.0;
            self.focus_action = None;
        }
    }

    pub fn configure(&mut self, plan: &NativePartyMenuPlan, origin: [f64; 2], scale: f64) {
        self.active = true;
        self.origin = origin;
        self.scale = scale;
        self.content_height = plan.content_height;
        self.visible_scroll = plan.scroll_y;
        self.rows = plan.rows.iter().map(|row| (row.id.clone(), row.bounds)).collect();
    }

    pub fn close(&mut self) {
        self.active = false;
        self.drag = None;
        self.focus_action = None;
    }

    pub fn copied(&mut self, copied: bool, now: f64) {
        self.copied = copied;
        self.copied_until = copied.then_some(now + 1500.0);
    }

    pub fn expire_copy(&mut self, now: f64) -> bool {
        if self.copied_until.is_some_and(|until| now >= until) {
            self.copied = false;
            self.copied_until = None;
            return true;
        }
        false
    }

    fn point(&self, event: &Event) -> [f64; 2] {
        // The legacy web-sys MouseEvent accessors narrow client coordinates to i32.
        // Native DOM properties preserve fractional pointer coordinates like React.
        let client = |name: &str| {
            js_sys::Reflect::get(event.as_ref(), &name.into())
                .expect("MouseEvent exposes its client coordinate")
                .as_f64()
                .expect("MouseEvent client coordinates are numbers")
        };
        [
            (client("clientX") - self.origin[0]) / self.scale,
            (client("clientY") - self.origin[1]) / self.scale,
        ]
    }

    fn scroll(&mut self, requested: f64) -> bool {
        let next = clamp_native_party_menu_scroll(requested, self.content_height);
        if self.visible_scroll == next {
            return false;
        }
        self.scroll_y = next;
        true
    }
}

pub(in crate::social) fn listen_to_party(
    listeners: &mut Vec<Listener>,
    modal: &HtmlElement,
    state: &Rc<RefCell<PartyMenuInteraction>>,
    intents: &Rc<RefCell<Vec<Intent>>>,
) -> Result<(), String> {
    let input = state.clone();
    let pending = intents.clone();
    listen(listeners, modal.as_ref(), "pointerdown", true, move |event| {
        let Some(pointer) = event.dyn_ref::<PointerEvent>() else { return };
        let mut state = input.borrow_mut();
        if !state.active || pointer.button() != 0 {
            return;
        }
        let [x, y] = state.point(&event);
        if !contains(NATIVE_PARTY_MENU_FRAME, x, y) {
            pending.borrow_mut().push(Intent::Close);
            return;
        }
        if state.tab == NativePartyMenuTab::Members && contains(NATIVE_PARTY_MENU_VIEWPORT, x, y) {
            state.drag = Some(Drag {
                pointer_id: pointer.pointer_id(),
                started_y: y,
                pointer_y: y,
                scroll_y: state.visible_scroll,
                moved: false,
            });
        }
    })?;
    let input = state.clone();
    let pending = intents.clone();
    let target = modal.clone();
    listen(listeners, modal.as_ref(), "pointermove", false, move |event| {
        let Some(pointer) = event.dyn_ref::<PointerEvent>() else { return };
        let mut state = input.borrow_mut();
        let Some(mut drag) = state.drag.take() else { return };
        if drag.pointer_id != pointer.pointer_id() {
            state.drag = Some(drag);
            return;
        }
        let y = state.point(&event)[1];
        let next = clamp_native_party_menu_scroll(
            drag.scroll_y + drag.pointer_y - y,
            state.content_height,
        );
        let capture = !drag.moved && (y - drag.started_y).abs() >= 3.0;
        if capture {
            drag.moved = true;
        }
        drag.pointer_y = y;
        drag.scroll_y = next;
        if drag.moved {
            event.prevent_default();
            state.scroll_y = next;
            pending.borrow_mut().push(Intent::PartyChanged);
        }
        state.drag = Some(drag);
        drop(state);
        if capture && let Err(error) = target.set_pointer_capture(pointer.pointer_id()) {
            web_sys::console::error_1(&error);
        }
    })?;
    for name in ["pointerup", "pointercancel"] {
        let input = state.clone();
        let target = modal.clone();
        listen(listeners, modal.as_ref(), name, false, move |event| {
            let Some(pointer) = event.dyn_ref::<PointerEvent>() else { return };
            let mut state = input.borrow_mut();
            let Some(drag) = state.drag.take() else { return };
            if drag.pointer_id != pointer.pointer_id() {
                state.drag = Some(drag);
                return;
            }
            state.suppress_click = drag.moved;
            drop(state);
            if target.has_pointer_capture(pointer.pointer_id())
                && let Err(error) = target.release_pointer_capture(pointer.pointer_id())
            {
                web_sys::console::error_1(&error);
            }
            if drag.moved {
                let input = input.clone();
                let callback = Closure::once_into_js(move || {
                    input.borrow_mut().suppress_click = false;
                });
                if let Some(window) = web_sys::window()
                    && let Err(error) = window
                        .set_timeout_with_callback_and_timeout_and_arguments_0(
                            callback.unchecked_ref(),
                            0,
                        )
                {
                    web_sys::console::error_1(&error);
                }
            }
        })?;
    }
    let input = state.clone();
    listen(listeners, modal.as_ref(), "lostpointercapture", false, move |_| {
        input.borrow_mut().drag = None;
    })?;
    let input = state.clone();
    listen(listeners, modal.as_ref(), "click", false, move |event| {
        let mut state = input.borrow_mut();
        if state.active && state.suppress_click {
            state.suppress_click = false;
            event.prevent_default();
            event.stop_propagation();
        }
    })?;
    let input = state.clone();
    let pending = intents.clone();
    listen(listeners, modal.as_ref(), "wheel", false, move |event| {
        let Some(wheel) = event.dyn_ref::<WheelEvent>() else { return };
        let mut state = input.borrow_mut();
        if !state.active || state.tab != NativePartyMenuTab::Members || wheel.delta_y() == 0.0 {
            return;
        }
        let [x, y] = state.point(&event);
        if !contains(NATIVE_PARTY_MENU_VIEWPORT, x, y) {
            return;
        }
        let requested =
            state.visible_scroll + wheel.delta_y().signum() * NATIVE_PARTY_MENU_WHEEL_STEP;
        if state.scroll(requested) {
            event.prevent_default();
            pending.borrow_mut().push(Intent::PartyChanged);
        }
    })?;
    let input = state.clone();
    let pending = intents.clone();
    listen(listeners, modal.as_ref(), "focusin", false, move |event| {
        let Some(target) = event.target().and_then(|node| node.dyn_into::<Element>().ok()) else {
            return;
        };
        let Some(id) = target.get_attribute("data-native-ui-party-row-id") else { return };
        let mut state = input.borrow_mut();
        let Some((_, row)) = state.rows.iter().find(|(row, _)| *row == id) else { return };
        let viewport = NATIVE_PARTY_MENU_VIEWPORT;
        let requested = if row.y < viewport.y {
            state.visible_scroll - (viewport.y - row.y)
        } else if row.y + row.height > viewport.y + viewport.height {
            state.visible_scroll + (row.y + row.height - viewport.y - viewport.height)
        } else {
            return;
        };
        if state.scroll(requested) {
            pending.borrow_mut().push(Intent::PartyChanged);
        }
    })?;
    let input = state.clone();
    let pending = intents.clone();
    listen(listeners, modal.as_ref(), "keydown", false, move |event| {
        let Some(key) = event.dyn_ref::<KeyboardEvent>() else { return };
        let Some(target) = event.target().and_then(|node| node.dyn_into::<Element>().ok()) else {
            return;
        };
        let mut state = input.borrow_mut();
        if !state.active {
            return;
        }
        if target.has_attribute("data-native-ui-party-tab-action")
            && matches!(key.key().as_str(), "ArrowLeft" | "ArrowRight")
        {
            event.prevent_default();
            let index = NativePartyMenuTab::ALL
                .iter()
                .position(|tab| *tab == state.tab)
                .expect("the selected tab exists");
            let step = if key.key() == "ArrowLeft" { 2 } else { 1 };
            let next = NativePartyMenuTab::ALL[(index + step) % 3];
            state.focus_action = Some(format!("tab:{}", next.id()));
            pending.borrow_mut().push(Intent::Action(format!("party-{}", next.id())));
        } else if target.has_attribute("data-native-ui-party-visibility")
            && matches!(key.key().as_str(), "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown")
        {
            event.prevent_default();
            let current =
                target.get_attribute("data-party-selected-visibility").unwrap_or_default();
            let index = NATIVE_PARTY_VISIBILITIES
                .iter()
                .position(|visibility| visibility_id(*visibility) == current)
                .unwrap_or(0);
            let step = if matches!(key.key().as_str(), "ArrowLeft" | "ArrowUp") { 2 } else { 1 };
            let next = visibility_id(NATIVE_PARTY_VISIBILITIES[(index + step) % 3]);
            state.focus_action = Some(format!("visibility:{next}"));
            pending.borrow_mut().push(Intent::Action(format!("visibility:{next}")));
            pending.borrow_mut().push(Intent::PartyChanged);
        } else if target.has_attribute("data-native-ui-party-row-id") {
            let delta = match key.key().as_str() {
                "ArrowUp" => -NATIVE_PARTY_MENU_WHEEL_STEP,
                "ArrowDown" => NATIVE_PARTY_MENU_WHEEL_STEP,
                "PageUp" => -NATIVE_PARTY_MENU_VIEWPORT.height,
                "PageDown" => NATIVE_PARTY_MENU_VIEWPORT.height,
                _ => return,
            };
            event.prevent_default();
            let requested = state.visible_scroll + delta;
            if state.scroll(requested) {
                pending.borrow_mut().push(Intent::PartyChanged);
            }
        }
    })?;
    Ok(())
}

pub(in crate::social) fn copy_party_code(
    target: &Element,
    state: &Rc<RefCell<PartyMenuInteraction>>,
    intents: &Rc<RefCell<Vec<Intent>>>,
) {
    let Some(code) = target.get_attribute("data-party-code").filter(|code| !code.is_empty()) else {
        return;
    };
    state.borrow_mut().copied_until = None;
    // Keep the platform invocation in the original trusted click, before awaiting.
    let promise = (|| {
        let navigator = web_sys::window()?.navigator();
        let clipboard = js_sys::Reflect::get(&navigator, &"clipboard".into()).ok()?;
        let write: js_sys::Function =
            js_sys::Reflect::get(&clipboard, &"writeText".into()).ok()?.dyn_into().ok()?;
        write.call1(&clipboard, &code.clone().into()).ok()?.dyn_into::<js_sys::Promise>().ok()
    })();
    let Some(promise) = promise else { return };
    let pending = intents.clone();
    spawn_local(async move {
        let copied = JsFuture::from(promise).await.is_ok();
        pending.borrow_mut().push(Intent::PartyCopied { code, copied });
    });
}

fn contains(bounds: HudRect, x: f64, y: f64) -> bool {
    x >= bounds.x && x < bounds.x + bounds.width && y >= bounds.y && y < bounds.y + bounds.height
}
