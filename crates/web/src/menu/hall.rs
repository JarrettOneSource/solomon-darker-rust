//! Local and hosted Hall records from Website `HallOfFameScene.tsx` and `HallOfFamePrimitives.tsx`.
//! The session supplies archived runs. This view owns only expansion, scrolling and art.

use std::collections::BTreeMap;

use solomon_sim::Vec2;
use solomon_sim::element_vfx::native_element_vfx_plan;
use solomon_sim::hall_of_fame::presentation::*;
use solomon_sim::hall_of_fame::{
    HallOfFameBoard, HallOfFameEntry, format_hall_of_fame_time, rank_hall_of_fame_entries,
};
use solomon_sim::hud::HudRect;
use solomon_sim::native_ui_text::{
    NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec, measure_native_ui_text,
};
use solomon_sim::profile::{WizardElement, native_skill_root, native_wizard_class_title};
use solomon_sim::skill_picker::SKILL_PICKER_ROOT_TINTS;
use web_sys::{Document, HtmlElement};

use super::HostedMenuAction;
use super::create::VfxView;
use crate::player_panels::chrome::{place, tint};
use crate::ui::portraits::StockPortraits;
use crate::ui::text::native_text;
use crate::ui::{StockAtlas, append, element, image, native_button, style};

/// A borrowed archive projection; setting this never creates a completed run or score.
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

struct VisibleRow {
    orb: VfxView,
    element: WizardElement,
    fill: Option<HtmlElement>,
    frame: Option<HtmlElement>,
}

pub(super) struct HallView {
    pub root: HtmlElement,
    stage: HtmlElement,
    scroller: HtmlElement,
    content: HtmlElement,
    tiles: HtmlElement,
    tabs: HtmlElement,
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
    header_dirty: bool,
    current_run_id: Option<String>,
    expanded: BTreeMap<String, bool>,
    row_tops: Vec<f64>,
    content_height: f64,
    range: Option<(usize, usize)>,
    visible: Vec<VisibleRow>,
    pending_scroll: Option<(f64, f64)>,
    center_on_open: bool,
    layout_dirty: bool,
    last_tick: Option<u64>,
    viewport: (i32, i32),
    ui: StockAtlas,
    skills: StockAtlas,
    inventory: StockAtlas,
    vfx: StockAtlas,
    portraits: StockPortraits,
    document: Document,
}

impl HallView {
    pub async fn new(document: &Document) -> Result<Self, String> {
        let root = element(document, "section", "menu-screen hall-screen")?;
        root.set_attribute("aria-label", "Hall of Fame").map_err(|_| "Hall label")?;
        let stage = element(document, "div", "hall-stage")?;
        append(&stage, &image(document, "hall-of-fame-background", "hall-background")?)?;
        let heading = element(document, "h1", "hall-phone-heading")?;
        heading.set_text_content(Some("HALL OF FAME"));
        append(&stage, &heading)?;
        let box_node = element(document, "div", "hall-box")?;
        let ui = StockAtlas::load("UI").await?;
        let tiles = ui.tiles(document, 49, 1320.0, 1056.0)?;
        tiles.set_class_name("hall-tiles");
        append(&box_node, &tiles)?;
        let scroller = element(document, "div", "hall-scroll")?;
        scroller.set_attribute("tabindex", "0").map_err(|_| "Hall scroll focus")?;
        scroller.set_attribute("aria-label", "Hall records").map_err(|_| "Hall scroll label")?;
        let content = element(document, "div", "hall-content")?;
        append(&scroller, &content)?;
        append(&box_node, &scroller)?;
        let header = element(document, "div", "hall-header")?;
        append(&header, &ui.tiles(document, 49, 1200.0, 55.0)?)?;
        let tabs = element(document, "div", "hall-tabs")?;
        append(&header, &tabs)?;
        for (side, rect) in hall_separator_halves(54.0).into_iter().enumerate() {
            let rule = element(
                document,
                "span",
                if side == 0 { "hall-rule hall-rule-left" } else { "hall-rule hall-rule-right" },
            )?;
            place(&rule, rect)?;
            append(&header, &rule)?;
        }
        append(&box_node, &header)?;
        append(&stage, &box_node)?;
        let back = native_button(document, "MAIN MENU", "hall-back")?;
        back.set_class_name("native-button hall-back");
        append(&stage, &back)?;
        append(&root, &stage)?;
        Ok(Self {
            root,
            stage,
            scroller,
            content,
            tiles,
            tabs,
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
            header_dirty: true,
            current_run_id: None,
            expanded: BTreeMap::new(),
            row_tops: Vec::new(),
            content_height: HALL_CONTENT_TOP,
            range: None,
            visible: Vec::new(),
            pending_scroll: None,
            center_on_open: true,
            layout_dirty: true,
            last_tick: None,
            viewport: (0, 0),
            ui,
            skills: StockAtlas::load("Skills").await?,
            inventory: StockAtlas::load("Inventory").await?,
            vfx: StockAtlas::load("BadGuys").await?,
            portraits: StockPortraits::load().await?,
            document: document.clone(),
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
        self.header_dirty = true;
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
        self.header_dirty = true;
        self.expanded.clear();
        self.scroller.set_scroll_top(0);
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
        self.scroller.set_scroll_top(0);
        self.pending_scroll = None;
        self.header_dirty = true;
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

    pub fn resize(&mut self, width: i32, height: i32) -> Result<(), String> {
        let scale = (f64::from(width) / 1600.0).min(f64::from(height) / 900.0);
        let compact = width < 900;
        self.root
            .set_attribute("data-hall-compact", if compact { "true" } else { "false" })
            .map_err(|_| "Hall responsive mode")?;
        if compact {
            style(&self.stage, "left:0;top:0;transform:none;width:100%;height:100%")?;
            // Preserve readable native rows on phones. The browser pans the same canvas;
            // first show its central wizard/record column, then allow both scroll axes.
            self.center_on_open = true;
        } else {
            style(
                &self.stage,
                &format!(
                    "left:{}px;top:{}px;transform:scale({scale});width:1600px;height:900px",
                    (f64::from(width) - 1600.0 * scale) / 2.0,
                    (f64::from(height) - 900.0 * scale) / 2.0
                ),
            )?;
            self.scroller.set_scroll_left(0);
        }
        self.viewport = (width, height);
        self.range = None;
        Ok(())
    }

    pub fn update(&mut self, now: f64) -> Result<(), String> {
        if self.header_dirty {
            self.draw_header()?;
            self.header_dirty = false;
        }
        if self.center_on_open {
            // Hidden DOM has no scroll extent. Center after the Hall is visible, when
            // browser layout can retain the native-canvas offset rather than clamp to zero.
            let left = if self.viewport.0 < 900 {
                (600 - self.scroller.client_width() / 2).max(0)
            } else {
                0
            };
            self.scroller.set_scroll_left(left);
            self.center_on_open = false;
        }
        if self.layout_dirty {
            let expanded: Vec<_> =
                self.entries.iter().map(|entry| self.is_expanded(entry)).collect();
            (self.row_tops, self.content_height) = hall_row_tops(&expanded);
            self.content
                .style()
                .set_property("height", &format!("{}px", self.content_height.max(HALL_BOX.height)))
                .map_err(|_| "Hall content height")?;
            self.root
                .set_attribute("data-hall-entry-count", &self.entries.len().to_string())
                .map_err(|_| "Hall entry count")?;
            if self.pending_scroll.is_some() {
                let target = self.entries.iter().position(|entry| self.is_current(entry)).map_or(
                    0.0,
                    |index| {
                        hall_current_row_scroll_target(self.row_tops[index], self.content_height)
                    },
                );
                self.pending_scroll = Some((target, now));
            }
            self.range = None;
            self.layout_dirty = false;
        }
        if let Some((target, started)) = self.pending_scroll {
            let tick = ((now - started).max(0.0) / 10.0).floor();
            self.scroller.set_scroll_top((hall_scroll_ease(tick) * target) as i32);
            if tick >= 89.0 {
                self.pending_scroll = None;
            }
        }
        let top = f64::from(self.scroller.scroll_top());
        self.tiles
            .style()
            .set_property("transform", &format!("translateY({}px)", -hall_tile_offset(top)))
            .map_err(|_| "Hall tile scroll")?;
        let expanded: Vec<_> = self.entries.iter().map(|entry| self.is_expanded(entry)).collect();
        let range = hall_visible_row_range(&self.row_tops, &expanded, top);
        if self.range != Some(range) {
            self.rebuild_visible(range)?;
            self.range = Some(range);
        }
        let tick = (now.max(0.0) / 10.0).floor() as u64;
        if self.last_tick == Some(tick) {
            return Ok(());
        }
        self.last_tick = Some(tick);
        for row in &mut self.visible {
            row.orb.update(
                &self.document,
                &self.vfx,
                &native_element_vfx_plan(row.element, tick as f64, 1.0),
            )?;
            if let Some(fill) = &row.fill {
                fill.style()
                    .set_property("opacity", &hall_highlight_fill_alpha(tick as f64).to_string())
                    .map_err(|_| "Hall current fill")?;
            }
            if let Some(frame) = &row.frame {
                frame
                    .style()
                    .set_property("opacity", &hall_current_frame_alpha(tick as f64).to_string())
                    .map_err(|_| "Hall current frame")?;
            }
        }
        Ok(())
    }

    fn is_current(&self, entry: &HallOfFameEntry) -> bool {
        self.current_run_id.as_deref() == Some(entry.run_id.as_str())
    }

    fn is_expanded(&self, entry: &HallOfFameEntry) -> bool {
        self.expanded.get(&entry.run_id).copied().unwrap_or_else(|| self.is_current(entry))
    }

    fn rebuild_visible(&mut self, (start, end): (usize, usize)) -> Result<(), String> {
        self.content.set_text_content(None);
        self.visible.clear();
        if self.entries.is_empty() {
            let result = self.global_ready();
            let error = result.and_then(|result| result.error.as_deref());
            let label = if self.scope == HallScope::Global && result.is_none() {
                "Loading global records..."
            } else {
                error.unwrap_or("No records yet.")
            };
            let status = element(&self.document, "div", "hall-status")?;
            status
                .set_attribute("role", if error.is_some() { "alert" } else { "status" })
                .map_err(|_| "Hall request status")?;
            status.set_attribute("aria-label", label).map_err(|_| "Hall status label")?;
            append(
                &status,
                &self.text(
                    label,
                    HallTextAnchor {
                        align: HallTextAlign::Center,
                        font: HallFont::Medium,
                        position: Vec2::new(600.0, 320.0),
                    },
                )?,
            )?;
            if error.is_some() {
                let width = self.measure_text("RETRY", HallFont::Medium);
                let retry =
                    self.tab("RETRY", "hall-retry", HallFont::Medium, 600.0 - width / 2.0, true)?;
                retry.style().set_property("top", "338px").map_err(|_| "Hall retry anchor")?;
                append(&status, &retry)?;
            }
            append(&self.content, &status)?;
        }
        for index in start..end {
            let entry = &self.entries[index];
            let expanded = self.is_expanded(entry);
            let current = self.is_current(entry);
            let rank = (index + 1).to_string();
            let score = format!("Awesomeness: {}", entry.awesomeness);
            let layout = hall_row_layout(
                self.row_tops[index],
                expanded,
                self.measure_text(&rank, HallFont::Heading),
                self.measure_text(&score, HallFont::Medium),
            );
            let row = element(&self.document, "article", "hall-row")?;
            row.set_attribute("data-hall-run", &entry.run_id).map_err(|_| "Hall row run")?;
            row.set_attribute("data-hall-expanded", if expanded { "true" } else { "false" })
                .map_err(|_| "Hall row expanded")?;
            row.set_attribute("aria-label", &format!("Rank {}: {}", index + 1, entry.wizard_name))
                .map_err(|_| "Hall row label")?;
            let fill = if current {
                let fill = element(&self.document, "span", "hall-current-fill")?;
                place(&fill, layout.highlight)?;
                append(&row, &fill)?;
                Some(fill)
            } else {
                None
            };
            let frame = self.frame(&row, 17, layout.highlight, if current { 0.5 } else { 0.2 })?;
            self.sprite(&row, &self.ui, 25, layout.ornament, 1.0, 0.0)?;
            append(&row, &self.text(&rank, layout.rank)?)?;
            let portrait = self.portraits.wizard(
                &self.document,
                entry.element,
                (entry.heading_index % 24) as usize,
                layout.wizard,
                1.25 * entry.portrait_scale,
            )?;
            let orb = VfxView::new(&self.document)?;
            append(&portrait.orb, &orb.root)?;
            append(&row, &portrait.root)?;
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
                append(&row, &self.text(&value, anchor)?)?;
            }
            if self.scope == HallScope::Global
                && let Some(account) = &entry.account_username
            {
                let name = self.text(&format!("@{account}"), layout.account)?;
                name.style().set_property("opacity", "0.7").map_err(|_| "Hall account opacity")?;
                append(&row, &name)?;
            }
            let toggle = element(&self.document, "button", "hall-toggle")?;
            toggle.set_attribute("type", "button").map_err(|_| "Hall toggle type")?;
            toggle
                .set_attribute("data-menu-action", &format!("hall-toggle:{index}"))
                .map_err(|_| "Hall toggle action")?;
            toggle
                .set_attribute("aria-expanded", if expanded { "true" } else { "false" })
                .map_err(|_| "Hall toggle state")?;
            toggle
                .set_attribute(
                    "aria-label",
                    &format!(
                        "{} details for {}",
                        if expanded { "Hide" } else { "Show" },
                        entry.wizard_name
                    ),
                )
                .map_err(|_| "Hall toggle label")?;
            style(
                &toggle,
                &format!(
                    "left:{}px;top:{}px",
                    layout.chevron.position.x - 22.0,
                    layout.chevron.position.y - 22.0
                ),
            )?;
            self.sprite(&toggle, &self.ui, 9, Vec2::new(22.0, 22.0), 1.0, layout.chevron.rotation)?;
            append(&row, &toggle)?;
            if let Some(detail) = layout.expanded {
                self.expanded_row(&row, entry, detail)?;
            }
            for (side, rect) in hall_separator_halves(layout.separator_y).into_iter().enumerate() {
                let rule = element(
                    &self.document,
                    "span",
                    if side == 0 {
                        "hall-rule hall-rule-left"
                    } else {
                        "hall-rule hall-rule-right"
                    },
                )?;
                place(&rule, rect)?;
                append(&row, &rule)?;
            }
            append(&self.content, &row)?;
            self.visible.push(VisibleRow {
                orb,
                element: entry.element,
                fill,
                frame: current.then_some(frame),
            });
        }
        self.last_tick = None;
        Ok(())
    }

    fn expanded_row(
        &self,
        row: &HtmlElement,
        entry: &HallOfFameEntry,
        layout: HallExpandedLayout,
    ) -> Result<(), String> {
        for (value, anchor) in [
            ("SURVIVAL".to_string(), layout.survival),
            ("Time:".to_string(), layout.time_label),
            (format_hall_of_fame_time(entry.elapsed_ticks), layout.time_value),
            ("Wave:".to_string(), layout.wave_label),
            (entry.wave.to_string(), layout.wave_value),
            ("HIGHEST SKILLS".to_string(), layout.highest_skills),
            ("PERKS USED".to_string(), layout.perks_used),
            (format!("Monsters Killed: {}", entry.monsters_killed), layout.monsters_killed),
            ("Awesomest Kill:".to_string(), layout.awesomest_label),
        ] {
            append(row, &self.text(&value, anchor)?)?;
        }
        self.frame(row, 50, layout.kills_frame, 0.5)?;
        if let Some(name) = &entry.awesomest_kill {
            append(row, &self.text(name, layout.awesomest_kill)?)?;
        }
        for (index, cell) in layout.skill_cells.into_iter().enumerate() {
            if let Some(skill) = entry.highest_skills.get(index) {
                let skill_id = usize::try_from(skill.skill_id).ok();
                let root = if skill.skill_id < 8 {
                    Some(skill.skill_id as usize)
                } else {
                    skill_id.and_then(native_skill_root)
                }
                .ok_or_else(|| format!("unknown native skill root for {}", skill.skill_id))?;
                let back = self.sprite(row, &self.skills, 164, cell.center, 1.0, 0.0)?;
                tint(&self.document, &back, SKILL_PICKER_ROOT_TINTS[root])?;
                let record = skill_id
                    .and_then(|id| id.checked_add(27))
                    .ok_or_else(|| format!("Skills has no Hall icon for {}", skill.skill_id))?;
                self.sprite(row, &self.skills, record, cell.center, 0.9, 0.0)?;
                let value = skill.rank.to_string();
                let badge = element(&self.document, "span", "hall-skill-badge")?;
                place(&badge, cell.badge(self.measure_text(&value, HallFont::Body)))?;
                append(row, &badge)?;
                let numeral = self.text(&value, cell.numeral)?;
                numeral
                    .style()
                    .set_property("color", "#fff")
                    .map_err(|_| "Hall skill rank tint")?;
                append(row, &numeral)?;
            }
            self.sprite(row, &self.inventory, 10, cell.center, 0.8, 0.0)?;
        }
        for (index, center) in layout.perk_centers.into_iter().enumerate() {
            self.sprite(row, &self.inventory, 10, center, 0.57, 0.0)?;
            if let Some(selector) = entry.perks_used.get(index) {
                let record = usize::try_from(*selector)
                    .ok()
                    .and_then(|selector| selector.checked_add(127))
                    .ok_or_else(|| format!("Skills has no Hall perk {selector}"))?;
                self.sprite(row, &self.skills, record, center, 0.7, 0.0)?;
            }
        }
        Ok(())
    }

    fn draw_header(&self) -> Result<(), String> {
        self.tabs.set_text_content(None);
        let mut x = 70.0;
        for (label, action, active) in [
            ("LOCAL", "hall-scope:local", self.scope == HallScope::Local),
            ("GLOBAL", "hall-scope:global", self.scope == HallScope::Global),
        ] {
            if action.ends_with("global") && !self.global_available {
                continue;
            }
            append(&self.tabs, &self.tab(label, action, HallFont::Medium, x, active)?)?;
            x += self.measure_text(label, HallFont::Medium) + 30.0;
        }
        if self.scope == HallScope::Global {
            let mut total = 24.0 * 3.0;
            for board in HallOfFameBoard::ALL {
                total += self.measure_text(&board.as_str().to_uppercase(), HallFont::Body);
            }
            let mut x = 1130.0 - total;
            for board in HallOfFameBoard::ALL {
                let label = board.as_str().to_uppercase();
                append(
                    &self.tabs,
                    &self.tab(
                        &label,
                        &format!("hall-board:{}", board.as_str()),
                        HallFont::Body,
                        x,
                        self.board == board,
                    )?,
                )?;
                x += self.measure_text(&label, HallFont::Body) + 24.0;
            }
        }
        self.root
            .set_attribute(
                "data-hall-scope",
                if self.scope == HallScope::Local { "local" } else { "global" },
            )
            .map_err(|_| "Hall active scope")?;
        self.root
            .set_attribute(
                "data-hall-board",
                if self.scope == HallScope::Local { "awesomeness" } else { self.board.as_str() },
            )
            .map_err(|_| "Hall active board")?;
        Ok(())
    }

    fn tab(
        &self,
        label: &str,
        action: &str,
        font: HallFont,
        x: f64,
        active: bool,
    ) -> Result<HtmlElement, String> {
        let node = element(&self.document, "button", "hall-tab")?;
        node.set_attribute("type", "button").map_err(|_| "Hall tab type")?;
        node.set_attribute("aria-label", label).map_err(|_| "Hall tab label")?;
        node.set_attribute("aria-pressed", if active { "true" } else { "false" })
            .map_err(|_| "Hall tab active")?;
        node.set_attribute("data-menu-action", action).map_err(|_| "Hall tab action")?;
        place(&node, HudRect::new(x - 8.0, 14.0, self.measure_text(label, font) + 16.0, 32.0))?;
        append(
            &node,
            &self.text(
                label,
                HallTextAnchor { align: HallTextAlign::Left, font, position: Vec2::new(8.0, 22.0) },
            )?,
        )?;
        append(&node, &element(&self.document, "span", "hall-tab-underline")?)?;
        Ok(node)
    }

    fn text(&self, value: &str, anchor: HallTextAnchor) -> Result<HtmlElement, String> {
        let value =
            if anchor.font == HallFont::Heading { value.to_uppercase() } else { value.to_owned() };
        let width = self.measure_text(&value, anchor.font);
        // `layoutHallText` centers with -ceil(width/2), not a half-pixel pen.
        let x = anchor.position.x
            - match anchor.align {
                HallTextAlign::Left => 0.0,
                HallTextAlign::Center => (width / 2.0).ceil(),
                HallTextAlign::Right => width,
            };
        native_text(
            &self.document,
            &NativeUiTextSpec {
                align: NativeUiTextAlign::Left,
                tint: 0xd9ba70,
                ..NativeUiTextSpec::new(hall_font(anchor.font), &value, x, anchor.position.y)
            },
            "hall-text",
        )
    }

    fn measure_text(&self, value: &str, font: HallFont) -> f64 {
        let value = if font == HallFont::Heading { value.to_uppercase() } else { value.to_owned() };
        measure_native_ui_text(&value, hall_font(font), 1.0)
    }

    fn sprite(
        &self,
        parent: &HtmlElement,
        atlas: &StockAtlas,
        record: usize,
        center: Vec2,
        scale: f64,
        rotation: f64,
    ) -> Result<HtmlElement, String> {
        let metrics = atlas.record(record)?;
        let node = atlas.sprite(&self.document, record, "hall-sprite")?;
        node.style().set_property("position", "absolute").map_err(|_| "Hall sprite positioning")?;
        for (property, value) in [
            ("left", (center.x - f64::from(metrics.logical_width) / 2.0 + 0.5).floor()),
            ("top", (center.y - f64::from(metrics.logical_height) / 2.0 + 0.5).floor()),
        ] {
            node.style()
                .set_property(property, &format!("{value}px"))
                .map_err(|_| "Hall sprite anchor")?;
        }
        node.style()
            .set_property("transform", &format!("rotate({rotation}deg) scale({scale})"))
            .map_err(|_| "Hall sprite transform")?;
        append(parent, &node)?;
        Ok(node)
    }

    fn frame(
        &self,
        parent: &HtmlElement,
        record: usize,
        rect: HudRect,
        alpha: f64,
    ) -> Result<HtmlElement, String> {
        let frame = self.ui.nine_slice_border(&self.document, record, rect.width, rect.height)?;
        place(&frame, rect)?;
        frame
            .style()
            .set_property("opacity", &alpha.to_string())
            .map_err(|_| "Hall frame alpha")?;
        append(parent, &frame)?;
        Ok(frame)
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
