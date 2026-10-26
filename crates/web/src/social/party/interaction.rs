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
use crate::platform::Surface;
use egui::Response;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, closure::Closure};
#[cfg(target_arch = "wasm32")]
use web_sys::{Document, Event, MouseEvent};

/// Browser clipboard capability is invoked synchronously from the trusted GPU hit.
/// Its promise result retains the displayed code, so a later snapshot cannot report a stale copy.
pub(in crate::social) struct PartyClipboard {
    #[cfg(target_arch = "wasm32")]
    document: Document,
    target: Rc<RefCell<Option<Response>>>,
    code: Rc<RefCell<Option<String>>>,
    handled: Rc<Cell<bool>>,
    pending: Rc<RefCell<Vec<Intent>>>,
    #[cfg(target_arch = "wasm32")]
    click: Closure<dyn FnMut(Event)>,
}
impl PartyClipboard {
    pub fn new(surface: &Surface, pending: Rc<RefCell<Vec<Intent>>>) -> Result<Self, String> {
        let target: Rc<RefCell<Option<Response>>> = Rc::new(RefCell::new(None));
        let code: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let handled = Rc::new(Cell::new(false));
        #[cfg(target_arch = "wasm32")]
        let document = surface.canvas().owner_document().ok_or("The game has no document")?;
        #[cfg(not(target_arch = "wasm32"))]
        let _ = surface;
        #[cfg(target_arch = "wasm32")]
        let click = {
            let clicked_target = target.clone();
            let clicked_code = code.clone();
            let clicked_handled = handled.clone();
            let clicked_pending = pending.clone();
            let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                let Some(mouse) = event.dyn_ref::<MouseEvent>() else { return };
                let point = egui::pos2(mouse.client_x() as f32, mouse.client_y() as f32);
                if clicked_target
                    .borrow()
                    .as_ref()
                    .is_some_and(|target| crate::ui::trusted_gesture::owns_point(target, point))
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
            click
        };
        Ok(Self {
            #[cfg(target_arch = "wasm32")]
            document,
            target,
            code,
            handled,
            pending,
            #[cfg(target_arch = "wasm32")]
            click,
        })
    }
    pub fn target(&self, response: &Response, code: String) {
        *self.target.borrow_mut() = Some(response.clone());
        *self.code.borrow_mut() = Some(code);
    }
    pub fn clear(&self) {
        self.target.borrow_mut().take();
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
    let request = crate::platform::copy_text(&code);
    crate::platform::spawn_local(async move {
        let copied = match request {
            Ok(request) => request.await.is_ok(),
            Err(_) => false,
        };
        pending.borrow_mut().push(Intent::PartyCopied { code, copied });
    });
}
#[cfg(target_arch = "wasm32")]
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
