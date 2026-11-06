//! Exact native skill card glyphs shared by Skillbook and earned level-up offers.
use crate::gpu_ui::UiFrame;
use solomon_sim::player_panels::skill_hover::{
    SKILL_HOVER_CONTENT_MARGIN, SKILL_HOVER_LINE_GAP, SkillHoverBoxLayout, skill_exact_text_runs,
    skill_hover_line_tint,
};
use solomon_sim::ui_text::{UiFontName, UiTextLayout, layout_ui_text_runs};
pub(crate) fn paint_scaled_text(
    frame: &UiFrame<'_>,
    painter: &egui::Painter,
    layout: &UiTextLayout,
    origin: egui::Pos2,
    scale: f32,
) {
    let mut text = layout.clone();
    for glyph in &mut text.glyphs {
        glyph.center_x = origin.x as f64 + glyph.center_x * scale as f64;
        glyph.center_y = origin.y as f64 + glyph.center_y * scale as f64;
        glyph.scale *= scale as f64;
    }
    crate::gpu_ui::paint_text_layout(frame, painter, &text);
}
pub(crate) fn paint_skill_hover_box(
    frame: &UiFrame<'_>,
    painter: &egui::Painter,
    layout: &SkillHoverBoxLayout,
    skill_id: usize,
    origin: egui::Pos2,
    scale: f32,
) {
    let rect = egui::Rect::from_min_size(
        origin,
        egui::vec2(layout.rect.width as f32, layout.rect.height as f32) * scale,
    );
    painter.rect_filled(rect, 0.0, egui::Color32::BLACK);
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, egui::Color32::WHITE),
        egui::StrokeKind::Outside,
    );
    let mut cursor = SKILL_HOVER_CONTENT_MARGIN;
    for line in &layout.rendered {
        for source in &line.sources {
            let runs = skill_exact_text_runs(source);
            let text = layout_ui_text_runs(
                UiFontName::Body,
                &runs,
                SKILL_HOVER_CONTENT_MARGIN,
                cursor,
                skill_hover_line_tint(line.kind, skill_id),
            );
            paint_scaled_text(frame, painter, &text, origin, scale);
            cursor += layout.line_height;
        }
        cursor += SKILL_HOVER_LINE_GAP;
    }
}
