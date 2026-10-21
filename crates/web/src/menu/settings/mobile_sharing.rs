//! Current `MobileUiLayoutSettingsAction.tsx`: actual submission and trusted copy.
//! The existing hosted owner reads the saved device document and returns its real code.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::{JsCast, closure::Closure};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Document, Event, HtmlElement};

use super::SettingsView;
use crate::hosted::SharedMobileUiLayout;
use crate::ui::settings::{NativeSettingsArt, settings_text};
use crate::ui::{append, element};

type LayoutCopyReceipt = Result<(), String>;

pub(super) struct SettingsLayoutPublication {
    pub root: HtmlElement,
    document: Document,
    button: HtmlElement,
    label: HtmlElement,
    note: HtmlElement,
    receipt: HtmlElement,
    code: HtmlElement,
    copy: HtmlElement,
    error: HtmlElement,
    pending: Option<u64>,
    account_available: bool,
    customized: bool,
    copy_code: Rc<RefCell<Option<String>>>,
    copy_result: Rc<RefCell<Option<LayoutCopyReceipt>>>,
    copy_listener: Closure<dyn FnMut(Event)>,
}

impl SettingsView {
    pub fn begin_mobile_layout_publication(&mut self, id: u64) -> Result<bool, String> {
        let Some(view) = &mut self.mobile_sharing else { return Ok(false) };
        view.begin(id)
    }

    pub fn mobile_layout_publication_pending(&self, id: u64) -> bool {
        self.mobile_sharing.as_ref().is_some_and(|view| view.pending == Some(id))
    }

    pub fn complete_mobile_layout_publication(
        &mut self,
        id: u64,
        result: Result<SharedMobileUiLayout, String>,
    ) -> Result<bool, String> {
        let Some(view) = &mut self.mobile_sharing else { return Ok(false) };
        if view.pending != Some(id) {
            return Ok(false);
        }
        view.pending = None;
        match result {
            Ok(layout) => {
                view.code.set_text_content(Some(&layout.code));
                *view.copy_code.borrow_mut() = Some(layout.code);
                view.receipt.set_hidden(false);
            }
            Err(error) => view.show_error(&error),
        }
        view.paint_admission()?;
        Ok(true)
    }
}

impl SettingsLayoutPublication {
    pub fn new(
        document: &Document,
        art: Rc<NativeSettingsArt>,
        account_available: bool,
        customized: bool,
    ) -> Result<Self, String> {
        let root = element(document, "div", "game-settings-mobile-ui-share")?;
        let button = element(document, "button", "game-settings-action")?;
        button.set_attribute("type", "button").map_err(|_| "mobile publication button")?;
        button
            .set_attribute("data-menu-action", "settings-mobile-publish")
            .map_err(|_| "mobile publication action")?;
        art.row(document, &button)?;
        let label = element(document, "span", "game-settings-label")?;
        append(&button, &label)?;
        art.arrow(document, &button)?;
        append(&root, &button)?;
        let note = element(document, "small", "")?;
        append(&root, &note)?;
        let receipt = element(document, "div", "game-settings-layout-code")?;
        receipt.set_attribute("role", "status").map_err(|_| "mobile share receipt")?;
        receipt.set_hidden(true);
        let heading = element(document, "span", "")?;
        heading.set_text_content(Some("SHARE CODE"));
        append(&receipt, &heading)?;
        let code = element(document, "output", "")?;
        append(&receipt, &code)?;
        let copy = element(document, "button", "")?;
        copy.set_attribute("type", "button").map_err(|_| "copy layout code button")?;
        copy.set_attribute("data-mobile-layout-copy", "true")
            .map_err(|_| "copy layout code owner")?;
        copy.set_text_content(Some("COPY"));
        append(&receipt, &copy)?;
        append(&root, &receipt)?;
        let error = element(document, "small", "game-settings-mobile-ui-share-error")?;
        error.set_attribute("role", "alert").map_err(|_| "mobile share error")?;
        error.set_hidden(true);
        append(&root, &error)?;
        let copy_code = Rc::new(RefCell::new(None::<String>));
        let copy_result = Rc::new(RefCell::new(None));
        let retained_code = copy_code.clone();
        let result = copy_result.clone();
        let copy_listener = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            event.stop_propagation();
            let Some(code) = retained_code.borrow().clone() else { return };
            // Invoke clipboard.writeText inside the trusted click, before awaiting.
            let clipboard = web_sys::window().expect("browser window").navigator().clipboard();
            let promise = clipboard.write_text(&code);
            let result = result.clone();
            spawn_local(async move {
                *result.borrow_mut() = Some(
                    JsFuture::from(promise)
                        .await
                        .map(|_| ())
                        .map_err(|_| "The code could not be copied.".to_owned()),
                );
            });
        });
        copy.add_event_listener_with_callback("click", copy_listener.as_ref().unchecked_ref())
            .map_err(|_| "mobile copy listener")?;
        let mut view = Self {
            root,
            document: document.clone(),
            button,
            label,
            note,
            receipt,
            code,
            copy,
            error,
            pending: None,
            account_available,
            customized,
            copy_code,
            copy_result,
            copy_listener,
        };
        view.paint_admission()?;
        Ok(view)
    }

    pub fn set_admission(
        &mut self,
        account_available: bool,
        customized: bool,
    ) -> Result<(), String> {
        self.account_available = account_available;
        self.customized = customized;
        self.paint_admission()
    }

    fn begin(&mut self, id: u64) -> Result<bool, String> {
        if self.pending.is_some() || !self.account_available || !self.customized {
            return Ok(false);
        }
        self.pending = Some(id);
        self.error.set_hidden(true);
        self.receipt.set_hidden(true);
        self.copy.set_text_content(Some("COPY"));
        *self.copy_code.borrow_mut() = None;
        self.paint_admission()?;
        Ok(true)
    }

    pub fn update(&mut self) -> Result<(), String> {
        let receipt = self.copy_result.borrow_mut().take();
        if let Some(result) = receipt {
            match result {
                Ok(()) => {
                    self.copy.set_text_content(Some("COPIED"));
                    self.error.set_hidden(true);
                }
                Err(error) => self.show_error(&error),
            }
        }
        Ok(())
    }

    fn paint_admission(&mut self) -> Result<(), String> {
        let label =
            if self.pending.is_some() { "SUBMITTING MOBILE UI..." } else { "SUBMIT TO DARK CLOUD" };
        self.label.set_text_content(None);
        append(&self.label, &settings_text(&self.document, label, 1.0, 0xd9bd72, None)?)?;
        self.button.set_attribute("aria-label", label).map_err(|_| "mobile submit label")?;
        self.button
            .toggle_attribute_with_force(
                "disabled",
                self.pending.is_some() || !self.account_available || !self.customized,
            )
            .map_err(|_| "mobile submission admission")?;
        let note = if !self.account_available {
            "SIGN IN TO SUBMIT A MOBILE UI LAYOUT."
        } else if !self.customized {
            "CUSTOMIZE AND SAVE A MOBILE UI LAYOUT FIRST."
        } else {
            ""
        };
        self.note.set_text_content(Some(note));
        self.note.set_hidden(note.is_empty());
        Ok(())
    }

    fn show_error(&self, error: &str) {
        self.error.set_text_content(Some(error));
        self.error.set_hidden(false);
    }
}

impl Drop for SettingsLayoutPublication {
    fn drop(&mut self) {
        if let Err(error) = self.copy.remove_event_listener_with_callback(
            "click",
            self.copy_listener.as_ref().unchecked_ref(),
        ) {
            web_sys::console::error_1(&error);
        }
    }
}
