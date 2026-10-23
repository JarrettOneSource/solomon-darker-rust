//! GPU Party scroll/copy state. The host retains membership and directory authority.

use solomon_sim::social::party_ui::NativePartyMenuTab;

#[derive(Default)]
pub(in crate::social) struct PartyMenuInteraction {
    pub scroll_y: f64,
    pub copied: bool,
    copied_until: Option<f64>,
}
impl PartyMenuInteraction {
    pub fn select_tab(&mut self, _tab: NativePartyMenuTab, opening: bool) {
        if opening {
            self.scroll_y = 0.0;
        }
    }
    pub fn copied(&mut self, copied: bool, now: f64) {
        self.copied = copied;
        self.copied_until = copied.then_some(now + 1500.0);
    }
    pub fn expire_copy(&mut self, now: f64) {
        if self.copied_until.is_some_and(|until| now >= until) {
            self.copied = false;
            self.copied_until = None;
        }
    }
}

use super::super::Intent;
use egui::Rect;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Event, MouseEvent};

/// Browser clipboard capability is invoked synchronously from the trusted GPU hit.
/// Its promise result retains the displayed code, so a later snapshot cannot report a stale copy.
pub(in crate::social) struct PartyClipboard {
    document: Document,
    bounds: Rc<Cell<Option<Rect>>>,
    code: Rc<RefCell<Option<String>>>,
    handled: Rc<Cell<bool>>,
    pending: Rc<RefCell<Vec<Intent>>>,
    click: Closure<dyn FnMut(Event)>,
}
impl PartyClipboard {
    pub fn new(document: &Document, pending: Rc<RefCell<Vec<Intent>>>) -> Result<Self, String> {
        let bounds: Rc<Cell<Option<Rect>>> = Rc::new(Cell::new(None));
        let code: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let handled = Rc::new(Cell::new(false));
        let clicked_bounds = bounds.clone();
        let clicked_code = code.clone();
        let clicked_handled = handled.clone();
        let clicked_pending = pending.clone();
        let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(mouse) = event.dyn_ref::<MouseEvent>() else { return };
            let point = egui::pos2(mouse.client_x() as f32, mouse.client_y() as f32);
            if clicked_bounds.get().is_some_and(|bounds| bounds.contains(point))
                && let Some(code) = clicked_code.borrow().clone()
            {
                event.prevent_default();
                clicked_handled.set(true);
                copy(code, clicked_pending.clone());
            }
        });
        document
            .add_event_listener_with_callback_and_bool(
                "click",
                click.as_ref().unchecked_ref(),
                true,
            )
            .map_err(|_| "Party clipboard trusted click")?;
        Ok(Self { document: document.clone(), bounds, code, handled, pending, click })
    }
    pub fn target(&self, bounds: Rect, code: String) {
        self.bounds.set(Some(bounds));
        *self.code.borrow_mut() = Some(code);
    }
    pub fn clear(&self) {
        self.bounds.set(None);
        self.code.borrow_mut().take();
    }
    pub fn copy(&self, code: String) {
        if !self.handled.replace(false) {
            copy(code, self.pending.clone());
        }
    }
    pub fn finish_frame(&self) {
        self.handled.set(false);
    }
}
fn copy(code: String, pending: Rc<RefCell<Vec<Intent>>>) {
    let promise = (|| {
        let navigator = web_sys::window()?.navigator();
        let clipboard = js_sys::Reflect::get(&navigator, &"clipboard".into()).ok()?;
        let write: js_sys::Function =
            js_sys::Reflect::get(&clipboard, &"writeText".into()).ok()?.dyn_into().ok()?;
        write.call1(&clipboard, &code.clone().into()).ok()?.dyn_into::<js_sys::Promise>().ok()
    })();
    let Some(promise) = promise else {
        pending.borrow_mut().push(Intent::PartyCopied { code, copied: false });
        return;
    };
    wasm_bindgen_futures::spawn_local(async move {
        let copied = wasm_bindgen_futures::JsFuture::from(promise).await.is_ok();
        pending.borrow_mut().push(Intent::PartyCopied { code, copied });
    });
}
impl Drop for PartyClipboard {
    fn drop(&mut self) {
        if let Err(error) = self.document.remove_event_listener_with_callback_and_bool(
            "click",
            self.click.as_ref().unchecked_ref(),
            true,
        ) {
            web_sys::console::error_1(&error);
        }
    }
}
