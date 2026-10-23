//! Current inventory contextual cards, painted from native line/layout kernels.
use super::sack_name::gpu_sack_name;
use crate::gpu_ui::UiFrame;
use solomon_sim::Vec2;
use solomon_sim::native_ui_text::{
    NativeUiTextAlign, NativeUiTextSpec, layout_native_ui_text, native_ui_font,
};
use solomon_sim::player_panels::item_text::{
    HUB_TOOLTIP_CONTENT_MARGIN, HUB_TOOLTIP_LINE_GAP, HubContextualHoverLayout, HubTooltipLine,
    hub_contextual_hover_layout,
};
pub(crate) fn item_hover_layout(
    frame: &UiFrame<'_>,
    lines: &[HubTooltipLine],
    source: Vec2,
    gap: f64,
) -> Result<HubContextualHoverLayout, String> {
    let name = lines
        .iter()
        .find(|line| line.sack_name_browser_font)
        .map(|line| gpu_sack_name(frame, &line.text, line.tint, 300.0, true))
        .transpose()?;
    Ok(hub_contextual_hover_layout(lines, source, gap, name.as_ref().map(|name| name.size)))
}
pub(crate) fn paint_item_hover_box(
    frame: &UiFrame<'_>,
    painter: &egui::Painter,
    lines: &[HubTooltipLine],
    layout: &HubContextualHoverLayout,
    origin: egui::Pos2,
    scale: f32,
) -> Result<(), String> {
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
    let mut cursor = HUB_TOOLTIP_CONTENT_MARGIN;
    for (line, rendered) in lines.iter().zip(&layout.rendered) {
        if line.sack_name_browser_font {
            let name = gpu_sack_name(frame, &line.text, line.tint, 300.0, true)?;
            name.paint(
                painter,
                origin
                    + egui::vec2(
                        HUB_TOOLTIP_CONTENT_MARGIN as f32,
                        (cursor - name.baseline_offset) as f32,
                    ) * scale,
                scale,
            );
        } else {
            let text = layout_native_ui_text(&NativeUiTextSpec {
                align: NativeUiTextAlign::Left,
                tint: line.tint,
                line_height: Some(native_ui_font(line.font).metrics[0]),
                ..NativeUiTextSpec::new(
                    line.font,
                    &rendered.sources.join("\n"),
                    HUB_TOOLTIP_CONTENT_MARGIN,
                    cursor,
                )
            });
            super::skill_hover::paint_scaled_text(frame, painter, &text, origin, scale);
        }
        cursor += rendered.height + HUB_TOOLTIP_LINE_GAP;
    }
    Ok(())
}
