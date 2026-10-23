//! GPU `NativeSaveTransferSettings`: native file selection and ZIP download stay
//! browser capabilities; inspection and durable replacement stay with the host.
use super::{SettingsView, widgets};
use crate::gpu_ui::UiFrame;
use crate::menu::HostedMenuAction;
use egui::{Rect, Ui, Vec2};
use solomon_assets::native_save::{
    GameSaveImportPreview, GameSaveImportSource, PortableExportResult,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Event, File, HtmlAnchorElement, HtmlInputElement};
#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestKind {
    Inspect,
    Export,
    Replace,
}
pub(super) struct SaveTransferView {
    document: Document,
    input: HtmlInputElement,
    can_export: bool,
    request: Option<(u64, RequestKind)>,
    pending: Option<GameSaveImportPreview>,
    export_pending: Option<PortableExportResult>,
    sequence: Rc<Cell<u64>>,
    files: Rc<RefCell<Vec<Vec<File>>>>,
    actions: Vec<HostedMenuAction>,
    error: Option<String>,
    status: Option<String>,
    change: Closure<dyn FnMut(Event)>,
}
impl SettingsView {
    pub fn set_save_transfer_available(&mut self, can_export: bool) -> Result<(), String> {
        self.save_transfer_available = true;
        self.save_can_export = can_export;
        if let Some(v) = &mut self.save_transfer {
            v.can_export = can_export;
        }
        Ok(())
    }
    pub fn take_save_transfer_actions(&mut self) -> Vec<HostedMenuAction> {
        self.save_transfer.as_mut().map_or_else(Vec::new, |v| std::mem::take(&mut v.actions))
    }
    pub(super) fn update_save_transfer(&mut self) -> Result<(), String> {
        if let Some(v) = &mut self.save_transfer {
            v.update()?;
        }
        Ok(())
    }
    pub fn complete_save_inspection(
        &mut self,
        id: u64,
        result: Result<GameSaveImportPreview, String>,
    ) -> Result<bool, String> {
        let Some(v) = &mut self.save_transfer else { return Ok(false) };
        if !v.complete(id, RequestKind::Inspect) {
            return Ok(false);
        }
        v.input.set_value("");
        match result {
            Ok(preview) => v.pending = Some(preview),
            Err(error) => v.error = Some(error),
        }
        Ok(true)
    }
    pub fn complete_save_export(
        &mut self,
        id: u64,
        result: Result<PortableExportResult, String>,
    ) -> Result<bool, String> {
        let Some(v) = &mut self.save_transfer else { return Ok(false) };
        if !v.complete(id, RequestKind::Export) {
            return Ok(false);
        }
        match result {
            Ok(exported) if !exported.warnings.is_empty() => v.export_pending = Some(exported),
            Ok(exported) => v.download(exported)?,
            Err(error) => v.error = Some(error),
        }
        Ok(true)
    }
    pub fn complete_save_replacement(
        &mut self,
        id: u64,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        let Some(v) = &mut self.save_transfer else { return Ok(false) };
        if !v.complete(id, RequestKind::Replace) {
            return Ok(false);
        }
        match result {
            Ok(()) => {
                v.pending = None;
                v.status = Some("Save imported. Close Settings and choose Last Game.".to_owned());
            }
            Err(error) => v.error = Some(error),
        }
        Ok(true)
    }
}
impl SaveTransferView {
    pub fn new(
        document: &Document,
        can_export: bool,
        sequence: Rc<Cell<u64>>,
    ) -> Result<Self, String> {
        let input: HtmlInputElement = document
            .create_element("input")
            .map_err(|_| "save file picker")?
            .dyn_into()
            .map_err(|_| "save file picker type")?;
        input.set_type("file");
        input.set_accept(".zip,.json,.cfg,.sav");
        input.set_multiple(true);
        let files = Rc::new(RefCell::new(Vec::new()));
        let selected = files.clone();
        let file_input = input.clone();
        let change = Closure::<dyn FnMut(Event)>::new(move |_| {
            let Some(files) = file_input.files() else { return };
            if files.length() > 0 {
                selected
                    .borrow_mut()
                    .push((0..files.length()).filter_map(|i| files.item(i)).collect());
            }
        });
        input
            .add_event_listener_with_callback("change", change.as_ref().unchecked_ref())
            .map_err(|_| "save file selection listener")?;
        Ok(Self {
            document: document.clone(),
            input,
            can_export,
            request: None,
            pending: None,
            export_pending: None,
            sequence,
            files,
            actions: Vec::new(),
            error: None,
            status: None,
            change,
        })
    }
    pub fn activate(&mut self, action: &str) -> Result<bool, String> {
        if self.request.is_some() {
            return Ok(action.starts_with("save-"));
        }
        match action {
            "save-import" => self.input.click(),
            "save-export" if self.can_export => {
                self.status = None;
                let request_id = self.begin(RequestKind::Export);
                self.actions.push(HostedMenuAction::ExportSave { request_id });
            }
            "save-replace" => {
                if let Some(preview) = &self.pending {
                    let document = preview.document.clone();
                    let request_id = self.begin(RequestKind::Replace);
                    self.actions
                        .push(HostedMenuAction::ReplaceImportedSave { request_id, document });
                }
            }
            "save-cancel" => self.pending = None,
            "save-export-confirm" => {
                if let Some(exported) = self.export_pending.take() {
                    self.download(exported)?;
                }
            }
            "save-export-cancel" => self.export_pending = None,
            _ => return Ok(false),
        }
        Ok(true)
    }
    fn update(&mut self) -> Result<(), String> {
        let files = std::mem::take(&mut *self.files.borrow_mut());
        for files in files {
            if self.request.is_some() {
                continue;
            }
            self.pending = None;
            self.status = None;
            let request_id = self.begin(RequestKind::Inspect);
            self.actions.push(HostedMenuAction::InspectSaveFiles { request_id, files });
        }
        Ok(())
    }
    fn begin(&mut self, kind: RequestKind) -> u64 {
        let id = self.sequence.get() + 1;
        self.sequence.set(id);
        self.request = Some((id, kind));
        self.error = None;
        id
    }
    fn complete(&mut self, id: u64, kind: RequestKind) -> bool {
        if self.request != Some((id, kind)) {
            return false;
        }
        self.request = None;
        true
    }
    pub fn paint(
        &self,
        settings: &SettingsView,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        width: f32,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        super::flow(
            ui,
            "Browser exports retain your inventory, equipment, and saved run. Import the ZIP to restore them. Stock-only saves import progression into the Hub.",
            width,
            13.0,
            widgets::color(0xc8b988),
        );
        let busy = self.request.is_some();
        super::group(frame, ui, "IMPORT SAVE", width);
        settings.action_row(
            frame,
            ui,
            "CHOOSE SAVE FILES",
            "save-import",
            width,
            !busy,
            actions,
        )?;
        super::group(frame, ui, "EXPORT SAVE", width);
        settings.action_row(
            frame,
            ui,
            "DOWNLOAD SAVE ARCHIVE",
            "save-export",
            width,
            !busy && self.can_export,
            actions,
        )?;
        if !self.can_export {
            super::flow(
                ui,
                "Create or import a wizard before exporting.",
                width,
                13.0,
                widgets::color(0xc8b988),
            );
        }
        if let Some(preview) = &self.pending {
            let top = ui.cursor().min;
            let mut summary = vec![preview.display_name.clone()];
            super::group(frame, ui, &preview.display_name, width);
            for text in [
                format!(
                    "Level {} · {} / {} · {} gold",
                    preview.level,
                    preview.element.as_str(),
                    preview.discipline.as_str(),
                    preview.gold
                ),
                format!(
                    "{} learned rows · {} Hagatha perks",
                    preview.learned_rows, preview.hagatha_perks
                ),
                match preview.source {
                    GameSaveImportSource::Browser => {
                        "Browser save: inventory, equipment, and the saved run will be restored."
                            .to_owned()
                    }
                    GameSaveImportSource::Stock => {
                        "Stock progression only: inventory is not imported; play starts in the Hub."
                            .to_owned()
                    }
                },
            ] {
                super::flow(ui, &text, width, 13.0, widgets::color(0xc8b988));
                summary.push(text);
            }
            for warning in &preview.warnings {
                super::flow(ui, warning, width, 13.0, widgets::ERROR);
                summary.push(warning.clone());
            }
            let (_, r) = ui.allocate_space(Vec2::new(width, 46.0));
            let half = (width - 10.0) / 2.0;
            for (label, id, x) in [
                ("REPLACE BROWSER SLOT", "save-replace", r.left()),
                ("CANCEL", "save-cancel", r.left() + half + 10.0),
            ] {
                if widgets::flat_button(
                    frame,
                    ui,
                    id,
                    label,
                    Rect::from_min_size(egui::pos2(x, r.top()), Vec2::new(half, 40.0)),
                    !busy,
                )
                .clicked()
                    && !busy
                {
                    actions.push(id.to_owned());
                }
            }
            frame.record_rect(
                "save-preview",
                &preview.display_name,
                "region",
                Rect::from_min_max(top, egui::pos2(top.x + width, ui.cursor().min.y)),
                false,
            );
            frame.record_value("save-preview", &summary.join("\n"), None);
        }
        if let Some(exported) = &self.export_pending {
            let top = ui.cursor().min;
            super::group(frame, ui, "EXPORT LIMITATIONS", width);
            super::flow(
                ui,
                "These limitations apply when loading in stock Solomon Dark. The browser save retains your inventory and run.",
                width,
                13.0,
                widgets::GOLD,
            );
            for warning in &exported.warnings {
                super::flow(ui, warning, width, 13.0, widgets::ERROR);
            }
            for (label, id) in
                [("EXPORT ANYWAY", "save-export-confirm"), ("CANCEL EXPORT", "save-export-cancel")]
            {
                settings.action_row(frame, ui, label, id, width, true, actions)?;
            }
            frame.record_rect(
                "save-export-limitations",
                "EXPORT LIMITATIONS",
                "region",
                Rect::from_min_max(top, egui::pos2(top.x + width, ui.cursor().min.y)),
                false,
            );
            frame.record_value("save-export-limitations", &exported.warnings.join("\n"), None);
        }
        if let Some(error) = &self.error {
            let rect = super::flow(ui, error, width, 13.0, widgets::ERROR);
            frame.record_rect("save-error", error, "alert", rect, false);
            frame.record_value("save-error", error, None);
            frame.announce(error);
        }
        if let Some(status) = &self.status {
            let rect = super::flow(ui, status, width, 13.0, widgets::color(0xb9d6ab));
            frame.record_rect("save-status", status, "status", rect, false);
            frame.record_value("save-status", status, None);
            frame.announce(status);
        }
        Ok(())
    }
    fn download(&mut self, exported: PortableExportResult) -> Result<(), String> {
        let options = web_sys::BlobPropertyBag::new();
        options.set_type("application/zip");
        let parts = js_sys::Array::new();
        parts.push(&js_sys::Uint8Array::from(exported.archive.as_slice()));
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
            .map_err(|_| "The stock save could not be exported.")?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)
            .map_err(|_| "The stock save download could not be opened.")?;
        let anchor: HtmlAnchorElement = self
            .document
            .create_element("a")
            .map_err(|_| "save download")?
            .dyn_into()
            .map_err(|_| "save download anchor")?;
        anchor.set_href(&url);
        anchor.set_download(&format!("solomon-dark-save-{}.zip", js_sys::Date::now().floor()));
        anchor.click();
        web_sys::Url::revoke_object_url(&url)
            .map_err(|_| "The stock save download could not be released.")?;
        self.status =
            Some("Save archive downloaded, including inventory and your saved run.".to_owned());
        Ok(())
    }
}
impl Drop for SaveTransferView {
    fn drop(&mut self) {
        if let Err(error) = self
            .input
            .remove_event_listener_with_callback("change", self.change.as_ref().unchecked_ref())
        {
            web_sys::console::error_1(&error);
        }
    }
}
