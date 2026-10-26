//! Native font runs, not DOM controls. SVG/Range measure typography only; an
//! unattached Canvas rasterizes text-only tiles into egui-managed GPU textures.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use egui::{Color32, Context, FontId, Painter, Pos2, Rect, TextureHandle, Vec2, pos2, vec2};

use super::geometry::{Caret, Geometry, Line, PaintSpan};
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
use browser as platform;
#[cfg(not(target_arch = "wasm32"))]
use native as platform;
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::gpu_ui) use native::shape as native_shape;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeFontStroke {
    pub width: f32,
    pub color: Color32,
}

const TILE_WIDTH: u32 = 2048;
const TILE_CONTENT: f32 = (TILE_WIDTH - 2) as f32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverflowWrap {
    /// CSS normal wrapping permits an unbreakable word to overflow its line box.
    Normal,
    /// Native editors retain grapheme fallback and their full editable advances.
    #[default]
    BreakWord,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FontRunOptions {
    pub wrap_width: Option<f32>,
    pub overflow_wrap: OverflowWrap,
    /// Preserve trailing text while excluding its spaces from line alignment.
    pub hang_line_end_spaces: bool,
    pub line_height: Option<f32>,
    pub tab_size: Option<f32>,
    pub letter_spacing: Option<f32>,
    pub stroke: Option<NativeFontStroke>,
}

fn wrap_rows(
    text: &str,
    width: Option<f32>,
    probe: &mut platform::FontProbe,
    tab_size: f32,
    overflow_wrap: OverflowWrap,
) -> Result<Vec<std::ops::Range<usize>>, String> {
    let Some(width) = width else { return Ok(std::iter::once(0..text.len()).collect()) };
    if text.is_empty() {
        return Ok(std::iter::once(0..0).collect());
    };
    let mut breaks: Vec<_> = unicode_linebreak::linebreaks(text).map(|(index, _)| index).collect();
    let complex = |character: char| {
        unicode_linebreak::break_property(character as u32)
            == unicode_linebreak::BreakClass::ComplexContext
    };
    if text.chars().any(complex) {
        breaks.extend(probe.word_boundaries(text)?);
        breaks.sort_unstable();
        breaks.dedup();
    }
    let mut start = 0;
    let mut rows = Vec::new();
    while start < text.len() {
        let mut fit = start;
        for &end in breaks.iter().filter(|&&end| end > start) {
            if probe.width(text[start..end].trim_end_matches([' ', '\t']), tab_size)? > width {
                break;
            }
            fit = end;
        }
        if fit == start {
            if overflow_wrap == OverflowWrap::Normal {
                fit = breaks
                    .iter()
                    .copied()
                    .find(|&end| end > start)
                    .expect("a nonempty run ends at a native line break");
            } else {
                for (index, grapheme) in text[start..].grapheme_indices(true) {
                    let end = start + index + grapheme.len();
                    if fit > start && probe.width(&text[start..end], tab_size)? > width {
                        break;
                    }
                    fit = end;
                }
            }
        }
        rows.push(start..fit);
        start = fit;
    }
    Ok(rows)
}

pub(super) fn geometry(
    font: &FontId,
    text: &str,
    options: FontRunOptions,
) -> Result<Arc<Geometry>, String> {
    platform::with_layout_probe(font, options.letter_spacing.unwrap_or(0.0), |probe| {
        let reference = probe.reference()?;
        let mut lines = Vec::new();
        let mut start = 0;
        let mut top = 0.0;
        for paragraph in text.split_inclusive('\n') {
            let body = paragraph.strip_suffix('\n').unwrap_or(paragraph);
            for span in wrap_rows(
                body,
                options.wrap_width,
                probe,
                options.tab_size.unwrap_or(8.0),
                options.overflow_wrap,
            )? {
                let offset = body[..span.start].encode_utf16().count();
                // Empty SVG text nodes still need an explicit node for Range.
                let mut line = if span.is_empty() {
                    empty_line(start + offset, top, reference)
                } else {
                    probe.line(
                        &body[span.clone()],
                        start + offset,
                        top,
                        reference,
                        options.tab_size.unwrap_or(8.0),
                    )?
                };
                if options.hang_line_end_spaces {
                    line.width = probe.width(
                        body[span].trim_end_matches([' ', '\t']),
                        options.tab_size.unwrap_or(8.0),
                    )?;
                }
                if let Some(height) = options.line_height {
                    let leading = (height - line.height) / 2.0;
                    line.baseline += leading;
                    for caret in &mut line.carets {
                        caret.rect =
                            Rect::from_min_size(pos2(caret.rect.min.x, top), vec2(0.0, height));
                    }
                    for cluster in &mut line.clusters {
                        for rect in &mut cluster.rects {
                            *rect = rect.translate(vec2(0.0, leading));
                        }
                    }
                    line.height = height;
                }
                top += line.height;
                lines.push(line);
            }
            start += paragraph.encode_utf16().count();
        }
        if lines.is_empty() || text.ends_with('\n') {
            let mut line = empty_line(start, top, reference);
            if let Some(height) = options.line_height {
                line.baseline += (height - line.height) / 2.0;
                line.height = height;
                line.carets[0].rect = Rect::from_min_size(pos2(0.0, top), vec2(0.0, height));
            }
            top += line.height;
            lines.push(line);
        }
        let width = lines.iter().map(|line| line.width).fold(0.0, f32::max);
        Ok(Arc::new(Geometry { lines, size: vec2(width, top) }))
    })
}

fn empty_line(start: usize, top: f32, reference: Rect) -> Line {
    Line {
        start,
        end: start,
        text: String::new(),
        raster_text: String::new(),
        paint_spans: vec![PaintSpan { source: f32::NEG_INFINITY..f32::INFINITY, offset: 0.0 }],
        top,
        baseline: top - reference.min.y,
        height: reference.height(),
        width: 0.0,
        carets: vec![Caret {
            index: start,
            rect: Rect::from_min_size(pos2(0.0, top), vec2(0.0, reference.height())),
        }],
        clusters: Vec::new(),
    }
}

fn apply_tab_stops(mut line: Line, adjustments: &[(usize, f32, f32)]) -> Line {
    let shift_at = |index: usize| {
        let count = adjustments.partition_point(|(tab, _, _)| *tab < index);
        if count == 0 { 0.0 } else { adjustments[count - 1].2 }
    };
    for caret in &mut line.carets {
        caret.rect = caret.rect.translate(vec2(shift_at(caret.index - line.start), 0.0));
    }
    for (cluster, grapheme) in line.clusters.iter_mut().zip(line.text.graphemes(true)) {
        let shift = shift_at(cluster.start - line.start);
        cluster.leading.x += shift;
        cluster.trailing.x += shift_at(cluster.end - line.start);
        if grapheme == "\t" {
            cluster.rects = vec![Rect::from_min_max(
                pos2(cluster.leading.x, line.top),
                pos2(cluster.trailing.x, line.top + line.height),
            )];
        } else {
            for rect in &mut cluster.rects {
                *rect = rect.translate(vec2(shift, 0.0));
            }
        }
    }
    let mut left = f32::NEG_INFINITY;
    let mut shift = 0.0;
    for (_, cut, next_shift) in adjustments {
        line.paint_spans.push(PaintSpan { source: left..*cut, offset: shift });
        left = *cut;
        shift = *next_shift;
    }
    line.paint_spans.push(PaintSpan { source: left..f32::INFINITY, offset: shift });
    line.width += shift;
    line
}

#[derive(Clone, PartialEq, Eq)]
struct RunKey {
    text: String,
    font: String,
    color: Color32,
    wrap: Option<u32>,
    overflow_wrap: OverflowWrap,
    hang_line_end_spaces: bool,
    line_height: Option<u32>,
    tab_size: Option<u32>,
    letter_spacing: Option<u32>,
    stroke: Option<(u32, Color32)>,
    pixels: u32,
}

#[derive(Clone, Default)]
struct RunCache {
    entries: VecDeque<(RunKey, Arc<NativeFontRun>)>,
    units: usize,
}

pub struct NativeFontRun {
    font: FontId,
    color: Color32,
    pub(crate) geometry: Arc<Geometry>,
    pixels: f32,
    letter_spacing: f32,
    stroke: Option<NativeFontStroke>,
    raster: Vec<RasterLine>,
    tiles: Mutex<BTreeMap<(usize, i32), TextureHandle>>,
}

#[derive(Clone, Copy)]
struct RasterLine {
    bounds: Rect,
    height: u32,
    baseline: f32,
}

fn aligned_origin(line: &Line, bounds: Rect, align: egui::Align, intrinsic_line_box: bool) -> Pos2 {
    let bounds = if intrinsic_line_box {
        Rect::from_min_size(bounds.min, vec2(bounds.width().max(line.width), bounds.height()))
    } else {
        bounds
    };
    pos2(align.align_size_within_range(line.width, bounds.x_range()).min, bounds.top())
}

impl NativeFontRun {
    pub fn size(&self) -> Vec2 {
        self.geometry.size
    }
    pub fn baseline(&self) -> f32 {
        self.geometry.lines[0].baseline
    }

    pub fn ink_bounds(&self) -> Rect {
        self.ink_bounds_aligned(Rect::from_min_size(Pos2::ZERO, self.size()), egui::Align::Min)
    }

    /// Align each native wrapped row within the supplied horizontal bounds.
    /// The top edge remains the run's first line-box edge.
    pub fn ink_bounds_aligned(&self, bounds: Rect, align: egui::Align) -> Rect {
        self.geometry.lines.iter().zip(&self.raster).filter(|(line, _)| !line.text.is_empty()).fold(
            Rect::NOTHING,
            |all, (line, raster)| {
                let origin = aligned_origin(line, bounds, align, false);
                line.paint_spans.iter().fold(all, |all, span| {
                    let source = raster.bounds.intersect(Rect::from_x_y_ranges(
                        span.source.start..=span.source.end,
                        f32::NEG_INFINITY..=f32::INFINITY,
                    ));
                    all.union(source.translate(origin.to_vec2() + vec2(span.offset, 0.0)))
                })
            },
        )
    }

    pub fn paint(
        &self,
        frame: &crate::gpu_ui::UiFrame<'_>,
        painter: &Painter,
        origin: Pos2,
        opacity: f32,
    ) -> Result<(), String> {
        self.paint_aligned(
            frame,
            painter,
            Rect::from_min_size(origin, self.size()),
            egui::Align::Min,
            opacity,
        )
    }

    /// Paint one nowrap line with a native ellipsis, retaining the original
    /// joining/ligatures instead of reshaping a truncated logical string.
    pub fn paint_elided(
        &self,
        frame: &crate::gpu_ui::UiFrame<'_>,
        painter: &Painter,
        bounds: Rect,
        opacity: f32,
    ) -> Result<Rect, String> {
        if self.geometry.lines.len() != 1 {
            return Err("elided font paint requires one unwrapped line".into());
        }
        let clip = painter.clip_rect().intersect(bounds);
        let painter = painter.with_clip_rect(clip);
        if self.size().x <= bounds.width() {
            self.paint(frame, &painter, bounds.min, opacity)?;
            return Ok(self.ink_bounds().translate(bounds.min.to_vec2()).intersect(clip));
        }
        let ellipsis = font_run_with_options(
            frame.ctx,
            self.font.clone(),
            self.color,
            "…",
            FontRunOptions {
                letter_spacing: Some(self.letter_spacing),
                stroke: self.stroke,
                ..Default::default()
            },
        )?;
        let edge =
            self.geometry.lines[0].elision_edge((bounds.width() - ellipsis.size().x).max(0.0));
        let text_clip = clip
            .intersect(Rect::from_min_max(bounds.min, pos2(bounds.left() + edge, bounds.bottom())));
        if edge > 0.0 {
            self.paint(frame, &painter.with_clip_rect(text_clip), bounds.min, opacity)?;
        }
        let origin = bounds.min + vec2(edge, self.baseline() - ellipsis.baseline());
        ellipsis.paint(frame, &painter, origin, opacity)?;
        Ok(self
            .ink_bounds()
            .translate(bounds.min.to_vec2())
            .intersect(text_clip)
            .union(ellipsis.ink_bounds().translate(origin.to_vec2()).intersect(clip)))
    }

    pub fn paint_aligned(
        &self,
        frame: &crate::gpu_ui::UiFrame<'_>,
        painter: &Painter,
        bounds: Rect,
        align: egui::Align,
        opacity: f32,
    ) -> Result<(), String> {
        self.paint_rows(frame, painter, bounds, align, opacity, false)
    }

    /// Paint within independent inline boxes whose width is at least `bounds.width()`.
    /// The caller owns that containing layout and clipping. Overlong rows can
    /// expand their own box without changing the alignment of fitting rows.
    pub fn paint_in_line_boxes(
        &self,
        frame: &crate::gpu_ui::UiFrame<'_>,
        painter: &Painter,
        bounds: Rect,
        align: egui::Align,
        opacity: f32,
    ) -> Result<(), String> {
        self.paint_rows(frame, painter, bounds, align, opacity, true)
    }

    fn paint_rows(
        &self,
        frame: &crate::gpu_ui::UiFrame<'_>,
        painter: &Painter,
        bounds: Rect,
        align: egui::Align,
        opacity: f32,
        intrinsic_line_box: bool,
    ) -> Result<(), String> {
        let pixels = self.pixels;
        for (index, line) in self.geometry.lines.iter().enumerate() {
            let origin = aligned_origin(line, bounds, align, intrinsic_line_box);
            let clip = painter.clip_rect().translate(-origin.to_vec2());
            let bounds = self.raster[index].bounds;
            if line.text.is_empty() {
                continue;
            }
            for span in &line.paint_spans {
                let source_clip =
                    clip.translate(vec2(-span.offset, 0.0)).intersect(Rect::from_x_y_ranges(
                        span.source.start..=span.source.end,
                        f32::NEG_INFINITY..=f32::INFINITY,
                    ));
                if !bounds.intersects(source_clip) {
                    continue;
                }
                let left =
                    (source_clip.min.x.max(bounds.min.x) * pixels / TILE_CONTENT).floor() as i32;
                let right =
                    (source_clip.max.x.min(bounds.max.x) * pixels / TILE_CONTENT).floor() as i32;
                for tile in left..=right {
                    let (handle, offset, size) = self.tile(painter.ctx(), index, tile, pixels)?;
                    let whole = Rect::from_min_size(origin + offset + vec2(span.offset, 0.0), size);
                    // The one-pixel texture gutter is sampled across a tile edge,
                    // but its geometry is cropped so adjacent tiles never overdraw.
                    let core = Rect::from_min_max(
                        whole.min + vec2(1.0 / pixels, 0.0),
                        whole.max - vec2(1.0 / pixels, 0.0),
                    );
                    let destination = whole.intersect(core).intersect(
                        source_clip.translate(origin.to_vec2() + vec2(span.offset, 0.0)),
                    );
                    if !destination.is_positive() {
                        continue;
                    }
                    let uv = Rect::from_min_max(
                        ((destination.min - whole.min) / size).to_pos2(),
                        ((destination.max - whole.min) / size).to_pos2(),
                    );
                    frame.managed_image(
                        painter,
                        handle.id(),
                        destination,
                        Some(uv),
                        Color32::WHITE.gamma_multiply(opacity),
                    );
                }
            }
        }
        Ok(())
    }

    fn tile(
        &self,
        ctx: &Context,
        index: usize,
        tile: i32,
        pixels: f32,
    ) -> Result<(TextureHandle, Vec2, Vec2), String> {
        let line = &self.geometry.lines[index];
        let mut cache = self.tiles.lock().expect("font texture cache");
        let RasterLine { height, baseline, .. } = self.raster[index];
        if let Some(handle) = cache.get(&(index, tile)) {
            return Ok((
                handle.clone(),
                vec2(
                    (tile as f32 * TILE_CONTENT - 1.0) / pixels,
                    line.baseline - baseline / pixels,
                ),
                vec2(TILE_WIDTH as f32 / pixels, height as f32 / pixels),
            ));
        }
        let image = platform::raster_tile(self, index, tile)?;
        let handle = ctx.load_texture("native font run", image, egui::TextureOptions::LINEAR);
        if cache.len() >= 64 {
            let first = *cache.keys().next().expect("nonempty font tile cache");
            cache.remove(&first);
        }
        cache.insert((index, tile), handle.clone());
        Ok((
            handle,
            vec2((tile as f32 * TILE_CONTENT - 1.0) / pixels, line.baseline - baseline / pixels),
            vec2(TILE_WIDTH as f32 / pixels, height as f32 / pixels),
        ))
    }
}

pub fn font_run(
    ctx: &Context,
    font: FontId,
    color: Color32,
    text: &str,
    wrap_width: Option<f32>,
) -> Result<Arc<NativeFontRun>, String> {
    font_run_with_options(
        ctx,
        font,
        color,
        text,
        FontRunOptions { wrap_width, ..Default::default() },
    )
}

pub fn font_run_with_options(
    ctx: &Context,
    font: FontId,
    color: Color32,
    text: &str,
    options: FontRunOptions,
) -> Result<Arc<NativeFontRun>, String> {
    let key = RunKey {
        text: text.to_owned(),
        font: super::super::fonts::font_css(&font.family, font.size),
        color,
        wrap: options.wrap_width.map(f32::to_bits),
        overflow_wrap: options.overflow_wrap,
        hang_line_end_spaces: options.hang_line_end_spaces,
        line_height: options.line_height.map(f32::to_bits),
        tab_size: options.tab_size.map(f32::to_bits),
        letter_spacing: options.letter_spacing.map(f32::to_bits),
        stroke: options.stroke.map(|stroke| (stroke.width.to_bits(), stroke.color)),
        pixels: ctx.pixels_per_point().to_bits(),
    };
    let id = egui::Id::unique("native-font-run-cache");
    let existing = ctx.data_mut(|data| {
        let cache = data.get_temp_mut_or_default::<RunCache>(id);
        let index = cache.entries.iter().position(|(candidate, _)| candidate == &key)?;
        let entry = cache.entries.remove(index).expect("known font cache entry");
        let run = entry.1.clone();
        cache.entries.push_back(entry);
        Some(run)
    });
    if let Some(run) = existing {
        return Ok(run);
    }
    let geometry = geometry(&font, text, options)?;
    let pixels = ctx.pixels_per_point();
    let raster = platform::raster_geometry(&font, &geometry, pixels, options)?;
    let run = Arc::new(NativeFontRun {
        font,
        color,
        geometry,
        pixels,
        letter_spacing: options.letter_spacing.unwrap_or(0.0),
        stroke: options.stroke,
        raster,
        tiles: Mutex::new(BTreeMap::new()),
    });
    ctx.data_mut(|data| {
        let cache = data.get_temp_mut_or_default::<RunCache>(id);
        let units = text.encode_utf16().count();
        cache.units += units;
        cache.entries.push_back((key, run.clone()));
        while cache.entries.len() > 1 && (cache.units > 96 * 1024 || cache.entries.len() > 256) {
            let (key, _) = cache.entries.pop_front().expect("font cache has entries");
            cache.units -= key.text.encode_utf16().count();
        }
    });
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_rows_follow_native_inline_overflow_and_fit_alignment() {
        // Native pinned Cloud h2: at701px, a445.375px inline row starts at148
        // inside its405px clip. A fitting169.09375px phone row starts110.453125.
        let mut line = empty_line(0, 0.0, Rect::from_min_size(Pos2::ZERO, vec2(0.0, 37.8)));
        line.width = 445.375;
        let bounds = Rect::from_min_size(pos2(148.0, 36.0), vec2(405.0, 37.8));
        assert_eq!(aligned_origin(&line, bounds, egui::Align::Center, true), pos2(148.0, 36.0));
        assert_eq!(
            aligned_origin(&line, bounds, egui::Align::Center, false),
            pos2(127.8125, 36.0),
            "existing centered nowrap callers preserve their original placement"
        );
        line.width = 169.093_75;
        let bounds = Rect::from_min_size(pos2(28.0, 80.0), vec2(334.0, 37.8));
        assert_eq!(
            aligned_origin(&line, bounds, egui::Align::Center, true),
            pos2(110.453_125, 80.0)
        );
    }
}
