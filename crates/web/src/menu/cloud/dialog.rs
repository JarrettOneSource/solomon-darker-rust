//! Native browser modal behavior matches Website `NativeUiDialog.tsx`: focus
//! containment, Escape cancellation, backdrop dismissal and focus restoration.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Event, HtmlDialogElement, HtmlElement, MouseEvent};

use crate::ui::element;

pub(super) struct CloudDialog {
    pub root: HtmlDialogElement,
    pub art: HtmlElement,
    _listeners: Vec<Closure<dyn FnMut(Event)>>,
}

impl CloudDialog {
    pub fn new(
        document: &Document,
        parent: &HtmlElement,
        class: &str,
        label: &str,
        events: Rc<RefCell<Vec<String>>>,
    ) -> Result<Self, String> {
        let root: HtmlDialogElement =
            element(document, "dialog", &format!("native-ui-dialog dark-cloud-panel {class}"))?
                .dyn_into()
                .map_err(|_| "Dark Cloud dialog element")?;
        root.set_attribute("aria-label", label).map_err(|_| "Dark Cloud dialog label")?;
        root.set_attribute("aria-modal", "true").map_err(|_| "Dark Cloud modal state")?;
        root.set_attribute("role", "dialog").map_err(|_| "Dark Cloud dialog role")?;
        let art = element(document, "div", "dark-cloud-native-panel-art")?;
        root.append_child(&art).map_err(|_| "Dark Cloud panel art")?;
        parent.append_child(&root).map_err(|_| "Dark Cloud dialog parent")?;
        let cancelled = events.clone();
        let cancel = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            event.prevent_default();
            cancelled.borrow_mut().push("cloud-dialog-close".to_owned());
        });
        root.add_event_listener_with_callback("cancel", cancel.as_ref().unchecked_ref())
            .map_err(|_| "Dark Cloud cancel listener")?;
        let mouse_root = root.clone();
        let mouse = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(event) = event.dyn_ref::<MouseEvent>() else {
                return;
            };
            if event.target().is_none_or(|target| target != mouse_root.clone().into()) {
                return;
            }
            let bounds = mouse_root.get_bounding_client_rect();
            let x = f64::from(event.client_x());
            let y = f64::from(event.client_y());
            if x < bounds.left() || x > bounds.right() || y < bounds.top() || y > bounds.bottom() {
                events.borrow_mut().push("cloud-dialog-close".to_owned());
            }
        });
        root.add_event_listener_with_callback("mousedown", mouse.as_ref().unchecked_ref())
            .map_err(|_| "Dark Cloud backdrop listener")?;
        Ok(Self { root, art, _listeners: vec![cancel, mouse] })
    }

    pub fn show(&self) -> Result<(), String> {
        self.root.show_modal().map_err(|_| "open Dark Cloud modal")?;
        if let Some(target) = self
            .root
            .query_selector("[data-game-default-focus]")
            .map_err(|_| "Dark Cloud default focus")?
        {
            target
                .dyn_into::<HtmlElement>()
                .map_err(|_| "Dark Cloud focus target")?
                .focus()
                .map_err(|_| "focus Dark Cloud modal")?;
        }
        Ok(())
    }

    pub fn element(&self) -> &HtmlElement {
        self.root.unchecked_ref()
    }
}

impl Drop for CloudDialog {
    fn drop(&mut self) {
        self.root.close();
        self.root.remove();
    }
}
