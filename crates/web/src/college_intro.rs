//! Stock College admission title and cover over the accepted controller state.

use crate::gpu_ui::{StockAtlas, UiFrame};
use solomon_sim::Vec2;
use solomon_sim::hub::college_intro::{
    NativeCollegeIntroPhase, NativeCollegeIntroState, native_college_title_presentation,
};
use solomon_sim::viewport::game_viewport;
use web_sys::Document;

pub struct CollegeIntroOverlay {
    atlas: StockAtlas,
    view: Option<(usize, f64, f64, f64)>,
    viewport: [f32; 2],
}

impl CollegeIntroOverlay {
    pub fn layer_id(&self) -> egui::LayerId {
        egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("college-intro"))
    }
    pub async fn attach(_document: &Document) -> Result<Self, String> {
        Ok(Self { atlas: StockAtlas::load("Title").await?, view: None, viewport: [0.0; 2] })
    }

    pub fn update(
        &mut self,
        state: Option<&NativeCollegeIntroState>,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        self.viewport = [width as f32, height as f32];
        self.view = state
            .filter(|state| state.phase == NativeCollegeIntroPhase::CourtyardWalk)
            .map(|state| {
                let view = native_college_title_presentation(state.title_cursor, state.cover_alpha);
                (view.record, view.y, view.alpha, state.cover_alpha)
            });
        Ok(())
    }

    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let Some((record, y, alpha, cover)) = self.view else { return Ok(()) };
        let scale = game_viewport(Vec2::new(self.viewport[0] as f64, self.viewport[1] as f64))
            .display_scale as f32;
        let painter = frame.ctx.layer_painter(self.layer_id());
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::from(self.viewport));
        painter.rect_filled(bounds, 0.0, egui::Color32::BLACK.gamma_multiply(cover as f32));
        frame.record_rect("college-intro", "College admission", "group", bounds, false);
        let sprite = self.atlas.record(record)?;
        let [left, top] = sprite.registered_origin();
        let center = egui::pos2(self.viewport[0] / 2.0, y as f32 * scale);
        let title = egui::Rect::from_min_size(
            center
                + egui::vec2(
                    left - sprite.logical_width as f32 / 2.0,
                    top - sprite.logical_height as f32 / 2.0,
                ) * scale,
            egui::vec2(sprite.width as f32, sprite.height as f32) * scale,
        );
        self.atlas.paint_centered(
            frame,
            &painter,
            record,
            center,
            scale,
            0.0,
            egui::Color32::WHITE.gamma_multiply(alpha as f32),
        )?;
        frame.record_rect(
            "college-intro:title",
            if record == 7 { "Raptisoft Games presents" } else { "Solomon Dark" },
            "image",
            title,
            false,
        );
        Ok(())
    }
}
