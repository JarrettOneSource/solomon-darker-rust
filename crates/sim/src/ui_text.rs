//! Stock bitmap typography: the game's fonts and their text measurement, wrapping and
//! glyph layout. Pens, wrapping and ink placement share the authored tables in
//! `catalog`; browser and world painters consume the same layout.

pub mod buttons;
mod catalog;
pub mod messages;

use std::collections::BTreeSet;

/// The texture atlas a font's glyphs come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiAtlasName {
    ControlPanel,
    Fonts,
}

impl UiAtlasName {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ControlPanel => "ControlPanel",
            Self::Fonts => "Fonts",
        }
    }

    #[must_use]
    pub const fn dimensions(self) -> [f64; 2] {
        match self {
            Self::ControlPanel => [512.0, 128.0],
            Self::Fonts => [512.0, 256.0],
        }
    }
}

/// The game's stock bitmap fonts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiFontName {
    Belt,
    Body,
    ControlPanel,
    Heading,
    Medium,
    Menu,
    SkillUppercase,
    SpecialUppercase,
    Timeline,
    WorldAndRoster,
}

impl UiFontName {
    pub const ALL: [Self; 10] = [
        Self::Belt,
        Self::Body,
        Self::ControlPanel,
        Self::Heading,
        Self::Medium,
        Self::Menu,
        Self::SkillUppercase,
        Self::SpecialUppercase,
        Self::Timeline,
        Self::WorldAndRoster,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Belt => "belt",
            Self::Body => "body",
            Self::ControlPanel => "control-panel",
            Self::Heading => "heading",
            Self::Medium => "medium",
            Self::Menu => "menu",
            Self::SkillUppercase => "skill-uppercase",
            Self::SpecialUppercase => "special-uppercase",
            Self::Timeline => "timeline",
            Self::WorldAndRoster => "world-and-roster",
        }
    }
}

/// One glyph of a bitmap font: code point, atlas record and frame, logical size, trim
/// origin and metrics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiGlyphRecord {
    pub code_point: u32,
    pub record: usize,
    pub frame: [f64; 4],
    pub logical_size: [f64; 2],
    pub trim_origin: [f64; 2],
    /// Advance, horizontal bearing, vertical bearing.
    pub metrics: [f64; 3],
}

/// A kerning adjustment between a left and a right code point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiKerning {
    pub left: u32,
    pub right: u32,
    pub adjustment: f64,
}

/// A stock bitmap font: its atlas, metrics, glyphs and kerning pairs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiBitmapFont {
    pub atlas: UiAtlasName,
    /// Line height, space advance, native scale word.
    pub metrics: [f64; 3],
    pub space_advance: f64,
    pub glyphs: &'static [UiGlyphRecord],
    pub kerning: &'static [UiKerning],
}

impl UiBitmapFont {
    #[must_use]
    pub fn glyph(&self, code_point: u32) -> Option<&'static UiGlyphRecord> {
        let index = self.glyphs.binary_search_by_key(&code_point, |glyph| glyph.code_point).ok()?;
        Some(&self.glyphs[index])
    }

    #[must_use]
    pub fn kerning(&self, left: Option<u32>, right: u32) -> f64 {
        let Some(left) = left else { return 0.0 };
        let Ok(index) =
            self.kerning.binary_search_by_key(&(left, right), |pair| (pair.left, pair.right))
        else {
            return 0.0;
        };
        self.kerning[index].adjustment
    }
}

#[must_use]
pub const fn ui_font(name: UiFontName) -> &'static UiBitmapFont {
    match name {
        UiFontName::Belt => &catalog::BELT,
        UiFontName::Body => &catalog::BODY,
        UiFontName::ControlPanel => &catalog::CONTROL_PANEL,
        UiFontName::Heading => &catalog::HEADING,
        UiFontName::Medium => &catalog::MEDIUM,
        UiFontName::Menu => &catalog::MENU,
        UiFontName::SkillUppercase => &catalog::SKILL_UPPERCASE,
        UiFontName::SpecialUppercase => &catalog::SPECIAL_UPPERCASE,
        UiFontName::Timeline => &catalog::TIMELINE,
        UiFontName::WorldAndRoster => &catalog::WORLD_AND_ROSTER,
    }
}

/// Horizontal text alignment about the text's `x`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiTextAlign {
    Left,
    Center,
    Right,
}

/// How `y` places text: as the first baseline, or as the top of a box in which the ink is
/// centered vertically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiTextPlacement {
    Baseline,
    Box,
}

/// A request to lay out text: font, string, position, alignment, placement, scale, alpha,
/// tint, line height and wrap width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiTextSpec<'a> {
    pub font: UiFontName,
    pub text: &'a str,
    pub x: f64,
    pub y: f64,
    pub align: UiTextAlign,
    pub placement: UiTextPlacement,
    pub scale: f64,
    pub alpha: f64,
    pub tint: u32,
    pub line_height: Option<f64>,
    pub max_width: Option<f64>,
}

impl<'a> UiTextSpec<'a> {
    #[must_use]
    pub const fn new(font: UiFontName, text: &'a str, x: f64, y: f64) -> Self {
        Self {
            font,
            text,
            x,
            y,
            align: UiTextAlign::Center,
            placement: UiTextPlacement::Baseline,
            scale: 1.0,
            alpha: 1.0,
            tint: 0xffffff,
            line_height: None,
            max_width: None,
        }
    }
}

/// A run of text with its own scale, advance scale, offset and italics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiTextRun<'a> {
    pub text: &'a str,
    pub scale: Option<f64>,
    pub advance_scale: Option<f64>,
    pub offset_x: f64,
    pub offset_y: f64,
    pub italic: bool,
}

impl<'a> UiTextRun<'a> {
    #[must_use]
    pub const fn new(text: &'a str) -> Self {
        Self { text, scale: None, advance_scale: None, offset_x: 0.0, offset_y: 0.0, italic: false }
    }
}

/// One placed glyph: its record, character, center, scale, alpha, tint and italics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiGlyphLayout {
    pub glyph: &'static UiGlyphRecord,
    pub character: char,
    pub center_x: f64,
    pub center_y: f64,
    pub scale: f64,
    pub alpha: f64,
    pub tint: u32,
    pub italic: bool,
}

/// The rectangle that a placed glyph's visible ink covers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiGlyphInkBounds {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

/// One laid-out line: its text, left edge, baseline and width.
#[derive(Clone, Debug, PartialEq)]
pub struct UiTextLineLayout {
    pub text: String,
    pub x: f64,
    pub baseline_y: f64,
    pub width: f64,
}

/// Laid-out text: its glyphs, lines and size, and the code points the font lacks.
#[derive(Clone, Debug, PartialEq)]
pub struct UiTextLayout {
    pub font: UiFontName,
    pub glyphs: Vec<UiGlyphLayout>,
    pub lines: Vec<UiTextLineLayout>,
    pub width: f64,
    pub height: f64,
    pub unsupported_code_points: Vec<u32>,
}

/// Tight atlas ink inside the glyph's authored logical sprite quad.
#[must_use]
pub fn ui_glyph_ink_bounds(layout: &UiGlyphLayout) -> UiGlyphInkBounds {
    let glyph = layout.glyph;
    UiGlyphInkBounds {
        left: layout.center_x + (glyph.trim_origin[0] - glyph.logical_size[0] / 2.0) * layout.scale,
        top: layout.center_y + (glyph.trim_origin[1] - glyph.logical_size[1] / 2.0) * layout.scale,
        width: glyph.frame[2] * layout.scale,
        height: glyph.frame[3] * layout.scale,
    }
}

#[must_use]
pub fn measure_ui_text(text: &str, font: UiFontName, scale: f64) -> f64 {
    assert!(scale.is_finite() && scale > 0.0, "native UI text scale must be positive");
    let spec = UiTextSpec { scale, ..UiTextSpec::new(font, text, 0.0, 0.0) };
    layout_line(&spec, &[UiTextRun::new(text)], None, None)
}

#[must_use]
pub fn wrap_ui_text(
    text: &str,
    font: UiFontName,
    max_width: Option<f64>,
    scale: f64,
) -> Vec<String> {
    let Some(max_width) = max_width.filter(|width| width.is_finite()) else {
        return text.split('\n').map(str::to_owned).collect();
    };
    assert!(max_width >= 0.0, "native UI text max width must be nonnegative");
    let unscaled_max_width = max_width / scale;
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut current = String::new();
        for word in paragraph.split(is_javascript_whitespace).filter(|word| !word.is_empty()) {
            let next =
                if current.is_empty() { word.to_owned() } else { format!("{current} {word}") };
            if !current.is_empty() && measure_ui_text(&next, font, 1.0) > unscaled_max_width {
                lines.push(current);
                current = word.to_owned();
            } else {
                current = next;
            }
        }
        lines.push(current);
    }
    lines
}

#[must_use]
pub fn layout_ui_text(spec: &UiTextSpec<'_>) -> UiTextLayout {
    assert!(spec.scale.is_finite() && spec.scale > 0.0, "native UI text scale must be positive");
    assert!(
        spec.alpha.is_finite() && (0.0..=1.0).contains(&spec.alpha),
        "native UI text alpha must be within [0, 1]"
    );
    let font = ui_font(spec.font);
    let lines = wrap_ui_text(spec.text, spec.font, spec.max_width, spec.scale);
    let line_height = spec.line_height.unwrap_or(font.metrics[0]) * spec.scale;
    let mut glyphs = Vec::new();
    let mut line_layouts = Vec::new();
    let mut unsupported = BTreeSet::new();
    let mut widest = 0.0_f64;
    for (line_index, text) in lines.into_iter().enumerate() {
        let width = measure_ui_text(&text, spec.font, spec.scale);
        widest = widest.max(width);
        let x = match spec.align {
            UiTextAlign::Left => spec.x,
            UiTextAlign::Center => spec.x - width / 2.0,
            UiTextAlign::Right => spec.x - width,
        };
        let baseline_y = spec.y + line_index as f64 * line_height;
        let line_spec = UiTextSpec { x, y: baseline_y, ..*spec };
        layout_line(
            &line_spec,
            &[UiTextRun::new(&text)],
            Some(&mut glyphs),
            Some(&mut unsupported),
        );
        line_layouts.push(UiTextLineLayout { text, x, baseline_y, width });
    }
    let mut layout = UiTextLayout {
        font: spec.font,
        glyphs,
        height: font.metrics[0] * spec.scale + (line_layouts.len() - 1) as f64 * line_height,
        lines: line_layouts,
        unsupported_code_points: unsupported.into_iter().collect(),
        width: widest,
    };
    if spec.placement == UiTextPlacement::Box {
        place_text_in_box(&mut layout);
    }
    layout
}

/// Styled runs retain kerning across boundaries and may scale their advance independently
/// of the visible glyphs, as Inventory's italic damage units do.
#[must_use]
pub fn layout_ui_text_runs(
    font: UiFontName,
    runs: &[UiTextRun<'_>],
    x: f64,
    y: f64,
    tint: u32,
) -> UiTextLayout {
    let spec = UiTextSpec { tint, ..UiTextSpec::new(font, "", x, y) };
    let mut glyphs = Vec::new();
    let mut unsupported = BTreeSet::new();
    let width = layout_line(&spec, runs, Some(&mut glyphs), Some(&mut unsupported));
    let text = runs.iter().map(|run| run.text).collect();
    UiTextLayout {
        font,
        glyphs,
        lines: vec![UiTextLineLayout { text, x, baseline_y: y, width }],
        width,
        height: ui_font(font).metrics[0],
        unsupported_code_points: unsupported.into_iter().collect(),
    }
}

/// Native dialogue wrapping preserves authored spaces and emphasis across run boundaries.
#[must_use]
pub fn wrap_ui_text_runs<'a>(
    runs: &[UiTextRun<'a>],
    font: UiFontName,
    max_width: f64,
) -> Vec<Vec<UiTextRun<'a>>> {
    let mut lines = Vec::new();
    let mut paragraph = Vec::new();
    for run in runs {
        for (index, character) in run.text.char_indices() {
            if character == '\n' {
                lines.extend(wrap_run_paragraph(&paragraph, font, max_width));
                paragraph.clear();
            } else {
                let text = &run.text[index..index + character.len_utf8()];
                paragraph.push(UiTextRun { text, ..*run });
            }
        }
    }
    lines.extend(wrap_run_paragraph(&paragraph, font, max_width));
    lines
}

/// `MsgBox` mutates stored text at spaces/hyphens; it deliberately carries the glyph
/// that overflowed into the next scan. This is not the renderer's greedy word wrap.
#[must_use]
pub fn wrap_ui_msg_box_text(text: &str, font: UiFontName, max_width: f64) -> Vec<String> {
    assert!(
        max_width.is_finite() && max_width >= 0.0,
        "native UI MsgBox width must be nonnegative"
    );
    let font = ui_font(font);
    let mut characters: Vec<char> = text.chars().collect();
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
                let start = index as usize;
                let mut count = 1;
                while characters.get(start + count) == Some(&' ') {
                    count += 1;
                }
                characters.drain(start..start + count);
                index -= 1;
            } else {
                advance = font.space_advance;
            }
        } else if let Some(glyph) = font.glyph(character as u32) {
            wrapped = false;
            advance = glyph.metrics[0];
        }
        let mut next_line_start = line_start;
        if line_width + advance > max_width {
            line_width = 0.0;
            let mut break_at = index;
            let mut cursor = index - 1;
            if cursor != line_start {
                loop {
                    break_at = cursor;
                    if matches!(characters.get(cursor as usize), Some(' ' | '-')) {
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
                characters.splice(index as usize..index as usize, ['-', '\n']);
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

fn layout_line(
    spec: &UiTextSpec<'_>,
    runs: &[UiTextRun<'_>],
    mut output: Option<&mut Vec<UiGlyphLayout>>,
    mut unsupported: Option<&mut BTreeSet<u32>>,
) -> f64 {
    let font = ui_font(spec.font);
    let mut cursor = spec.x;
    let mut previous = None;
    for run in runs {
        let glyph_scale = spec.scale * run.scale.unwrap_or(1.0);
        let advance_scale = spec.scale * run.advance_scale.or(run.scale).unwrap_or(1.0);
        for character in run.text.chars() {
            let code_point = character as u32;
            if character == ' ' {
                cursor += font.space_advance * advance_scale;
                previous = Some(code_point);
                continue;
            }
            let Some(glyph) = font.glyph(code_point) else {
                if let Some(unsupported) = unsupported.as_deref_mut() {
                    unsupported.insert(code_point);
                }
                previous = Some(code_point);
                continue;
            };
            cursor += font.kerning(previous, code_point) * advance_scale;
            if let Some(output) = output.as_deref_mut() {
                output.push(UiGlyphLayout {
                    glyph,
                    character,
                    center_x: cursor + glyph.metrics[1] * glyph_scale + run.offset_x,
                    center_y: spec.y + glyph.metrics[2] * glyph_scale + run.offset_y,
                    scale: glyph_scale,
                    alpha: spec.alpha,
                    tint: spec.tint,
                    italic: run.italic,
                });
            }
            cursor += glyph.metrics[0] * advance_scale;
            previous = Some(code_point);
        }
    }
    cursor - spec.x
}

fn place_text_in_box(layout: &mut UiTextLayout) {
    if layout.glyphs.is_empty() {
        return;
    }
    let mut ink_top = f64::INFINITY;
    let mut ink_bottom = f64::NEG_INFINITY;
    for glyph in &layout.glyphs {
        let bounds = ui_glyph_ink_bounds(glyph);
        let baseline = glyph.center_y - glyph.glyph.metrics[2] * glyph.scale;
        ink_top = ink_top.min(bounds.top - baseline);
        ink_bottom = ink_bottom.max(bounds.top + bounds.height - baseline);
    }
    let ink_height = ink_bottom - ink_top;
    let line_span =
        layout.lines.last().expect("text has a line").baseline_y - layout.lines[0].baseline_y;
    let line_height = (layout.height - line_span).max(ink_height);
    layout.height = line_span + line_height;
    let offset_y = (line_height - ink_height) / 2.0 - ink_top;
    for glyph in &mut layout.glyphs {
        glyph.center_y += offset_y;
    }
    for line in &mut layout.lines {
        line.baseline_y += offset_y;
    }
}

fn wrap_run_paragraph<'a>(
    paragraph: &[UiTextRun<'a>],
    font: UiFontName,
    max_width: f64,
) -> Vec<Vec<UiTextRun<'a>>> {
    let spec = UiTextSpec::new(font, "", 0.0, 0.0);
    let mut lines = Vec::new();
    let mut line = Vec::new();
    let mut index = 0;
    while index < paragraph.len() {
        let mut spaces = Vec::new();
        while paragraph.get(index).is_some_and(|run| run.text == " ") {
            spaces.push(paragraph[index]);
            index += 1;
        }
        let mut word = Vec::new();
        while paragraph.get(index).is_some_and(|run| run.text != " ") {
            word.push(paragraph[index]);
            index += 1;
        }
        let mut candidate = line.clone();
        candidate.extend(spaces);
        candidate.extend(&word);
        if !line.is_empty()
            && !word.is_empty()
            && layout_line(&spec, &candidate, None, None) > max_width
        {
            lines.push(line);
            line = word;
        } else {
            line = candidate;
        }
    }
    lines.push(line);
    lines
}

// ECMAScript `\s` includes BOM and excludes NEL, unlike Rust's Unicode whitespace set.
pub(crate) fn is_javascript_whitespace(character: char) -> bool {
    matches!(character, '\t'..='\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}'
        | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_name_limit_uses_authored_advances_and_menu_runs_keep_cross_run_kerning() {
        assert_eq!(measure_ui_text("AAAAAAAAAAA", UiFontName::Heading, 1.0), 363.0);
        assert_eq!(measure_ui_text("AAAAAAAAAAAA", UiFontName::Heading, 1.0), 396.0);
        let runs = [UiTextRun::new("A"), UiTextRun { italic: true, ..UiTextRun::new("V") }];
        let layout = layout_ui_text_runs(UiFontName::Menu, &runs, 10.0, 20.0, 0xffffff);
        assert_eq!(layout.width, 37.0);
        assert_eq!(layout.glyphs[1].center_x, 37.0);
        assert!(layout.glyphs[1].italic);
    }
}
