//! Full stock-button labels fitted around the authored UI.54 skulls.
//!
//! Party and other client-authored controls may be narrower than a retail menu
//! row. Fit their actual glyph ink without changing chrome, hit bounds or the
//! caller's authored baseline and nominal font size when the label already fits.

use crate::hud::HudRect;

use super::{NativeUiTextSpec, layout_native_ui_text, native_ui_glyph_ink_bounds};

pub const NATIVE_UI_BUTTON_LABEL_Y_OFFSET: f64 = 9.0;
pub const NATIVE_UI_BUTTON_PRESSED_OFFSET: f64 = 6.0;
const SURROUND: f64 = 6.0;

// Nontransparent right edge (exclusive) of every UI.54 row, measured from the
// 70x85 source at (679,394). Original atlas SHA-256:
// 37d5e8fc543af12a9d8019e738dbe1e29b648211144a3782c3a32e71f76cd2eb.
const SURROUND_INK_RIGHT: [u8; 85] = [
    70, 70, 70, 70, 70, 70, 70, 70, 70, 70, 70, 54, 54, 53, 53, 51, 48, 48, 47, 43, 42, 42, 42, 42,
    39, 37, 38, 38, 38, 39, 39, 39, 39, 39, 39, 39, 39, 39, 38, 38, 39, 40, 40, 41, 41, 41, 41, 41,
    41, 42, 49, 49, 49, 50, 50, 50, 50, 50, 51, 51, 54, 57, 57, 57, 57, 58, 62, 63, 63, 66, 68, 68,
    68, 68, 70, 70, 70, 70, 70, 70, 70, 70, 70, 70, 70,
];

/// Fits an unpressed, centred label and reserves the six-unit press displacement.
/// Chrome and text scales remain independent. The caller may retain its explicit
/// baseline or centre the fitted font's baseline offset before applying a press.
#[must_use]
pub fn fit_native_ui_button_label(
    mut spec: NativeUiTextSpec<'_>,
    bounds: HudRect,
    chrome_scale: f64,
) -> NativeUiTextSpec<'_> {
    let layout = layout_native_ui_text(&spec);
    if layout.glyphs.is_empty() {
        return spec;
    }
    let center_x = bounds.x + bounds.width / 2.0;
    let center_y = bounds.y + bounds.height / 2.0;
    let mut reach = 0.0_f64;
    // Include both possible baseline anchors so every uniformly smaller label
    // remains inside this envelope, including callers with a fixed baseline.
    let baseline_offset = (spec.y - center_y) / chrome_scale;
    let mut top = baseline_offset.min(0.0);
    let mut bottom = baseline_offset.max(0.0);
    for glyph in &layout.glyphs {
        let ink = native_ui_glyph_ink_bounds(glyph);
        reach = reach
            .max((center_x - ink.left) / chrome_scale)
            .max((ink.left + ink.width - center_x) / chrome_scale);
        top = top.min((ink.top - center_y) / chrome_scale);
        bottom = bottom.max((ink.top + ink.height - center_y) / chrome_scale);
    }
    let center = bounds.height / chrome_scale / 2.0 + SURROUND;
    let first_row = (center + top).floor().max(0.0) as usize;
    let last_row = (center + bottom + NATIVE_UI_BUTTON_PRESSED_OFFSET)
        .ceil()
        .clamp(0.0, SURROUND_INK_RIGHT.len() as f64) as usize;
    let end_ink =
        (first_row..last_row).map(|row| SURROUND_INK_RIGHT[row]).max().map_or(0.0, f64::from);
    let half_width = bounds.width / chrome_scale / 2.0
        - (end_ink - SURROUND).max(0.0)
        - NATIVE_UI_BUTTON_PRESSED_OFFSET;
    assert!(half_width > 0.0, "native UI button has no room for label ink");
    spec.scale *= (half_width / reach).min(1.0);
    spec
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_ui_text::NativeUiFontName;

    fn centered_label(text: &str, bounds: HudRect, text_scale: f64) -> NativeUiTextSpec<'_> {
        NativeUiTextSpec {
            scale: text_scale,
            ..NativeUiTextSpec::new(
                NativeUiFontName::Menu,
                text,
                bounds.x + bounds.width / 2.0,
                bounds.y + bounds.height / 2.0 + NATIVE_UI_BUTTON_LABEL_Y_OFFSET * text_scale,
            )
        }
    }

    #[test]
    fn full_leave_labels_clear_both_skulls_at_every_button_scale_and_press_position() {
        for scale in [0.55, 1.0, 1.6] {
            for text in ["LEAVE COLLEGE", "LEAVE PARTY"] {
                let bounds = HudRect::new(100.0, 200.0, 260.0 * scale, 69.0 * scale);
                let label =
                    fit_native_ui_button_label(centered_label(text, bounds, scale), bounds, scale);
                for pressed in [false, true] {
                    let press = if pressed { NATIVE_UI_BUTTON_PRESSED_OFFSET * scale } else { 0.0 };
                    let layout = layout_native_ui_text(&NativeUiTextSpec {
                        x: label.x + press,
                        y: bounds.y
                            + bounds.height / 2.0
                            + NATIVE_UI_BUTTON_LABEL_Y_OFFSET * label.scale
                            + press,
                        ..label
                    });
                    for glyph in &layout.glyphs {
                        let ink = native_ui_glyph_ink_bounds(glyph);
                        // UI.54's complete label band ends at source x=50; the
                        // cap begins six units outside the button's body.
                        assert!(
                            ink.left >= bounds.x + 44.0 * scale - 1e-9,
                            "{text} left at {scale}"
                        );
                        assert!(
                            ink.left + ink.width <= bounds.x + bounds.width - 44.0 * scale + 1e-9,
                            "{text} right at {scale}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn short_labels_and_empty_ink_keep_the_authored_specification() {
        for (text, width, scale) in [
            ("CLOSE", 260.0, 1.0),
            ("KICK", 353.0, 0.55),
            ("RESUME GAME", 353.0, 1.0),
            ("", 140.0, 1.0),
            ("   ", 140.0, 0.55),
        ] {
            let bounds = HudRect::new(0.0, 0.0, width * scale, 69.0 * scale);
            let label = centered_label(text, bounds, scale);
            assert_eq!(fit_native_ui_button_label(label, bounds, scale), label, "{text:?}");
        }
    }

    #[test]
    fn fitting_uses_visible_ink_bearings_instead_of_only_text_advances() {
        // The centre has 80 units per side after UI.54 and the six-unit press
        // motion. LEAVE COLLEGE reaches 114; PARTY's final Y reaches 94 although
        // its advance is only 93 units from the centre.
        let bounds = HudRect::new(0.0, 0.0, 260.0, 69.0);
        for (text, reach) in [("LEAVE COLLEGE", 114.0), ("LEAVE PARTY", 94.0)] {
            assert_eq!(
                fit_native_ui_button_label(centered_label(text, bounds, 1.0), bounds, 1.0).scale,
                80.0 / reach
            );
        }
    }

    #[test]
    fn compact_panel_chrome_preserves_the_authored_larger_font_and_baseline_when_it_fits() {
        for (text, width, height) in [("Boasts", 220.0, 50.0), ("CLOTH", 240.0, 44.0)] {
            let bounds = HudRect::new(0.0, 0.0, width, height);
            let label = centered_label(text, bounds, 1.0);
            assert_eq!(fit_native_ui_button_label(label, bounds, height / 69.0), label, "{text}");
        }
        let bounds = HudRect::new(0.0, 0.0, 250.0, 44.0);
        let label = NativeUiTextSpec {
            scale: 44.0 / 69.0,
            ..NativeUiTextSpec::new(
                NativeUiFontName::Menu,
                "RENAME SACK",
                125.0,
                42.5 * 44.0 / 69.0,
            )
        };
        assert_eq!(fit_native_ui_button_label(label, bounds, 44.0 / 69.0), label);
    }

    #[test]
    fn narrow_notice_actions_fit_while_retaining_their_explicit_native_baseline() {
        let bounds = HudRect::new(0.0, 0.0, 197.0, 69.0);
        for text in ["UNFORGE", "CANCEL", "OKAY"] {
            let original = NativeUiTextSpec::new(NativeUiFontName::Menu, text, 98.5, 45.0);
            let fitted = fit_native_ui_button_label(original, bounds, 1.0);
            assert_eq!(fitted.x, original.x);
            assert_eq!(fitted.y, original.y);
            if text == "OKAY" {
                assert_eq!(fitted, original);
            } else {
                assert!(fitted.scale < 1.0, "{text}");
            }
            for press in [0.0, 6.0] {
                let layout = layout_native_ui_text(&NativeUiTextSpec {
                    x: fitted.x + press,
                    y: fitted.y + press,
                    ..fitted
                });
                for glyph in &layout.glyphs {
                    let ink = native_ui_glyph_ink_bounds(glyph);
                    assert!(ink.left >= 45.0 - 1e-9, "{text} left");
                    assert!(ink.left + ink.width <= 152.0 + 1e-9, "{text} right");
                }
            }
        }
    }
}
