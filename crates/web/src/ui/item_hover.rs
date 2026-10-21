//! Current hub-inventory/items.ts contextual cards. Stock rows use the shared mask
//! glyph painter; only a custom Sack name unsupported by the atlas uses browser text.

use solomon_sim::Vec2;
use solomon_sim::native_ui_text::{
    NativeUiTextAlign, NativeUiTextSpec, layout_native_ui_text, native_ui_font,
};
use solomon_sim::player_panels::item_text::{
    HUB_TOOLTIP_CONTENT_MARGIN, HUB_TOOLTIP_LINE_GAP, HubContextualHoverLayout, HubTooltipLine,
    hub_contextual_hover_layout,
};
use web_sys::{Document, HtmlElement};

use super::sack_name::browser_sack_name;
use super::text::paint_text_layout;
use super::{add_style, append, element, style};

pub(crate) fn paint_item_hover_box(
    document: &Document,
    parent: &HtmlElement,
    lines: &[HubTooltipLine],
    source: Vec2,
    source_gap: f64,
) -> Result<HubContextualHoverLayout, String> {
    let mut browser_name = lines
        .iter()
        .find(|line| line.sack_name_browser_font)
        .map(|line| browser_sack_name(document, &line.text, line.tint, 300.0, true))
        .transpose()?;
    let browser_metrics = browser_name.as_ref().map(|name| name.size);
    let layout = hub_contextual_hover_layout(lines, source, source_gap, browser_metrics);
    parent.set_text_content(None);
    let frame = element(document, "div", "native-item-hover-content")?;
    style(
        &frame,
        &format!(
            "position:absolute;left:0;top:0;width:{}px;height:{}px;background:black;outline:1px solid white;transform:scale(var(--native-hover-scale,1));transform-origin:top left;pointer-events:none",
            layout.rect.width, layout.rect.height,
        ),
    )?;
    let mut cursor_y = HUB_TOOLTIP_CONTENT_MARGIN;
    for (line, rendered) in lines.iter().zip(&layout.rendered) {
        if line.sack_name_browser_font {
            let name = browser_name.take().expect("measured custom Sack name");
            add_style(
                &name.node,
                &format!(
                    "position:absolute;left:{HUB_TOOLTIP_CONTENT_MARGIN}px;top:{}px;visibility:visible",
                    cursor_y - name.baseline_offset,
                ),
            )?;
            append(&frame, &name.node)?;
        } else {
            let text = layout_native_ui_text(&NativeUiTextSpec {
                align: NativeUiTextAlign::Left,
                tint: line.tint,
                line_height: Some(native_ui_font(line.font).metrics[0]),
                ..NativeUiTextSpec::new(
                    line.font,
                    &rendered.sources.join("\n"),
                    HUB_TOOLTIP_CONTENT_MARGIN,
                    cursor_y,
                )
            });
            let node = element(document, "span", "native-item-hover-line")?;
            style(&node, "position:absolute;inset:0;pointer-events:none")?;
            node.set_attribute("data-native-tooltip-font", line.font.as_str())
                .map_err(|_| "native item tooltip font")?;
            paint_text_layout(document, &node, &text)?;
            append(&frame, &node)?;
        }
        cursor_y += rendered.height + HUB_TOOLTIP_LINE_GAP;
    }
    let spoken: Vec<_> = lines.iter().map(|line| line.text.as_str()).collect();
    parent.set_attribute("aria-label", &spoken.join("\n")).map_err(|_| "item tooltip label")?;
    parent.set_attribute("data-native-item-hover", "true").map_err(|_| "item tooltip owner")?;
    append(parent, &frame)?;
    Ok(layout)
}
