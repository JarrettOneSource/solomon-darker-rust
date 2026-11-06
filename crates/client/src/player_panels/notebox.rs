//! Non-modal stock NPC instruction/failure. Its presentation clock never changes
//! the accepted session; clicking the visible box starts the native dismissal fade.
use crate::gpu_ui::{StockAtlas, UiFrame, native_text};
use solomon_sim::ui_notebox::{NoteboxKind, notebox_duration_ms, notebox_layout, notebox_opacity};
use solomon_sim::ui_text::{UiFontName, UiTextSpec};
struct Notice {
    kind: NoteboxKind,
    text: String,
    started_at: f64,
    dismissed: Option<f64>,
}
pub(super) struct NpcNotebox {
    active: Option<Notice>,
}
impl NpcNotebox {
    pub fn layer_id() -> egui::LayerId {
        egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("npc-notebox"))
    }
    pub fn new() -> Self {
        Self { active: None }
    }
    pub fn show(
        &mut self,
        _atlas: &StockAtlas,
        kind: NoteboxKind,
        text: &str,
        now: f64,
    ) -> Result<(), String> {
        self.active = Some(Notice { kind, text: text.into(), started_at: now, dismissed: None });
        Ok(())
    }
    pub fn update(&mut self, now: f64) {
        if self.active.as_ref().is_some_and(|notice| {
            now - notice.started_at
                >= notice
                    .dismissed
                    .map_or_else(|| notebox_duration_ms(notice.kind), |at| at + 200.0)
        }) {
            self.reset();
        }
    }
    pub fn reset(&mut self) {
        self.active = None;
    }
    pub fn paint(
        &mut self,
        frame: &UiFrame<'_>,
        atlas: &StockAtlas,
        now: f64,
        viewport: (i32, i32),
    ) -> Result<(), String> {
        let Some(notice) = &mut self.active else { return Ok(()) };
        let layout = notebox_layout(&notice.text);
        let scale = (viewport.0 as f32 / 1600.0).min(viewport.1 as f32 / 900.0);
        let layer = Self::layer_id();
        frame.ctx.set_transform_layer(
            layer,
            egui::emath::TSTransform::new(
                egui::vec2(
                    (viewport.0 as f32 - 1600.0 * scale) / 2.0,
                    (viewport.1 as f32 - 900.0 * scale) / 2.0,
                ),
                scale,
            ),
        );
        let elapsed = (now - notice.started_at).max(0.0);
        let opacity = notebox_opacity(notice.kind, elapsed, notice.dismissed) as f32;
        let tint = if notice.kind == NoteboxKind::Failure {
            egui::Color32::from_rgb(255, 64, 64)
        } else {
            egui::Color32::WHITE
        }
        .gamma_multiply(opacity);
        let mut error = None;
        egui::Area::new(layer.id)
            .fade_in(false)
            .order(layer.order)
            .fixed_pos(egui::Pos2::ZERO)
            .constrain(false)
            .show(frame.ctx, |ui| {
                let r = egui::Rect::from_min_size(
                    egui::pos2(layout.panel_left as f32, layout.panel_top as f32),
                    egui::vec2(layout.panel_width as f32, layout.panel_height as f32),
                );
                let response =
                    ui.interact(r, egui::Id::unique("npc:notebox"), egui::Sense::click());
                frame.record(
                    "npc:notebox",
                    &notice.text,
                    if notice.kind == NoteboxKind::Failure { "alert" } else { "status" },
                    &response,
                );
                if response.clicked() && notice.dismissed.is_none() {
                    notice.dismissed = Some(elapsed);
                }
                for _ in 0..2 {
                    if let Err(message) =
                        atlas.paint_stock_frame(frame, ui.painter(), 64, r, true, tint)
                    {
                        error = Some(message);
                    }
                }
                native_text(
                    frame,
                    ui.painter(),
                    &UiTextSpec {
                        tint: if notice.kind == NoteboxKind::Failure { 0xff4040 } else { 0xd9ba70 },
                        alpha: opacity as f64,
                        ..UiTextSpec::new(
                            UiFontName::Menu,
                            &notice.text,
                            layout.panel_left + layout.panel_width / 2.0,
                            layout.panel_top + layout.text_baseline,
                        )
                    },
                );
            });
        error.map_or(Ok(()), Err)
    }
}
