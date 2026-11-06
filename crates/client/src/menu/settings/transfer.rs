//! GPU `NativeSaveTransferSettings`: platform file selection/export at the edge;
//! inspection and durable replacement stay with the host.
use super::{SettingsView, widgets};
use crate::gpu_ui::UiFrame;
use crate::menu::HostedMenuAction;
use crate::platform::{
    Surface,
    files::{FilePicker, save_file},
};
use egui::{Rect, Ui, Vec2};
use solomon_assets::native_save::{
    GameSaveImportPreview, GameSaveImportSource, PortableExportResult,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestKind {
    Inspect,
    Export,
    Replace,
}
pub(super) struct SaveTransferView {
    surface: Surface,
    picker: FilePicker,
    saving: bool,
    downloaded: Rc<RefCell<Option<Result<bool, String>>>>,
    can_export: bool,
    request: Option<(u64, RequestKind)>,
    pending: Option<GameSaveImportPreview>,
    export_pending: Option<PortableExportResult>,
    sequence: Rc<Cell<u64>>,
    actions: Vec<HostedMenuAction>,
    error: Option<String>,
    status: Option<String>,
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
        surface: &Surface,
        can_export: bool,
        sequence: Rc<Cell<u64>>,
    ) -> Result<Self, String> {
        let picker = FilePicker::new(surface, &["zip", "json", "cfg", "sav"], true)?;
        Ok(Self {
            surface: surface.clone(),
            picker,
            saving: false,
            downloaded: Rc::new(RefCell::new(None)),
            can_export,
            request: None,
            pending: None,
            export_pending: None,
            sequence,
            actions: Vec::new(),
            error: None,
            status: None,
        })
    }
    pub fn activate(&mut self, action: &str) -> Result<bool, String> {
        if self.request.is_some() || self.saving {
            return Ok(action.starts_with("save-"));
        }
        match action {
            "save-import" => self.picker.choose()?,
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
        if let Some(result) = self.downloaded.borrow_mut().take() {
            self.saving = false;
            match result {
                Ok(true) => {
                    self.status = Some(
                        "Save archive exported, including inventory and your saved run.".into(),
                    )
                }
                Ok(false) => {}
                Err(error) => self.error = Some(error),
            }
        }
        let files = self.picker.take();
        for files in files {
            if self.request.is_some() || self.saving {
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
            "Save archives retain your inventory, equipment, and saved run. Import the ZIP to restore them. Stock-only saves import progression into the Hub.",
            width,
            13.0,
            widgets::color(0xc8b988),
        );
        let busy = self.request.is_some() || self.saving;
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
                    GameSaveImportSource::Game => {
                        "Inventory, equipment, and the saved run will be restored.".to_owned()
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
                ("REPLACE SAVED GAME", "save-replace", r.left()),
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
                "These limitations apply when loading in stock Solomon Dark. The full save retains your inventory and run.",
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
            frame.announce("save-error", error);
        }
        if let Some(status) = &self.status {
            let rect = super::flow(ui, status, width, 13.0, widgets::color(0xb9d6ab));
            frame.record_rect("save-status", status, "status", rect, false);
            frame.record_value("save-status", status, None);
            frame.announce("save-status", status);
        }
        Ok(())
    }
    fn download(&mut self, exported: PortableExportResult) -> Result<(), String> {
        self.saving = true;
        self.status = None;
        let surface = self.surface.clone();
        let downloaded = self.downloaded.clone();
        crate::platform::spawn_local(async move {
            let name = format!("solomon-dark-save-{}.zip", crate::platform::epoch_ms().floor());
            *downloaded.borrow_mut() =
                Some(save_file(&surface, &name, exported.archive, "application/zip").await);
        });
        Ok(())
    }
}
