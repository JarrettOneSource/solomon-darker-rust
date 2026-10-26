#![cfg(not(target_arch = "wasm32"))]

use solomon_graphics::text::parley::layout::PositionedLayoutItem;
use solomon_graphics::text::{FontSystem, TextStroke, TextStyle};

fn fonts() -> FontSystem {
    let mut fonts = FontSystem::default();
    fonts
        .register_woff2(include_bytes!(
            "../../../web/assets/stock/fonts/alegreya-latin-400-normal.woff2"
        ))
        .unwrap();
    fonts
}

fn style() -> TextStyle<'static> {
    TextStyle {
        families: "Alegreya, serif",
        size: 28.0,
        weight: 400.0,
        italic: false,
        letter_spacing: 0.0,
        line_height: None,
    }
}

#[test]
fn stock_font_runs_shape_kern_and_rasterize_with_real_ink_and_alpha() {
    let mut fonts = fonts();
    let a = fonts.shape("A", style()).unwrap().layout.full_width();
    let v = fonts.shape("V", style()).unwrap().layout.full_width();
    let pair = fonts.shape("AV", style()).unwrap();
    assert!(pair.layout.full_width() < a + v, "native shaping must retain kerning");
    let text = fonts.shape("AV ffi e\u{301}", style()).unwrap();
    assert!(text.ink.width() > 50.0 && text.ink.height() > 15.0);
    let glyphs: usize = text
        .layout
        .lines()
        .flat_map(|line| line.items())
        .map(|item| match item {
            PositionedLayoutItem::GlyphRun(run) => run.glyphs().count(),
            _ => 0,
        })
        .sum();
    assert!(
        glyphs < "AV ffi e\u{301}".chars().count(),
        "ligatures and combining marks remain shaped"
    );
    let bitmap = fonts.rasterize(
        &text,
        [256, 96],
        [16.0, 16.0],
        [255, 210, 120, 255],
        Some(TextStroke { width: 1.5, color: [20, 10, 0, 255] }),
    );
    let occupied = bitmap.rgba.as_chunks::<4>().0.iter().filter(|pixel| pixel[3] != 0).count();
    assert!(occupied > 100 && occupied < 256 * 96 / 2);
    assert!(
        bitmap
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[..3].iter().all(|channel| *channel <= pixel[3])),
        "text tiles contain premultiplied RGBA"
    );
    assert!(bitmap.rgba[..256 * 4].iter().all(|byte| *byte == 0), "ink fits inside its tile");
}

#[test]
fn mixed_direction_text_retains_visual_clusters_and_logical_source_ranges() {
    let mut fonts = fonts();
    let source = "Wizard مرحبا e\u{301}";
    let text = fonts.shape(source, style()).unwrap();
    let mut rtl = Vec::new();
    for line in text.layout.lines() {
        for run in line.runs() {
            for cluster in run.visual_clusters() {
                let range = cluster.text_range();
                assert!(source.is_char_boundary(range.start) && source.is_char_boundary(range.end));
                if cluster.is_rtl() {
                    rtl.push(range.start);
                }
            }
        }
    }
    assert!(
        rtl.len() >= 4 && rtl.windows(2).all(|pair| pair[0] > pair[1]),
        "Arabic is painted in visual order while retaining original byte ranges"
    );
}

#[test]
fn corrupt_stock_fonts_are_reported_before_ui_readiness() {
    let mut fonts = FontSystem::default();
    assert!(fonts.register_woff2(b"not a font").is_err());
    let mut bad = style();
    bad.size = f32::NAN;
    assert!(fonts.shape("Wizard", bad).is_err());
}

#[test]
fn native_symbol_and_emoji_fallbacks_produce_real_glyphs_and_ink() {
    let mut fonts = fonts();
    for source in ["Native clipboard ✓", "😀"] {
        let text = fonts.shape(source, TextStyle { families: "monospace", ..style() }).unwrap();
        for run in text.layout.lines().flat_map(|line| line.runs()) {
            for glyph in run.visual_clusters().flat_map(|cluster| cluster.glyphs()) {
                assert_ne!(glyph.id, 0, "missing glyph in {source:?}");
            }
        }
        assert!(text.ink.area() > 0.0, "missing raster bounds for {source:?}");
        let bitmap = fonts.rasterize(&text, [320, 96], [12.0, 12.0], [255; 4], None);
        assert!(bitmap.rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] > 0));
    }
}

#[test]
fn synthetic_italic_pixels_fit_the_declared_font_ink() {
    let mut fonts = fonts();
    let text = fonts.shape("Hf", TextStyle { size: 72.0, italic: true, ..style() }).unwrap();
    assert!(
        text.layout
            .lines()
            .flat_map(|line| line.runs())
            .any(|run| run.synthesis().skew().is_some())
    );
    let bitmap = fonts.rasterize(&text, [256, 128], [32.0, 8.0], [255; 4], None);
    for (index, pixel) in
        bitmap.rgba.as_chunks::<4>().0.iter().enumerate().filter(|(_, pixel)| pixel[3] > 32)
    {
        let x = (index % 256) as f64 - 32.0;
        let y = (index / 256) as f64 - 8.0;
        assert!(
            x >= text.ink.x0 - 2.0
                && x <= text.ink.x1 + 2.0
                && y >= text.ink.y0 - 2.0
                && y <= text.ink.y1 + 2.0,
            "opaque pixel ({x}, {y}) falls outside declared ink {:?}: {pixel:?}",
            text.ink
        );
    }
}
