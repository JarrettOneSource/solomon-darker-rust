//! Source `native-ui/native-ui-notebox.ts`: one timed instruction/failure notice.

use crate::native_ui_text::{NativeUiFontName, NativeUiTextSpec, layout_native_ui_text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeNoteboxKind {
    Failure,
    Instruction,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct NativeNoteboxLayout {
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
pub const fn native_notebox_duration_ms(kind: NativeNoteboxKind) -> f64 {
    match kind {
        NativeNoteboxKind::Failure => 5200.0,
        NativeNoteboxKind::Instruction => 10200.0,
    }
}

#[must_use]
pub fn native_notebox_opacity(
    kind: NativeNoteboxKind,
    elapsed_ms: f64,
    dismissed_at_ms: Option<f64>,
) -> f64 {
    let elapsed = elapsed_ms.max(0.0);
    let reveal = (elapsed / 100.0).min(1.0);
    let normal_fade = ((native_notebox_duration_ms(kind) - elapsed) / 200.0).clamp(0.0, 1.0);
    let dismissed_fade = match dismissed_at_ms {
        None => 1.0,
        Some(dismissed) if elapsed <= dismissed => 1.0,
        Some(dismissed) => (1.0 - (elapsed - dismissed) / 200.0).clamp(0.0, 1.0),
    };
    reveal * normal_fade.min(dismissed_fade)
}

#[must_use]
pub fn native_notebox_layout(text: &str) -> NativeNoteboxLayout {
    let text =
        layout_native_ui_text(&NativeUiTextSpec::new(NativeUiFontName::Menu, text, 0.0, 0.0));
    let panel_width = text.width + 70.0;
    let panel_height = text.height + 70.0;
    NativeNoteboxLayout {
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
