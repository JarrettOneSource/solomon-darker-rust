//! GPU native spectator rails/text from `NativeSpectatorStatus.tsx`.

use crate::gpu_ui::{self, StockAtlas, UiFrame};
use egui::{Color32, Rect, pos2, vec2};
use solomon_sim::boneyard::spectator::{
    BoneyardSpectatorStatusPresentation, NativeSpectatorHudLayout, native_spectator_hud_layout,
};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec};
use solomon_sim::viewport::GameViewport;

pub struct SpectatorStatusView {
    atlas: StockAtlas,
    previous: Option<(BoneyardSpectatorStatusPresentation, GameViewport)>,
}
impl SpectatorStatusView {
    pub fn layer_id(&self) -> egui::LayerId {
        egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("spectator/status"))
    }

    pub async fn attach() -> Result<Self, String> {
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
        let painter = frame.ctx.layer_painter(self.layer_id());
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
        frame.record_value("spectator/status",&serde_json::json!({"runId":status.run_id,"targetPlayerId":status.target_player_id,"displayText":status.display_text}).to_string(),None);
        frame.record_value("spectator/respawn",&serde_json::json!({"text":status.respawn_text,"activeEnemies":status.active_enemy_count,"incomingEnemies":status.incoming_enemy_count,"waveOrdinal":status.wave_ordinal}).to_string(),None);
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
        for (horizontal, rail) in layout
            .horizontal_rails
            .iter()
            .map(|rail| (true, rail))
            .chain(layout.vertical_rails.iter().map(|rail| (false, rail)))
        {
            let record = self.atlas.record(rail.record)?;
            let bounds = rail.bounds;
            let target = Rect::from_min_size(
                origin + vec2(bounds.x as f32, bounds.y as f32) * scale,
                vec2(bounds.width as f32, bounds.height as f32) * scale,
            );
            let clipped = painter.with_clip_rect(painter.clip_rect().intersect(target));
            let extent = if horizontal { bounds.width } else { bounds.height };
            let step = if horizontal {
                f64::from(record.logical_width)
            } else {
                f64::from(record.logical_height)
            };
            let mut offset = 0.0;
            while offset < extent {
                let point = target.min
                    + if horizontal {
                        vec2(offset as f32 * scale, 0.0)
                    } else {
                        vec2(0.0, offset as f32 * scale)
                    };
                self.atlas.paint_raw(
                    frame,
                    &clipped,
                    rail.record,
                    Rect::from_min_size(
                        point,
                        vec2(record.width as f32, record.height as f32) * scale,
                    ),
                    Color32::WHITE,
                )?;
                offset += step;
            }
        }
        for corner in &layout.corners {
            let record = self.atlas.record(corner.record)?;
            let top_left = origin
                + vec2(
                    corner.center.x as f32 - record.logical_width as f32 / 2.0,
                    corner.center.y as f32 - record.logical_height as f32 / 2.0,
                ) * scale;
            self.atlas.paint_raw(
                frame,
                painter,
                corner.record,
                Rect::from_min_size(
                    top_left,
                    vec2(record.width as f32, record.height as f32) * scale,
                ),
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
