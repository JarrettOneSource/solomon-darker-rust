//! Stock social chrome painted into the shared GPU frame.

use egui::{
    Color32, FontFamily, FontId, Id, Painter, Pos2, Rect, Response, RichText, Sense, Ui, Vec2,
    pos2, vec2,
};
use solomon_sim::hud::HudRect;
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextSpec};

use crate::gpu_ui::{self, StockAtlas, UiFrame};

pub(crate) const GOLD: Color32 = Color32::from_rgb(217, 186, 112);
pub(crate) const PALE: Color32 = Color32::from_rgb(232, 220, 185);

pub(crate) fn rich(text: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(text).font(FontId::new(size, FontFamily::Name("Georgia".into()))).color(color)
}

pub(super) fn stage(size: (i32, i32)) -> (Pos2, f32) {
    let scale = (size.0 as f32 / 1600.0).min(size.1 as f32 / 900.0).max(0.01);
    (pos2((size.0 as f32 - 1600.0 * scale) / 2.0, (size.1 as f32 - 900.0 * scale) / 2.0), scale)
}

pub(super) fn rect(bounds: HudRect, origin: Pos2, scale: f32) -> Rect {
    Rect::from_min_size(
        origin + vec2(bounds.x as f32, bounds.y as f32) * scale,
        vec2(bounds.width as f32, bounds.height as f32) * scale,
    )
}

pub(super) fn native_text(
    frame: &mut UiFrame<'_>,
    painter: &Painter,
    spec: &NativeUiTextSpec<'_>,
    origin: Pos2,
    scale: f32,
) {
    let mut spec = *spec;
    spec.x = f64::from(origin.x) + spec.x * f64::from(scale);
    spec.y = f64::from(origin.y) + spec.y * f64::from(scale);
    spec.scale *= f64::from(scale);
    spec.max_width = spec.max_width.map(|w| w * f64::from(scale));
    gpu_ui::native_text(frame, painter, &spec);
}

pub(super) fn stock_button(
    frame: &mut UiFrame<'_>,
    ui: &mut Ui,
    atlas: &StockAtlas,
    id: &str,
    label: &str,
    bounds: Rect,
    enabled: bool,
) -> Result<Response, String> {
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            ui.interact(bounds, Id::unique(("social", id)), Sense::click())
        })
        .inner;
    let painter = ui.painter();
    let tint = if enabled { Color32::WHITE } else { Color32::from_white_alpha(100) };
    let path = if enabled && (response.hovered() || response.has_focus()) {
        "assets/stock/menu/main-menu-button-hover.png"
    } else {
        "assets/stock/menu/main-menu-button.png"
    };
    frame.image(painter, path, bounds, None, tint);
    let scale = bounds.height() / 69.0;
    let corner = vec2(24.0, 30.0) * scale;
    for (center, mirror) in [
        (pos2(bounds.left() + corner.x / 2.0, bounds.center().y), false),
        (pos2(bounds.right() - corner.x / 2.0, bounds.center().y), true),
    ] {
        let uv =
            if mirror { Some(Rect::from_min_max(pos2(1.0, 0.0), pos2(0.0, 1.0))) } else { None };
        frame.image(
            painter,
            "assets/stock/menu/main-menu-button-corner.png",
            Rect::from_center_size(center, corner),
            uv,
            tint,
        );
    }
    atlas.paint_native_frame(frame, painter, 54, bounds, false, tint)?;
    let mut spec = NativeUiTextSpec::new(NativeUiFontName::Menu, label, 0.0, 0.0);
    spec.x = f64::from(bounds.center().x);
    spec.y = f64::from(bounds.center().y + 9.0 * scale);
    spec.scale = f64::from(scale);
    spec.tint = 0xd9ba70;
    spec.alpha = if enabled { 1.0 } else { 0.4 };
    gpu_ui::native_text(frame, painter, &spec);
    frame.record(&format!("social/{id}"), label, "button", &response);
    Ok(response)
}

pub(super) fn brackets(
    frame: &mut UiFrame<'_>,
    painter: &Painter,
    atlas: &StockAtlas,
    bounds: Rect,
    scale: f32,
) -> Result<(), String> {
    let record = atlas.record(50)?;
    let size = vec2(record.logical_width as f32, record.logical_height as f32) * scale;
    for (point, sx, sy) in [
        (bounds.left_top(), 1.0, 1.0),
        (bounds.right_top(), -1.0, 1.0),
        (bounds.left_bottom(), 1.0, -1.0),
        (bounds.right_bottom(), -1.0, -1.0),
    ] {
        let left = point.x + if sx < 0.0 { -size.x } else { 0.0 };
        let top = point.y + if sy < 0.0 { -size.y } else { 0.0 };
        let source = Rect::from_min_max(
            pos2(
                if sx < 0.0 { record.width as f32 } else { 0.0 },
                if sy < 0.0 { record.height as f32 } else { 0.0 },
            ),
            pos2(
                if sx < 0.0 { 0.0 } else { record.width as f32 },
                if sy < 0.0 { 0.0 } else { record.height as f32 },
            ),
        );
        atlas.paint_slice(
            frame,
            painter,
            50,
            source,
            Rect::from_min_size(pos2(left, top), size),
            Color32::WHITE,
        )?;
    }
    Ok(())
}

pub(super) fn browser_button(
    frame: &mut UiFrame<'_>,
    ui: &mut Ui,
    id: &str,
    label: &str,
    size: Vec2,
    enabled: bool,
) -> Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(rich(label, 14.0, PALE))
            .fill(Color32::from_rgb(24, 21, 33))
            .stroke(egui::Stroke::new(1.0, GOLD))
            .corner_radius(0.0)
            .min_size(size),
    );
    frame.record(&format!("social/{id}"), label, "button", &response);
    response
}
