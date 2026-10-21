//! `renderer/native-skill-hover-box.ts`: one stock glyph painter for both skill surfaces.

use solomon_sim::native_ui_text::{NativeUiFontName, layout_native_ui_text_runs};
use solomon_sim::player_panels::skill_hover::{
    NATIVE_SKILL_HOVER_CONTENT_MARGIN, NATIVE_SKILL_HOVER_LINE_GAP, NativeSkillHoverBoxLayout,
    native_skill_exact_text_runs, native_skill_hover_line_tint,
};
use web_sys::{Document, HtmlElement};

use super::{append, element, style, text::paint_text_layout};

pub(crate) fn paint_skill_hover_box(
    document: &Document,
    parent: &HtmlElement,
    layout: &NativeSkillHoverBoxLayout,
    skill_id: usize,
) -> Result<(), String> {
    parent.set_text_content(None);
    let frame = element(document, "div", "native-skill-hover-content")?;
    style(
        &frame,
        &format!(
            "position:absolute;left:0;top:0;width:{}px;height:{}px;background:black;outline:1px solid white;transform:scale(var(--native-hover-scale,1));transform-origin:top left;pointer-events:none",
            layout.rect.width, layout.rect.height,
        ),
    )?;
    let mut cursor_y = NATIVE_SKILL_HOVER_CONTENT_MARGIN;
    let mut spoken = Vec::new();
    for line in &layout.rendered {
        for source in &line.sources {
            let runs = native_skill_exact_text_runs(source);
            let text = layout_native_ui_text_runs(
                NativeUiFontName::Body,
                &runs,
                NATIVE_SKILL_HOVER_CONTENT_MARGIN,
                cursor_y,
                native_skill_hover_line_tint(line.kind, skill_id),
            );
            let node = element(document, "span", "native-skill-hover-line")?;
            style(&node, "position:absolute;inset:0;pointer-events:none")?;
            node.set_attribute("data-native-hover-line", line.kind.as_str())
                .map_err(|_| "native skill semantic line")?;
            paint_text_layout(document, &node, &text)?;
            append(&frame, &node)?;
            spoken.push(runs.iter().map(|run| run.text).collect::<String>());
            cursor_y += layout.line_height;
        }
        cursor_y += NATIVE_SKILL_HOVER_LINE_GAP;
    }
    parent
        .set_attribute("aria-label", &spoken.join("\n"))
        .map_err(|_| "native skill detail label")?;
    parent.set_attribute("data-native-hover", "true").map_err(|_| "native skill detail owner")?;
    append(parent, &frame)
}
