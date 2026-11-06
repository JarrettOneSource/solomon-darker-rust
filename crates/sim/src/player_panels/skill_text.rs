//! Exact stock skill page text: display names, wrapped lines, text heights and root tints.
//! These authored page wraps count glyph advances without kerning. Ordinary UI
//! wrapping remains in ui_text and must not replace this separate contract.

use crate::profile::primary::SKILL_ROOT_COLORS;
use crate::ui_text::{UiFontName, ui_font};

pub const SKILL_PAGE_TEXT_WRAP_WIDTH: f64 = 140.0;

#[must_use]
pub fn skill_page_display_name(source: &str, effective_rank: u32) -> String {
    let name = source.to_uppercase();
    if effective_rank > 1 { format!("{name} {effective_rank}") } else { name }
}

/// Wraps skill page text, preserving authored CR/LF handling, replaced hyphen/space
/// breaks and the inserted hyphen for an overlong unbroken word.
#[must_use]
pub fn skill_page_wrapped_lines(source: &str) -> Vec<String> {
    let mut characters: Vec<char> = source.chars().collect();
    let font = ui_font(UiFontName::Medium);
    let mut current_width = 0.0;
    let mut previous_break = 0;
    let mut restarting_line = false;
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if character == '\n' || character == '\r' {
            current_width = 0.0;
        }
        let mut advance = 0.0;
        if character == ' ' {
            if !restarting_line {
                advance = font.space_advance;
            }
        } else if let Some(glyph) = font.glyph(character as u32) {
            restarting_line = false;
            advance = glyph.metrics[0];
        }
        if current_width + advance <= SKILL_PAGE_TEXT_WRAP_WIDTH {
            current_width += advance;
            index += 1;
            continue;
        }
        let mut break_at = index;
        for candidate in (previous_break + 1..index).rev() {
            if characters[candidate] == ' ' || characters[candidate] == '-' {
                break_at = candidate;
                break;
            }
        }
        if characters[break_at] != ' ' && characters[break_at] != '-' {
            let insert_at = previous_break.max(if break_at > 1 { break_at - 1 } else { break_at });
            characters.splice(insert_at..insert_at, ['-', '\n']);
            current_width = 0.0;
            previous_break = insert_at + 1;
            restarting_line = true;
            index = insert_at + 2;
        } else {
            characters[break_at] = '\n';
            current_width = advance;
            previous_break = break_at;
            restarting_line = true;
            index = break_at + 1;
        }
    }
    characters.into_iter().collect::<String>().split('\n').map(str::to_owned).collect()
}

#[must_use]
pub fn skill_page_text_height(lines: &[String]) -> f64 {
    if lines.is_empty() {
        return 0.0;
    }
    let line_height = ui_font(UiFontName::Medium).metrics[0];
    line_height + (lines.len() - 1) as f64 * (line_height + 1.0)
}

#[must_use]
pub fn skill_page_tint(root: usize) -> u32 {
    let source = SKILL_ROOT_COLORS[root];
    let luminance = source[0] * 0.3086 + source[1] * 0.6094 + source[2] * 0.082;
    let channel = |value: f64| ((luminance * 0.85 + value * 0.15) * 255.0).round() as u32;
    channel(source[0]) << 16 | channel(source[1]) << 8 | channel(source[2])
}
