//! Website `DarkCloudLayouts.tsx`. Loading is acknowledged only after the root
//! validates and installs the real device layout; submission reads that same store.

use std::cell::RefCell;
use std::rc::Rc;

use solomon_sim::native_ui_text::NativeUiFontName::{Medium, Menu};
use wasm_bindgen::{JsCast, closure::Closure};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Document, Event, HtmlElement, HtmlInputElement};

use super::widgets::{self, control_action, control_panel, narrative, text};
use crate::hosted::SharedMobileUiLayout;
use crate::menu::HostedMenuAction;
use crate::ui::{append, element};

#[derive(Clone, Copy)]
enum LayoutAction {
    Load,
    Publish,
}

type LayoutCopyReceipt = (u64, Result<(), String>);

pub(super) struct LayoutsView {
    pub root: HtmlElement,
    document: Document,
    account: Option<String>,
    customized: bool,
    input: HtmlInputElement,
    load: HtmlElement,
    publish: HtmlElement,
    message: HtmlElement,
    receipt: HtmlElement,
    notice: HtmlElement,
    pending: Option<(u64, LayoutAction)>,
    shared: Option<(LayoutAction, SharedMobileUiLayout)>,
    actions: Vec<HostedMenuAction>,
    copy_result: Rc<RefCell<Option<LayoutCopyReceipt>>>,
    copy_generation: u64,
    copy_listener: Option<Closure<dyn FnMut(Event)>>,
}

impl LayoutsView {
    pub fn new(document: &Document) -> Result<Self, String> {
        let root = element(document, "section", "dark-cloud-layouts")?;
        root.set_attribute("aria-label", "Shared mobile UI layouts")
            .map_err(|_| "shared layouts label")?;
        root.set_hidden(true);
        let header = element(document, "header", "")?;
        text(document, &header, "h2", "", "MOBILE UI LAYOUTS", Menu, 1.0, 0xd9ba70, false)?;
        append(&root, &header)?;
        let sections = element(document, "div", "dark-cloud-layout-sections")?;
        let section = element(document, "section", "dark-cloud-layout-section")?;
        text(document, &section, "h3", "", "LOAD A LAYOUT", Medium, 1.0, 0xd9ba70, false)?;
        let panel = control_panel(document, &section, "", false)?;
        let form = element(document, "form", "")?;
        form.set_attribute("data-menu-action", "cloud-layout-load")
            .map_err(|_| "shared layout form action")?;
        let row = element(document, "div", "native-ui-control-field-row")?;
        let label = text(document, &row, "label", "", "SHARE CODE", Medium, 1.0, 0xd9ba70, false)?;
        label
            .set_attribute("for", "dark-cloud-layout-code")
            .map_err(|_| "shared code input label")?;
        let field = control_panel(document, &row, "native-ui-control-field", true)?;
        let input: HtmlInputElement =
            element(document, "input", "")?.dyn_into().map_err(|_| "shared layout code input")?;
        input.set_id("dark-cloud-layout-code");
        input.set_max_length(9);
        input.set_placeholder("ABCD-EFGH");
        input.set_spellcheck(false);
        input.set_attribute("autocomplete", "off").map_err(|_| "shared code completion")?;
        input.set_attribute("inputmode", "text").map_err(|_| "shared code keyboard")?;
        field.append_child(&input).map_err(|_| "shared code input parent")?;
        append(&form, &row)?;
        let load =
            control_action(document, &form, "LOAD LAYOUT", "cloud-layout-load", false, true)?;
        load.set_attribute("type", "submit").map_err(|_| "shared code submit")?;
        append(&panel, &form)?;
        narrative(document, &section, "p", "", "Replaces the layout saved on this device.")?;
        append(&sections, &section)?;
        let section = element(document, "section", "dark-cloud-layout-section")?;
        text(document, &section, "h3", "", "SUBMIT YOUR LAYOUT", Medium, 1.0, 0xd9ba70, false)?;
        let panel = control_panel(document, &section, "", false)?;
        let publish = control_action(
            document,
            &panel,
            "SUBMIT CURRENT LAYOUT",
            "cloud-layout-publish",
            false,
            true,
        )?;
        let message = narrative(document, &section, "p", "", "Sign in to submit.")?;
        append(&sections, &section)?;
        append(&root, &sections)?;
        let receipt = element(document, "div", "dark-cloud-layout-receipt")?;
        receipt.set_attribute("role", "status").map_err(|_| "shared layout receipt role")?;
        receipt.set_hidden(true);
        append(&root, &receipt)?;
        let notice = element(document, "p", "dark-cloud-layout-error")?;
        notice.set_attribute("role", "alert").map_err(|_| "shared layout error role")?;
        notice.set_hidden(true);
        append(&root, &notice)?;
        Ok(Self {
            root,
            document: document.clone(),
            account: None,
            customized: false,
            input,
            load,
            publish,
            message,
            receipt,
            notice,
            pending: None,
            shared: None,
            actions: Vec::new(),
            copy_result: Rc::new(RefCell::new(None)),
            copy_generation: 0,
            copy_listener: None,
        })
    }

    pub fn set_account(&mut self, account: Option<&str>) -> Result<(), String> {
        self.account = account.map(str::to_owned);
        self.paint_admission()
    }
    pub fn set_customized(&mut self, customized: bool) -> Result<(), String> {
        self.customized = customized;
        self.paint_admission()
    }

    pub fn close(&mut self) {
        self.pending = None;
        self.shared = None;
        self.actions.clear();
        self.copy_generation += 1;
        self.copy_result.borrow_mut().take();
        self.receipt.set_text_content(None);
        self.receipt.set_hidden(true);
        self.copy_listener = None;
        self.input.set_value("");
        self.notice.set_hidden(true);
    }

    pub fn open(&mut self) -> Result<(), String> {
        self.close();
        self.paint_admission()
    }

    pub fn activate(&mut self, action: &str, next: &mut u64) -> Result<bool, String> {
        if action == "cloud-layout-edited" {
            let compact: String = self
                .input
                .value()
                .to_uppercase()
                .chars()
                .filter(|character| "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(*character))
                .take(8)
                .collect();
            let code = if compact.len() > 4 {
                format!("{}-{}", &compact[..4], &compact[4..])
            } else {
                compact
            };
            self.input.set_value(&code);
            self.paint_admission()?;
            return Ok(true);
        }
        let action = match action {
            "cloud-layout-load" => LayoutAction::Load,
            "cloud-layout-publish" => LayoutAction::Publish,
            _ => return Ok(false),
        };
        if self.pending.is_some() {
            return Ok(true);
        }
        if matches!(action, LayoutAction::Load) && self.input.value().len() != 9 {
            return Ok(true);
        }
        if matches!(action, LayoutAction::Publish) && (self.account.is_none() || !self.customized) {
            return Ok(true);
        }
        *next += 1;
        self.pending = Some((*next, action));
        self.shared = None;
        self.copy_generation += 1;
        self.receipt.set_text_content(None);
        self.receipt.set_hidden(true);
        self.copy_listener = None;
        self.notice.set_hidden(true);
        self.actions.push(match action {
            LayoutAction::Load => {
                HostedMenuAction::LoadMobileLayout { request_id: *next, code: self.input.value() }
            }
            LayoutAction::Publish => HostedMenuAction::PublishMobileLayout { request_id: *next },
        });
        self.paint_admission()?;
        Ok(true)
    }

    pub fn complete(
        &mut self,
        request_id: u64,
        result: Result<SharedMobileUiLayout, String>,
    ) -> Result<bool, String> {
        let Some((pending, action)) = self.pending else {
            return Ok(false);
        };
        if pending != request_id {
            return Ok(false);
        }
        self.pending = None;
        match result {
            Ok(layout) => {
                self.input.set_value(&layout.code);
                self.shared = Some((action, layout));
                self.paint_receipt()?;
            }
            Err(error) => {
                self.notice.set_text_content(Some(&error));
                self.notice.set_hidden(false);
            }
        }
        self.paint_admission()?;
        Ok(true)
    }

    pub fn update(&mut self) -> Result<(), String> {
        let result = self.copy_result.borrow_mut().take();
        if let Some((generation, result)) = result
            && generation == self.copy_generation
        {
            match result {
                Ok(()) => {
                    self.notice.set_hidden(true);
                    if let Some(copy) = self
                        .receipt
                        .query_selector("button")
                        .map_err(|_| "shared code copy target")?
                    {
                        copy.set_attribute("aria-label", "COPIED")
                            .map_err(|_| "shared code copied semantics")?;
                        copy.set_attribute("data-cloud-label", "COPIED")
                            .map_err(|_| "shared code copied art")?;
                        copy.remove_attribute("data-cloud-art-size")
                            .map_err(|_| "shared code copied refresh")?;
                    }
                }
                Err(error) => {
                    self.notice.set_text_content(Some(&error));
                    self.notice.set_hidden(false);
                }
            }
        }
        Ok(())
    }

    pub fn take_actions(&mut self) -> Vec<HostedMenuAction> {
        std::mem::take(&mut self.actions)
    }

    fn paint_admission(&self) -> Result<(), String> {
        let busy = self.pending.is_some();
        let loading = self.pending.is_some_and(|(_, action)| matches!(action, LayoutAction::Load));
        let publishing =
            self.pending.is_some_and(|(_, action)| matches!(action, LayoutAction::Publish));
        for (node, label, disabled) in [
            (
                &self.load,
                if loading { "LOADING..." } else { "LOAD LAYOUT" },
                busy || self.input.value().len() != 9,
            ),
            (
                &self.publish,
                if publishing { "SUBMITTING..." } else { "SUBMIT CURRENT LAYOUT" },
                busy || self.account.is_none() || !self.customized,
            ),
        ] {
            node.toggle_attribute_with_force("disabled", disabled)
                .map_err(|_| "shared layout admission")?;
            node.set_text_content(None);
            append(
                node,
                &crate::ui::cloud::cloud_text(
                    &self.document,
                    label,
                    Medium,
                    1.0,
                    if disabled { 0x8e7c4d } else { 0xd9ba70 },
                    false,
                )?,
            )?;
        }
        self.message.set_text_content(Some(if self.account.is_none() {
            "Sign in to submit."
        } else if !self.customized {
            "Save a layout in Game Settings first."
        } else {
            "Each submission creates a new share code."
        }));
        Ok(())
    }

    fn paint_receipt(&mut self) -> Result<(), String> {
        self.receipt.set_text_content(None);
        let Some((action, layout)) = &self.shared else {
            return Ok(());
        };
        text(
            &self.document,
            &self.receipt,
            "span",
            "",
            if matches!(action, LayoutAction::Load) { "LAYOUT LOADED" } else { "LAYOUT PUBLISHED" },
            Medium,
            1.0,
            0xbfffbf,
            false,
        )?;
        text(
            &self.document,
            &self.receipt,
            "output",
            "",
            &layout.code,
            Menu,
            1.0,
            0xd9ba70,
            false,
        )?;
        text(
            &self.document,
            &self.receipt,
            "small",
            "",
            &format!("BY {}", layout.author.username),
            Medium,
            1.0,
            0xd9ba70,
            true,
        )?;
        let copy = widgets::button(
            &self.document,
            "COPY CODE",
            "cloud-copy-layout",
            "",
            None,
            44.0,
            0.55,
            false,
        )?;
        let code = layout.code.clone();
        let generation = self.copy_generation;
        let results = self.copy_result.clone();
        let copied = Closure::<dyn FnMut(Event)>::new(move |_| {
            // Clipboard invocation stays in the trusted click, before awaiting
            // its result; Safari's activation cannot be recreated by the frame loop.
            let result = (|| {
                let navigator = web_sys::window().ok_or(())?.navigator();
                let clipboard =
                    js_sys::Reflect::get(&navigator, &"clipboard".into()).map_err(|_| ())?;
                if clipboard.is_null() || clipboard.is_undefined() {
                    return Err(());
                }
                let method: js_sys::Function =
                    js_sys::Reflect::get(&clipboard, &"writeText".into())
                        .map_err(|_| ())?
                        .dyn_into()
                        .map_err(|_| ())?;
                method
                    .call1(&clipboard, &code.clone().into())
                    .map_err(|_| ())?
                    .dyn_into::<js_sys::Promise>()
                    .map_err(|_| ())
            })();
            match result {
                Ok(promise) => {
                    let results = results.clone();
                    spawn_local(async move {
                        let result = JsFuture::from(promise)
                            .await
                            .map(|_| ())
                            .map_err(|_| "The code could not be copied.".to_owned());
                        *results.borrow_mut() = Some((generation, result));
                    });
                }
                Err(()) => {
                    *results.borrow_mut() =
                        Some((generation, Err("The code could not be copied.".to_owned())))
                }
            }
        });
        copy.add_event_listener_with_callback("click", copied.as_ref().unchecked_ref())
            .map_err(|_| "shared code trusted copy listener")?;
        append(&self.receipt, &copy)?;
        self.copy_listener = Some(copied);
        self.receipt.set_hidden(false);
        Ok(())
    }
}
