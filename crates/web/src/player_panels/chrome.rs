//! Native panel painting and hit regions. This canvas is a direct GPU painter in
//! native design coordinates; simulation actions remain in `PlayerPanels`.
use super::{
    PanelEvent, PlayerPanel, PlayerPanelSnapshot, PlayerPanels, Target, target_from_action,
};
use crate::gpu_ui::{StockAtlas, UiFrame};
use egui::{Color32, Pos2, Rect, Vec2};
use solomon_sim::economy::{EquipmentType, HubInventoryItem};
use solomon_sim::hub::interaction::HubNpcSelectorId;
use solomon_sim::hud::HudRect;
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextLayout, NativeUiTextSpec};
use std::cell::RefCell;

#[derive(Clone)]
pub(super) struct PanelHit {
    pub target: Target,
    pub npc: Option<HubNpcSelectorId>,
    pub rect: Rect,
    pub enabled: bool,
}

pub(crate) struct PanelCanvas<'a, 'gpu> {
    pub frame: &'a UiFrame<'gpu>,
    pub ui: &'a egui::Ui,
    pub painter: egui::Painter,
    pub origin: Vec2,
    pub alpha: f32,
    pub enabled: bool,
    direct: bool,
    blend: solomon_graphics::BlendMode,
    namespace: &'static str,
    hits: &'a RefCell<Vec<PanelHit>>,
    events: &'a RefCell<Vec<PanelEvent>>,
}
impl<'a, 'gpu> PanelCanvas<'a, 'gpu> {
    pub(super) fn new(
        frame: &'a UiFrame<'gpu>,
        ui: &'a egui::Ui,
        hits: &'a RefCell<Vec<PanelHit>>,
        events: &'a RefCell<Vec<PanelEvent>>,
        alpha: f32,
        enabled: bool,
    ) -> Self {
        Self {
            frame,
            ui,
            painter: ui.painter().clone(),
            origin: Vec2::ZERO,
            alpha,
            enabled,
            hits,
            events,
            direct: false,
            blend: solomon_graphics::BlendMode::Normal,
            namespace: "panel",
        }
    }
    pub fn at(&self, x: f64, y: f64) -> Self {
        Self {
            frame: self.frame,
            ui: self.ui,
            painter: self.painter.clone(),
            origin: self.origin + egui::vec2(x as f32, y as f32),
            alpha: self.alpha,
            enabled: self.enabled,
            hits: self.hits,
            events: self.events,
            direct: self.direct,
            blend: self.blend,
            namespace: self.namespace,
        }
    }
    pub fn on_ui<'b>(&'b self, ui: &'b egui::Ui) -> PanelCanvas<'b, 'gpu> {
        PanelCanvas {
            frame: self.frame,
            ui,
            painter: ui.painter().clone(),
            origin: Vec2::ZERO,
            alpha: self.alpha,
            enabled: self.enabled,
            hits: self.hits,
            events: self.events,
            direct: self.direct,
            blend: self.blend,
            namespace: self.namespace,
        }
    }
    pub fn namespace(&self, namespace: &'static str) -> Self {
        let mut c = self.at(0.0, 0.0);
        c.namespace = namespace;
        c
    }
    pub fn direct(&self) -> Self {
        let mut c = self.at(0.0, 0.0);
        c.direct = true;
        c
    }
    pub fn additive(&self) -> Self {
        let mut c = self.at(0.0, 0.0);
        c.blend = solomon_graphics::BlendMode::Add;
        c
    }
    pub fn target_rect(&self, target: Target) -> Option<Rect> {
        self.hits
            .borrow()
            .iter()
            .find(|hit| hit.target == target && hit.rect.is_positive())
            .map(|hit| hit.rect)
    }
    pub fn faded(&self, alpha: f32) -> Self {
        let mut c = self.at(0.0, 0.0);
        c.alpha *= alpha;
        c
    }
    pub fn admitted(&self, enabled: bool) -> Self {
        let mut c = self.at(0.0, 0.0);
        c.enabled &= enabled;
        c
    }
    pub fn clip(&self, r: HudRect) -> Self {
        let mut c = self.at(0.0, 0.0);
        c.painter = c.painter.with_clip_rect(self.rect(r));
        c
    }
    pub fn rect(&self, r: HudRect) -> Rect {
        rect(r).translate(self.origin)
    }
    pub fn color(&self, tint: u32) -> Color32 {
        color(tint).gamma_multiply(self.alpha)
    }
    pub fn fill(&self, r: HudRect, tint: u32, alpha: f32) {
        self.painter.rect_filled(self.rect(r), 0.0, self.color(tint).gamma_multiply(alpha));
    }
    pub fn stroke(&self, r: HudRect, tint: u32, width: f32) {
        self.painter.rect_stroke(
            self.rect(r),
            0.0,
            egui::Stroke::new(width, self.color(tint)),
            egui::StrokeKind::Inside,
        );
    }
    pub fn text(&self, spec: &NativeUiTextSpec<'_>) {
        crate::gpu_ui::native_text(
            self.frame,
            &self.painter,
            &NativeUiTextSpec {
                x: spec.x + self.origin.x as f64,
                y: spec.y + self.origin.y as f64,
                alpha: spec.alpha * self.alpha as f64,
                ..*spec
            },
        );
    }
    pub fn layout(&self, layout: &NativeUiTextLayout) {
        let mut layout = layout.clone();
        for glyph in &mut layout.glyphs {
            glyph.center_x += self.origin.x as f64;
            glyph.center_y += self.origin.y as f64;
            glyph.alpha *= self.alpha as f64;
        }
        crate::gpu_ui::paint_text_layout(self.frame, &self.painter, &layout);
    }
    pub fn shadow_text(&self, spec: &NativeUiTextSpec<'_>) {
        self.text(&NativeUiTextSpec { x: spec.x + 1.0, y: spec.y + 1.0, tint: 0, ..*spec });
        self.text(spec);
    }
    pub fn browser_text(&self, text: &str, x: f64, baseline: f64, size: f32, tint: u32) {
        self.painter.text(
            egui::pos2(x as f32, baseline as f32) + self.origin,
            egui::Align2::CENTER_BOTTOM,
            text,
            egui::FontId::new(size, egui::FontFamily::Name("Georgia".into())),
            self.color(tint),
        );
    }
    pub fn art(
        &self,
        atlas: &StockAtlas,
        index: usize,
        center: [f64; 2],
        scale: [f64; 2],
        degrees: f64,
        tint: u32,
    ) -> Result<Rect, String> {
        let r = atlas.record(index)?;
        let Some(size) = self.frame.image_size(atlas.path()) else {
            return Err(format!("{} not ready", atlas.path()));
        };
        let [left, top] = r.registered_origin();
        let left = left as f64 - r.logical_width as f64 / 2.0;
        let top = top as f64 - r.logical_height as f64 / 2.0;
        let angle = degrees.to_radians();
        let (sin, cos) = angle.sin_cos();
        let point = |x: f64, y: f64| {
            let x = x * scale[0];
            let y = y * scale[1];
            egui::pos2(
                (center[0] + x * cos - y * sin) as f32,
                (center[1] + x * sin + y * cos) as f32,
            ) + self.origin
        };
        let positions = [
            point(left, top),
            point(left + r.width as f64, top),
            point(left + r.width as f64, top + r.height as f64),
            point(left, top + r.height as f64),
        ];
        self.frame.image_quad_blended(
            &self.painter,
            atlas.path(),
            positions,
            Rect::from_min_max(
                egui::pos2(r.x as f32 / size.x, r.y as f32 / size.y),
                egui::pos2((r.x + r.width) as f32 / size.x, (r.y + r.height) as f32 / size.y),
            ),
            self.color(tint),
            self.blend,
        );
        Ok(positions
            .into_iter()
            .fold(Rect::NOTHING, |bounds, point| bounds.union(Rect::from_min_max(point, point))))
    }
    pub fn split_art(
        &self,
        atlas: &StockAtlas,
        index: usize,
        center: [f64; 2],
        scale: [f64; 2],
        tints: [u32; 2],
    ) -> Result<(), String> {
        paint_split_art(
            self.frame,
            &self.painter,
            atlas,
            index,
            egui::pos2(center[0] as f32, center[1] as f32) + self.origin,
            scale[0] as f32,
            tints,
            self.alpha,
        )
    }
    pub fn mod_frame(
        &self,
        url: &str,
        dimensions: [u32; 2],
        frame: solomon_sim::modding::assets::ModSpriteFrame,
        center: [f64; 2],
        mirrored: bool,
        tint: u32,
    ) {
        let x = center[0] - frame.width / 2.0 + frame.center_offset_x;
        let y = center[1] - frame.height / 2.0 + frame.center_offset_y;
        let mut destination = self.rect(HudRect::new(x, y, frame.width, frame.height));
        if mirrored {
            let cx = center[0] as f32 + self.origin.x;
            destination = Rect::from_min_max(
                egui::pos2(2.0 * cx - destination.right(), destination.top()),
                egui::pos2(2.0 * cx - destination.left(), destination.bottom()),
            );
        }
        let mut uv = Rect::from_min_max(
            egui::pos2(
                frame.x as f32 / dimensions[0] as f32,
                frame.y as f32 / dimensions[1] as f32,
            ),
            egui::pos2(
                (frame.x + frame.width) as f32 / dimensions[0] as f32,
                (frame.y + frame.height) as f32 / dimensions[1] as f32,
            ),
        );
        if mirrored {
            std::mem::swap(&mut uv.min.x, &mut uv.max.x);
        }
        self.frame.image(&self.painter, url, destination, Some(uv), self.color(tint));
    }
    pub fn raw(
        &self,
        atlas: &StockAtlas,
        index: usize,
        r: HudRect,
        tint: u32,
    ) -> Result<(), String> {
        atlas.paint_raw(
            &self.frame.with_blend(self.blend),
            &self.painter,
            index,
            self.rect(r),
            self.color(tint),
        )
    }
    pub fn tiles(
        &self,
        atlas: &StockAtlas,
        index: usize,
        r: HudRect,
        scale: f32,
        tint: u32,
    ) -> Result<(), String> {
        atlas.paint_tiles(
            &self.frame.with_blend(self.blend),
            &self.painter,
            index,
            self.rect(r),
            scale,
            self.color(tint),
        )
    }
    pub fn frame(
        &self,
        atlas: &StockAtlas,
        index: usize,
        r: HudRect,
        border: f32,
        tint: u32,
    ) -> Result<(), String> {
        atlas.paint_frame(
            &self.frame.with_blend(self.blend),
            &self.painter,
            index,
            self.rect(r),
            border,
            self.color(tint),
        )
    }
    pub fn native_frame(
        &self,
        atlas: &StockAtlas,
        index: usize,
        r: HudRect,
        fill: bool,
        tint: u32,
    ) -> Result<(), String> {
        atlas.paint_native_frame(
            &self.frame.with_blend(self.blend),
            &self.painter,
            index,
            self.rect(r),
            fill,
            self.color(tint),
        )
    }
    pub fn semantic(&self, id: &str, label: &str, role: &'static str, r: HudRect) {
        let local = self.rect(r).intersect(self.painter.clip_rect());
        let transform =
            self.frame.ctx.layer_transform_to_global(self.painter.layer_id()).unwrap_or_default();
        self.frame.record_rect(id, label, role, transform * local, false);
    }
    pub fn semantic_value(
        &self,
        id: &str,
        label: &str,
        role: &'static str,
        r: HudRect,
        value: &str,
    ) {
        self.semantic(id, label, role, r);
        self.frame.record_value(id, value, None);
    }
    pub fn button(&self, action: &str, label: &str, r: HudRect) -> Self {
        self.npc_button_named(action, None, label, r, None)
    }
    pub fn named_button(&self, action: &str, id: &str, label: &str, r: HudRect) -> Self {
        self.npc_button_named(action, Some(id), label, r, None)
    }
    pub fn npc_button(
        &self,
        action: &str,
        label: &str,
        r: HudRect,
        npc: Option<HubNpcSelectorId>,
    ) -> Self {
        self.npc_button_named(action, None, label, r, npc)
    }
    fn npc_button_named(
        &self,
        action: &str,
        id: Option<&str>,
        label: &str,
        r: HudRect,
        npc: Option<HubNpcSelectorId>,
    ) -> Self {
        let target = target_from_action(action).expect("painted panel action is defined");
        let rect = self.rect(r);
        let visible = rect.intersect(self.painter.clip_rect());
        let mut child = child_ui(self.ui, "panel-admission", self.ui.max_rect());
        if !(self.enabled) {
            child.disable();
        }
        child.set_clip_rect(self.painter.clip_rect());
        let id = if let Some(id) = id {
            id.to_owned()
        } else if let Some(HubNpcSelectorId::ModBoast(selection)) = &npc {
            format!("{}:npc:{}:{}", self.namespace, selection.mod_id, selection.content_id)
        } else if target == Target::ShopCell {
            format!("{}:shop-cell:{}:{}", self.namespace, rect.min.x, rect.min.y)
        } else {
            format!("{}:{action}", self.namespace)
        };
        let response =
            child.interact(visible, egui::Id::unique(&id), egui::Sense::click_and_drag());
        self.hits.borrow_mut().push(PanelHit {
            target,
            npc: npc.clone(),
            rect: visible,
            enabled: self.enabled,
        });
        if response.gained_focus() {
            if let Some(npc) = npc.clone() {
                self.events.borrow_mut().push(PanelEvent::FocusNpc(npc));
            } else {
                self.events.borrow_mut().push(PanelEvent::Focus(target));
            }
        }
        if response.clicked()
            && (self.direct || !self.ui.ctx().input(|input| input.pointer.any_released()))
        {
            if let Some(npc) = npc {
                self.events.borrow_mut().push(PanelEvent::SelectNpc(npc));
            } else {
                self.events.borrow_mut().push(PanelEvent::Activate(target));
            }
        }
        if response.has_focus() {
            self.painter.rect_stroke(
                visible,
                2.0,
                egui::Stroke::new(1.0, self.color(0xdbc594)),
                egui::StrokeKind::Outside,
            );
        }
        self.frame.record(&id, label, "button", &response);
        self.at(r.x, r.y)
    }
    pub fn native_button_art(
        &self,
        _atlas: &StockAtlas,
        action: &str,
        label: &str,
        r: HudRect,
    ) -> Result<Self, String> {
        let c = self.button(action, label, r);
        let id = format!("{}:{action}", self.namespace);
        let response = self
            .ui
            .ctx()
            .read_response(egui::Id::unique(&id))
            .expect("native button owns the actual response");
        let mut art_ui = child_ui(self.ui, "native-button-art", self.ui.max_rect());
        art_ui.set_clip_rect(self.painter.clip_rect());
        art_ui.set_opacity(self.alpha);
        crate::menu::widgets::paint_stock_button(
            self.frame,
            &art_ui,
            &response,
            self.rect(r),
            r.height as f32 / 69.0,
            self.enabled,
        );
        Ok(c)
    }
    pub fn native_button(
        &self,
        atlas: &StockAtlas,
        action: &str,
        label: &str,
        r: HudRect,
    ) -> Result<Self, String> {
        let c = self.native_button_art(atlas, action, label, r)?;
        c.text(&NativeUiTextSpec {
            tint: 0xd9ba70,
            ..NativeUiTextSpec::new(
                NativeUiFontName::Menu,
                label,
                r.width / 2.0,
                r.height / 2.0 + 9.0,
            )
        });
        Ok(c)
    }
}
pub(crate) fn rect(r: HudRect) -> Rect {
    Rect::from_min_size(
        egui::pos2(r.x as f32, r.y as f32),
        egui::vec2(r.width as f32, r.height as f32),
    )
}
pub(crate) fn color(tint: u32) -> Color32 {
    Color32::from_rgb((tint >> 16) as u8, (tint >> 8) as u8, tint as u8)
}
impl PlayerPanels {
    pub(super) fn draw_item(
        &self,
        c: &PanelCanvas<'_, '_>,
        item: &HubInventoryItem,
        x: f64,
        y: f64,
        quantity: u64,
    ) -> Result<Rect, String> {
        let mut bounds = Rect::NOTHING;
        if let Some(layers) = self.mod_icons.get(&item.id) {
            let mut painter = c.painter.clone();
            painter.multiply_opacity(c.alpha);
            for layer in layers {
                bounds = bounds.union(
                    layer
                        .geometry(egui::pos2(x as f32, y as f32) + c.origin, 1.0, false)
                        .destination,
                );
            }
            crate::ui::mod_icon::paint_mod_item_icon_layers(
                c.frame,
                &painter,
                layers,
                egui::pos2(x as f32, y as f32) + c.origin,
                1.0,
            );
        } else {
            let (offset, rotation) = match item.equipment_type {
                Some(EquipmentType::Amulet) => ([0.0, -5.0], 0.0),
                Some(EquipmentType::Staff) => ([-22.94306, 32.76608], 35.0),
                Some(EquipmentType::Wand) => ([0.0, 0.0], 45.0),
                _ => ([0.0, 0.0], 0.0),
            };
            let tints = item.clothing_tints();
            for (layer, record) in item.icon_records.iter().enumerate() {
                let tint = if matches!(
                    item.equipment_type,
                    Some(EquipmentType::Hat | EquipmentType::Robe)
                ) {
                    tints.and_then(|colors| colors.get(layer).copied()).unwrap_or(0xffffff)
                } else {
                    0xffffff
                };
                bounds = bounds.union(c.art(
                    &self.inventory,
                    *record,
                    [x + offset[0], y + offset[1]],
                    [1.0, 1.0],
                    rotation,
                    tint,
                )?);
            }
        }
        if quantity > 1 {
            c.text(&NativeUiTextSpec {
                tint: 0xf4e5b4,
                ..NativeUiTextSpec::new(
                    NativeUiFontName::Medium,
                    &quantity.to_string(),
                    x + 25.0,
                    y + 18.0,
                )
            });
        }
        Ok(bounds)
    }
    pub(super) fn record_mod_item(
        &self,
        c: &PanelCanvas<'_, '_>,
        item: &HubInventoryItem,
        x: f64,
        y: f64,
        bounds: Rect,
    ) {
        let Some(layers) = self.mod_icons.get(&item.id) else { return };
        let Some(first) = layers.first() else { return };
        let frame = first.frame();
        let value = serde_json::json!({"url":first.url(),"frame":[frame.x,frame.y,frame.width,frame.height],"dimensions":first.dimensions(),"layers":layers.iter().map(|layer|{let geometry=layer.geometry(egui::pos2(x as f32,y as f32)+c.origin,1.0,false);serde_json::json!({"url":layer.url(),"rect":[geometry.destination.left(),geometry.destination.top(),geometry.destination.width(),geometry.destination.height()],"uv":[geometry.uv.left(),geometry.uv.top(),geometry.uv.right(),geometry.uv.bottom()]})}).collect::<Vec<_>>()});
        let transform =
            c.frame.ctx.layer_transform_to_global(c.painter.layer_id()).unwrap_or_default();
        let id = format!("{}:mod-icon:{}", c.namespace, item.id);
        c.frame.record_rect(
            &id,
            &item.name,
            "image",
            transform * bounds.intersect(c.painter.clip_rect()),
            false,
        );
        c.frame.record_value(&id, &value.to_string(), None);
    }
    pub(super) fn draw_footer(
        &self,
        c: &PanelCanvas<'_, '_>,
        snapshot: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        use solomon_sim::belt::{
            NativeBeltEntry, NativeBeltPotionKind, native_belt_entry_item,
            native_belt_potion_projection,
        };
        let hud = solomon_sim::hud::native_hud_control_layout(1600.0, 900.0, Some(1.0));
        for (record, r, action, label) in [
            (
                47,
                hud.backpack,
                if self.panel == Some(PlayerPanel::Inventory) { "close" } else { "inventory" },
                "Open or close inventory",
            ),
            (
                48,
                hud.tome,
                if self.panel == Some(PlayerPanel::Skills) { "close" } else { "skills" },
                "Open or close skills",
            ),
        ] {
            c.art(&self.ui, record, [r.x + 29.0, r.y + 31.0], [1.0, 1.0], 0.0, 0xffffff)?;
            if self.compact {
                c.named_button(action, &format!("panel:footer:{action}"), label, r);
            } else {
                c.button(action, label, r);
            }
        }
        c.art(&self.ui, 82, [800.5, 872.0], [1.0, 1.0], 0.0, 0xffffff)?;
        for (visible, r) in hud.belt.iter().enumerate() {
            let slot = self.hotbar.slot(visible);
            let [x, y] = [r.x + r.width / 2.0, r.y + r.height / 2.0];
            c.art(&self.ui, 2, [x, y], [1.0, 1.0], 0.0, 0xffffff)?;
            let entry = snapshot.belt.entries[slot];
            if let Some(NativeBeltEntry::Skill { skill_id }) = entry {
                c.art(
                    &self.skills,
                    snapshot.book.skill_icon_record(skill_id),
                    [x, y],
                    [1.0, 1.0],
                    0.0,
                    0xffffff,
                )?;
            } else if let Some(entry) = entry {
                let item = native_belt_entry_item(entry, snapshot.economy);
                let quantity = match entry {
                    NativeBeltEntry::HealthPotion => {
                        native_belt_potion_projection(
                            &snapshot.economy.backpack,
                            NativeBeltPotionKind::Health,
                        )
                        .count
                    }
                    NativeBeltEntry::ManaPotion => {
                        native_belt_potion_projection(
                            &snapshot.economy.backpack,
                            NativeBeltPotionKind::Mana,
                        )
                        .count
                    }
                    _ => item.map_or(0, |item| item.quantity as u64),
                };
                if let Some(item) = item {
                    self.draw_item(c, item, x, y, quantity)?;
                }
            }
            c.button(
                &format!("belt:{slot}"),
                &format!(
                    "Belt {}, {}",
                    visible + 1,
                    if entry.is_some() {
                        "assign or drag away to remove"
                    } else {
                        "empty, assign selected skill or item"
                    }
                ),
                *r,
            );
            if self.target_belt == Some(slot) {
                c.stroke(*r, 0xe7c169, 2.0);
            }
        }
        c.art(&self.ui, 21, [38.0, 868.0], [1.0, 1.0], 0.0, 0xffffff)?;
        c.painter.text(
            egui::pos2(48.0, 870.0) + c.origin,
            egui::Align2::LEFT_BOTTOM,
            snapshot.economy.gold.to_string(),
            egui::FontId::new(19.0, egui::FontFamily::Name("Georgia".into())),
            c.color(0xd9ba70),
        );
        c.semantic_value(
            "panel:gold",
            &snapshot.economy.gold.to_string(),
            "text",
            HudRect::new(48.0, 851.0, 130.0, 22.0),
            &snapshot.economy.gold.to_string(),
        );
        if matches!(
            self.panel,
            Some(
                PlayerPanel::Inventory
                    | PlayerPanel::Service(solomon_sim::economy::HubTraderId::Luthacus)
            )
        ) {
            c.art(&self.ui, 75, [1562.0, 868.0], [1.0, 1.0], 0.0, 0xffffff)?;
            c.button("unforge", "Unforge selected item", HudRect::new(1500.0, 800.0, 100.0, 100.0));
        }
        for (slot, burst) in self.bursts.borrow().iter() {
            burst.paint(
                c.frame,
                &c.painter,
                &self.ui,
                rect(hud.belt[slot % 8]).center() + c.origin,
                self.now,
                1.0,
                &format!("panel:belt-burst:{slot}:{}", burst.sequence()),
            )?;
        }
        Ok(())
    }
    pub(super) fn paint_dragger(
        &self,
        c: &PanelCanvas<'_, '_>,
        snapshot: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        let Some(press) = self.press.filter(|p| p.dragging) else { return Ok(()) };
        let mut bounds = Rect::NOTHING;
        if let Target::Skill(id) = press.target {
            bounds = c.art(
                &self.skills,
                snapshot.book.skill_icon_record(id),
                [self.pointer.x, self.pointer.y],
                [1.25, 1.25],
                0.0,
                0xffffff,
            )?;
        } else if let Some(id) = press.inventory_item_id
            && let Some(item) = solomon_sim::belt::native_belt_owned_item(snapshot.economy, id)
        {
            bounds = self.draw_item(c, item, self.pointer.x, self.pointer.y, 1)?;
            self.record_mod_item(c, item, self.pointer.x, self.pointer.y, bounds);
        }
        if bounds.is_positive() {
            let transform =
                c.frame.ctx.layer_transform_to_global(c.painter.layer_id()).unwrap_or_default();
            c.frame.record_rect(
                "panel:dragger",
                "Dragged item or skill",
                "image",
                transform * bounds,
                false,
            );
        }
        Ok(())
    }
}

impl PlayerPanels {
    pub(super) fn modal_canvas(
        &mut self,
        frame: &mut UiFrame<'_>,
        id: &str,
        scale: f32,
        origin: Pos2,
        curtain: f32,
        draw: impl FnOnce(&Self, &PanelCanvas<'_, '_>) -> Result<(), String>,
    ) -> Result<(), String> {
        let hits = RefCell::new(Vec::new());
        let events = RefCell::new(Vec::new());
        let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique(id));
        frame
            .ctx
            .set_transform_layer(layer, egui::emath::TSTransform::new(origin.to_vec2(), scale));
        let mut result = Ok(());
        egui::Area::new(layer.id).order(layer.order).fixed_pos(Pos2::ZERO).constrain(false).show(
            frame.ctx,
            |ui| {
                frame.input_scope(id);
                let c = PanelCanvas::new(frame, ui, &hits, &events, 1.0, !self.input_suspended)
                    .direct();
                let size = frame.ctx.content_rect().size();
                c.fill(
                    HudRect::new(
                        -origin.x as f64 / scale as f64,
                        -origin.y as f64 / scale as f64,
                        size.x as f64 / scale as f64,
                        size.y as f64 / scale as f64,
                    ),
                    0,
                    curtain,
                );
                result = draw(self, &c);
            },
        );
        self.events.extend(events.into_inner());
        result
    }
}

/// Native Spell Welding divides its glow along the top-right/bottom-left diagonal.
pub(crate) fn paint_split_art(
    frame: &UiFrame<'_>,
    painter: &egui::Painter,
    atlas: &StockAtlas,
    index: usize,
    center: Pos2,
    scale: f32,
    tints: [u32; 2],
    alpha: f32,
) -> Result<(), String> {
    let r = atlas.record(index)?;
    let size = frame.image_size(atlas.path()).ok_or("Weld glow atlas is not ready")?;
    let [left, top] = r.registered_origin();
    let min = center
        + egui::vec2(
            (left as f64 - r.logical_width as f64 / 2.0) as f32,
            (top as f64 - r.logical_height as f64 / 2.0) as f32,
        ) * scale;
    let dest = Rect::from_min_size(min, egui::vec2(r.width as f32, r.height as f32) * scale);
    let uv = Rect::from_min_max(
        egui::pos2(r.x as f32 / size.x, r.y as f32 / size.y),
        egui::pos2((r.x + r.width) as f32 / size.x, (r.y + r.height) as f32 / size.y),
    );
    frame.image_triangle(
        painter,
        atlas.path(),
        [dest.left_top(), dest.right_top(), dest.left_bottom()],
        [uv.left_top(), uv.right_top(), uv.left_bottom()],
        color(tints[0]).gamma_multiply(alpha),
        solomon_graphics::BlendMode::Normal,
    );
    frame.image_triangle(
        painter,
        atlas.path(),
        [dest.right_top(), dest.right_bottom(), dest.left_bottom()],
        [uv.right_top(), uv.right_bottom(), uv.left_bottom()],
        color(tints[1]).gamma_multiply(alpha),
        solomon_graphics::BlendMode::Normal,
    );
    Ok(())
}

pub(crate) fn child_ui(parent: &egui::Ui, salt: &str, bounds: Rect) -> egui::Ui {
    let mut ui = egui::Ui::new(
        parent.ctx().clone(),
        parent.scope_id().with(salt),
        egui::UiBuilder::new().layer_id(parent.layer_id()).max_rect(bounds),
    );
    ui.set_clip_rect(parent.clip_rect());
    if !parent.is_enabled() {
        ui.disable();
    }
    ui
}
