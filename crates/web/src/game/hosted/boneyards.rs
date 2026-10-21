//! Website `DarkCloudBoneyards` remains an editor-owned same-origin document.
//! Its one game message forwards the configured menu key to the retained Cloud menu.

use std::{cell::Cell, rc::Rc};

use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{HtmlElement, HtmlIFrameElement, MessageEvent, Window};

use crate::hosted::WebsiteMount;

pub(super) struct BoneyardLibrary {
    frame: HtmlIFrameElement,
    window: Window,
    menu_requested: Rc<Cell<bool>>,
    message: Closure<dyn FnMut(MessageEvent)>,
}

impl BoneyardLibrary {
    pub(super) fn mount(
        mount: &HtmlElement,
        website: &WebsiteMount,
        menu_key: &str,
    ) -> Result<Self, String> {
        let document = mount.owner_document().ok_or("The Boneyard library has no document")?;
        let window = document.default_view().ok_or("The Boneyard library has no window")?;
        let origin = window.location().origin().map_err(|_| "Boneyard library origin")?;
        let frame: HtmlIFrameElement =
            crate::ui::element(&document, "iframe", "boneyard-library-frame")?
                .dyn_into()
                .map_err(|_| "Boneyard library frame")?;
        frame.set_title("Your boneyards");
        let url = web_sys::Url::new_with_base(&website.path("/boneyard-library"), &origin)
            .map_err(|_| "Boneyard library address")?;
        url.search_params().set("menu-key", menu_key);
        frame.set_src(&url.href());
        let menu_requested = Rc::new(Cell::new(false));
        let request = menu_requested.clone();
        let source_frame = frame.clone();
        let message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            if event.origin() != origin {
                return;
            }
            let (Some(source), Some(frame_window)) =
                (event.source(), source_frame.content_window())
            else {
                return;
            };
            if !js_sys::Object::is(source.as_ref(), frame_window.as_ref()) {
                return;
            }
            let data = event.data();
            if !data.is_object() || data.is_null() {
                return;
            }
            if let Ok(kind) = js_sys::Reflect::get(&data, &JsValue::from_str("type"))
                && kind.as_string().as_deref() == Some("sdr-boneyard-library-menu")
            {
                request.set(true);
            }
        });
        let library = Self { frame, window, menu_requested, message };
        library
            .window
            .add_event_listener_with_callback("message", library.message.as_ref().unchecked_ref())
            .map_err(|_| "Could not connect the Boneyard library menu")?;
        mount.append_child(&library.frame).map_err(|_| "Could not mount the Boneyard library")?;
        Ok(library)
    }

    pub(super) fn take_menu_requested(&self) -> bool {
        self.menu_requested.replace(false)
    }
}

impl Drop for BoneyardLibrary {
    fn drop(&mut self) {
        if let Err(error) = self
            .window
            .remove_event_listener_with_callback("message", self.message.as_ref().unchecked_ref())
        {
            web_sys::console::error_1(&error);
        }
        self.frame.remove();
    }
}
