//! Screen-space world labels from `renderer/native-world-{nameplate,speech}.ts`.
//! Text uses the shared authored bitmap metrics; camera scale affects anchors only.

use crate::Vec2;
use crate::native_ui_text::{
    NativeUiFontName, NativeUiGlyphLayout, NativeUiTextAlign, NativeUiTextSpec,
    layout_native_ui_text, measure_native_ui_text, native_ui_glyph_ink_bounds,
};

pub const WORLD_NAMEPLATE_GLYPH_SCALE: f64 = 0.56;
pub const WORLD_NAMEPLATE_WORLD_OFFSET_Y: f64 = -45.0;
pub const WORLD_SPEECH_GLYPH_SCALE: f64 = 0.5;
pub const WORLD_SPEECH_WORLD_OFFSET_Y: f64 = -78.0;
pub const WORLD_SPEECH_LINE_HEIGHT: f64 = 16.0;
pub const WORLD_SPEECH_CONTENT_MAX_WIDTH: f64 = 240.0;

#[derive(Clone, Debug, PartialEq)]
pub struct WorldNameplateVisualLayout {
    pub cap_box: [f64; 2],
    pub glyph_bounds: [f64; 2],
    pub glyph_offset: Vec2,
    pub glyphs: Vec<NativeUiGlyphLayout>,
    pub width: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeWorldSpeechLine {
    pub advance: f64,
    pub glyphs: Vec<NativeUiGlyphLayout>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeWorldSpeechLayout {
    pub glyph_count: usize,
    pub height: f64,
    pub lines: Vec<NativeWorldSpeechLine>,
    pub width: f64,
}

#[must_use]
pub fn native_world_nameplate_health_ratio(current: f64, maximum: f64) -> Option<f64> {
    if !current.is_finite() || !maximum.is_finite() || maximum <= 0.0 {
        return None;
    }
    Some((current / maximum).clamp(0.0, 1.0))
}

#[must_use]
pub fn world_nameplate_visual_layout(display_name: &str) -> WorldNameplateVisualLayout {
    let layout = layout_native_ui_text(&NativeUiTextSpec {
        align: NativeUiTextAlign::Left,
        scale: WORLD_NAMEPLATE_GLYPH_SCALE,
        ..NativeUiTextSpec::new(NativeUiFontName::WorldAndRoster, display_name, 0.0, 0.0)
    });
    let mut left = 0.0_f64;
    let mut right = layout.width;
    for glyph in &layout.glyphs {
        let ink = native_ui_glyph_ink_bounds(glyph);
        left = left.min(ink.left);
        right = right.max(ink.left + ink.width);
    }
    let cap_top = -19.0 * WORLD_NAMEPLATE_GLYPH_SCALE;
    let cap_bottom = 6.0 * WORLD_NAMEPLATE_GLYPH_SCALE;
    // Name area: plate top -16, inset one; rail top 2, gap one.
    let name_center_y = (-15.0 + 1.0) / 2.0;
    let offset = Vec2::new(-(left + right) / 2.0, name_center_y - (cap_top + cap_bottom) / 2.0);
    WorldNameplateVisualLayout {
        cap_box: [cap_top + offset.y, cap_bottom + offset.y],
        glyph_bounds: [left + offset.x, right + offset.x],
        glyph_offset: offset,
        glyphs: layout.glyphs,
        width: 56.0_f64.max(2.0 * ((right - left + 18.0) / 2.0).ceil()),
    }
}

#[must_use]
pub fn layout_native_world_speech(text: &str) -> NativeWorldSpeechLayout {
    let mut lines = Vec::new();
    let mut content_width = 0.0_f64;
    let mut glyph_count = 0;
    for text in wrap_world_speech(text) {
        let layout = layout_native_ui_text(&NativeUiTextSpec {
            align: NativeUiTextAlign::Left,
            scale: WORLD_SPEECH_GLYPH_SCALE,
            ..NativeUiTextSpec::new(NativeUiFontName::WorldAndRoster, &text, 0.0, 0.0)
        });
        content_width = content_width.max(layout.width);
        glyph_count += layout.glyphs.len();
        lines.push(NativeWorldSpeechLine { advance: layout.width, glyphs: layout.glyphs, text });
    }
    NativeWorldSpeechLayout {
        glyph_count,
        height: lines.len() as f64 * WORLD_SPEECH_LINE_HEIGHT + 12.0,
        lines,
        width: 44.0_f64.max(content_width.ceil() + 16.0),
    }
}

/// `projectNativeWorldPoint` culls the anchor before its whole-pixel placement.
#[must_use]
pub fn project_native_world_point(
    point: Vec2,
    translation: Vec2,
    scale: f64,
    viewport: Vec2,
) -> Option<Vec2> {
    if !point.x.is_finite()
        || !point.y.is_finite()
        || !translation.x.is_finite()
        || !translation.y.is_finite()
        || !scale.is_finite()
        || scale <= 0.0
        || !viewport.x.is_finite()
        || !viewport.y.is_finite()
        || viewport.x <= 0.0
        || viewport.y <= 0.0
    {
        return None;
    }
    let point = Vec2::new(translation.x + point.x * scale, translation.y + point.y * scale);
    if point.x < 0.0 || point.y < 0.0 || point.x > viewport.x || point.y > viewport.y {
        return None;
    }
    Some(point)
}

fn speech_text_width(text: &str) -> f64 {
    measure_native_ui_text(text, NativeUiFontName::WorldAndRoster, WORLD_SPEECH_GLYPH_SCALE)
}

fn wrap_world_speech(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split(is_javascript_whitespace).filter(|word| !word.is_empty()) {
        if speech_text_width(word) > WORLD_SPEECH_CONTENT_MAX_WIDTH {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            for character in word.chars() {
                let mut candidate = current.clone();
                candidate.push(character);
                if !current.is_empty()
                    && speech_text_width(&candidate) > WORLD_SPEECH_CONTENT_MAX_WIDTH
                {
                    lines.push(current);
                    current = character.to_string();
                } else {
                    current = candidate;
                }
            }
            continue;
        }
        let candidate =
            if current.is_empty() { word.to_owned() } else { format!("{current} {word}") };
        if !current.is_empty() && speech_text_width(&candidate) > WORLD_SPEECH_CONTENT_MAX_WIDTH {
            lines.push(current);
            current = word.to_owned();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

// Source \s treats a BOM as whitespace, but keeps NEL inside a word.
fn is_javascript_whitespace(character: char) -> bool {
    matches!(character, '\t'..='\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}'
        | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
