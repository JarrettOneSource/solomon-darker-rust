//! Stock College and run HUD. Simulation `hud`/`mobile_ui` kernels own its geometry
//! and meter rules; `GameHud.tsx` and `SkillQuickbar.tsx` are the presentation reference.
mod quickbar;

use crate::gpu_ui::{StockAtlas, UiFrame};
use crate::hotbar::{HotbarControls, HotbarSelection};
use solomon_sim::belt::NATIVE_BELT_SLOT_COUNT;
use solomon_sim::hud::{
    HudBeltEntry, HudHealthLayerKind, HudRect, HudState, clamp_unit, native_health_hud_layers,
    native_health_hud_presentation, native_hud_control_layout, native_mana_hud_presentation,
};
use solomon_sim::mobile_ui::{MobileUiElementId, MobileUiLayoutState, default_mobile_ui_geometry};
use std::cell::{Cell, RefCell};
use web_sys::Document;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudAction {
    Pause,
    Inventory,
    Skills,
    Map,
    Shortcut(usize),
    Belt(usize),
    SkillBinding(usize),
}

/// Website GameHud's `mode`: College shortcuts/help/map are absent from a live run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudMode {
    Hub,
    Run,
}

/// Presentation of canonical Tutorial access; the session still admits every action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudAccess {
    pub combat: bool,
    pub inventory: bool,
    pub quickbar: bool,
    pub skills: bool,
    pub spell: bool,
}

impl HudAccess {
    const UNRESTRICTED: Self =
        Self { combat: true, inventory: true, quickbar: true, skills: true, spell: true };
}

/// Actual CSS-pixel control face and press-time admission retained by gameplay input.
#[derive(Clone, Copy, Debug)]
pub struct HudBeltHit {
    pub slot: usize,
    pub face: egui::Rect,
    pub rotation: f32,
    pub input_scale: f64,
    pub cast_eligible: bool,
}
impl HudBeltHit {
    pub fn contains(self, point: egui::Pos2) -> bool {
        self.face.contains(
            self.face.center()
                + egui::emath::Rot2::from_angle(-self.rotation) * (point - self.face.center()),
        )
    }
}

pub struct Hud {
    document: Document,
    visible: Cell<bool>,
    frame_intervals: u64,
    frame_sample_started_at: Option<f64>,
    fps: String,
    ping: String,
    hotbar: HotbarSelection,
    hotbar_controls: HotbarControls,
    ui: StockAtlas,
    skills: StockAtlas,
    inventory: StockAtlas,
    college: StockAtlas,
    portraits: StockAtlas,
    actions: RefCell<Vec<HudAction>>,
    belt_hits: RefCell<Vec<HudBeltHit>>,
    disabled_actions: RefCell<Vec<HudAction>>,
    snapshot: Option<HudState>,
    viewport: (i32, i32),
    actions_enabled: bool,
    hotbar_enabled: bool,
    ui_scale: f64,
    mode: HudMode,
    controller_belt_slot: Option<usize>,
    tutorial_access: HudAccess,
    mobile_layout: MobileUiLayoutState,
    mod_belt_icons: [Option<Vec<crate::ui::mod_icon::ModIconLayer>>; 8],
    quickbar: [Option<quickbar::QuickbarSlot>; 8],
    bursts: RefCell<Vec<(usize, crate::ui::belt_pull_off::BeltPullOffBurst)>>,
    now: f64,
    bank: usize,
}

impl Hud {
    pub async fn attach(document: &Document, hotbar: HotbarSelection) -> Result<Self, String> {
        let bank = hotbar.bank();
        Ok(Self {
            document: document.clone(),
            visible: Cell::new(true),
            frame_intervals: 0,
            frame_sample_started_at: None,
            fps: "-- FPS".into(),
            ping: "-- ms".into(),
            hotbar_controls: HotbarControls::new(document, hotbar.clone(), None)?,
            hotbar,
            ui: StockAtlas::load("UI").await?,
            skills: StockAtlas::load("Skills").await?,
            inventory: StockAtlas::load("Inventory").await?,
            college: StockAtlas::load("College").await?,
            portraits: StockAtlas::load("LevelPicker").await?,
            actions: RefCell::new(Vec::new()),
            belt_hits: RefCell::new(Vec::new()),
            disabled_actions: RefCell::new(Vec::new()),
            snapshot: None,
            viewport: (0, 0),
            actions_enabled: false,
            hotbar_enabled: false,
            ui_scale: 1.0,
            mode: HudMode::Hub,
            controller_belt_slot: None,
            tutorial_access: HudAccess::UNRESTRICTED,
            mobile_layout: MobileUiLayoutState::default(),
            mod_belt_icons: std::array::from_fn(|_| None),
            quickbar: std::array::from_fn(|_| None),
            bursts: RefCell::new(Vec::new()),
            now: 0.0,
            bank,
        })
    }
    pub fn set_actions_enabled(&mut self, enabled: bool) -> Result<(), String> {
        self.actions_enabled = enabled;
        Ok(())
    }
    pub fn take_actions(&self) -> Vec<HudAction> {
        std::mem::take(&mut *self.actions.borrow_mut())
    }
    pub fn set_hotbar_enabled(&mut self, enabled: bool) {
        self.hotbar_enabled = enabled;
    }
    pub fn set_tutorial_access(&mut self, access: Option<HudAccess>) -> Result<(), String> {
        self.tutorial_access = access.unwrap_or(HudAccess::UNRESTRICTED);
        Ok(())
    }
    pub fn set_mode(&mut self, mode: HudMode) -> Result<(), String> {
        self.mode = mode;
        Ok(())
    }
    pub fn set_controller_belt_slot(&mut self, slot: Option<usize>) {
        assert!(slot.is_none_or(|slot| slot < 8));
        self.controller_belt_slot = slot;
    }
    pub fn set_action_enabled(&self, action: HudAction, enabled: bool) -> Result<(), String> {
        let mut disabled = self.disabled_actions.borrow_mut();
        disabled.retain(|a| *a != action);
        if !enabled {
            disabled.push(action);
        }
        Ok(())
    }
    pub fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
    }
    pub fn set_ui_scale(&mut self, scale: f64) {
        self.ui_scale = scale;
    }
    pub fn update_diagnostics(&mut self, now_ms: f64, ping_ms: Option<f64>) -> Result<(), String> {
        self.now = now_ms;
        if let Some(started) = self.frame_sample_started_at {
            self.frame_intervals += 1;
            let elapsed = now_ms - started;
            if elapsed >= 1000.0 {
                self.fps = format!(
                    "{} FPS",
                    (self.frame_intervals as f64 * 1000.0 / elapsed + 0.5).floor()
                );
                self.frame_intervals = 0;
                self.frame_sample_started_at = Some(now_ms);
            }
        } else {
            self.frame_sample_started_at = Some(now_ms);
        }
        self.ping = ping_ms.map_or_else(|| "-- ms".into(), |ping| format!("{ping} ms"));
        Ok(())
    }
    pub fn set_mobile_layout(&mut self, state: &MobileUiLayoutState) -> Result<(), String> {
        if self.mobile_layout != *state {
            self.mobile_layout = state.clone();
        }
        Ok(())
    }
    pub fn set_mod_belt_icons(
        &mut self,
        player: Option<(
            &solomon_sim::belt::PlayerBeltComponent,
            &solomon_sim::economy::HubEconomyState,
        )>,
        images: Option<&crate::mod_presentation_assets::ModPresentationAssets>,
    ) -> Result<(), String> {
        let mut icons = std::array::from_fn(|_| None);
        if let Some((belt, economy)) = player {
            for (slot, entry) in belt
                .entries
                .iter()
                .skip(self.hotbar.start())
                .take(NATIVE_BELT_SLOT_COUNT)
                .enumerate()
            {
                if let Some(item) = entry
                    .and_then(|entry| solomon_sim::belt::native_belt_entry_item(entry, economy))
                {
                    icons[slot] = crate::ui::mod_icon::mod_item_icon_layers(images, item)?;
                }
            }
        }
        self.mod_belt_icons = icons;
        Ok(())
    }
    pub fn update(&mut self, state: &HudState, width: i32, height: i32) -> Result<(), String> {
        self.viewport = (width, height);
        self.snapshot = Some(state.clone());
        if self.hotbar_controls.update(!self.actions_enabled || !self.hotbar_enabled)?
            || self.bank != self.hotbar.bank()
        {
            self.bursts.borrow_mut().clear();
            self.bank = self.hotbar.bank();
        }
        Ok(())
    }
    pub fn show_belt_pull_off(&self, slot: usize) -> Result<(), String> {
        self.bursts
            .borrow_mut()
            .push((slot, crate::ui::belt_pull_off::BeltPullOffBurst::new(self.now)));
        Ok(())
    }
    fn coarse(&self) -> bool {
        self.document
            .default_view()
            .and_then(|window| {
                window.match_media("(hover:none) and (pointer:coarse)").ok().flatten()
            })
            .is_some_and(|query| query.matches())
    }
    fn action(
        &self,
        frame: &UiFrame<'_>,
        ui: &egui::Ui,
        action: HudAction,
        rect: egui::Rect,
        label: &str,
    ) -> egui::Response {
        self.action_rotated(frame, ui, action, rect, label, 0.0)
    }
    fn action_rotated(
        &self,
        frame: &UiFrame<'_>,
        ui: &egui::Ui,
        action: HudAction,
        rect: egui::Rect,
        label: &str,
        rotation: f32,
    ) -> egui::Response {
        let enabled = self.actions_enabled && !self.disabled_actions.borrow().contains(&action);
        let mut child = crate::player_panels::chrome::child_ui(ui, "hud-admission", ui.max_rect());
        if !(enabled) {
            child.disable();
        }
        let id = format!("hud:{}", action_id(action));
        let response = child.interact(
            rotated_bounds(rect, rect.center(), rotation),
            egui::Id::unique(&id),
            egui::Sense::click(),
        );
        frame.record_rotated_rect(
            &id,
            label,
            "button",
            &response,
            rect.center(),
            rect.size(),
            rotation,
        );
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect.expand(3.0),
                2.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(219, 197, 148)),
                egui::StrokeKind::Outside,
            );
        }
        let valid_pointer =
            ui.ctx().input(|input| input.pointer.interact_pos()).is_none_or(|pos| {
                let transform =
                    ui.ctx().layer_transform_to_global(ui.layer_id()).unwrap_or_default();
                let local = transform.inverse() * pos;
                let unrotated = rect.center()
                    + egui::emath::Rot2::from_angle(-rotation) * (local - rect.center());
                rect.contains(unrotated)
            });
        if response.clicked()
            && enabled
            && !matches!(action, HudAction::Belt(_))
            && (valid_pointer || !ui.ctx().input(|input| input.pointer.any_released()))
        {
            self.actions.borrow_mut().push(action);
        }
        response
    }
    pub fn belt_input_geometry(&self) -> Vec<HudBeltHit> {
        self.belt_hits.borrow().clone()
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        self.belt_hits.borrow_mut().clear();
        if !self.visible.get() {
            return Ok(());
        }
        let Some(state) = self.snapshot.as_ref() else { return Ok(()) };
        let scale = ((self.viewport.0 as f64 / 1600.0).min(self.viewport.1 as f64 / 900.0).min(1.0)
            * self.ui_scale) as f32;
        let size = egui::vec2(self.viewport.0 as f32 / scale, self.viewport.1 as f32 / scale);
        let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::unique("game-hud"));
        frame.ctx.set_transform_layer(layer, egui::emath::TSTransform::from_scaling(scale));
        let coarse = self.coarse();
        let mut error = None;
        egui::Area::new(layer.id)
            .order(layer.order)
            .fixed_pos(egui::Pos2::ZERO)
            .constrain(false)
            .show(frame.ctx, |ui| {
                ui.set_min_size(size);
                let result = self.paint_contents(frame, ui, state, size, scale, coarse);
                if let Err(value) = result {
                    error = Some(value);
                }
            });
        error.map_or(Ok(()), Err)
    }
    fn mobile_rect(
        &self,
        id: MobileUiElementId,
        natural: egui::Rect,
        size: egui::Vec2,
        scale: f32,
        coarse: bool,
    ) -> (egui::Rect, f32) {
        if !coarse {
            return (natural, 0.0);
        }
        let geometry = default_mobile_ui_geometry(
            self.viewport.0 as f64,
            self.viewport.1 as f64,
            self.ui_scale,
        );
        let transform = if self.mobile_layout.customized {
            self.mobile_layout.layout.get(id)
        } else {
            geometry.layout.get(id)
        };
        let dimensions = geometry.sizes[id as usize];
        let rect = egui::Rect::from_center_size(
            egui::pos2(size.x * transform.x as f32 / 100.0, size.y * transform.y as f32 / 100.0),
            egui::vec2(dimensions.width as f32 / scale, dimensions.height as f32 / scale)
                * transform.scale as f32,
        );
        (rect, transform.rotation.to_radians() as f32)
    }
    fn mobile_scale(&self, id: MobileUiElementId, coarse: bool) -> f32 {
        if coarse && self.mobile_layout.customized {
            self.mobile_layout.layout.get(id).scale as f32
        } else {
            1.0
        }
    }
    fn paint_contents(
        &self,
        frame: &UiFrame<'_>,
        ui: &egui::Ui,
        state: &HudState,
        size: egui::Vec2,
        scale: f32,
        coarse: bool,
    ) -> Result<(), String> {
        let p = ui.painter();
        let layout = native_hud_control_layout(size.x as f64, size.y as f64, None);
        let white = egui::Color32::WHITE;
        if self.tutorial_access.combat {
            let pause = egui::Rect::from_min_size(egui::pos2(11.0, 7.0), egui::vec2(31.0, 33.0));
            let (rect, rotation) =
                self.mobile_rect(MobileUiElementId::Pause, pause, size, scale, coarse);
            self.ui.paint_centered(
                frame,
                p,
                42,
                rect.center(),
                if coarse { rect.width() / 31.0 * 0.82 } else { 1.0 },
                rotation,
                white,
            )?;
            self.action_rotated(frame, ui, HudAction::Pause, rect, "Menu (Esc)", rotation);
            let (rect, rotation) = self.mobile_rect(
                MobileUiElementId::Diagnostics,
                egui::Rect::from_min_size(egui::pos2(50.0, 12.0), egui::vec2(160.0, 16.0)),
                size,
                scale,
                coarse,
            );
            let custom = self.mobile_scale(MobileUiElementId::Diagnostics, coarse);
            let font_size = if coarse { 12.0 * 0.85 / scale * custom } else { 12.0 };
            let diagnostics = format!("{}   {}", self.fps, self.ping);
            let painted = paint_rotated_text(
                p,
                &diagnostics,
                egui::FontId::new(font_size, egui::FontFamily::Name("JetBrains Mono".into())),
                egui::Color32::from_rgb(216, 212, 200),
                if coarse { rect.center() } else { rect.min },
                if coarse { egui::Align2::CENTER_CENTER } else { egui::Align2::LEFT_TOP },
                (rect.center(), rotation),
            );
            let transform = frame.ctx.layer_transform_to_global(ui.layer_id()).unwrap_or_default();
            frame.record_rect("hud:diagnostics", &diagnostics, "text", transform * painted, false);
            frame.record_value("hud:diagnostics", &diagnostics, None);
            self.paint_meters(frame, p, state, size, scale, coarse)?;
            let (rect, rotation) = self.mobile_rect(
                MobileUiElementId::Xp,
                egui::Rect::from_min_size(
                    egui::pos2(size.x / 2.0 - 2.0, size.y - 72.0),
                    egui::vec2(12.0, 56.0),
                ),
                size,
                scale,
                coarse,
            );
            let art_scale = rect.width() / 12.0;
            let rotated = frame.with_rotation(rect.center(), rotation);
            let record = self.ui.record(81)?;
            let [ox, oy] = record.registered_origin();
            let cut = ((1.0 - clamp_unit(state.experience_progress)) as f32
                * record.logical_height as f32
                - oy)
                .clamp(0.0, record.height as f32);
            if cut < record.height as f32 {
                let source = egui::Rect::from_min_size(
                    egui::pos2(0.0, cut),
                    egui::vec2(record.width as f32, record.height as f32 - cut),
                );
                let dest = egui::Rect::from_min_size(
                    rect.min + (egui::vec2(3.5, 4.0) + egui::vec2(ox, oy + cut)) * art_scale,
                    source.size() * art_scale,
                );
                self.ui.paint_slice(&rotated, p, 81, source, dest, white)?;
            }
            self.ui.paint(&rotated, p, 82, rect.min, art_scale, white)?;
            let transform = frame.ctx.layer_transform_to_global(p.layer_id()).unwrap_or_default();
            frame.record_rect(
                "hud:xp",
                &format!(
                    "Experience: {}%",
                    (clamp_unit(state.experience_progress) * 100.0).round()
                ),
                "progressbar",
                transform * rotated_bounds(rect, rect.center(), rotation),
                false,
            );
        }
        for (id, action, record, natural, label, show) in [
            (
                MobileUiElementId::Inventory,
                HudAction::Inventory,
                47,
                layout.backpack,
                format!("Inventory, {} gold (I)", state.gold),
                self.tutorial_access.inventory,
            ),
            (
                MobileUiElementId::Skillbook,
                HudAction::Skills,
                48,
                layout.tome,
                "Skills (K)".into(),
                self.tutorial_access.skills,
            ),
        ] {
            if !show {
                continue;
            }
            let (rect, rotation) = self.mobile_rect(id, rect(natural), size, scale, coarse);
            let art_scale = if coarse { rect.width() / 130.0 * 2.0 } else { 1.0 };
            let center = if coarse {
                egui::pos2(
                    rect.left() + 7.0 * rect.width() / 130.0 + 29.0 * art_scale,
                    rect.bottom() - 3.0 * rect.width() / 130.0 - 31.0 * art_scale,
                )
            } else {
                rect.center()
            };
            self.ui.paint_centered(frame, p, record, center, art_scale, rotation, white)?;
            self.action_rotated(frame, ui, action, rect, &label, rotation);
        }
        if self.tutorial_access.spell {
            let records = [
                (Some(state.primary_icon_record), 12),
                (state.concentration_icon_records[1], 20),
                (state.concentration_icon_records[0], 16),
            ];
            let count = records.iter().filter(|(r, _)| r.is_some()).count();
            let mut index = 0;
            for (record, binding) in records {
                let Some(record) = record else { continue };
                let x = size.x / 2.0 - 20.0 * (count as f32 - 1.0) + index as f32 * 40.0;
                self.skills.paint_centered(
                    frame,
                    p,
                    record,
                    egui::pos2(x, 25.5),
                    0.75,
                    0.0,
                    white.gamma_multiply(0.75),
                )?;
                self.action(
                    frame,
                    ui,
                    HudAction::SkillBinding(binding),
                    egui::Rect::from_min_size(egui::pos2(x - 20.0, -7.0), egui::vec2(40.0, 65.0)),
                    if binding == 12 { "Select primary attack" } else { "Select concentration" },
                );
                index += 1;
            }
        }
        if self.mode == HudMode::Hub {
            self.college.paint(frame, p, 37, egui::pos2(size.x - 41.0, 8.0), 1.0, white)?;
            if !coarse {
                for (index, (record, name)) in [
                    (0, "Provokatus"),
                    (6, "Hagatha"),
                    (4, "Luthacus"),
                    (5, "Fomentius"),
                    (2, "Shlorio"),
                ]
                .into_iter()
                .enumerate()
                {
                    let rect = egui::Rect::from_min_size(
                        egui::pos2(size.x - 100.0, 55.0 + index as f32 * 95.0),
                        egui::vec2(95.0, 93.0),
                    );
                    self.portraits.paint(frame, p, record, rect.min, 1.0, white)?;
                    self.action(frame, ui, HudAction::Shortcut(index), rect, name);
                }
            }
            let rect = egui::Rect::from_min_size(
                egui::pos2(size.x - 138.0, if coarse { 55.0 } else { size.y - 134.0 }),
                egui::vec2(121.0, 118.0),
            );
            for record in [16, 18] {
                self.college.paint(frame, p, record, rect.min, 1.0, white)?;
            }
            self.action(frame, ui, HudAction::Map, rect, "Enter the Boneyard");
        }
        if self.tutorial_access.quickbar {
            self.paint_belt(frame, ui, state, size, scale, coarse)?;
        }
        Ok(())
    }
    fn paint_meters(
        &self,
        frame: &UiFrame<'_>,
        p: &egui::Painter,
        state: &HudState,
        size: egui::Vec2,
        _scale: f32,
        coarse: bool,
    ) -> Result<(), String> {
        let health = native_health_hud_presentation(
            state.current_health,
            state.maximum_health,
            state.shield_current,
            state.shield_maximum,
        );
        let mana = native_mana_hud_presentation(
            state.current_mana,
            state.maximum_mana,
            state.reserved_mana,
        );
        let mut center = egui::pos2(size.x / 2.0, 24.5);
        let mut multiplier = 1.0;
        let mut rotation = 0.0;
        if coarse && self.mobile_layout.customized {
            let t = self.mobile_layout.layout.get(MobileUiElementId::Meters);
            center = egui::pos2(size.x * t.x as f32 / 100.0, size.y * t.y as f32 / 100.0);
            multiplier = t.scale as f32;
            rotation = t.rotation.to_radians() as f32;
        }
        let additive = frame.with_blend(solomon_graphics::BlendMode::Add);
        let rotated = additive.with_rotation(center, rotation);
        for (health_meter, track, core, label) in [
            (true, health.track_width, health.core_width, "Health"),
            (false, mana.track_width, mana.core_width, "Mana"),
        ] {
            let x = if health_meter {
                center.x - (50.0 + track as f32) * multiplier
            } else {
                center.x + 50.0 * multiplier
            };
            let r = egui::Rect::from_min_size(
                egui::pos2(x, center.y - 10.0 * multiplier),
                egui::vec2(track as f32 * multiplier, 20.0 * multiplier),
            );
            self.ui.paint_strip(&rotated, p, 70, r, egui::Color32::WHITE)?;
            let fill = egui::Rect::from_min_size(
                r.min + egui::vec2(5.0, 5.0) * multiplier,
                egui::vec2(core as f32, 10.0) * multiplier,
            );
            if health_meter {
                for layer in native_health_hud_layers(
                    health.fill_progress,
                    health.shield_progress,
                    state.poisoned,
                ) {
                    let clip = egui::Rect::from_min_size(
                        fill.min,
                        egui::vec2(fill.width() * layer.progress as f32, fill.height()),
                    );
                    self.ui.paint_strip_clipped(
                        &rotated,
                        p,
                        layer.record,
                        fill,
                        clip,
                        if layer.kind == HudHealthLayerKind::Shield {
                            egui::Color32::from_rgb(128, 255, 255)
                        } else {
                            egui::Color32::WHITE
                        },
                    )?;
                }
            } else {
                let clip = egui::Rect::from_min_size(
                    fill.min,
                    egui::vec2(fill.width() * mana.fill_progress as f32, fill.height()),
                );
                self.ui.paint_strip_clipped(&rotated, p, 40, fill, clip, egui::Color32::WHITE)?;
                if mana.reserve_progress > 0.0 {
                    let reserve = egui::Rect::from_min_size(
                        egui::pos2(
                            r.right() - 5.0 * multiplier - mana.reserve_width as f32 * multiplier,
                            fill.top(),
                        ),
                        egui::vec2(mana.reserve_width as f32 * multiplier, fill.height()),
                    );
                    self.ui.paint_strip(&rotated, p, 41, reserve, egui::Color32::WHITE)?;
                }
            }
            let id = if health_meter { "hud:health" } else { "hud:mana" };
            let transform = frame.ctx.layer_transform_to_global(p.layer_id()).unwrap_or_default();
            frame.record_rect(
                id,
                label,
                "meter",
                transform * rotated_bounds(r, center, rotation),
                false,
            );
            frame.record_value(
                id,
                &if health_meter { state.current_health } else { state.current_mana }.to_string(),
                None,
            );
        }
        Ok(())
    }
    fn paint_belt(
        &self,
        frame: &UiFrame<'_>,
        ui: &egui::Ui,
        state: &HudState,
        size: egui::Vec2,
        scale: f32,
        coarse: bool,
    ) -> Result<(), String> {
        let p = ui.painter();
        let mut belt = [HudRect::new(0.0, 0.0, 53.0, 53.0); 8];
        let health = state
            .belt
            .iter()
            .position(|entry| matches!(entry, Some(HudBeltEntry::HealthPotion { .. })));
        let mana = state
            .belt
            .iter()
            .position(|entry| matches!(entry, Some(HudBeltEntry::ManaPotion { .. })));
        for (slot, entry) in state.belt.iter().enumerate() {
            let natural = egui::Rect::from_min_size(
                egui::pos2(
                    size.x / 2.0
                        + [-332.0, -272.0, -212.0, -152.0, 98.0, 158.0, 218.0, 278.0][slot],
                    size.y - 67.5,
                ),
                egui::vec2(53.0, 53.0),
            );
            let mobile_id = if health == Some(slot) {
                MobileUiElementId::HealthPotion
            } else if mana == Some(slot) {
                MobileUiElementId::ManaPotion
            } else {
                MobileUiElementId::SLOTS[slot]
            };
            let (rect, rotation) = self.mobile_rect(mobile_id, natural, size, scale, coarse);
            belt[slot] = HudRect::new(
                rect.left() as f64,
                rect.top() as f64,
                rect.width() as f64,
                rect.height() as f64,
            );
            if entry.is_none() && self.controller_belt_slot != Some(slot) {
                continue;
            }
            let control_scale = self.mobile_scale(mobile_id, coarse);
            let icon_frame = frame.with_rotation(rect.center(), rotation);
            if coarse {
                paint_rotated_rect(
                    p,
                    rect,
                    14.0 * control_scale,
                    rotation,
                    egui::Color32::from_rgba_unmultiplied(9, 13, 18, 158),
                    egui::Stroke::new(
                        2.0 * control_scale,
                        egui::Color32::from_rgba_unmultiplied(226, 216, 181, 107),
                    ),
                    egui::StrokeKind::Inside,
                );
            }
            if self.controller_belt_slot == Some(slot)
                || self.quickbar[slot].as_ref().is_some_and(|row| row.active)
            {
                paint_rotated_rect(
                    p,
                    rect.expand(4.0 * control_scale),
                    6.0 * control_scale,
                    rotation,
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(2.0 * control_scale, egui::Color32::from_rgb(231, 193, 105)),
                    egui::StrokeKind::Outside,
                );
            }
            let Some(entry) = entry else {
                let mut disabled = crate::player_panels::chrome::child_ui(
                    ui,
                    "empty-controller-belt-slot",
                    ui.max_rect(),
                );
                disabled.disable();
                let id = format!("hud:belt:{}", self.hotbar.slot(slot));
                let response = disabled.interact(
                    rotated_bounds(rect, rect.center(), rotation),
                    egui::Id::unique(&id),
                    egui::Sense::click(),
                );
                frame.record_rotated_rect(
                    &id,
                    "Empty belt slot, controller selected",
                    "button",
                    &response,
                    rect.center(),
                    rect.size(),
                    rotation,
                );
                continue;
            };
            let row = self.quickbar[slot].as_ref();
            let (icon_record, skill, count, unavailable, label) = match entry {
                HudBeltEntry::Skill { record, unavailable } => {
                    (Some(*record), true, 0, *unavailable, "Secondary skill")
                }
                HudBeltEntry::HealthPotion { count } => {
                    (Some(46), false, *count, *count == 0, "Health Potion")
                }
                HudBeltEntry::ManaPotion { count } => {
                    (Some(47), false, *count, *count == 0, "Mana Potion")
                }
                HudBeltEntry::Item { record, count } => (*record, false, *count, false, "Item"),
            };
            let unavailable =
                row.map_or(unavailable, |row| row.combat_disabled || row.insufficient_mana);
            let alpha = row.and_then(|row| row.icon_alpha).unwrap_or(if unavailable {
                0.35
            } else if skill {
                0.75
            } else {
                1.0
            }) as f32;
            let tint = if skill && self.mode == HudMode::Hub {
                egui::Color32::from_gray(64)
            } else {
                egui::Color32::WHITE
            }
            .gamma_multiply(alpha);
            if let Some(layers) = &self.mod_belt_icons[slot] {
                crate::ui::mod_icon::paint_mod_item_icon_layers(
                    &icon_frame,
                    p,
                    layers,
                    rect.center(),
                    control_scale,
                );
            } else if let Some(record) = icon_record {
                if skill {
                    self.skills.paint_centered(
                        &icon_frame,
                        p,
                        record,
                        rect.center(),
                        control_scale,
                        0.0,
                        tint,
                    )?;
                } else {
                    self.inventory.paint_centered(
                        &icon_frame,
                        p,
                        record,
                        rect.center(),
                        control_scale,
                        0.0,
                        tint,
                    )?;
                }
            }
            if let Some(row) = row
                && row.cooldown.remaining > 0.0
                && row.cooldown.capacity > 0.0
            {
                let points = crate::gpu_ui::cooldown_sector_points(
                    row.cooldown.remaining,
                    row.cooldown.capacity,
                );
                let overlay = egui::Rect::from_center_size(
                    rect.center(),
                    egui::Vec2::splat(53.0 * control_scale),
                );
                let rot = egui::emath::Rot2::from_angle(rotation);
                let mut mesh = egui::Mesh::default();
                let fill = egui::Color32::from_rgba_unmultiplied(128, 26, 26, 191);
                mesh.colored_vertex(rect.center(), fill);
                for point in &points {
                    let point = overlay.min + point.to_vec2() * control_scale;
                    mesh.colored_vertex(rect.center() + rot * (point - rect.center()), fill);
                }
                for index in 1..points.len() {
                    mesh.add_triangle(0, index as u32, index as u32 + 1);
                }
                p.add(egui::Shape::mesh(mesh));
            }
            let count = row.and_then(|row| row.quantity).unwrap_or(count as u64);
            if count > 1 {
                paint_rotated_text(
                    p,
                    &count.to_string(),
                    egui::FontId::new(
                        12.0 * control_scale,
                        egui::FontFamily::Name("Georgia".into()),
                    ),
                    egui::Color32::from_rgb(244, 229, 180),
                    rect.right_bottom() - egui::vec2(1.0, 1.0) * control_scale,
                    egui::Align2::RIGHT_BOTTOM,
                    (rect.center(), rotation),
                );
            }
            if !coarse {
                self.paint_binding(frame, p, slot, rect)?;
            }
            let response = self.action_rotated(
                frame,
                ui,
                HudAction::Belt(self.hotbar.slot(slot)),
                rect,
                &row.map_or_else(
                    || format!("{label}, belt slot {}", slot + 1),
                    |row| row.label.clone(),
                ),
                rotation,
            );
            let transform = frame.ctx.layer_transform_to_global(ui.layer_id()).unwrap_or_default();
            self.belt_hits.borrow_mut().push(HudBeltHit {
                slot: self.hotbar.slot(slot),
                face: transform * rect,
                rotation,
                input_scale: scale as f64 * control_scale as f64,
                cast_eligible: !row.is_some_and(|row| row.combat_disabled),
            });
            frame.record_value(&format!("hud:belt:{}",self.hotbar.slot(slot)),&serde_json::json!({"inputScale":scale as f64*control_scale as f64,"castEligible":!row.is_some_and(|row|row.combat_disabled),"authoritySlot":self.hotbar.slot(slot),"count":count}).to_string(),None);
            if slot == 0 {
                frame.record_rect(
                    "hud:secondary-slot",
                    label,
                    "button",
                    transform * response.interact_rect,
                    response.enabled(),
                );
            }
            if health == Some(slot) {
                frame.record_rect(
                    "hud:health-potion",
                    label,
                    "button",
                    transform * response.interact_rect,
                    response.enabled(),
                );
            }
        }
        self.hotbar_controls.paint(frame, ui, &belt, coarse, size);
        self.bursts.borrow_mut().retain(|(_, burst)| !burst.finished(self.now));
        for (slot, burst) in self.bursts.borrow().iter() {
            if *slot / 8 == self.hotbar.bank() {
                let visible = slot % 8;
                let id = if health == Some(visible) {
                    MobileUiElementId::HealthPotion
                } else if mana == Some(visible) {
                    MobileUiElementId::ManaPotion
                } else {
                    MobileUiElementId::SLOTS[visible]
                };
                let angle = if coarse && self.mobile_layout.customized {
                    self.mobile_layout.layout.get(id).rotation.to_radians() as f32
                } else {
                    0.0
                };
                let center = rect(belt[visible]).center();
                burst.paint(
                    frame,
                    p,
                    &self.ui,
                    crate::ui::belt_pull_off::BeltPullOffDraw {
                        center,
                        now: self.now,
                        scale: self.mobile_scale(id, coarse),
                        rotation: angle,
                        id: &format!("hud:belt-burst:{slot}:{}", burst.sequence()),
                    },
                )?;
            }
        }
        Ok(())
    }
    fn paint_binding(
        &self,
        frame: &UiFrame<'_>,
        p: &egui::Painter,
        slot: usize,
        rect: egui::Rect,
    ) -> Result<(), String> {
        let code = self.quickbar[slot].as_ref().map_or_else(
            || if slot == 0 { "Mouse2".into() } else { format!("Digit{slot}") },
            |row| row.binding_code.clone(),
        );
        if code == "Mouse2" {
            return self.ui.paint(
                frame,
                p,
                100,
                rect.min + egui::vec2(15.5, 44.5),
                1.0,
                egui::Color32::WHITE.gamma_multiply(0.6),
            );
        }
        let label = self.quickbar[slot]
            .as_ref()
            .map_or_else(
                || solomon_sim::settings::game_binding_label(&code),
                |row| row.binding_label.clone(),
            )
            .to_uppercase();
        let binding = solomon_sim::hud::quickbar::layout_native_quickbar_binding(&label);
        let backing = egui::Rect::from_min_size(
            rect.min + egui::vec2(binding.backing_left as f32, 53.0),
            egui::vec2(binding.backing_width as f32, 15.0),
        );
        for (dest_x, width, source_x) in [
            (0.0, 5.0, 0.0),
            (5.0, (backing.width() - 10.0).max(0.0), 5.0),
            (backing.width() - 5.0, 5.0, 10.0),
        ] {
            frame.image(
                p,
                "assets/stock/hub-hud-key-backing.png",
                egui::Rect::from_min_size(
                    backing.min + egui::vec2(dest_x, 0.0),
                    egui::vec2(width, 15.0),
                ),
                Some(egui::Rect::from_min_max(
                    egui::pos2(source_x / 15.0, 0.0),
                    egui::pos2((source_x + 5.0) / 15.0, 1.0),
                )),
                egui::Color32::WHITE.gamma_multiply(0.6),
            );
        }
        let mut layout = binding.text;
        for glyph in &mut layout.glyphs {
            glyph.center_x += rect.min.x as f64;
            glyph.center_y += rect.min.y as f64;
        }
        crate::gpu_ui::paint_text_layout(frame, p, &layout);
        Ok(())
    }
}
fn rect(r: HudRect) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(r.x as f32, r.y as f32),
        egui::vec2(r.width as f32, r.height as f32),
    )
}
fn action_id(action: HudAction) -> String {
    match action {
        HudAction::Pause => "pause".into(),
        HudAction::Inventory => "inventory".into(),
        HudAction::Skills => "skills".into(),
        HudAction::Map => "map".into(),
        HudAction::Shortcut(i) => format!("shortcut:{i}"),
        HudAction::Belt(i) => format!("belt:{i}"),
        HudAction::SkillBinding(i) => format!("skill-binding:{i}"),
    }
}

fn rotated_bounds(rect: egui::Rect, pivot: egui::Pos2, angle: f32) -> egui::Rect {
    let rotation = egui::emath::Rot2::from_angle(angle);
    [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()]
        .into_iter()
        .map(|point| pivot + rotation * (point - pivot))
        .fold(egui::Rect::NOTHING, |bounds, point| {
            bounds.union(egui::Rect::from_min_max(point, point))
        })
}
fn paint_rotated_rect(
    painter: &egui::Painter,
    rect: egui::Rect,
    radius: f32,
    angle: f32,
    fill: egui::Color32,
    stroke: egui::Stroke,
    kind: egui::StrokeKind,
) {
    let mut shape = egui::epaint::RectShape::new(rect, radius, fill, stroke, kind);
    shape.angle = angle;
    painter.add(shape);
}
fn paint_rotated_text(
    painter: &egui::Painter,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
    anchor: egui::Pos2,
    align: egui::Align2,
    rotation: (egui::Pos2, f32),
) -> egui::Rect {
    let (pivot, angle) = rotation;
    let galley = painter.layout_no_wrap(text.into(), font, color);
    let rect = align.anchor_size(anchor, galley.size());
    let painted = rotated_bounds(galley.mesh_bounds.translate(rect.min.to_vec2()), pivot, angle);
    let position = pivot + egui::emath::Rot2::from_angle(angle) * (rect.min - pivot);
    painter.add(egui::epaint::TextShape::new(position, galley, color).with_angle(angle));
    painted
}
