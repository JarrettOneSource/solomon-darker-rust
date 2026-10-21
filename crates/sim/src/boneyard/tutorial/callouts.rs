//! Authored Tutorial frames from `tutorial-modal-callouts.ts::tutorialCalloutGeometry`.

use crate::hud::HudRect;
use crate::native_ui_text::{NativeUiFontName, measure_native_ui_text, native_ui_font};

#[derive(Clone, Debug, PartialEq)]
pub struct TutorialCalloutLine {
    pub text: String,
    pub width: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TutorialCalloutGeometry {
    pub center_x: f64,
    pub center_y: f64,
    pub frame: HudRect,
    pub lines: Vec<TutorialCalloutLine>,
    pub text_height: f64,
    pub text_width: f64,
}

/// Pens truncate toward zero before the native stage is scaled to the viewport.
/// A callout has explicit authored newlines; it does not wrap through browser metrics.
pub fn tutorial_callout_geometry(
    text: &str,
    center_x: f64,
    center_y: f64,
) -> Result<TutorialCalloutGeometry, String> {
    if !center_x.is_finite() || !center_y.is_finite() {
        return Err("tutorial callout centre must be finite".into());
    }
    let mut lines = Vec::new();
    let mut text_width = 0.0_f64;
    for (index, line) in text.split('\n').enumerate() {
        let width = measure_native_ui_text(line, NativeUiFontName::Menu, 1.0);
        text_width = text_width.max(width);
        lines.push(TutorialCalloutLine {
            text: line.into(),
            width,
            x: (center_x - width / 2.0).trunc(),
            y: center_y.trunc() + 25.0 * index as f64,
        });
    }
    let text_height =
        native_ui_font(NativeUiFontName::Menu).metrics[0] + 25.0 * (lines.len() - 1) as f64;
    let width = text_width + 28.0;
    let height = text_height + 20.0;
    Ok(TutorialCalloutGeometry {
        center_x,
        center_y,
        frame: HudRect::new(center_x - width / 2.0, center_y + 4.0 - height / 2.0, width, height),
        lines,
        text_height,
        text_width,
    })
}
