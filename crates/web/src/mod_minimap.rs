//! GPU authored minimap from `mod-ui/ModMinimap.tsx`.
//! The accepted projection supplies every marker and the exact displayed action center.

use crate::gpu_ui::{UiFrame, clip_polygon_ellipse};
use crate::ui::javascript_string;
use egui::{Color32, FontFamily, FontId, Id, Rect, Stroke, pos2, vec2};
use solomon_sim::Vec2;
use solomon_sim::modding::LuaObject;
use solomon_sim::modding::projection::ModContentProjection;
use solomon_sim::modding::ui::{
    ModMinimapMarkerId, ModMinimapMarkerKind, ModMinimapSnapshot, project_mod_minimap,
};
use wasm_bindgen::JsValue;
use web_sys::Document;

pub struct ModMinimapAction {
    pub content_id: String,
    pub action: String,
    pub center: Vec2,
}
struct Marker {
    id: String,
    kind: ModMinimapMarkerKind,
    position: Vec2,
}
struct Minimap {
    label: String,
    content_id: String,
    mount: String,
    size: Vec2,
    range: f64,
    center: Vec2,
    markers: Vec<Marker>,
    binding: Option<String>,
    actions: Vec<String>,
}
pub struct ModMinimapView {
    model: Option<Minimap>,
    actions: Vec<ModMinimapAction>,
}
impl ModMinimapView {
    pub fn layer_id(&self) -> egui::LayerId {
        egui::LayerId::new(egui::Order::Middle, egui::Id::unique("mod/minimap"))
    }

    pub fn new(_document: &Document) -> Result<Self, String> {
        Ok(Self { model: None, actions: Vec::new() })
    }
    pub fn update(
        &mut self,
        snapshot: &ModMinimapSnapshot<'_>,
        content: Option<&ModContentProjection>,
        viewer_id: &str,
        runtime: Option<&LuaObject>,
    ) -> Result<(), String> {
        let model =
            content.and_then(|content| project_mod_minimap(snapshot, content, viewer_id, runtime));
        self.model = if let Some(model) = model {
            let binding = if let Some((name, value)) =
                model.bindings.and_then(|bindings| bindings.iter().next())
            {
                let json = serde_json::to_string(value).map_err(|error| error.to_string())?;
                let value = js_sys::JSON::parse(&json)
                    .map_err(|_| "Could not read authored minimap binding")?;
                Some(format!("{name}: {}", javascript_string(&value)))
            } else {
                None
            };
            Some(Minimap {
                label: model.accessible_name.to_owned(),
                content_id: model.content_id.to_owned(),
                mount: model.mount.to_owned(),
                size: model.size,
                range: model.range,
                center: model.center,
                markers: model
                    .markers
                    .iter()
                    .map(|marker| Marker {
                        id: marker_id(marker.id),
                        kind: marker.kind,
                        position: marker.position,
                    })
                    .collect(),
                binding,
                actions: model.actions.iter().map(|action| (*action).to_owned()).collect(),
            })
        } else {
            None
        };
        Ok(())
    }
    pub fn hide(&mut self) {
        self.model = None;
    }
    pub fn reset(&mut self) {
        self.hide();
        self.actions.clear();
    }
    pub fn take_actions(&mut self) -> Vec<ModMinimapAction> {
        std::mem::take(&mut self.actions)
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let Some(model) = &self.model else { return Ok(()) };
        let ctx = frame.ctx.clone();
        let screen = ctx.content_rect();
        let size = vec2(
            (model.size.x as f32).min(screen.width() * 0.42).min(352.0),
            (model.size.y as f32).min(screen.height() * 0.42).min(352.0),
        );
        let point = match model.mount.as_str() {
            "hud.top_left" => pos2(16.0, 16.0),
            "hud.bottom_left" => pos2(16.0, screen.height() - 80.0 - size.y),
            "hud.bottom_right" => {
                pos2(screen.width() - 16.0 - size.x, screen.height() - 80.0 - size.y)
            }
            "hud.overlay" => screen.center() - size / 2.0,
            _ => pos2(screen.width() - 16.0 - size.x, 16.0),
        };
        egui::Area::new(self.layer_id().id)
            .fade_in(false)
            .order(self.layer_id().order)
            .fixed_pos(point)
            .show(&ctx, |ui| -> Result<(), String> {
                let bounds = Rect::from_min_size(ui.min_rect().min, size);
                ui.set_min_size(size);
                if !model.actions.is_empty() {
                    let body =
                        ui.interact(bounds, Id::unique("mod/minimap-body"), egui::Sense::CLICK);
                    frame.record_ellipse("mod/minimap-body", &model.label, "region", &body);
                }
                let painter = ui.painter();
                let outline: Vec<_> = (0..64)
                    .map(|i| {
                        let angle = i as f32 * std::f32::consts::TAU / 64.0;
                        bounds.center()
                            + vec2(angle.cos() * size.x / 2.0, angle.sin() * size.y / 2.0)
                    })
                    .collect();
                painter.add(egui::Shape::convex_polygon(
                    outline,
                    Color32::from_rgba_unmultiplied(10, 12, 17, 199),
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(210, 190, 139, 178)),
                ));
                for marker in &model.markers {
                    let x = 0.5
                        + ((marker.position.x - model.center.x).clamp(-model.range, model.range)
                            / model.range
                            * 0.5) as f32;
                    let y = 0.5
                        + ((marker.position.y - model.center.y).clamp(-model.range, model.range)
                            / model.range
                            * 0.5) as f32;
                    let center = bounds.min + vec2(x * size.x, y * size.y);
                    let color = match marker.kind {
                        ModMinimapMarkerKind::SelfPlayer => Color32::WHITE,
                        ModMinimapMarkerKind::Party => Color32::from_rgb(87, 200, 255),
                        ModMinimapMarkerKind::Enemy => Color32::from_rgb(238, 68, 68),
                        ModMinimapMarkerKind::Powerup => Color32::from_rgb(121, 240, 107),
                    };
                    let points: Vec<_> = (0..24)
                        .map(|i| {
                            let angle = i as f32 * std::f32::consts::TAU / 24.0;
                            center + vec2(angle.cos(), angle.sin()) * 3.6
                        })
                        .collect();
                    let points = clip_polygon_ellipse(points, bounds);
                    if points.len() > 2 {
                        painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
                    }
                    frame.record_rect(
                        &format!("mod/minimap-marker:{}:{}", marker.kind.as_str(), marker.id),
                        marker.kind.as_str(),
                        "img",
                        Rect::from_center_size(center, vec2(7.2, 7.2)).intersect(bounds),
                        true,
                    );
                }
                if let Some(binding) = &model.binding {
                    crate::social::gpu::browser_text(
                        frame,
                        painter,
                        bounds.left_bottom() + vec2(7.2, -5.6),
                        egui::Align2::LEFT_BOTTOM,
                        binding,
                        FontId::new(10.0, FontFamily::Name("Alegreya".into())),
                        Color32::from_rgb(238, 227, 196),
                    )?;
                }
                // Source overlays each declared action at the same anchor, preserving order.
                for action in &model.actions {
                    let font = FontId::new(10.0, FontFamily::Name("Alegreya".into()));
                    let label = crate::gpu_ui::font_run(
                        frame.ctx,
                        font,
                        Color32::from_rgb(238, 227, 196),
                        action,
                        None,
                    )?;
                    let rect = Rect::from_center_size(
                        bounds.center_bottom() - vec2(0.0, 22.0),
                        label.size() + vec2(11.2, 6.4),
                    );
                    let response = ui.interact(
                        rect,
                        Id::unique(("mod/minimap-action", action)),
                        egui::Sense::click(),
                    );
                    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(24, 21, 33));
                    ui.painter().rect_stroke(
                        rect,
                        0.0,
                        Stroke::new(1.0, Color32::from_rgb(210, 190, 139)),
                        egui::StrokeKind::Inside,
                    );
                    label.paint(frame, ui.painter(), rect.center() - label.size() / 2.0, 1.0)?;
                    frame.record(
                        &format!("mod/minimap-action:{action}"),
                        action,
                        "button",
                        &response,
                    );
                    if response.clicked() {
                        self.actions.push(ModMinimapAction {
                            content_id: model.content_id.clone(),
                            action: action.clone(),
                            center: model.center,
                        });
                    }
                }
                frame.record_rect("mod/minimap", &model.label, "img", bounds, true);
                Ok(())
            })
            .inner?;
        Ok(())
    }
}
fn marker_id(id: ModMinimapMarkerId<'_>) -> String {
    match id {
        ModMinimapMarkerId::Player(id) => id.to_owned(),
        ModMinimapMarkerId::Enemy(id) => {
            format!("enemy-{}", javascript_string(&JsValue::from_f64(id)))
        }
        ModMinimapMarkerId::ModEnemy(id) => {
            format!("mod-enemy-{}", javascript_string(&JsValue::from_f64(id)))
        }
        ModMinimapMarkerId::Powerup(id) => {
            format!("powerup-{}", javascript_string(&JsValue::from_f64(id)))
        }
    }
}
