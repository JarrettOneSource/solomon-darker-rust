//! Source `MatchLoadingScreen.tsx`: the accepted transition owns progress and delay.
//! Stock artwork, status and its progress track are painted in the same GPU frame.

use crate::gpu_ui::UiFrame;
use egui::{Align2, Color32, FontFamily, FontId, Id, Order, Rect, Stroke, pos2, vec2};
use solomon_sim::menu::loading::MatchLoadingState;
use web_sys::Document;

const ART: &str = "assets/stock/menu/match-loading-background.png";

#[derive(Default)]
pub struct MatchLoadingView {
    loading: Option<MatchLoadingState>,
    delay_elapsed: bool,
}

impl MatchLoadingView {
    pub fn layer_id(&self) -> egui::LayerId {
        egui::LayerId::new(Order::Foreground, Id::unique("match-loading"))
    }
    pub fn attach(_document: &Document) -> Result<Self, String> {
        Ok(Self::default())
    }
    pub fn update(
        &mut self,
        loading: Option<&MatchLoadingState>,
        now_ms: f64,
    ) -> Result<(), String> {
        self.loading = loading.copied();
        self.delay_elapsed = loading.is_some_and(|loading| loading.should_present(now_ms));
        Ok(())
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let Some(loading) = self.loading else { return Ok(()) };
        frame.input_scope("match-loading", self.layer_id());
        let ctx = frame.ctx.clone();
        let screen = ctx.content_rect();
        egui::Area::new(self.layer_id().id)
            .order(self.layer_id().order)
            .fade_in(false)
            .fixed_pos(screen.min)
            .show(&ctx, |ui| {
                ui.set_min_size(screen.size());
                ui.interact(
                    screen,
                    Id::unique("match-loading-hold"),
                    egui::Sense::click_and_drag(),
                );
                if !self.delay_elapsed || frame.image_size(ART).is_none() {
                    return;
                }
                let painter = ui.painter();
                frame.image(painter, ART, screen, None, Color32::WHITE);
                let scrim = Rect::from_min_max(
                    pos2(screen.left(), screen.top() + screen.height() * 0.82),
                    screen.max,
                );
                let mut gradient = egui::Mesh::default();
                for (pos, color) in [
                    (scrim.left_top(), Color32::TRANSPARENT),
                    (scrim.right_top(), Color32::TRANSPARENT),
                    (scrim.right_bottom(), Color32::from_black_alpha(179)),
                    (scrim.left_bottom(), Color32::from_black_alpha(179)),
                ] {
                    gradient.colored_vertex(pos, color);
                }
                gradient.indices.extend([0, 1, 2, 0, 2, 3]);
                painter.add(egui::Shape::mesh(gradient));
                let font_size = (screen.height() * 0.022_222_22).clamp(16.8, 36.0);
                let baseline = pos2(
                    screen.center().x,
                    screen.bottom()
                        - screen.height() * 0.075
                        - (screen.height() * 0.012_962_96).max(12.0)
                        - 0.5,
                );
                let font = FontId::new(font_size, FontFamily::Name("Alegreya Bold".into()));
                painter.text(
                    baseline + vec2(0.0, 1.0),
                    Align2::CENTER_BOTTOM,
                    loading.label(),
                    font.clone(),
                    Color32::BLACK,
                );
                let label = painter.text(
                    baseline,
                    Align2::CENTER_BOTTOM,
                    loading.label(),
                    font,
                    Color32::from_rgb(242, 229, 199),
                );
                frame.record_rect("match-loading:label", loading.label(), "status", label, true);
                frame.announce("match-loading:label", loading.label());
                let track = Rect::from_min_size(
                    screen.min + vec2(screen.width() * 0.2 - 0.5, screen.height() * 0.925 - 0.5),
                    vec2(screen.width() * 0.6, (screen.height() * 0.0083).clamp(8.0, 10.0)),
                );
                painter.rect_filled(track, 0.0, Color32::from_rgba_unmultiplied(20, 17, 13, 235));
                painter.rect_filled(
                    Rect::from_min_size(
                        track.min,
                        vec2(track.width() * loading.progress as f32, track.height()),
                    ),
                    0.0,
                    Color32::from_rgb(202, 161, 77),
                );
                painter.rect_stroke(
                    track,
                    0.0,
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(105, 82, 42, 230)),
                    egui::StrokeKind::Outside,
                );
                frame.record_rect(
                    "match-loading:progress",
                    loading.label(),
                    "progressbar",
                    track,
                    true,
                );
                frame.record_value(
                    "match-loading:progress",
                    &(loading.progress * 100.0).to_string(),
                    None,
                );
            });
        Ok(())
    }
}
