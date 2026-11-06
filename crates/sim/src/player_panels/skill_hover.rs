//! Skill hover boxes for the Skillbook and the skill picker: their rows, inline text
//! commands and layout. The catalog owns authored text; the box owns its independent
//! 380/400-pixel wraps.

use crate::Vec2;
use crate::hud::HudRect;
use crate::profile::{
    PlayerSkillOfferOption, SKILL_CATALOG, skill_category, skill_root, weld_build,
};
use crate::ui_text::{UiFontName, UiTextRun, measure_ui_text, ui_font, wrap_ui_msg_box_text};

use super::format_skill_tooltip_line;
use super::skill_text::skill_page_tint;

pub const SKILL_HOVER_CONTENT_MARGIN: f64 = 25.0;
pub const SKILL_HOVER_LINE_GAP: f64 = 10.0;

/// What a skill hover row shows; the kind also selects the row's tint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillHoverLineKind {
    Bonus,
    Boost,
    Category,
    Description,
    Level,
    Spacer,
    Stat,
    Title,
}

impl SkillHoverLineKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bonus => "bonus",
            Self::Boost => "boost",
            Self::Category => "category",
            Self::Description => "description",
            Self::Level => "level",
            Self::Spacer => "spacer",
            Self::Stat => "stat",
            Self::Title => "title",
        }
    }
}

/// One skill hover row before wrapping.
#[derive(Clone, Debug, PartialEq)]
pub struct SkillHoverLine {
    pub kind: SkillHoverLineKind,
    pub text: String,
}

/// One skill hover row after wrapping into display lines.
#[derive(Clone, Debug, PartialEq)]
pub struct SkillHoverRenderedLine {
    pub kind: SkillHoverLineKind,
    pub sources: Vec<String>,
}

/// A laid-out skill hover box: wrapped rows, content width, screen rectangle and line
/// height.
#[derive(Clone, Debug, PartialEq)]
pub struct SkillHoverBoxLayout {
    pub rendered: Vec<SkillHoverRenderedLine>,
    pub content_width: f64,
    pub rect: HudRect,
    pub line_height: f64,
}

enum InlineCommand {
    ToggleItalic,
    Scale(f64),
    Offset(Vec2),
}

#[must_use]
pub fn skill_book_tooltip_lines(
    skill_id: usize,
    permanent_rank: u32,
    effective_rank: u32,
) -> Vec<SkillHoverLine> {
    use SkillHoverLineKind as Kind;
    let skill = &SKILL_CATALOG[skill_id];
    let mut lines = Vec::new();
    if effective_rank > permanent_rank {
        push_line(
            &mut lines,
            Kind::Boost,
            if permanent_rank == 0 { "GRANTED BY ITEM" } else { "BOOSTED" },
        );
    }
    let maximum = if skill.cap_level > 0 {
        skill.cap_level
    } else if skill.maximum_level > 0 {
        skill.maximum_level
    } else {
        effective_rank
    };
    let mut title = skill.name.to_uppercase();
    if maximum > 1 {
        title.push_str(&format!(" _s(.7)_o(0,1){effective_rank}/{maximum}"));
    }
    push_line(&mut lines, Kind::Title, &title);
    let category = skill_category(skill_id).expect("native tooltip skill category");
    push_line(
        &mut lines,
        Kind::Category,
        match category {
            1 => "PRIMARY CAST",
            2 => "SECONDARY CAST",
            3 => "CONCENTRATION",
            _ => "PASSIVE SKILL",
        },
    );
    push_line(&mut lines, Kind::Description, skill.description);
    push_line(&mut lines, Kind::Spacer, "");
    push_line(&mut lines, Kind::Level, &format!("   Current Level: {effective_rank}"));
    for template in skill.stat_lines {
        let value = format_skill_tooltip_line(template, skill, effective_rank);
        push_line(&mut lines, Kind::Stat, &format!("   {value}"));
    }
    if category == 3 {
        for template in skill.bonus_lines {
            let value = format_skill_tooltip_line(template, skill, effective_rank);
            push_line(&mut lines, Kind::Bonus, &format!("   {value}"));
        }
    }
    lines
}

#[must_use]
pub fn skill_picker_detail_lines(option: &PlayerSkillOfferOption) -> Vec<SkillHoverLine> {
    let mut lines =
        skill_book_tooltip_lines(option.skill_id, option.target_rank, option.target_rank);
    if let Some(build_id) = option.weld_build_id {
        let build = weld_build(build_id);
        for line in &mut lines {
            match line.kind {
                SkillHoverLineKind::Title => {
                    let suffix = line
                        .text
                        .find(" _s(")
                        .map(|index| line.text[index..].to_owned())
                        .unwrap_or_default();
                    line.text = format!("{}{suffix}", build.synthetic_name.to_uppercase());
                }
                SkillHoverLineKind::Description => line.text = build.pair_description.to_owned(),
                _ => {}
            }
        }
    }
    if option.insight {
        push_line(&mut lines, SkillHoverLineKind::Bonus, "Insight Bonus: Skill +2");
    }
    lines
}

fn push_line(lines: &mut Vec<SkillHoverLine>, kind: SkillHoverLineKind, text: &str) {
    lines.push(SkillHoverLine { kind, text: text.to_owned() });
}

#[must_use]
pub fn skill_hover_line_tint(kind: SkillHoverLineKind, skill_id: usize) -> u32 {
    let tint = skill_page_tint(skill_root(skill_id).expect("native hover skill root"));
    match kind {
        SkillHoverLineKind::Boost => 0xff80ff,
        SkillHoverLineKind::Bonus => 0xd9ba70,
        SkillHoverLineKind::Title => tint,
        SkillHoverLineKind::Category => {
            let channel = |shift: u32| ((((tint >> shift) & 0xff) as f64) * 0.75).round() as u32;
            channel(16) << 16 | channel(8) << 8 | channel(0)
        }
        _ => 0xbfbfbf,
    }
}

/// Splits authored skill text into runs at its `_i`, `_s(scale)` and `_o(x,y)` commands.
/// Malformed commands remain literal text. Only plain decimals are accepted;
/// exponent/plus/whitespace forms are not commands.
#[must_use]
pub fn skill_exact_text_runs(source: &str) -> Vec<UiTextRun<'_>> {
    let mut runs = Vec::new();
    let mut state = UiTextRun::new("");
    state.scale = Some(1.0);
    let mut start = 0;
    let mut cursor = 0;
    while cursor < source.len() {
        let rest = &source[cursor..];
        let Some((length, command)) = inline_command(rest) else {
            cursor += rest.chars().next().expect("remaining source character").len_utf8();
            continue;
        };
        if cursor > start {
            runs.push(UiTextRun { text: &source[start..cursor], ..state });
        }
        match command {
            InlineCommand::ToggleItalic => state.italic = !state.italic,
            InlineCommand::Scale(scale) => state.scale = Some(scale),
            InlineCommand::Offset(offset) => {
                state.offset_x = offset.x;
                state.offset_y = offset.y;
            }
        }
        cursor += length;
        start = cursor;
    }
    if cursor > start {
        runs.push(UiTextRun { text: &source[start..cursor], ..state });
    }
    runs
}

fn inline_command(source: &str) -> Option<(usize, InlineCommand)> {
    if source.starts_with("_i") {
        return Some((2, InlineCommand::ToggleItalic));
    }
    if !source.starts_with("_s(") && !source.starts_with("_o(") {
        return None;
    }
    let end = source.find(')')?;
    let value = &source[3..end];
    let command = if source.starts_with("_s(") {
        if !is_source_decimal(value) {
            return None;
        }
        InlineCommand::Scale(value.parse().expect("validated source scale"))
    } else {
        let (x, y) = value.split_once(',')?;
        if !is_source_decimal(x) || !is_source_decimal(y) {
            return None;
        }
        InlineCommand::Offset(Vec2::new(
            x.parse().expect("validated source x offset"),
            y.parse().expect("validated source y offset"),
        ))
    };
    Some((end + 1, command))
}

fn is_source_decimal(source: &str) -> bool {
    let source = source.strip_prefix('-').unwrap_or(source);
    if let Some((integer, fraction)) = source.split_once('.') {
        return (integer.is_empty() || integer.bytes().all(|value| value.is_ascii_digit()))
            && fraction.bytes().all(|value| value.is_ascii_digit())
            && (!integer.is_empty() || !fraction.is_empty());
    }
    !source.is_empty() && source.bytes().all(|value| value.is_ascii_digit())
}

#[must_use]
pub fn measure_skill_exact_text(source: &str) -> f64 {
    let mut line_width = 0.0_f64;
    let mut maximum_width = 0.0_f64;
    for run in skill_exact_text_runs(source) {
        let parts: Vec<_> = run.text.split('\n').collect();
        for (index, part) in parts.iter().enumerate() {
            line_width += measure_ui_text(part, UiFontName::Body, run.scale.unwrap_or(1.0));
            maximum_width = maximum_width.max(line_width);
            if index + 1 < parts.len() {
                line_width = 0.0;
            }
        }
    }
    maximum_width
}

#[must_use]
pub fn skill_hover_box_layout(lines: &[SkillHoverLine], source: Vec2) -> SkillHoverBoxLayout {
    let mut rendered = Vec::new();
    let mut content_width = 0.0_f64;
    let line_height = ui_font(UiFontName::Body).metrics[0];
    let mut content_height = 0.0;
    for (index, line) in lines.iter().enumerate() {
        let text = if line.kind == SkillHoverLineKind::Description {
            wrap_ui_msg_box_text(&line.text, UiFontName::Body, 380.0).join("\n")
        } else {
            line.text.clone()
        };
        let sources = if measure_skill_exact_text(&text) > 400.0 {
            wrap_ui_msg_box_text(&text, UiFontName::Body, 400.0)
        } else {
            text.split('\n').map(str::to_owned).collect()
        };
        for text in &sources {
            content_width = content_width.max(measure_skill_exact_text(text));
        }
        content_height += sources.len() as f64 * line_height;
        if index + 1 < lines.len() {
            content_height += SKILL_HOVER_LINE_GAP;
        }
        rendered.push(SkillHoverRenderedLine { kind: line.kind, sources });
    }
    let margin = SKILL_HOVER_CONTENT_MARGIN;
    let width = content_width + margin * 2.0;
    let height = content_height + margin * 2.0;
    // Use max/min rather than clamp: very tall authored boxes still pin to 25.
    let x = margin.max((1600.0 - margin - width).min(source.x - width / 2.0));
    let mut y = source.y - 50.0 - height;
    if y < margin {
        y = source.y + 50.0;
    }
    y = margin.max((900.0 - margin - height).min(y));
    SkillHoverBoxLayout {
        rendered,
        content_width,
        rect: HudRect::new(x, y, width, height),
        line_height,
    }
}
