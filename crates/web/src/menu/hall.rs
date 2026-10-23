//! GPU Hall of Fame from `HallOfFameScene` and `HallOfFamePrimitives`.
//! Ranking, authored geometry, scroll easing and highlights remain canonical simulation plans.
use super::create::paint_vfx;
use super::widgets::Stage;
use super::{HostedMenuAction, widgets};
use crate::gpu_ui::{StockAtlas, UiFrame};
use crate::ui::portraits::StockPortraits;
use egui::{Color32, Rect, Ui, Vec2};
use solomon_sim::hall_of_fame::presentation::*;
use solomon_sim::hall_of_fame::{
    HallOfFameBoard, HallOfFameEntry, format_hall_of_fame_time, rank_hall_of_fame_entries,
};
use solomon_sim::native_ui_text::{
    NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec, measure_native_ui_text,
};
use solomon_sim::profile::{native_skill_root, native_wizard_class_title};
use solomon_sim::skill_picker::SKILL_PICKER_ROOT_TINTS;
use std::collections::BTreeMap;
#[derive(Clone, Copy)]
pub struct HallSnapshot<'a> {
    pub entries: &'a [HallOfFameEntry],
    pub current_run_id: Option<&'a str>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HallScope {
    Local,
    Global,
}
struct GlobalHallResult {
    board: HallOfFameBoard,
    refresh: u64,
    entries: Vec<HallOfFameEntry>,
    error: Option<String>,
}
pub(super) struct HallView {
    local_entries: Vec<HallOfFameEntry>,
    entries: Vec<HallOfFameEntry>,
    scope: HallScope,
    board: HallOfFameBoard,
    global_available: bool,
    global_result: Option<GlobalHallResult>,
    global_request: Option<u64>,
    next_request_id: u64,
    refresh: u64,
    hosted_actions: Vec<HostedMenuAction>,
    current_run_id: Option<String>,
    expanded: BTreeMap<String, bool>,
    row_tops: Vec<f64>,
    content_height: f64,
    pending_scroll: Option<(f64, f64)>,
    center_on_open: bool,
    layout_dirty: bool,
    scroll_reset: bool,
    now: f64,
    ui: StockAtlas,
    skills: StockAtlas,
    inventory: StockAtlas,
    vfx: StockAtlas,
    portraits: StockPortraits,
}
impl HallView {
    pub async fn new() -> Result<Self, String> {
        Ok(Self {
            local_entries: Vec::new(),
            entries: Vec::new(),
            scope: HallScope::Local,
            board: HallOfFameBoard::Awesomeness,
            global_available: false,
            global_result: None,
            global_request: None,
            next_request_id: 0,
            refresh: 0,
            hosted_actions: Vec::new(),
            current_run_id: None,
            expanded: BTreeMap::new(),
            row_tops: Vec::new(),
            content_height: HALL_CONTENT_TOP,
            pending_scroll: None,
            center_on_open: true,
            layout_dirty: true,
            scroll_reset: true,
            now: 0.0,
            ui: StockAtlas::load("UI").await?,
            skills: StockAtlas::load("Skills").await?,
            inventory: StockAtlas::load("Inventory").await?,
            vfx: StockAtlas::load("BadGuys").await?,
            portraits: StockPortraits::load().await?,
        })
    }
    pub fn set_snapshot(&mut self, snapshot: HallSnapshot<'_>) {
        let same_entries = self.local_entries == snapshot.entries;
        let same_current = self.current_run_id.as_deref() == snapshot.current_run_id;
        if same_entries && same_current {
            return;
        }
        self.local_entries = snapshot.entries.to_vec();
        self.current_run_id = snapshot.current_run_id.map(str::to_owned);
        if self.scope == HallScope::Local {
            self.select_entries();
        } else if !same_current {
            self.layout_dirty = true;
        }
    }

    pub fn set_global_available(&mut self, available: bool) {
        if self.global_available == available {
            return;
        }
        self.global_available = available;
        if !available && self.scope == HallScope::Global {
            self.scope = HallScope::Local;
            self.global_request = None;
            self.select_entries();
        }
    }

    pub fn take_hosted_actions(&mut self) -> Vec<HostedMenuAction> {
        std::mem::take(&mut self.hosted_actions)
    }

    /// A board/scope change invalidates the previous request exactly as the source effect
    /// cleanup does. Late data never appears below another board's heading.
    pub fn complete_global(
        &mut self,
        request_id: u64,
        result: Result<Vec<HallOfFameEntry>, String>,
    ) -> bool {
        if self.scope != HallScope::Global || self.global_request != Some(request_id) {
            return false;
        }
        self.global_request = None;
        let (entries, error) = match result {
            Ok(entries) => (entries, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        self.global_result =
            Some(GlobalHallResult { board: self.board, refresh: self.refresh, entries, error });
        self.select_entries();
        true
    }

    pub fn open(&mut self, now: f64) {
        self.scope = HallScope::Local;
        self.board = HallOfFameBoard::Awesomeness;
        self.global_result = None;
        self.global_request = None;
        self.refresh = 0;
        self.expanded.clear();
        self.scroll_reset = true;
        self.select_entries();
        self.pending_scroll = Some((0.0, now));
        self.center_on_open = true;
    }

    pub fn close(&mut self) {
        self.global_request = None;
    }

    pub fn activate(&mut self, action: &str) -> bool {
        match action {
            "hall-scope:local" => {
                if self.scope == HallScope::Local {
                    return true;
                }
                self.scope = HallScope::Local;
                self.global_request = None;
            }
            "hall-scope:global" if self.global_available => {
                if self.scope == HallScope::Global {
                    return true;
                }
                self.scope = HallScope::Global;
                self.request_global();
            }
            "hall-retry" if self.scope == HallScope::Global => {
                self.refresh += 1;
                self.request_global();
            }
            _ => {
                let Some(id) = action.strip_prefix("hall-board:") else { return false };
                let Some(board) =
                    HallOfFameBoard::ALL.into_iter().find(|board| board.as_str() == id)
                else {
                    return false;
                };
                if self.scope != HallScope::Global || self.board == board {
                    return true;
                }
                self.board = board;
                self.request_global();
            }
        }
        self.expanded.clear();
        self.scroll_reset = true;
        self.pending_scroll = None;
        self.select_entries();
        true
    }

    fn request_global(&mut self) {
        self.next_request_id += 1;
        self.global_request = Some(self.next_request_id);
        self.hosted_actions.push(HostedMenuAction::LoadGlobalHall {
            request_id: self.next_request_id,
            board: self.board,
        });
    }

    fn global_ready(&self) -> Option<&GlobalHallResult> {
        self.global_result
            .as_ref()
            .filter(|result| result.board == self.board && result.refresh == self.refresh)
    }

    fn select_entries(&mut self) {
        let (entries, board) = match self.scope {
            HallScope::Local => (self.local_entries.as_slice(), HallOfFameBoard::Awesomeness),
            HallScope::Global => (
                self.global_ready().map_or(&[][..], |result| result.entries.as_slice()),
                self.board,
            ),
        };
        self.entries =
            rank_hall_of_fame_entries(entries, board, 100).into_iter().cloned().collect();
        self.layout_dirty = true;
    }

    pub fn toggle(&mut self, index: usize) {
        if let Some(entry) = self.entries.get(index) {
            let expanded = self.is_expanded(entry);
            self.expanded.insert(entry.run_id.clone(), !expanded);
            self.layout_dirty = true;
            self.pending_scroll = None;
        }
    }

    pub fn update(&mut self, now: f64) -> Result<(), String> {
        self.now = now;
        if self.layout_dirty {
            let expanded: Vec<_> = self.entries.iter().map(|e| self.is_expanded(e)).collect();
            (self.row_tops, self.content_height) = hall_row_tops(&expanded);
            if self.pending_scroll.is_some() {
                let target =
                    self.entries.iter().position(|e| self.is_current(e)).map_or(0.0, |i| {
                        hall_current_row_scroll_target(self.row_tops[i], self.content_height)
                    });
                self.pending_scroll = Some((target, now));
            }
            self.layout_dirty = false;
        }
        Ok(())
    }
    fn is_current(&self, entry: &HallOfFameEntry) -> bool {
        self.current_run_id.as_deref() == Some(entry.run_id.as_str())
    }
    fn is_expanded(&self, entry: &HallOfFameEntry) -> bool {
        self.expanded.get(&entry.run_id).copied().unwrap_or_else(|| self.is_current(entry))
    }
    pub fn paint(
        &mut self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        view: Rect,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        ui.painter().rect_filled(view, 0.0, widgets::color(0x080909));
        let compact = view.width() < 900.0;
        let stage = Stage::fit(view);
        let box_rect = if compact {
            widgets::label(
                ui,
                "HALL OF FAME",
                egui::pos2(view.center().x, view.top() + 14.0),
                egui::Align2::CENTER_TOP,
                "Palatino",
                24.0,
                widgets::GOLD,
            );
            Rect::from_min_max(view.min + Vec2::new(12.0, 56.0), view.max - Vec2::new(12.0, 100.0))
        } else {
            widgets::image(
                frame,
                ui,
                "hall-of-fame-background",
                stage.rect(0.0, 0.0, 1600.0, 900.0),
                Color32::WHITE,
            );
            stage.rect(200.0, 80.0, 1200.0, 695.0)
        };
        let scale = if compact { 1.0 } else { stage.scale };
        frame.record_rect("hall-records", "Hall records", "region", box_rect, true);
        self.ui.paint_tiles(frame, ui.painter(), 49, box_rect, scale, Color32::WHITE)?;
        let mut child =
            ui.new_child(egui::UiBuilder::new().id_salt("hall-content").max_rect(box_rect));
        child.set_clip_rect(box_rect);
        widgets::scrollbar_style(&mut child, 0x8e7950, 0x151311, true);
        let mut scroll = egui::ScrollArea::both()
            .id_salt("hall-records")
            .scroll_bar_visibility(if compact {
                egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded
            } else {
                egui::scroll_area::ScrollBarVisibility::AlwaysHidden
            })
            .auto_shrink([false, false]);
        if !compact {
            scroll = scroll.horizontal_scroll_offset(0.0);
        }
        if self.center_on_open {
            scroll = scroll.horizontal_scroll_offset(if compact {
                (600.0 - box_rect.width() / 2.0).max(0.0)
            } else {
                0.0
            });
            self.center_on_open = false;
        }
        if self.scroll_reset {
            scroll = scroll.vertical_scroll_offset(0.0);
            self.scroll_reset = false;
        }
        if let Some((target, started)) = self.pending_scroll {
            let tick = ((self.now - started).max(0.0) / 10.0).floor();
            scroll =
                scroll.vertical_scroll_offset((hall_scroll_ease(tick) * target) as f32 * scale);
            if tick >= 89.0 {
                self.pending_scroll = None;
            }
        }
        let mut result = Ok(());
        scroll.show_viewport(&mut child, |ui, viewport| {
            let origin = ui.cursor().min;
            let content = Stage { origin, scale };
            ui.allocate_space(Vec2::new(
                1200.0 * scale,
                self.content_height.max(HALL_BOX.height) as f32 * scale,
            ));
            if self.entries.is_empty() {
                let ready = self.global_ready();
                let error = ready.and_then(|r| r.error.as_deref());
                let text = if self.scope == HallScope::Global && ready.is_none() {
                    "Loading global records..."
                } else {
                    error.unwrap_or("No records yet.")
                };
                frame.record_rect("hall-status", text, "status", box_rect, false);
                frame.record_value("hall-status", text, None);
                hall_text(
                    frame,
                    ui,
                    content,
                    text,
                    HallTextAnchor {
                        align: HallTextAlign::Center,
                        font: HallFont::Medium,
                        position: solomon_sim::Vec2::new(600.0, 320.0),
                    },
                );
                if error.is_some()
                    && widgets::stock_button(
                        frame,
                        ui,
                        "hall-retry",
                        "RETRY",
                        content.rect(500.0, 338.0, 200.0, 44.0),
                        0.55 * scale,
                        true,
                    )
                    .clicked()
                {
                    actions.push("hall-retry".to_owned());
                }
            }
            let expanded: Vec<_> = self.entries.iter().map(|e| self.is_expanded(e)).collect();
            let range = hall_visible_row_range(
                &self.row_tops,
                &expanded,
                f64::from(viewport.top() / scale),
            );
            for index in range.0..range.1 {
                if let Err(error) = self.paint_row(frame, ui, content, index, actions) {
                    result = Err(error);
                    break;
                }
            }
        });
        std::mem::replace(&mut result, Ok(()))?;
        let header = Rect::from_min_size(box_rect.min, Vec2::new(box_rect.width(), 55.0 * scale));
        self.ui.paint_tiles(frame, ui.painter(), 49, header, scale, Color32::WHITE)?;
        let head_stage = Stage { origin: box_rect.min, scale };
        paint_separator(&ui.painter().with_clip_rect(header), head_stage, 54.0);
        if compact {
            ui.painter().rect_stroke(
                box_rect,
                0.0,
                egui::Stroke::new(1.0, widgets::GOLD),
                egui::StrokeKind::Outside,
            );
        }
        let mut x = if compact { 16.0 } else { 70.0 };
        for (label, id, active) in [
            ("LOCAL", "hall-scope:local", self.scope == HallScope::Local),
            ("GLOBAL", "hall-scope:global", self.scope == HallScope::Global),
        ] {
            if id.ends_with("global") && !self.global_available {
                continue;
            }
            let width = measure(label, HallFont::Medium);
            let rect = head_stage.rect(x - 8.0, 14.0, width + 16.0, 32.0);
            let response = widgets::interact_unstyled(frame, ui, id, label, "tab", rect, true);
            if response.clicked() {
                actions.push(id.to_owned());
            }
            frame.record_value(id, if active { "true" } else { "false" }, Some(active));
            hall_text_alpha(
                frame,
                ui,
                head_stage,
                label,
                HallTextAnchor {
                    align: HallTextAlign::Left,
                    font: HallFont::Medium,
                    position: solomon_sim::Vec2::new(x, 36.0),
                },
                tab_opacity(frame, &response, active),
            );
            widgets::focus_outline(
                frame,
                ui,
                &response,
                scale,
                2.0 * scale,
                widgets::alpha(0xedd289, 0.9),
            );
            if active {
                ui.painter().line_segment(
                    [head_stage.at(x, 45.0), head_stage.at(x + width, 45.0)],
                    egui::Stroke::new(2.0 * scale, widgets::GOLD),
                );
            }
            x += width + 30.0;
        }
        if self.scope == HallScope::Global {
            let total = HallOfFameBoard::ALL
                .iter()
                .map(|b| measure(&b.as_str().to_uppercase(), HallFont::Body))
                .sum::<f64>()
                + 72.0;
            let mut x = if compact { x + 20.0 } else { 1130.0 - total };
            for board in HallOfFameBoard::ALL {
                let label = board.as_str().to_uppercase();
                let width = measure(&label, HallFont::Body);
                let id = format!("hall-board:{}", board.as_str());
                let response = widgets::interact_unstyled(
                    frame,
                    ui,
                    &id,
                    &label,
                    "tab",
                    head_stage.rect(x - 8.0, 14.0, width + 16.0, 32.0),
                    true,
                );
                frame.record_value(&id, board.as_str(), Some(self.board == board));
                if response.clicked() {
                    actions.push(id);
                }
                hall_text_alpha(
                    frame,
                    ui,
                    head_stage,
                    &label,
                    HallTextAnchor {
                        align: HallTextAlign::Left,
                        font: HallFont::Body,
                        position: solomon_sim::Vec2::new(x, 36.0),
                    },
                    tab_opacity(frame, &response, self.board == board),
                );
                widgets::focus_outline(
                    frame,
                    ui,
                    &response,
                    scale,
                    2.0 * scale,
                    widgets::alpha(0xedd289, 0.9),
                );
                if self.board == board {
                    ui.painter().line_segment(
                        [head_stage.at(x, 45.0), head_stage.at(x + width, 45.0)],
                        egui::Stroke::new(2.0 * scale, widgets::GOLD),
                    );
                }
                x += width + 24.0;
            }
        }
        let back = if compact {
            Rect::from_center_size(
                egui::pos2(view.center().x, view.bottom() - 45.0),
                Vec2::new(353.0_f32.min(view.width() - 48.0), 62.0),
            )
        } else {
            stage.rect(617.5, 809.5, 365.0, 85.0)
        };
        if compact {
            if widgets::stock_button(
                frame,
                ui,
                "hall-back",
                "MAIN MENU",
                back,
                back.height() / 69.0,
                true,
            )
            .clicked()
            {
                actions.push("hall-back".to_owned());
            }
        } else {
            let response = widgets::interact_unstyled(
                frame,
                ui,
                "hall-back",
                "MAIN MENU",
                "button",
                back,
                true,
            );
            if response.hovered() || frame.focus_visible(&response) {
                ui.painter().rect_stroke(
                    back.shrink(8.0 * scale),
                    0.0,
                    egui::Stroke::new(2.0 * scale, widgets::alpha(0xedd289, 0.9)),
                    egui::StrokeKind::Inside,
                );
            }
            if response.clicked() {
                actions.push("hall-back".to_owned());
            }
        }
        Ok(())
    }
    fn paint_row(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &Ui,
        stage: Stage,
        index: usize,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let entry = &self.entries[index];
        let expanded = self.is_expanded(entry);
        let current = self.is_current(entry);
        let rank = (index + 1).to_string();
        let score = format!("Awesomeness: {}", entry.awesomeness);
        let layout = hall_row_layout(
            self.row_tops[index],
            expanded,
            measure(&rank, HallFont::Heading),
            measure(&score, HallFont::Medium),
        );
        let tick = (self.now.max(0.0) / 10.0).floor();
        let rect = stage.rect(
            layout.highlight.x,
            layout.highlight.y,
            layout.highlight.width,
            layout.highlight.height,
        );
        if current {
            ui.painter().rect_filled(
                rect,
                0.0,
                widgets::alpha(0xd9ba70, hall_highlight_fill_alpha(tick)),
            );
        }
        self.ui.paint_native_frame_scaled(
            frame,
            ui.painter(),
            17,
            rect,
            stage.scale,
            false,
            widgets::alpha(0xffffff, if current { hall_current_frame_alpha(tick) } else { 0.2 }),
        )?;
        self.ui.paint_centered(
            frame,
            ui.painter(),
            25,
            stage.at(layout.ornament.x, layout.ornament.y),
            stage.scale,
            0.0,
            Color32::WHITE,
        )?;
        hall_text(frame, ui, stage, &rank, layout.rank);
        let portrait_scale = stage.scale * 1.25 * entry.portrait_scale as f32;
        self.portraits.paint_wizard_with_orb(
            frame,
            ui.painter(),
            entry.element,
            (entry.heading_index % 24) as usize,
            stage.at(layout.wizard.x, layout.wizard.y),
            portrait_scale,
            |frame, painter, orb| {
                paint_vfx(
                    frame,
                    painter,
                    &self.vfx,
                    orb,
                    portrait_scale,
                    &solomon_sim::element_vfx::native_element_vfx_plan(entry.element, tick, 1.0),
                )
            },
        )?;
        for (value, anchor) in [
            (entry.wizard_name.clone(), layout.name),
            (
                format!(
                    "Level {} {}",
                    entry.level,
                    native_wizard_class_title(entry.element, entry.discipline)
                ),
                layout.level,
            ),
            (score, layout.awesomeness),
        ] {
            hall_text(frame, ui, stage, &value, anchor);
        }
        if self.scope == HallScope::Global
            && let Some(account) = &entry.account_username
        {
            hall_text_alpha(frame, ui, stage, &format!("@{account}"), layout.account, 0.7);
        }
        let id = format!("hall-toggle:{index}");
        let center = stage.at(layout.chevron.position.x, layout.chevron.position.y);
        let response = widgets::interact(
            frame,
            ui,
            &id,
            &format!(
                "{} details for {}",
                if expanded { "Hide" } else { "Show" },
                entry.wizard_name
            ),
            "button",
            Rect::from_center_size(center, Vec2::splat(44.0 * stage.scale)),
            true,
        );
        frame.record_value(&id, &entry.run_id, Some(expanded));
        if response.clicked() {
            actions.push(id);
        }
        self.ui.paint_centered(
            frame,
            ui.painter(),
            9,
            center,
            stage.scale,
            layout.chevron.rotation.to_radians() as f32,
            Color32::WHITE,
        )?;
        if let Some(detail) = layout.expanded {
            self.expanded_row(frame, ui, stage, entry, detail)?;
        }
        paint_separator(ui.painter(), stage, layout.separator_y);
        Ok(())
    }
    fn expanded_row(
        &self,
        frame: &UiFrame<'_>,
        ui: &Ui,
        stage: Stage,
        entry: &HallOfFameEntry,
        layout: HallExpandedLayout,
    ) -> Result<(), String> {
        for (value, anchor) in [
            ("SURVIVAL".to_owned(), layout.survival),
            ("Time:".to_owned(), layout.time_label),
            (format_hall_of_fame_time(entry.elapsed_ticks), layout.time_value),
            ("Wave:".to_owned(), layout.wave_label),
            (entry.wave.to_string(), layout.wave_value),
            ("HIGHEST SKILLS".to_owned(), layout.highest_skills),
            ("PERKS USED".to_owned(), layout.perks_used),
            (format!("Monsters Killed: {}", entry.monsters_killed), layout.monsters_killed),
            ("Awesomest Kill:".to_owned(), layout.awesomest_label),
        ] {
            hall_text(frame, ui, stage, &value, anchor);
        }
        self.ui.paint_native_frame_scaled(
            frame,
            ui.painter(),
            50,
            stage.rect(
                layout.kills_frame.x,
                layout.kills_frame.y,
                layout.kills_frame.width,
                layout.kills_frame.height,
            ),
            stage.scale,
            false,
            widgets::alpha(0xffffff, 0.5),
        )?;
        if let Some(name) = &entry.awesomest_kill {
            hall_text(frame, ui, stage, name, layout.awesomest_kill);
        }
        for (index, cell) in layout.skill_cells.into_iter().enumerate() {
            let center = stage.at(cell.center.x, cell.center.y);
            if let Some(skill) = entry.highest_skills.get(index) {
                let id = usize::try_from(skill.skill_id).map_err(|_| "Hall skill identity")?;
                let root = if id < 8 { Some(id) } else { native_skill_root(id) }
                    .ok_or_else(|| format!("unknown native skill root for {id}"))?;
                self.skills.paint_centered(
                    frame,
                    ui.painter(),
                    164,
                    center,
                    stage.scale,
                    0.0,
                    widgets::color(SKILL_PICKER_ROOT_TINTS[root]),
                )?;
                self.skills.paint_centered(
                    frame,
                    ui.painter(),
                    id + 27,
                    center,
                    stage.scale * 0.9,
                    0.0,
                    Color32::WHITE,
                )?;
                let value = skill.rank.to_string();
                let badge = cell.badge(measure(&value, HallFont::Body));
                ui.painter().rect_filled(
                    stage.rect(badge.x, badge.y, badge.width, badge.height),
                    0.0,
                    egui::Color32::from_black_alpha(128),
                );
                hall_text(frame, ui, stage, &value, cell.numeral);
            }
            self.inventory.paint_centered(
                frame,
                ui.painter(),
                10,
                center,
                stage.scale * 0.8,
                0.0,
                Color32::WHITE,
            )?;
        }
        for (index, center) in layout.perk_centers.into_iter().enumerate() {
            let center = stage.at(center.x, center.y);
            self.inventory.paint_centered(
                frame,
                ui.painter(),
                10,
                center,
                stage.scale * 0.57,
                0.0,
                Color32::WHITE,
            )?;
            if let Some(selector) = entry.perks_used.get(index) {
                self.skills.paint_centered(
                    frame,
                    ui.painter(),
                    usize::try_from(*selector).map_err(|_| "Hall perk identity")? + 127,
                    center,
                    stage.scale * 0.7,
                    0.0,
                    Color32::WHITE,
                )?;
            }
        }
        Ok(())
    }
}
fn hall_font(font: HallFont) -> NativeUiFontName {
    match font {
        HallFont::Body => NativeUiFontName::Body,
        HallFont::Heading => NativeUiFontName::Heading,
        HallFont::Medium => NativeUiFontName::Medium,
        HallFont::Menu => NativeUiFontName::Menu,
    }
}
fn measure(text: &str, font: HallFont) -> f64 {
    measure_native_ui_text(
        &if font == HallFont::Heading { text.to_uppercase() } else { text.to_owned() },
        hall_font(font),
        1.0,
    )
}
fn hall_text(frame: &UiFrame<'_>, ui: &Ui, stage: Stage, text: &str, anchor: HallTextAnchor) {
    hall_text_alpha(frame, ui, stage, text, anchor, 1.0);
}
fn hall_text_alpha(
    frame: &UiFrame<'_>,
    ui: &Ui,
    stage: Stage,
    text: &str,
    anchor: HallTextAnchor,
    alpha: f64,
) {
    let text = if anchor.font == HallFont::Heading { text.to_uppercase() } else { text.to_owned() };
    let width = measure(&text, anchor.font);
    let x = anchor.position.x
        - match anchor.align {
            HallTextAlign::Left => 0.0,
            HallTextAlign::Center => (width / 2.0).ceil(),
            HallTextAlign::Right => width,
        };
    let position = stage.at(x, anchor.position.y);
    crate::gpu_ui::native_text(
        frame,
        ui.painter(),
        &NativeUiTextSpec {
            align: NativeUiTextAlign::Left,
            scale: f64::from(stage.scale),
            alpha,
            tint: 0xd9ba70,
            ..NativeUiTextSpec::new(
                hall_font(anchor.font),
                &text,
                f64::from(position.x),
                f64::from(position.y),
            )
        },
    );
}

fn tab_opacity(frame: &UiFrame<'_>, response: &egui::Response, active: bool) -> f64 {
    let highlighted = active || response.hovered() || frame.focus_visible(response);
    let progress = frame.ctx.animate_bool_with_time_and_easing(
        response.id.with("label-opacity"),
        highlighted,
        0.12,
        |progress| crate::gpu_ui::css_bezier(progress, 0.0, 0.0, 0.58, 1.0),
    );
    0.55 + 0.45 * f64::from(progress)
}

fn paint_separator(painter: &egui::Painter, stage: Stage, y: f64) {
    for (index, rect) in hall_separator_halves(y).into_iter().enumerate() {
        let rect = stage.rect(rect.x, rect.y, rect.width, rect.height);
        let (left, right) = if index == 0 {
            (Color32::TRANSPARENT, widgets::GOLD)
        } else {
            (widgets::GOLD, Color32::TRANSPARENT)
        };
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(rect.left_top(), left);
        mesh.colored_vertex(rect.right_top(), right);
        mesh.colored_vertex(rect.right_bottom(), right);
        mesh.colored_vertex(rect.left_bottom(), left);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(egui::Shape::mesh(mesh));
    }
}
