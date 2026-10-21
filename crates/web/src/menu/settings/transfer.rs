//! Pinned `NativeSaveTransferSettings.tsx`. The view owns file selection, preview
//! and download gestures; conversion and durable replacement stay with the host.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use solomon_assets::native_save::{
    GameSaveImportPreview, GameSaveImportSource, PortableExportResult,
};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Element, Event, File, HtmlAnchorElement, HtmlElement, HtmlInputElement};

use crate::menu::HostedMenuAction;
use crate::ui::{append, element};

use super::{SettingsContext, SettingsView};

enum TransferEvent {
    Inspect(Vec<File>),
    Export,
    Replace,
    Cancel,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestKind {
    Inspect,
    Export,
    Replace,
}

pub(super) struct SaveTransferView {
    root: HtmlElement,
    document: Document,
    input: HtmlInputElement,
    choose: HtmlElement,
    export: HtmlElement,
    note: HtmlElement,
    preview: HtmlElement,
    error: HtmlElement,
    status: HtmlElement,
    can_export: bool,
    request: Option<(u64, RequestKind)>,
    pending: Option<GameSaveImportPreview>,
    sequence: Rc<Cell<u64>>,
    events: Rc<RefCell<Vec<TransferEvent>>>,
    actions: Vec<HostedMenuAction>,
    click: Closure<dyn Fn(Event)>,
    change: Closure<dyn FnMut(Event)>,
}

impl SettingsView {
    pub fn set_save_transfer_available(&mut self, can_export: bool) -> Result<(), String> {
        self.save_transfer_available = true;
        self.save_can_export = can_export;
        if let Some(view) = &mut self.save_transfer {
            view.can_export = can_export;
            view.paint_availability()?;
        }
        Ok(())
    }

    pub fn take_save_transfer_actions(&mut self) -> Vec<HostedMenuAction> {
        self.save_transfer.as_mut().map_or_else(Vec::new, |view| std::mem::take(&mut view.actions))
    }

    pub(super) fn build_save_transfer(&mut self) -> Result<(), String> {
        self.content
            .class_list()
            .add_1("native-save-transfer-settings")
            .map_err(|_| "save transfer content")?;
        append_copy(
            &self.document,
            &self.content,
            "p",
            "native-save-transfer-intro",
            "Browser exports retain your inventory, equipment, and saved run. Import the ZIP to restore them. Stock-only saves import progression into the Hub.",
        )?;
        let input: HtmlInputElement = element(&self.document, "input", "native-ui-sr-only")?
            .dyn_into()
            .map_err(|_| "save transfer input")?;
        input.set_type("file");
        input.set_accept(".zip,.json,.cfg,.sav");
        input.set_multiple(true);
        append(&self.content, &input)?;
        let group = self.group("IMPORT SAVE")?;
        self.action(&group, "CHOOSE SAVE FILES", "save-import")?;
        let choose = transfer_button(&group, "save-import", "choose")?;
        choose
            .set_attribute("data-game-default-focus", "true")
            .map_err(|_| "save transfer focus")?;
        let group = self.group("EXPORT SAVE")?;
        self.action(&group, "DOWNLOAD SAVE ARCHIVE", "save-export")?;
        let export = transfer_button(&group, "save-export", "export")?;
        let note = append_copy(
            &self.document,
            &group,
            "p",
            "native-save-transfer-note",
            "Create or import a wizard before exporting.",
        )?;
        let preview = element(&self.document, "section", "native-save-transfer-preview")?;
        preview
            .set_attribute("aria-label", "Save import preview")
            .map_err(|_| "save preview label")?;
        append(&self.content, &preview)?;
        let error = element(&self.document, "p", "native-save-transfer-error")?;
        error.set_attribute("role", "alert").map_err(|_| "save error role")?;
        append(&self.content, &error)?;
        let status = element(&self.document, "p", "native-save-transfer-status")?;
        status.set_attribute("role", "status").map_err(|_| "save status role")?;
        append(&self.content, &status)?;
        let events = Rc::new(RefCell::new(Vec::new()));
        let click_events = events.clone();
        let file_input = input.clone();
        // Opening the input synchronously bubbles a second click through this root.
        let click = Closure::<dyn Fn(Event)>::new(move |event: Event| {
            let result = (|| -> Result<(), String> {
                let Some(target) =
                    event.target().and_then(|target| target.dyn_into::<Element>().ok())
                else {
                    return Ok(());
                };
                let Some(button) = target
                    .closest("[data-save-transfer-action]")
                    .map_err(|_| "save action target")?
                else {
                    return Ok(());
                };
                if button.has_attribute("disabled") {
                    return Ok(());
                }
                event.stop_propagation();
                match button.get_attribute("data-save-transfer-action").as_deref() {
                    // Keep the file chooser inside the actual trusted click/tap.
                    Some("choose") => file_input.click(),
                    Some("export") => click_events.borrow_mut().push(TransferEvent::Export),
                    Some("replace") => click_events.borrow_mut().push(TransferEvent::Replace),
                    Some("cancel") => click_events.borrow_mut().push(TransferEvent::Cancel),
                    _ => {}
                }
                Ok(())
            })();
            report(result);
        });
        self.content
            .add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
            .map_err(|_| "save transfer click listener")?;
        let change_events = events.clone();
        let file_input = input.clone();
        let change = Closure::<dyn FnMut(Event)>::new(move |_| {
            let Some(files) = file_input.files() else { return };
            if files.length() == 0 {
                return;
            }
            let selected = (0..files.length()).filter_map(|index| files.item(index)).collect();
            change_events.borrow_mut().push(TransferEvent::Inspect(selected));
        });
        input
            .add_event_listener_with_callback("change", change.as_ref().unchecked_ref())
            .map_err(|_| "save transfer file listener")?;
        let view = SaveTransferView {
            root: self.content.clone(),
            document: self.document.clone(),
            input,
            choose,
            export,
            note,
            preview,
            error,
            status,
            can_export: self.save_can_export,
            request: None,
            pending: None,
            sequence: self.save_request_sequence.clone(),
            events,
            actions: Vec::new(),
            click,
            change,
        };
        view.paint_preview()?;
        view.paint_availability()?;
        self.save_transfer = Some(view);
        Ok(())
    }

    pub(super) fn update_save_transfer(&mut self) -> Result<(), String> {
        if let Some(view) = &mut self.save_transfer {
            view.update()?;
        }
        Ok(())
    }

    pub fn complete_save_inspection(
        &mut self,
        id: u64,
        result: Result<GameSaveImportPreview, String>,
    ) -> Result<bool, String> {
        let Some(view) = &mut self.save_transfer else { return Ok(false) };
        if !view.complete(id, RequestKind::Inspect) {
            return Ok(false);
        }
        view.input.set_value("");
        match result {
            Ok(preview) => view.pending = Some(preview),
            Err(error) => view.error.set_text_content(Some(&error)),
        }
        view.paint_preview()?;
        view.paint_availability()?;
        Ok(true)
    }

    pub fn complete_save_export(
        &mut self,
        id: u64,
        result: Result<PortableExportResult, String>,
    ) -> Result<bool, String> {
        let Some(view) = &mut self.save_transfer else { return Ok(false) };
        if !view.complete(id, RequestKind::Export) {
            return Ok(false);
        }
        match result.and_then(|exported| download(&view.document, exported)) {
            Ok(true) => view.status.set_text_content(Some(
                "Save archive downloaded, including inventory and your saved run.",
            )),
            Ok(false) => {}
            Err(error) => view.error.set_text_content(Some(&error)),
        }
        view.paint_availability()?;
        Ok(true)
    }

    pub fn complete_save_replacement(
        &mut self,
        id: u64,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        let Some(view) = &mut self.save_transfer else { return Ok(false) };
        if !view.complete(id, RequestKind::Replace) {
            return Ok(false);
        }
        match result {
            Ok(()) => {
                view.pending = None;
                view.status
                    .set_text_content(Some("Save imported. Close Settings and choose Last Game."));
            }
            Err(error) => view.error.set_text_content(Some(&error)),
        }
        view.paint_preview()?;
        view.paint_availability()?;
        Ok(true)
    }

    pub(super) fn add_save_transfer_route(&mut self) -> Result<(), String> {
        if self.context == SettingsContext::Title && self.save_transfer_available {
            let group = self.group("SAVE TRANSFER")?;
            self.action(&group, "STOCK / BROWSER SAVE", "settings-page:save-transfer")?;
        }
        Ok(())
    }
}

impl SaveTransferView {
    fn update(&mut self) -> Result<(), String> {
        let events = std::mem::take(&mut *self.events.borrow_mut());
        for event in events {
            if self.request.is_some() {
                continue;
            }
            match event {
                TransferEvent::Inspect(files) => {
                    self.pending = None;
                    self.status.set_text_content(None);
                    let request_id = self.begin(RequestKind::Inspect);
                    self.actions.push(HostedMenuAction::InspectSaveFiles { request_id, files });
                    self.paint_preview()?;
                }
                TransferEvent::Export if self.can_export => {
                    self.status.set_text_content(None);
                    let request_id = self.begin(RequestKind::Export);
                    self.actions.push(HostedMenuAction::ExportSave { request_id });
                }
                TransferEvent::Replace if self.pending.is_some() => {
                    let document =
                        self.pending.as_ref().expect("admitted preview").document.clone();
                    let request_id = self.begin(RequestKind::Replace);
                    self.actions
                        .push(HostedMenuAction::ReplaceImportedSave { request_id, document });
                }
                TransferEvent::Cancel => {
                    self.pending = None;
                    self.paint_preview()?;
                }
                _ => {}
            }
        }
        self.paint_availability()
    }

    fn begin(&mut self, kind: RequestKind) -> u64 {
        let id = self.sequence.get() + 1;
        self.sequence.set(id);
        self.request = Some((id, kind));
        self.error.set_text_content(None);
        id
    }

    fn complete(&mut self, id: u64, kind: RequestKind) -> bool {
        if self.request != Some((id, kind)) {
            return false;
        }
        self.request = None;
        true
    }

    fn paint_availability(&self) -> Result<(), String> {
        let busy = self.request.is_some();
        self.choose
            .toggle_attribute_with_force("disabled", busy)
            .map_err(|_| "save import availability")?;
        self.export
            .toggle_attribute_with_force("disabled", busy || !self.can_export)
            .map_err(|_| "save export availability")?;
        self.note.set_hidden(self.can_export);
        self.error.set_hidden(self.error.text_content().is_none_or(|text| text.is_empty()));
        self.status.set_hidden(self.status.text_content().is_none_or(|text| text.is_empty()));
        let buttons =
            self.preview.query_selector_all("button").map_err(|_| "save preview actions")?;
        for index in 0..buttons.length() {
            let button = buttons
                .item(index)
                .ok_or("save preview action disappeared")?
                .dyn_into::<Element>()
                .map_err(|_| "save preview action element")?;
            button
                .toggle_attribute_with_force("disabled", busy)
                .map_err(|_| "save replacement availability")?;
        }
        Ok(())
    }

    fn paint_preview(&self) -> Result<(), String> {
        self.preview.set_text_content(None);
        self.preview.set_hidden(self.pending.is_none());
        let Some(preview) = &self.pending else { return Ok(()) };
        append_copy(&self.document, &self.preview, "h3", "", &preview.display_name)?;
        append_copy(
            &self.document,
            &self.preview,
            "p",
            "",
            &format!(
                "Level {} · {} / {} · {} gold",
                preview.level,
                preview.element.as_str(),
                preview.discipline.as_str(),
                preview.gold
            ),
        )?;
        append_copy(
            &self.document,
            &self.preview,
            "p",
            "",
            &format!(
                "{} learned rows · {} Hagatha perks",
                preview.learned_rows, preview.hagatha_perks
            ),
        )?;
        append_copy(
            &self.document,
            &self.preview,
            "p",
            "",
            match preview.source {
                GameSaveImportSource::Browser => {
                    "Browser save: inventory, equipment, and the saved run will be restored."
                }
                GameSaveImportSource::Stock => {
                    "Stock progression only: inventory is not imported; play starts in the Hub."
                }
            },
        )?;
        for warning in &preview.warnings {
            append_copy(&self.document, &self.preview, "p", "", warning)?;
        }
        let actions = element(&self.document, "div", "")?;
        for (label, action) in [("REPLACE BROWSER SLOT", "replace"), ("CANCEL", "cancel")] {
            let button = append_copy(&self.document, &actions, "button", "", label)?;
            button.set_attribute("type", "button").map_err(|_| "save preview button")?;
            button
                .set_attribute("data-save-transfer-action", action)
                .map_err(|_| "save preview action")?;
        }
        append(&self.preview, &actions)
    }
}

impl Drop for SaveTransferView {
    fn drop(&mut self) {
        report(
            self.root
                .remove_event_listener_with_callback("click", self.click.as_ref().unchecked_ref())
                .map_err(|_| "save transfer click cleanup".to_owned()),
        );
        report(
            self.input
                .remove_event_listener_with_callback("change", self.change.as_ref().unchecked_ref())
                .map_err(|_| "save transfer input cleanup".to_owned()),
        );
    }
}

fn download(document: &Document, exported: PortableExportResult) -> Result<bool, String> {
    if !exported.warnings.is_empty() {
        let warning = format!(
            "These limitations apply when loading in stock Solomon Dark. The browser save retains your inventory and run.\n\n{}\n\nExport anyway?",
            exported.warnings.join("\n\n")
        );
        if !document
            .default_view()
            .ok_or("save export window")?
            .confirm_with_message(&warning)
            .map_err(|_| "The export confirmation could not be opened.")?
        {
            return Ok(false);
        }
    }
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/zip");
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(exported.archive.as_slice()));
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .map_err(|_| "The stock save could not be exported.")?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "The stock save download could not be opened.")?;
    let anchor: HtmlAnchorElement =
        element(document, "a", "")?.dyn_into().map_err(|_| "save download anchor")?;
    anchor.set_href(&url);
    anchor.set_download(&format!("solomon-dark-save-{}.zip", js_sys::Date::now().floor()));
    anchor.click();
    web_sys::Url::revoke_object_url(&url)
        .map_err(|_| "The stock save download could not be released.")?;
    Ok(true)
}

fn transfer_button(
    group: &HtmlElement,
    old_action: &str,
    action: &str,
) -> Result<HtmlElement, String> {
    let button: HtmlElement = group
        .query_selector(&format!("[data-menu-action='{old_action}']"))
        .map_err(|_| "save action selector")?
        .ok_or("save action missing")?
        .dyn_into()
        .map_err(|_| "save action button")?;
    button.remove_attribute("data-menu-action").map_err(|_| "save action ownership")?;
    button
        .set_attribute("data-save-transfer-action", action)
        .map_err(|_| "save action identity")?;
    Ok(button)
}

fn append_copy(
    document: &Document,
    parent: &HtmlElement,
    tag: &str,
    class: &str,
    text: &str,
) -> Result<HtmlElement, String> {
    let node = element(document, tag, class)?;
    node.set_text_content(Some(text));
    append(parent, &node)?;
    Ok(node)
}

fn report(result: Result<(), String>) {
    if let Err(error) = result {
        web_sys::console::error_1(&error.into());
    }
}
