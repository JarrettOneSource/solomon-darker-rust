//! GPU native spectator rails/text from `NativeSpectatorStatus.tsx`.

use crate::gpu_ui::{self, StockAtlas, UiFrame};
use egui::{Color32, Id, Order, Rect, pos2, vec2};
use solomon_sim::boneyard::spectator::{
    BoneyardSpectatorStatusPresentation, NativeSpectatorHudLayout, native_spectator_hud_layout,
};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec};
use solomon_sim::viewport::GameViewport;
use web_sys::Document;

pub struct SpectatorStatusView {
    atlas: StockAtlas,
    previous: Option<(BoneyardSpectatorStatusPresentation, GameViewport)>,
}
impl SpectatorStatusView {
    pub async fn attach(_document: &Document) -> Result<Self, String> {
        Ok(Self { atlas: StockAtlas::load("UI").await?, previous: None })
    }
    pub fn hide(&mut self) {
        self.previous = None;
    }
    pub fn update(
        &mut self,
        status: Option<&BoneyardSpectatorStatusPresentation>,
        viewport: GameViewport,
    ) -> Result<(), String> {
        self.previous = status.map(|status| (status.clone(), viewport));
        Ok(())
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let Some((status, viewport)) = &self.previous else { return Ok(()) };
        let layout = native_spectator_hud_layout(viewport.logical_size);
        let scale = viewport.display_scale as f32;
        let painter = frame
            .ctx
            .layer_painter(egui::LayerId::new(Order::Foreground, Id::unique("spectator/status")));
        let origin = pos2(layout.surface.x as f32 * scale, layout.surface.y as f32 * scale);
        self.panel(frame, &painter, &layout, &status.display_text, origin, scale)?;
        let second = origin + vec2(0.0, (layout.surface.height as f32 + 8.0) * scale);
        self.panel(frame, &painter, &layout, &status.respawn_text, second, scale)?;
        frame.record_rect(
            "spectator/status",
            &status.accessible_label,
            "status",
            Rect::from_min_size(
                origin,
                vec2(layout.surface.width as f32, layout.surface.height as f32) * scale,
            ),
            true,
        );
        frame.record_rect(
            "spectator/respawn",
            &status.respawn_text,
            "status",
            Rect::from_min_size(
                second,
                vec2(layout.surface.width as f32, layout.surface.height as f32) * scale,
            ),
            true,
        );
        Ok(())
    }
    fn panel(
        &self,
        frame: &mut UiFrame<'_>,
        painter: &egui::Painter,
        layout: &NativeSpectatorHudLayout,
        text: &str,
        origin: egui::Pos2,
        scale: f32,
    ) -> Result<(), String> {
        for rail in layout.horizontal_rails.iter().chain(&layout.vertical_rails) {
            let bounds = rail.bounds;
            self.atlas.paint_tiles(
                frame,
                painter,
                rail.record,
                Rect::from_min_size(
                    origin + vec2(bounds.x as f32, bounds.y as f32) * scale,
                    vec2(bounds.width as f32, bounds.height as f32) * scale,
                ),
                scale,
                Color32::WHITE,
            )?;
        }
        for corner in &layout.corners {
            self.atlas.paint_centered(
                frame,
                painter,
                corner.record,
                origin + vec2(corner.center.x as f32, corner.center.y as f32) * scale,
                scale,
                0.0,
                Color32::WHITE,
            )?;
        }
        gpu_ui::native_text(
            frame,
            painter,
            &NativeUiTextSpec {
                align: NativeUiTextAlign::Left,
                scale: f64::from(scale),
                tint: 0xffe68c,
                ..NativeUiTextSpec::new(
                    NativeUiFontName::Medium,
                    text,
                    f64::from(origin.x) + layout.text.x * f64::from(scale),
                    f64::from(origin.y) + layout.text.y * f64::from(scale),
                )
            },
        );
        Ok(())
    }
}
