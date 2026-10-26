//! Native metrics and raster tiles for the shared wrapping/editing/paint owner.

use egui::{ColorImage, FontId, Rect, pos2, vec2};
use parley::editing::{Cursor, Selection};
use parley::layout::Affinity;
use solomon_graphics::text::{ShapedText, TextStroke, TextStyle, parley};
use unicode_segmentation::UnicodeSegmentation;

use super::{FontRunOptions, RasterLine, TILE_CONTENT, TILE_WIDTH};
use crate::gpu_ui::edit::geometry::{Caret, Cluster, Geometry, Line};
use crate::gpu_ui::fonts::{font_spec, with_fonts};

pub(super) struct FontProbe {
    font: FontId,
    spacing: f32,
}

pub(super) fn with_layout_probe<T>(
    font: &FontId,
    spacing: f32,
    run: impl FnOnce(&mut FontProbe) -> Result<T, String>,
) -> Result<T, String> {
    run(&mut FontProbe { font: font.clone(), spacing })
}

impl FontProbe {
    fn shape(&self, text: &str) -> Result<ShapedText, String> {
        shape(&self.font, text, self.spacing)
    }

    pub(super) fn reference(&self) -> Result<Rect, String> {
        let shaped = self.shape("|ÉqÅM")?;
        let line = shaped.layout.lines().next().ok_or("Native font reference has no line")?;
        let metrics = line.metrics();
        Ok(Rect::from_min_size(pos2(0.0, -metrics.baseline), vec2(0.0, metrics.line_height)))
    }

    pub(super) fn word_boundaries(&self, text: &str) -> Result<Vec<usize>, String> {
        let shaped = self.shape(text)?;
        let complex = |character: char| {
            unicode_linebreak::break_property(character as u32)
                == unicode_linebreak::BreakClass::ComplexContext
        };
        let mut boundaries = Vec::new();
        for line in shaped.layout.lines() {
            for run in line.runs() {
                for cluster in run.visual_clusters().filter(|cluster| cluster.is_word_boundary()) {
                    let byte = cluster.text_range().start;
                    if text[..byte].chars().next_back().is_some_and(complex)
                        || text[byte..].chars().next().is_some_and(complex)
                    {
                        boundaries.push(byte);
                    }
                }
            }
        }
        Ok(boundaries)
    }

    fn tab_adjustments(
        &self,
        text: &str,
        shaped: &ShapedText,
        tab_size: f32,
    ) -> Result<Vec<(usize, f32, f32)>, String> {
        if !text.contains('\t') {
            return Ok(Vec::new());
        }
        let space = self.shape(" ")?.layout.full_width();
        let interval = space * tab_size;
        let mut shift = 0.0;
        let mut units = 0;
        let mut adjustments = Vec::new();
        for (byte, character) in text.char_indices() {
            if character == '\t' {
                let start = Cursor::from_byte_index(&shaped.layout, byte, Affinity::Downstream)
                    .geometry(&shaped.layout, 0.0)
                    .x0 as f32;
                let end = Cursor::from_byte_index(&shaped.layout, byte + 1, Affinity::Upstream)
                    .geometry(&shaped.layout, 0.0)
                    .x0 as f32;
                let x = start + shift;
                let mut stop =
                    if interval > 0.0 { ((x / interval).floor() + 1.0) * interval } else { x };
                if interval > 0.0 && stop - x < space / 2.0 {
                    stop += interval;
                }
                stop = (stop * 64.0).ceil() / 64.0;
                shift += stop - x - (end - start);
                adjustments.push((units, (start + end) / 2.0, shift));
            }
            units += character.len_utf16();
        }
        Ok(adjustments)
    }

    pub(super) fn width(&mut self, text: &str, tab_size: f32) -> Result<f32, String> {
        let shaped = self.shape(&text.replace('\t', " "))?;
        let shift =
            self.tab_adjustments(text, &shaped, tab_size)?.last().map_or(0.0, |entry| entry.2);
        Ok(shaped.layout.full_width() + shift)
    }

    pub(super) fn line(
        &mut self,
        text: &str,
        start: usize,
        top: f32,
        empty_metrics: Rect,
        tab_size: f32,
    ) -> Result<Line, String> {
        if text.is_empty() {
            return Ok(super::empty_line(start, top, empty_metrics));
        }
        let raster_text = text.replace('\t', " ");
        let shaped = self.shape(&raster_text)?;
        let first = shaped.layout.lines().next().ok_or("Native font run has no line")?;
        let metrics = first.metrics();
        let baseline = top + metrics.baseline;
        let height = metrics.line_height;
        let mut carets = Vec::new();
        let mut clusters = Vec::new();
        let mut units = 0;
        for (byte, grapheme) in text.grapheme_indices(true) {
            let end_byte = byte + grapheme.len();
            let end_units = units + grapheme.encode_utf16().count();
            let leading = Cursor::from_byte_index(&shaped.layout, byte, Affinity::Downstream);
            let trailing = Cursor::from_byte_index(&shaped.layout, end_byte, Affinity::Upstream);
            let caret =
                from_bounds(leading.geometry(&shaped.layout, 0.0)).translate(vec2(0.0, top));
            carets.push(Caret { index: start + units, rect: caret });
            let last = trailing.geometry(&shaped.layout, 0.0);
            let rects = Selection::new(leading, trailing)
                .geometry(&shaped.layout)
                .into_iter()
                .map(|(rect, _)| from_bounds(rect).translate(vec2(0.0, top)))
                .filter(Rect::is_positive)
                .collect();
            clusters.push(Cluster {
                start: start + units,
                end: start + end_units,
                leading: pos2(caret.left(), baseline),
                trailing: pos2(last.x0 as f32, baseline),
                rects,
            });
            units = end_units;
        }
        let last = Cursor::from_byte_index(&shaped.layout, raster_text.len(), Affinity::Upstream)
            .geometry(&shaped.layout, 0.0);
        carets.push(Caret {
            index: start + units,
            rect: from_bounds(last).translate(vec2(0.0, top)),
        });
        let adjustments = self.tab_adjustments(text, &shaped, tab_size)?;
        Ok(super::apply_tab_stops(
            Line {
                start,
                end: start + units,
                text: text.to_owned(),
                raster_text,
                paint_spans: Vec::new(),
                top,
                baseline,
                height,
                width: shaped.layout.full_width(),
                carets,
                clusters,
            },
            &adjustments,
        ))
    }
}

fn from_bounds(bounds: parley::BoundingBox) -> Rect {
    Rect::from_min_max(
        pos2(bounds.x0 as f32, bounds.y0 as f32),
        pos2(bounds.x1 as f32, bounds.y1 as f32),
    )
}

pub(in crate::gpu_ui) fn shape(
    font: &FontId,
    text: &str,
    spacing: f32,
) -> Result<ShapedText, String> {
    let (weight, style, family) = font_spec(&font.family);
    let family = family.replace("SDGPU ", "");
    with_fonts(|fonts| {
        fonts.shape(
            text,
            TextStyle {
                families: &family,
                size: font.size,
                weight: weight.parse().expect("font weight"),
                italic: style == "italic",
                letter_spacing: spacing,
                line_height: None,
            },
        )
    })
}

pub(super) fn raster_geometry(
    font: &FontId,
    geometry: &Geometry,
    pixels: f32,
    options: FontRunOptions,
) -> Result<Vec<RasterLine>, String> {
    let font = FontId::new(font.size * pixels, font.family.clone());
    geometry
        .lines
        .iter()
        .map(|line| {
            let text =
                shape(&font, &line.raster_text, options.letter_spacing.unwrap_or(0.0) * pixels)?;
            let baseline = text.layout.lines().next().map_or(0.0, |line| line.metrics().baseline);
            let ink = text.ink;
            let bleed = options.stroke.map_or(0.0, |stroke| stroke.width * pixels * 2.0).ceil();
            let (ascent, descent) = if ink.area() == 0.0 {
                (bleed, bleed)
            } else {
                (
                    (baseline - ink.y0 as f32).ceil() + bleed,
                    (ink.y1 as f32 - baseline).ceil() + bleed,
                )
            };
            Ok(RasterLine {
                height: (ascent + descent + 2.0).max(1.0) as u32,
                baseline: ascent + 1.0,
                bounds: Rect::from_min_max(
                    pos2(
                        (ink.x0.floor() as f32 - bleed - 1.0) / pixels,
                        line.baseline - (ascent + 1.0) / pixels,
                    ),
                    pos2(
                        (ink.x1.ceil() as f32 + bleed + 1.0) / pixels,
                        line.baseline + (descent + 1.0) / pixels,
                    ),
                ),
            })
        })
        .collect()
}

pub(super) fn raster_tile(
    run: &super::NativeFontRun,
    index: usize,
    tile: i32,
) -> Result<ColorImage, String> {
    let font = &run.font;
    let color = run.color;
    let letter_spacing = run.letter_spacing;
    let stroke = run.stroke;
    let pixels = run.pixels;
    let line = &run.geometry.lines[index];
    let raster = run.raster[index];
    let font = FontId::new(font.size * pixels, font.family.clone());
    let text = shape(&font, &line.raster_text, letter_spacing * pixels)?;
    let baseline = text.layout.lines().next().map_or(0.0, |line| line.metrics().baseline);
    let height = u16::try_from(raster.height).map_err(|_| "Native font tile is too tall")?;
    let bitmap = with_fonts(|fonts| {
        Ok(fonts.rasterize(
            &text,
            [TILE_WIDTH as u16, height],
            [
                1.0 - f64::from(tile) * f64::from(TILE_CONTENT),
                f64::from(raster.baseline - baseline),
            ],
            color.to_srgba_unmultiplied(),
            stroke.map(|stroke| TextStroke {
                width: stroke.width * pixels,
                color: stroke.color.to_srgba_unmultiplied(),
            }),
        ))
    })?;
    Ok(ColorImage::from_rgba_premultiplied(
        [bitmap.width as usize, bitmap.height as usize],
        &bitmap.rgba,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_tab_advances_without_painting_a_control_glyph() {
        let _fonts = crate::gpu_ui::fonts::test_fonts();
        let font = FontId::new(24.0, egui::FontFamily::Name("Alegreya".into()));
        let geometry = super::super::geometry(&font, "\t", FontRunOptions::default()).unwrap();
        assert!(geometry.lines[0].width > 0.0);
        let shaped = shape(&font, &geometry.lines[0].raster_text, 0.0).unwrap();
        let bitmap =
            with_fonts(|fonts| Ok(fonts.rasterize(&shaped, [128, 64], [0.0, 0.0], [255; 4], None)))
                .unwrap();
        assert!(bitmap.rgba.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 0));
    }
}
