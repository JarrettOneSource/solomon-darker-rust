//! The notebox: one timed instruction or failure notice, with its fade and layout.

use crate::ui_text::{UiFontName, UiTextSpec, layout_ui_text};

/// Whether a notebox shows a failure, for 5.2 seconds, or an instruction, for 10.2
/// seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteboxKind {
    Failure,
    Instruction,
}

/// A notebox's panel and text placement, sized to fit its text.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct NoteboxLayout {
    pub panel_height: f64,
    pub panel_left: f64,
    pub panel_top: f64,
    pub panel_width: f64,
    pub text_height: f64,
    pub text_left: f64,
    pub text_baseline: f64,
    pub text_width: f64,
}

#[must_use]
pub const fn notebox_duration_ms(kind: NoteboxKind) -> f64 {
    match kind {
        NoteboxKind::Failure => 5200.0,
        NoteboxKind::Instruction => 10200.0,
    }
}

#[must_use]
pub fn notebox_opacity(kind: NoteboxKind, elapsed_ms: f64, dismissed_at_ms: Option<f64>) -> f64 {
    let elapsed = elapsed_ms.max(0.0);
    let reveal = (elapsed / 100.0).min(1.0);
    let normal_fade = ((notebox_duration_ms(kind) - elapsed) / 200.0).clamp(0.0, 1.0);
    let dismissed_fade = match dismissed_at_ms {
        None => 1.0,
        Some(dismissed) if elapsed <= dismissed => 1.0,
        Some(dismissed) => (1.0 - (elapsed - dismissed) / 200.0).clamp(0.0, 1.0),
    };
    reveal * normal_fade.min(dismissed_fade)
}

#[must_use]
pub fn notebox_layout(text: &str) -> NoteboxLayout {
    let text = layout_ui_text(&UiTextSpec::new(UiFontName::Menu, text, 0.0, 0.0));
    let panel_width = text.width + 70.0;
    let panel_height = text.height + 70.0;
    NoteboxLayout {
        panel_height,
        panel_left: 800.0 - panel_width / 2.0,
        panel_top: 250.0 - panel_height / 2.0,
        panel_width,
        text_height: text.height,
        text_left: 35.0,
        text_baseline: 53.0,
        text_width: text.width,
    }
}
