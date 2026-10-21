//! `native-ui-plan.ts::layoutNativeUiSingleActionMessage` and the MsgBox-specific
//! wrapping scan in `native-ui-text.ts`. Ordinary word wrapping has different rules.

use crate::Vec2;
use crate::hud::HudRect;

use super::{NativeUiFontName, NativeUiTextSpec, layout_native_ui_text, native_ui_font};

#[derive(Clone, Copy, Debug)]
pub struct NativeUiMessageDataLineSpec<'a> {
    pub font: NativeUiFontName,
    pub gap_after: f64,
    pub text: &'a str,
    pub tint: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeUiMessageDataLineLayout {
    pub baseline_y: f64,
    pub font: NativeUiFontName,
    pub gap_after: f64,
    pub height: f64,
    pub line_height: f64,
    pub text: String,
    pub tint: u32,
    pub width: f64,
    pub x: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeUiSingleActionMessageLayout {
    pub action_bounds: HudRect,
    pub action_text_baseline_y: f64,
    pub frame_bounds: HudRect,
    pub lines: Vec<NativeUiMessageDataLineLayout>,
    pub panel_bounds: HudRect,
}

pub fn layout_native_ui_single_action_message(
    anchor: Vec2,
    source: &[NativeUiMessageDataLineSpec<'_>],
) -> Result<NativeUiSingleActionMessageLayout, String> {
    if source.is_empty() {
        return Err("native UI MsgBox requires at least one DataLine".into());
    }
    if !anchor.x.is_finite() || !anchor.y.is_finite() {
        return Err("native UI MsgBox anchor must be finite".into());
    }
    let mut lines = Vec::new();
    let mut content_width = 0.0_f64;
    let mut content_height = 0.0;
    for line in source {
        if !line.gap_after.is_finite() || line.gap_after < 0.0 {
            return Err("native UI MsgBox DataLine gap must be finite and nonnegative".into());
        }
        let text = wrap_msgbox_text(line.text, line.font).join("\n");
        let line_height = native_ui_font(line.font).metrics[0] + 1.0;
        let layout = layout_native_ui_text(&NativeUiTextSpec {
            line_height: Some(line_height),
            ..NativeUiTextSpec::new(line.font, &text, 0.0, 0.0)
        });
        content_width = content_width.max(layout.width);
        content_height += layout.height + line.gap_after;
        lines.push(NativeUiMessageDataLineLayout {
            baseline_y: 0.0,
            font: line.font,
            gap_after: line.gap_after,
            height: layout.height,
            line_height,
            text,
            tint: line.tint,
            width: layout.width,
            x: 0.0,
        });
    }
    let width = content_width + 50.0;
    let height = content_height + 150.0;
    let panel_bounds = HudRect::new(anchor.x - width / 2.0, anchor.y - height / 2.0, width, height);
    let frame_bounds =
        HudRect::new(panel_bounds.x - 49.0, panel_bounds.y - 50.0, width + 98.0, height + 100.0);
    let mut baseline = panel_bounds.y + 20.0 + native_ui_font(source[0].font).metrics[0];
    for line in &mut lines {
        line.x = panel_bounds.x + 25.0;
        line.baseline_y = baseline;
        baseline += line.height + line.gap_after + 1.0;
    }
    let action_bounds = HudRect::new(
        panel_bounds.x + width / 2.0 - 98.0,
        panel_bounds.y + height - 60.0 - 34.5,
        196.0,
        69.0,
    );
    Ok(NativeUiSingleActionMessageLayout {
        action_bounds,
        action_text_baseline_y: action_bounds.y + 42.5,
        frame_bounds,
        lines,
        panel_bounds,
    })
}

/// The native MsgBox wraps at 400 pixels, carries the overflow glyph's advance,
/// removes following spaces, and may replace a hyphen. Preserve that scan rather
/// than substituting the ordinary renderer's greedy word wrapper.
fn wrap_msgbox_text(text: &str, font_name: NativeUiFontName) -> Vec<String> {
    let font = native_ui_font(font_name);
    let mut characters: Vec<_> = text.chars().collect();
    let mut index = 0_isize;
    let mut line_start = 0_isize;
    let mut line_width = 0.0;
    let mut wrapped = false;
    while index < characters.len() as isize {
        let mut character = characters[index as usize];
        if character == '\n' || character == '\r' {
            line_width = 0.0;
        }
        let mut advance = 0.0;
        if character == ' ' {
            if wrapped {
                let mut count = 1;
                while characters.get(index as usize + count) == Some(&' ') {
                    count += 1;
                }
                drop(characters.drain(index as usize..index as usize + count));
                index -= 1;
            } else {
                advance = font.space_advance;
            }
        } else if let Some(glyph) = font.glyph(u32::from(character)) {
            wrapped = false;
            advance = glyph.metrics[0];
        }
        let mut next_line_start = line_start;
        if line_width + advance > 400.0 {
            line_width = 0.0;
            let mut break_at = index;
            let mut cursor = index - 1;
            if cursor != line_start {
                loop {
                    break_at = cursor;
                    if matches!(characters[cursor as usize], ' ' | '-') {
                        break;
                    }
                    cursor -= 1;
                    break_at = index;
                    if cursor == line_start {
                        break;
                    }
                }
            }
            character = characters[break_at as usize];
            index = break_at;
            if character == ' ' || character == '-' {
                characters[break_at as usize] = '\n';
            } else {
                if break_at > 1 {
                    index = break_at - 1;
                }
                drop(characters.splice(index as usize..index as usize, ['-', '\n']));
            }
            wrapped = true;
            next_line_start = break_at;
        }
        line_width += advance;
        index += 1;
        line_start = next_line_start;
    }
    characters.into_iter().collect::<String>().split('\n').map(str::to_owned).collect()
}
