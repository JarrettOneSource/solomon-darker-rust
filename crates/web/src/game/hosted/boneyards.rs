//! Rust GPU `DarkCloudBoneyards` / installed `BoneyardLibrary` authoring shelf.
//! Editor documents and cloud rows use their ordinary stores and one-use bridge;
//! the external Website editor retains all editing and disposable Test ownership.
mod store;

use crate::{
    gpu_ui::{UiFrame, paint_text_layout},
    hosted::{BoneyardDraft, BoneyardDraftSummary, WebsiteApi},
    menu::widgets,
};
use egui::{Align2, Color32, FontFamily, FontId, Id, Rect, Ui, Vec2};
use serde_json::{Value, json};
use solomon_sim::native_ui_text::{
    NativeUiFontName, NativeUiTextAlign, NativeUiTextPlacement, NativeUiTextSpec,
    layout_native_ui_text,
};
use std::{cell::RefCell, rc::Rc};
use store::DraftMeta;
use wasm_bindgen::{JsCast, closure::Closure};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Event, File, HtmlInputElement, Window};

#[derive(Clone, Debug, PartialEq, Eq)]
enum ShelfRow {
    Local(String),
    Cloud(u64),
}
impl ShelfRow {
    fn key(&self) -> String {
        match self {
            Self::Local(id) => format!("local:{id}"),
            Self::Cloud(id) => format!("cloud:{id}"),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum RowAction {
    Edit,
    Test,
    Delete,
}
enum Busy {
    Import,
    Row { row: ShelfRow, action: RowAction },
}
enum Completion {
    Catalog {
        request: u64,
        result: Result<Vec<BoneyardDraftSummary>, String>,
    },
    Open {
        request: u64,
        item: BoneyardDraftSummary,
        test: bool,
        result: Result<BoneyardDraft, String>,
    },
    Delete {
        request: u64,
        item: BoneyardDraftSummary,
        result: Result<(), String>,
    },
    Import {
        request: u64,
        name: String,
        result: Result<Value, String>,
    },
    Workshop {
        request: u64,
        result: Result<String, String>,
    },
}
pub(super) struct BoneyardLibrary {
    window: Window,
    api: WebsiteApi,
    account: Option<String>,
    date_format: js_sys::Intl::DateTimeFormat,
    local: Vec<DraftMeta>,
    cloud: Option<Vec<BoneyardDraftSummary>>,
    cloud_error: Option<String>,
    confirming: Option<ShelfRow>,
    busy: Option<Busy>,
    opening: bool,
    error: Option<String>,
    status: String,
    sequence: u64,
    catalog_request: Option<u64>,
    row_request: Option<u64>,
    workshop_request: Option<u64>,
    focus_new: bool,
    cloud_map_fallback: Option<Value>,
    completions: Rc<RefCell<Vec<Completion>>>,
    files: Rc<RefCell<Vec<File>>>,
    input: HtmlInputElement,
    change: Closure<dyn FnMut(Event)>,
}
impl BoneyardLibrary {
    pub(super) fn new(
        window: Window,
        api: WebsiteApi,
        account: Option<&str>,
    ) -> Result<Self, String> {
        let document = window.document().ok_or("Boneyard library document")?;
        let input: HtmlInputElement = document
            .create_element("input")
            .map_err(|_| "Boneyard file picker")?
            .dyn_into()
            .map_err(|_| "Boneyard file picker type")?;
        input.set_type("file");
        input.set_accept(".boneyard,.json,application/json");
        input.set_hidden(true);
        let files = Rc::new(RefCell::new(Vec::new()));
        let selected = files.clone();
        let picker = input.clone();
        let change = Closure::<dyn FnMut(Event)>::new(move |_| {
            let file = picker.files().and_then(|files| files.item(0));
            picker.set_value("");
            if let Some(file) = file {
                selected.borrow_mut().push(file);
            }
        });
        input
            .add_event_listener_with_callback("change", change.as_ref().unchecked_ref())
            .map_err(|_| "Boneyard file selection")?;
        let mut library = Self {
            local: store::list(&window),
            window,
            api,
            account: account.map(str::to_owned),
            date_format: date_format()?,
            cloud: None,
            cloud_error: None,
            confirming: None,
            busy: None,
            opening: false,
            error: None,
            status: String::new(),
            sequence: 0,
            catalog_request: None,
            row_request: None,
            workshop_request: None,
            focus_new: false,
            cloud_map_fallback: None,
            completions: Rc::new(RefCell::new(Vec::new())),
            files,
            input,
            change,
        };
        library.load_cloud();
        Ok(library)
    }
    pub(super) fn set_account(&mut self, account: Option<&str>) {
        if self.account.as_deref() == account {
            return;
        }
        self.account = account.map(str::to_owned);
        self.confirming = None;
        self.busy = None;
        self.row_request = None;
        self.load_cloud();
    }
    fn next(&mut self) -> u64 {
        self.sequence += 1;
        self.sequence
    }
    fn load_cloud(&mut self) {
        self.cloud = None;
        self.cloud_error = None;
        if self.account.is_none() {
            self.catalog_request = None;
            return;
        }
        let request = self.next();
        self.catalog_request = Some(request);
        let api = self.api.clone();
        let queue = self.completions.clone();
        spawn_local(async move {
            let result = api.boneyard_drafts().await.map_err(|e| e.message);
            queue.borrow_mut().push(Completion::Catalog { request, result });
        });
    }
    fn begin(&mut self, busy: Busy) -> u64 {
        self.busy = Some(busy);
        self.error = None;
        self.status.clear();
        let id = self.next();
        self.row_request = Some(id);
        id
    }
    fn workshop(&mut self, request: Value) {
        self.opening = true;
        self.error = None;
        let id = self.next();
        self.workshop_request = Some(id);
        let window = self.window.clone();
        let mount = self.api.mount().clone();
        let queue = self.completions.clone();
        spawn_local(async move {
            let result = store::workshop_url(&window, &mount, request).await;
            queue.borrow_mut().push(Completion::Workshop { request: id, result });
        });
    }
    fn open_local(&mut self, meta: &DraftMeta, test: bool) {
        match store::load(&self.window,&meta.id){Ok(document)=>self.workshop(json!({"kind":"open","document":document,"draftId":meta.id,"savedAt":meta.updated_at,"test":test})),Err(_)=>self.error=Some(format!("{} could not be opened from this device.",display_name(&meta.name)))}
    }
    fn open_cloud(&mut self, item: BoneyardDraftSummary, test: bool) {
        let request = self.begin(Busy::Row {
            row: ShelfRow::Cloud(item.id),
            action: if test { RowAction::Test } else { RowAction::Edit },
        });
        let api = self.api.clone();
        let queue = self.completions.clone();
        spawn_local(async move {
            let result = api.boneyard_draft(item.id).await.map_err(|e| e.message);
            queue.borrow_mut().push(Completion::Open { request, item, test, result });
        });
    }
    fn delete_cloud(&mut self, item: BoneyardDraftSummary) {
        let request =
            self.begin(Busy::Row { row: ShelfRow::Cloud(item.id), action: RowAction::Delete });
        let api = self.api.clone();
        let queue = self.completions.clone();
        spawn_local(async move {
            let result = api.remove_boneyard_draft(item.id).await.map_err(|e| e.message);
            queue.borrow_mut().push(Completion::Delete { request, item, result });
        });
    }
    fn import(&mut self, file: File) {
        let request = self.begin(Busy::Import);
        let queue = self.completions.clone();
        spawn_local(async move {
            let name = file.name();
            let result = import_file(&file).await;
            queue.borrow_mut().push(Completion::Import { request, name, result });
        });
    }
    fn deleted(&mut self, message: String) {
        self.confirming = None;
        self.status = message;
        self.focus_new = true;
    }
    pub(super) fn update(&mut self) -> Result<(), String> {
        let files = std::mem::take(&mut *self.files.borrow_mut());
        for file in files {
            if self.busy.is_none() && !self.opening {
                self.import(file);
            }
        }
        let completed = std::mem::take(&mut *self.completions.borrow_mut());
        for completion in completed {
            match completion {
                Completion::Catalog { request, result }
                    if self.catalog_request == Some(request) =>
                {
                    self.catalog_request = None;
                    match result {
                        Ok(rows) => {
                            self.cloud = Some(rows);
                            self.cloud_error = None;
                        }
                        Err(error) => self.cloud_error = Some(error),
                    }
                }
                Completion::Open { request, item, test, result }
                    if self.row_request == Some(request) =>
                {
                    self.row_request = None;
                    self.busy = None;
                    match result.and_then(|draft|{store::validate_file(&draft.document)?;let id=store::new_id()?;store::set_cloud_id(&self.window,&id,item.id,&mut self.cloud_map_fallback);Ok((id,draft.document))}){Ok((id,document))=>self.workshop(json!({"kind":"open","document":document,"draftId":id,"savedAt":js_sys::Date::now(),"test":test})),Err(error)=>self.error=Some(format!("{}: {error}",item.name))}
                }
                Completion::Delete { request, item, result }
                    if self.row_request == Some(request) =>
                {
                    self.row_request = None;
                    self.busy = None;
                    match result {
                        Ok(()) => {
                            if let Some(rows) = &mut self.cloud {
                                rows.retain(|r| r.id != item.id);
                            }
                            self.deleted(format!("Deleted {} from the cloud.", item.name));
                        }
                        Err(error) => self.error = Some(format!("{}: {error}", item.name)),
                    }
                }
                Completion::Import { request, name, result }
                    if self.row_request == Some(request) =>
                {
                    self.row_request = None;
                    self.busy = None;
                    match result {
                        Ok(doc) => {
                            let result =
                                store::new_id().and_then(|id| store::save(&self.window, &id, doc));
                            match result {
                                Ok(meta) => {
                                    self.local = store::list(&self.window);
                                    self.status = format!("Imported {}.", display_name(&meta.name));
                                }
                                Err(_) => {
                                    self.error = Some(format!(
                                        "{name} could not be saved on this device. Delete a boneyard here to make room, then try again."
                                    ))
                                }
                            }
                        }
                        Err(error) => self.error = Some(format!("{name}: {error}")),
                    }
                }
                Completion::Workshop { request, result }
                    if self.workshop_request == Some(request) =>
                {
                    self.workshop_request = None;
                    match result {
                        Ok(url) => {
                            if self.window.location().assign(&url).is_err() {
                                self.opening = false;
                                self.error=Some("The workshop could not be opened on this device. Check browser storage, then try again.".to_owned());
                            }
                        }
                        Err(_) => {
                            self.opening = false;
                            self.error=Some("The workshop could not be opened on this device. Check browser storage, then try again.".to_owned());
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn activate(&mut self, action: &str) -> Result<(), String> {
        if self.busy.is_some() || self.opening {
            return Ok(());
        }
        match action {
            "new" => {
                self.workshop(json!({"kind":"new"}));
                return Ok(());
            }
            "import" => {
                self.input.click();
                return Ok(());
            }
            "retry" => {
                self.load_cloud();
                return Ok(());
            }
            _ => {}
        }
        let Some((verb, key)) = action.split_once(':') else { return Ok(()) };
        let row = if let Some(id) = key.strip_prefix("local:") {
            ShelfRow::Local(id.to_owned())
        } else if let Some(id) = key.strip_prefix("cloud:") {
            ShelfRow::Cloud(id.parse().map_err(|_| "Boneyard row identity")?)
        } else {
            return Ok(());
        };
        if verb == "safe" {
            self.confirming = if self.confirming.as_ref() == Some(&row) { None } else { Some(row) };
            return Ok(());
        }
        match row {
            ShelfRow::Local(id) => {
                let Some(meta) = self.local.iter().find(|r| r.id == id).cloned() else {
                    return Ok(());
                };
                match verb {
                    "edit" | "test" => self.open_local(&meta, verb == "test"),
                    "confirm" if self.confirming.as_ref() == Some(&ShelfRow::Local(id.clone())) => {
                        match store::delete(&self.window, &id) {
                            Ok(()) => {
                                self.local = store::list(&self.window);
                                self.deleted(format!(
                                    "Deleted {} from this device.",
                                    display_name(&meta.name)
                                ));
                            }
                            Err(error) => {
                                self.error = Some(format!("{}: {error}", display_name(&meta.name)))
                            }
                        }
                    }
                    _ => {}
                }
            }
            ShelfRow::Cloud(id) => {
                let Some(item) =
                    self.cloud.as_ref().and_then(|rows| rows.iter().find(|r| r.id == id)).cloned()
                else {
                    return Ok(());
                };
                match verb {
                    "edit" | "test" => self.open_cloud(item, verb == "test"),
                    "confirm" if self.confirming.as_ref() == Some(&ShelfRow::Cloud(id)) => {
                        self.delete_cloud(item)
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
    pub(super) fn paint(
        &mut self,
        frame: &mut UiFrame<'_>,
        bounds: Rect,
        interactive: bool,
    ) -> Result<(), String> {
        let ctx = frame.ctx.clone();
        let mut actions = Vec::new();
        let layer = crate::menu::library_layer();
        ctx.set_sublayer(crate::menu::scene_layer(), layer);
        egui::Area::new(layer.id)
            .fade_in(false)
            .fixed_pos(bounds.min)
            .order(layer.order)
            .show(&ctx, |ui| {
                ui.set_min_size(bounds.size());
                ui.set_max_size(bounds.size());
                ui.set_clip_rect(bounds);
                widgets::scrollbar_style(ui, 0x876d37, 0x080808, true);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                if !interactive || self.opening {
                    ui.disable();
                    ui.set_opacity(1.0);
                }
                egui::ScrollArea::vertical()
                    .id_salt("boneyard-shelf-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let width = ui.available_width();
                        self.paint_contents(frame, ui, width, &mut actions)
                    })
                    .inner
            })
            .inner?;
        if self.focus_new && interactive {
            ctx.memory_mut(|memory| memory.request_focus(Id::unique("boneyards/new")));
            self.focus_new = false;
        }
        for action in actions {
            self.activate(&action)?;
        }
        Ok(())
    }
    fn paint_contents(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        physical_width: f32,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let phone = frame.ctx.input(|input| input.content_rect().width() <= 700.0);
        let padding_x = if phone { 4.0 } else { 10.0 };
        let padding_y = if phone { 8.0 } else { 10.0 };
        let width = (physical_width - padding_x * 2.0).min(980.0);
        let min = ui.cursor().min + Vec2::new((physical_width - width) / 2.0, padding_y);
        let mut content =
            ui.new_child(egui::UiBuilder::new().id_salt("boneyard-shelf-content").max_rect(
                Rect::from_min_max(min, egui::pos2(min.x + width, ui.max_rect().bottom())),
            ));
        self.paint_shelf(frame, &mut content, width, actions)?;
        let height = content.min_rect().bottom() - min.y;
        ui.allocate_space(Vec2::new(physical_width, height + padding_y * 2.0));
        Ok(())
    }
    fn paint_shelf(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        width: f32,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let origin = ui.cursor().min;
        let narrow = frame.ctx.input(|input| input.content_rect().width() <= 700.0);
        // Source title is 252px; its fixed tools are 390px with a 24px header gap.
        let wrapped = narrow || width < 666.0;
        let columns = if narrow && width < 310.0 { 1 } else { 2 };
        let header_height = if wrapped { if columns == 1 { 134.0 } else { 80.0 } } else { 44.0 };
        ui.allocate_space(Vec2::new(ui.max_rect().width(), header_height + 14.0));
        stock_text(
            frame,
            ui,
            "YOUR BONEYARDS",
            NativeUiFontName::Menu,
            1.0,
            0xd9ba70,
            Rect::from_min_size(
                origin + Vec2::new(0.0, if wrapped { 0.0 } else { 10.0 }),
                Vec2::new(if wrapped { width } else { 252.0 }, 24.0),
            ),
            narrow,
        );
        let tool_top = origin.y + if wrapped { 36.0 } else { 0.0 };
        let tool_left = if wrapped { origin.x } else { origin.x + width - 390.0 };
        let tool_width =
            if narrow { if columns == 1 { width } else { (width - 10.0) / 2.0 } } else { 190.0 };
        for (index, (label, action)) in [
            ("NEW BONEYARD", "new"),
            (
                if matches!(self.busy, Some(Busy::Import)) {
                    "IMPORTING..."
                } else {
                    "IMPORT FILE"
                },
                "import",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let r = Rect::from_min_size(
                egui::pos2(
                    tool_left + if columns == 1 { 0.0 } else { index as f32 * (tool_width + 10.0) },
                    tool_top + if columns == 1 { index as f32 * 54.0 } else { 0.0 },
                ),
                Vec2::new(tool_width, 44.0),
            );
            if widgets::cloud_button(
                frame,
                ui,
                &format!("boneyards/{action}"),
                label,
                r,
                0.55,
                self.busy.is_none(),
            )
            .clicked()
            {
                actions.push(action.to_owned());
            }
        }
        if !self.status.is_empty() {
            message(frame, ui, "boneyards/status", &self.status, width, 0xbfffbf);
        }
        if let Some(error) = &self.error {
            message(frame, ui, "boneyards/error", error, width, 0xe4a39a);
        }
        shelf_heading(frame, ui, "ON THIS DEVICE", width);
        if self.local.is_empty() {
            note(ui, "Nothing saved here yet. Start a new boneyard or import a file.", width);
        } else {
            for meta in &self.local {
                let row = ShelfRow::Local(meta.id.clone());
                let detail = format!(
                    "{} placed · {}",
                    meta.residents,
                    updated(&self.date_format, meta.updated_at)?
                );
                self.paint_row(
                    frame,
                    ui,
                    &row,
                    display_name(&meta.name),
                    &detail,
                    "Delete from this device? This can't be undone.",
                    width,
                    actions,
                );
            }
        }
        shelf_heading(frame, ui, "IN THE CLOUD", width);
        if self.account.is_none() {
            note(ui, "Sign in to keep boneyards in the cloud.", width);
        } else if let Some(error) = &self.cloud_error {
            note(ui, error, width);
            let (_, r) = ui.allocate_space(Vec2::new(width, 50.0));
            let r = Rect::from_min_size(r.min, Vec2::new(120.0, 44.0));
            if widgets::cloud_button(
                frame,
                ui,
                "boneyards/retry",
                "RETRY",
                r,
                0.55,
                self.busy.is_none(),
            )
            .clicked()
            {
                actions.push("retry".to_owned());
            }
        } else if let Some(rows) = &self.cloud {
            if rows.is_empty() {
                note(ui, "Nothing in the cloud yet. Use Save to cloud in the workshop.", width);
            } else {
                for item in rows {
                    let row = ShelfRow::Cloud(item.id);
                    let detail = format!(
                        "Updated {}",
                        updated(&self.date_format, js_sys::Date::parse(&item.updated_at))?
                    );
                    self.paint_row(
                        frame,
                        ui,
                        &row,
                        &item.name,
                        &detail,
                        "Delete from the cloud? Copies on this device stay.",
                        width,
                        actions,
                    );
                }
            }
        } else {
            note(ui, "Loading your cloud boneyards…", width);
        }
        Ok(())
    }
    // Shelf text, source geometry and emitted actions stay explicit at this painter.
    #[allow(clippy::too_many_arguments)]
    fn paint_row(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        row: &ShelfRow,
        name: &str,
        detail: &str,
        prompt: &str,
        width: f32,
        actions: &mut Vec<String>,
    ) {
        let confirming = self.confirming.as_ref() == Some(row);
        let viewport = frame.ctx.input(|input| input.content_rect());
        let phone = viewport.width() <= 700.0;
        let count = if confirming { 2 } else { 3 };
        let horizontal_pad = if phone { 4.0 } else { 10.0 };
        let right_pad = if phone { 4.0 } else { 8.0 };
        let vertical_pad = if phone { 10.0 } else { 6.0 };
        let natural_tools = count as f32 * 130.0 + (count - 1) as f32 * 10.0;
        let wrapped = phone || width < 220.0 + 16.0 + natural_tools + 18.0;
        let button_width = if phone {
            (width - horizontal_pad - right_pad - (count - 1) as f32 * 10.0) / count as f32
        } else {
            130.0
        };
        let tool_width = count as f32 * button_width + (count - 1) as f32 * 10.0;
        let copy_width =
            width - horizontal_pad - right_pad - if wrapped { 0.0 } else { tool_width + 16.0 };
        let detail_color = if confirming {
            Color32::from_rgb(228, 163, 154)
        } else {
            Color32::from_rgb(178, 167, 141)
        };
        let font = FontId::new(14.0, FontFamily::Name("IM FELL English".into()));
        let text = if confirming { prompt } else { detail };
        let galley = if confirming {
            ui.painter().layout(text.to_owned(), font, detail_color, copy_width)
        } else {
            ui.painter().layout_no_wrap(text.to_owned(), font, detail_color)
        };
        let name_height = 24.0 * 0.85;
        let detail_height = 17.5 * galley.rows.len() as f32;
        let copy_height = name_height + 3.0 + detail_height;
        let minimum = if viewport.height() <= 620.0 && !phone { 64.0_f32 } else { 76.0_f32 };
        let height = minimum.max(
            vertical_pad * 2.0
                + if wrapped { copy_height + 8.0 + 44.0 } else { copy_height.max(44.0) },
        );
        let (_, rect) = ui.allocate_space(Vec2::new(width, height));
        if confirming {
            ui.painter().rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(228, 163, 154, 15));
        }
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(210, 178, 99, 31)),
        );
        let copy_top =
            if wrapped { rect.top() + vertical_pad } else { rect.center().y - copy_height / 2.0 };
        let copy = Rect::from_min_size(
            egui::pos2(rect.left() + horizontal_pad, copy_top),
            Vec2::new(copy_width, name_height),
        );
        stock_text(frame, ui, name, NativeUiFontName::Menu, 0.85, 0xd9ba70, copy, false);
        let text_rect = Rect::from_min_size(
            copy.left_bottom() + Vec2::new(0.0, 3.0),
            Vec2::new(copy_width, detail_height),
        );
        ui.painter().with_clip_rect(text_rect.intersect(ui.clip_rect())).galley(
            text_rect.min,
            galley,
            detail_color,
        );
        let key = row.key();
        let top = if wrapped { copy_top + copy_height + 8.0 } else { rect.center().y - 22.0 };
        let right = rect.right() - right_pad;
        let left = right - tool_width;
        let busy_action = match &self.busy {
            Some(Busy::Row { row: busy, action }) if busy == row => Some(*action),
            _ => None,
        };
        let enabled = self.busy.is_none();
        let choices = if confirming {
            vec![
                (
                    if busy_action == Some(RowAction::Delete) { "DELETING..." } else { "DELETE" },
                    "confirm",
                    format!("Delete {name}"),
                ),
                ("KEEP", "safe", format!("Keep {name}")),
            ]
        } else {
            vec![
                (
                    if busy_action == Some(RowAction::Edit) { "OPENING..." } else { "EDIT" },
                    "edit",
                    format!("Edit {name}"),
                ),
                (
                    if busy_action == Some(RowAction::Test) { "OPENING..." } else { "TEST" },
                    "test",
                    format!("Test {name}"),
                ),
                ("DELETE", "safe", format!("Delete {name}")),
            ]
        };
        for (index, (label, action, spoken)) in choices.into_iter().enumerate() {
            let id = format!("boneyards/{action}:{key}");
            let r = Rect::from_min_size(
                egui::pos2(left + index as f32 * (button_width + 10.0), top),
                Vec2::new(button_width, 44.0),
            );
            let response =
                widgets::cloud_button_with_label(frame, ui, &id, label, &spoken, r, 0.55, enabled);
            frame.record_value(&id, name, Some(confirming));
            if response.clicked() {
                actions.push(format!("{action}:{key}"));
            }
        }
    }
}
impl Drop for BoneyardLibrary {
    fn drop(&mut self) {
        if let Err(error) = self
            .input
            .remove_event_listener_with_callback("change", self.change.as_ref().unchecked_ref())
        {
            web_sys::console::error_1(&error);
        }
    }
}
fn display_name(name: &str) -> &str {
    let trimmed = name.trim();
    if trimmed.is_empty() { "Untitled Acre" } else { trimmed }
}
// Bitmap font, placement and tint are the authored drawing contract.
#[allow(clippy::too_many_arguments)]
fn stock_text(
    frame: &UiFrame<'_>,
    ui: &Ui,
    text: &str,
    font: NativeUiFontName,
    scale: f64,
    tint: u32,
    rect: Rect,
    center: bool,
) {
    let spec = NativeUiTextSpec {
        placement: NativeUiTextPlacement::Box,
        align: if center { NativeUiTextAlign::Center } else { NativeUiTextAlign::Left },
        scale,
        tint,
        ..NativeUiTextSpec::new(
            font,
            text,
            if center { f64::from(rect.width() / 2.0) } else { 0.0 },
            0.0,
        )
    };
    let mut layout = layout_native_ui_text(&spec);
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    if layout.unsupported_code_points.is_empty() {
        for glyph in &mut layout.glyphs {
            glyph.center_x += f64::from(rect.left());
            glyph.center_y += f64::from(rect.top() + (rect.height() - layout.height as f32) / 2.0);
        }
        paint_text_layout(frame, &painter, &layout);
    } else {
        painter.text(
            rect.left_center(),
            Align2::LEFT_CENTER,
            text,
            FontId::new(
                solomon_sim::native_ui_text::native_ui_font(font).metrics[0] as f32 * scale as f32,
                FontFamily::Name("Cinzel".into()),
            ),
            Color32::from_rgb((tint >> 16) as u8, (tint >> 8) as u8, tint as u8),
        );
    }
}
fn shelf_heading(frame: &UiFrame<'_>, ui: &mut Ui, text: &str, width: f32) {
    let viewport = frame.ctx.input(|input| input.content_rect());
    ui.add_space(if viewport.width() >= 701.0 && viewport.height() <= 620.0 { 12.0 } else { 18.0 });
    let (_, r) = ui.allocate_space(Vec2::new(width, 22.0));
    stock_text(
        frame,
        ui,
        text,
        NativeUiFontName::Medium,
        1.0,
        0xd9ba70,
        Rect::from_min_size(r.min + Vec2::new(10.0, 0.0), Vec2::new(width - 10.0, 16.0)),
        false,
    );
}
fn note(ui: &mut Ui, text: &str, width: f32) {
    let galley = ui.painter().layout(
        text.to_owned(),
        FontId::new(15.0, FontFamily::Name("IM FELL English".into())),
        Color32::from_rgb(178, 167, 141),
        width - 20.0,
    );
    // Source's 8px note margin collapses with the heading's 6px bottom margin.
    let (_, r) = ui.allocate_space(Vec2::new(width, galley.rows.len() as f32 * 21.0 + 2.0));
    ui.painter().galley(r.min + Vec2::new(10.0, 2.0), galley, Color32::from_rgb(178, 167, 141));
}
fn message(frame: &UiFrame<'_>, ui: &mut Ui, id: &str, text: &str, width: f32, tint: u32) {
    let color = Color32::from_rgb((tint >> 16) as u8, (tint >> 8) as u8, tint as u8);
    let galley = ui.painter().layout(
        text.to_owned(),
        FontId::new(15.0, FontFamily::Name("IM FELL English".into())),
        color,
        width,
    );
    let (_, r) = ui.allocate_space(Vec2::new(width, galley.size().y + 8.0));
    ui.painter().galley(r.min, galley, color);
    frame.record_rect(id, text, if id.ends_with("error") { "alert" } else { "status" }, r, false);
    frame.record_value(id, text, None);
    frame.announce(text);
}
fn date_format() -> Result<js_sys::Intl::DateTimeFormat, String> {
    let options = js_sys::Object::new();
    for (key, value) in [("dateStyle", "medium"), ("timeStyle", "short")] {
        js_sys::Reflect::set(&options, &key.into(), &value.into())
            .map_err(|_| "Boneyard date format options could not be set")?;
    }
    Ok(js_sys::Intl::DateTimeFormat::new(&js_sys::Array::new(), &options))
}
fn updated(formatter: &js_sys::Intl::DateTimeFormat, time: f64) -> Result<String, String> {
    if !time.is_finite() {
        return Err("Boneyard update time is not a valid date".to_owned());
    }
    formatter
        .format()
        .call1(&wasm_bindgen::JsValue::UNDEFINED, &time.into())
        .map_err(|_| "Boneyard update time could not be formatted")?
        .as_string()
        .ok_or_else(|| "Boneyard update time is not text".to_owned())
}
async fn import_file(file: &File) -> Result<Value, String> {
    let data = JsFuture::from(file.array_buffer()).await.map_err(|_| "It could not be read.")?;
    let bytes = js_sys::Uint8Array::new(&data).to_vec();
    if file.name().ends_with(".boneyard") {
        let mut doc = solomon_assets::boneyard::parse_editor_boneyard(&bytes)?;
        if let Some(spawn) = doc["geometry"].get("playerSpawn").cloned() {
            let mut spawn = spawn;
            spawn["facingDeg"] =
                doc["geometry"].get("playerSpawnFacingDeg").cloned().unwrap_or(json!(0));
            doc["spawn"] = spawn;
        }
        Ok(doc)
    } else {
        let text =
            String::from_utf8_lossy(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes));
        let file: Value =
            serde_json::from_str(&text).map_err(|_| "Not JSON. The Archivist has standards.")?;
        store::validate_file(&file)?;
        Ok(file["doc"].clone())
    }
}
