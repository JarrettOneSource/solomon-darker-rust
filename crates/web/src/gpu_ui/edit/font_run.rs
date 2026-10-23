//! Native font runs, not DOM controls. SVG/Range measure typography only; an
//! unattached Canvas rasterizes text-only tiles into egui-managed GPU textures.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use egui::{
    Color32, ColorImage, Context, FontId, Painter, Pos2, Rect, TextureHandle, Vec2, pos2, vec2,
};
use unicode_segmentation::UnicodeSegmentation;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, Document, Element, HtmlCanvasElement, SvgTextContentElement,
};

use super::geometry::{Caret, Cluster, Geometry, Line, PaintSpan};

#[derive(Clone, Copy, Debug, Default)]
pub struct FontRunOptions {
    pub wrap_width: Option<f32>,
    pub line_height: Option<f32>,
    pub tab_size: Option<f32>,
}

struct FontProbe {
    root: Element,
    text: SvgTextContentElement,
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    document: Document,
}

thread_local! {
    static PROBE: std::cell::RefCell<Option<FontProbe>> = const { std::cell::RefCell::new(None) };
}

impl FontProbe {
    fn new() -> Result<Self, String> {
        let document = web_sys::window()
            .ok_or("font geometry needs a window")?
            .document()
            .ok_or("font geometry needs a document")?;
        let namespace = Some("http://www.w3.org/2000/svg");
        let root =
            document.create_element_ns(namespace, "svg").map_err(|e| format!("font SVG: {e:?}"))?;
        for (name, value) in [
            ("data-gpu-font-metrics", ""),
            ("aria-hidden", "true"),
            ("focusable", "false"),
            ("width", "1"),
            ("height", "1"),
            (
                "style",
                "position:fixed;left:0;top:0;visibility:hidden;pointer-events:none;overflow:hidden",
            ),
        ] {
            root.set_attribute(name, value).map_err(|e| format!("font SVG attribute: {e:?}"))?;
        }
        let text: SvgTextContentElement = document
            .create_element_ns(namespace, "text")
            .map_err(|e| format!("font text: {e:?}"))?
            .dyn_into()
            .map_err(|_| "font text is not SVG text")?;
        text.set_attribute("x", "0").map_err(|e| format!("font origin: {e:?}"))?;
        text.set_attribute("y", "0").map_err(|e| format!("font baseline: {e:?}"))?;
        root.append_child(&text).map_err(|e| format!("font text attachment: {e:?}"))?;
        let canvas: HtmlCanvasElement = document
            .create_element("canvas")
            .map_err(|e| format!("font canvas: {e:?}"))?
            .dyn_into()
            .map_err(|_| "font canvas is not a canvas")?;
        let context: CanvasRenderingContext2d = canvas
            .get_context("2d")
            .map_err(|e| format!("font context: {e:?}"))?
            .ok_or("no font context")?
            .dyn_into()
            .map_err(|_| "font context is not 2D")?;
        Ok(Self { root, text, canvas, context, document })
    }

    fn set_font(&self, css: &str) -> Result<(), String> {
        self.text
            .set_attribute("style", &format!("font:{css};white-space:pre"))
            .map_err(|e| format!("font style: {e:?}"))?;
        self.context.set_font(css);
        self.context.set_text_align("left");
        self.context.set_text_baseline("alphabetic");
        Ok(())
    }

    fn width(&self, text: &str, tab_size: f32) -> f32 {
        self.text.set_text_content(Some(&text.replace('\t', "\u{1f}")));
        self.text.get_computed_text_length()
            + self.tab_adjustments(text, tab_size).last().map_or(0.0, |adjustment| adjustment.2)
    }

    fn tab_adjustments(&self, text: &str, tab_size: f32) -> Vec<(usize, f32, f32)> {
        let space = self.context.measure_text(" ").expect("native space metrics").width() as f32;
        let interval = space * tab_size;
        let mut shift = 0.0;
        let mut index = 0;
        let mut result = Vec::new();
        for character in text.chars() {
            if character == '\t' {
                let start = self
                    .text
                    .get_start_position_of_char(index as u32)
                    .expect("native tab start")
                    .x();
                let end =
                    self.text.get_end_position_of_char(index as u32).expect("native tab end").x();
                let x = start + shift;
                let mut stop =
                    if interval > 0.0 { ((x / interval).floor() + 1.0) * interval } else { x };
                if interval > 0.0 && stop - x < space / 2.0 {
                    stop += interval;
                }
                // Browser inline typography quantizes CSS advances to 1/64px.
                stop = (stop * 64.0).ceil() / 64.0;
                shift += stop - x - (end - start);
                result.push((index, (start + end) / 2.0, shift));
            }
            index += character.len_utf16();
        }
        result
    }

    fn line(
        &self,
        text: &str,
        start: usize,
        top: f32,
        empty_metrics: Rect,
        tab_size: f32,
    ) -> Result<Line, String> {
        let raster_text = text.replace('\t', "\u{1f}");
        self.text.set_text_content(Some(&raster_text));
        let node = self.text.first_child().ok_or("font run has no text node")?;
        let offset = self.root.get_bounding_client_rect();
        let offset = vec2(offset.x() as f32, offset.y() as f32);
        let range = self.document.create_range().map_err(|e| format!("font range: {e:?}"))?;
        let units = text.encode_utf16().count();
        range.set_start(&node, 0).map_err(|e| format!("font range start: {e:?}"))?;
        range.set_end(&node, units as u32).map_err(|e| format!("font range end: {e:?}"))?;
        let native = from_dom(&range.get_bounding_client_rect()).translate(-offset);
        let metrics = if text.is_empty() { empty_metrics } else { native };
        let baseline = top - metrics.min.y;
        let shift = vec2(0.0, baseline) - offset;
        let mut carets = Vec::new();
        let mut clusters = Vec::new();
        let mut index = 0;
        for grapheme in text.graphemes(true) {
            let end = index + grapheme.encode_utf16().count();
            range.set_start(&node, index as u32).map_err(|e| format!("font caret: {e:?}"))?;
            range.set_end(&node, index as u32).map_err(|e| format!("font caret: {e:?}"))?;
            carets.push(Caret {
                index: start + index,
                rect: from_dom(&range.get_bounding_client_rect()).translate(shift),
            });
            range.set_end(&node, end as u32).map_err(|e| format!("font selection: {e:?}"))?;
            let rects = range.get_client_rects().ok_or("native font range has no rectangles")?;
            let rects: Vec<_> = (0..rects.length())
                .filter_map(|i| rects.item(i))
                .map(|r| from_dom(&r).translate(shift))
                .filter(|r| r.is_positive())
                .collect();
            let leading = self
                .text
                .get_start_position_of_char(index as u32)
                .map_err(|e| format!("font cluster start: {e:?}"))?;
            let trailing = self
                .text
                .get_end_position_of_char((end - 1) as u32)
                .map_err(|e| format!("font cluster end: {e:?}"))?;
            clusters.push(Cluster {
                start: start + index,
                end: start + end,
                leading: pos2(leading.x(), leading.y() + baseline),
                trailing: pos2(trailing.x(), trailing.y() + baseline),
                rects,
            });
            index = end;
        }
        range.set_start(&node, units as u32).map_err(|e| format!("font final caret: {e:?}"))?;
        range.set_end(&node, units as u32).map_err(|e| format!("font final caret: {e:?}"))?;
        let rect = if text.is_empty() {
            Rect::from_min_size(pos2(0.0, top), vec2(0.0, empty_metrics.height()))
        } else {
            from_dom(&range.get_bounding_client_rect()).translate(shift)
        };
        carets.push(Caret { index: start + units, rect });
        let adjustments = self.tab_adjustments(text, tab_size);
        let shift_at = |index: usize| {
            let count = adjustments.partition_point(|(tab, _, _)| *tab < index);
            if count == 0 { 0.0 } else { adjustments[count - 1].2 }
        };
        for caret in &mut carets {
            caret.rect = caret.rect.translate(vec2(shift_at(caret.index - start), 0.0));
        }
        for (cluster, grapheme) in clusters.iter_mut().zip(text.graphemes(true)) {
            let shift = shift_at(cluster.start - start);
            cluster.leading.x += shift;
            cluster.trailing.x += shift_at(cluster.end - start);
            if grapheme == "\t" {
                cluster.rects = vec![Rect::from_min_max(
                    pos2(cluster.leading.x, top),
                    pos2(cluster.trailing.x, top + metrics.height()),
                )];
            } else {
                for rect in &mut cluster.rects {
                    *rect = rect.translate(vec2(shift, 0.0));
                }
            }
        }
        let mut paint_spans = Vec::new();
        let mut left = f32::NEG_INFINITY;
        let mut shift = 0.0;
        for (_, cut, next_shift) in &adjustments {
            paint_spans.push(PaintSpan { source: left..*cut, offset: shift });
            left = *cut;
            shift = *next_shift;
        }
        paint_spans.push(PaintSpan { source: left..f32::INFINITY, offset: shift });
        Ok(Line {
            start,
            end: start + units,
            text: text.to_owned(),
            raster_text,
            paint_spans,
            top,
            baseline,
            height: metrics.height(),
            width: self.text.get_computed_text_length() + shift,
            carets,
            clusters,
        })
    }
}

fn from_dom(rect: &web_sys::DomRect) -> Rect {
    Rect::from_min_size(
        pos2(rect.x() as f32, rect.y() as f32),
        vec2(rect.width() as f32, rect.height() as f32),
    )
}

fn with_probe<T>(f: impl FnOnce(&FontProbe) -> Result<T, String>) -> Result<T, String> {
    PROBE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(FontProbe::new()?);
        }
        let probe = slot.as_ref().expect("font probe was initialized");
        probe
            .document
            .body()
            .ok_or("no font measurement body")?
            .append_child(&probe.root)
            .map_err(|e| format!("font measurement attachment: {e:?}"))?;
        let result = f(probe);
        probe.root.remove();
        result
    })
}

fn wrap_rows(
    text: &str,
    width: Option<f32>,
    probe: &FontProbe,
    tab_size: f32,
) -> Result<Vec<std::ops::Range<usize>>, String> {
    let Some(width) = width else { return Ok(vec![0..text.len()]) };
    if text.is_empty() {
        return Ok(vec![0..0]);
    };
    let mut breaks: Vec<_> = unicode_linebreak::linebreaks(text).map(|(index, _)| index).collect();
    let complex = |character: char| {
        unicode_linebreak::break_property(character as u32)
            == unicode_linebreak::BreakClass::ComplexContext
    };
    if text.chars().any(complex) {
        let options = js_sys::Intl::SegmenterOptions::new();
        options.set_granularity(js_sys::Intl::SegmenterGranularity::Word);
        let segmenter = js_sys::Intl::Segmenter::new(&js_sys::Array::new(), options.as_ref());
        let segments = segmenter.segment(text);
        let iterator = js_sys::try_iter(&segments)
            .map_err(|error| format!("native word boundaries: {error:?}"))?
            .ok_or("native word boundaries are not iterable")?;
        for segment in iterator {
            let segment: js_sys::Intl::SegmentData = segment
                .map_err(|error| format!("native word boundary: {error:?}"))?
                .unchecked_into();
            let byte = super::model::byte_index(text, segment.index() as usize);
            if text[..byte].chars().next_back().is_some_and(complex)
                || text[byte..].chars().next().is_some_and(complex)
            {
                breaks.push(byte);
            }
        }
        breaks.sort_unstable();
        breaks.dedup();
    }
    let mut start = 0;
    let mut rows = Vec::new();
    while start < text.len() {
        let mut fit = start;
        for &end in breaks.iter().filter(|&&end| end > start) {
            if probe.width(text[start..end].trim_end_matches([' ', '\t']), tab_size) > width {
                break;
            }
            fit = end;
        }
        if fit == start {
            for (index, grapheme) in text[start..].grapheme_indices(true) {
                let end = start + index + grapheme.len();
                if fit > start && probe.width(&text[start..end], tab_size) > width {
                    break;
                }
                fit = end;
            }
        }
        rows.push(start..fit);
        start = fit;
    }
    Ok(rows)
}

pub fn geometry(
    font: &FontId,
    text: &str,
    options: FontRunOptions,
) -> Result<Arc<Geometry>, String> {
    let css = super::super::fonts::font_css(&font.family, font.size);
    with_probe(|probe| {
        probe.set_font(&css)?;
        probe.text.set_text_content(Some("|ÉqÅM"));
        let node = probe.text.first_child().ok_or("no font reference text")?;
        let range =
            probe.document.create_range().map_err(|e| format!("font reference range: {e:?}"))?;
        range
            .select_node_contents(&node)
            .map_err(|e| format!("font reference selection: {e:?}"))?;
        let reference = from_dom(&range.get_bounding_client_rect());
        let origin = probe.root.get_bounding_client_rect();
        let reference = reference.translate(-vec2(origin.x() as f32, origin.y() as f32));
        if reference.height() <= 0.0 || !reference.is_finite() {
            return Err("native font metrics are not ready".into());
        }
        let mut lines = Vec::new();
        let mut start = 0;
        let mut top = 0.0;
        for paragraph in text.split_inclusive('\n') {
            let body = paragraph.strip_suffix('\n').unwrap_or(paragraph);
            for span in wrap_rows(body, options.wrap_width, probe, options.tab_size.unwrap_or(8.0))?
            {
                let offset = body[..span.start].encode_utf16().count();
                // Empty SVG text nodes still need an explicit node for Range.
                let mut line = if span.is_empty() {
                    empty_line(start + offset, top, reference)
                } else {
                    probe.line(
                        &body[span],
                        start + offset,
                        top,
                        reference,
                        options.tab_size.unwrap_or(8.0),
                    )?
                };
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

#[derive(Clone, PartialEq, Eq)]
struct RunKey {
    text: String,
    font: String,
    color: Color32,
    wrap: Option<u32>,
    line_height: Option<u32>,
    tab_size: Option<u32>,
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
    raster: Vec<RasterLine>,
    tiles: Mutex<BTreeMap<(usize, i32), TextureHandle>>,
}

#[derive(Clone, Copy)]
struct RasterLine {
    bounds: Rect,
    height: u32,
    baseline: f32,
}

impl NativeFontRun {
    pub fn size(&self) -> Vec2 {
        self.geometry.size
    }
    pub fn baseline(&self) -> f32 {
        self.geometry.lines[0].baseline
    }

    pub fn ink_bounds(&self) -> Rect {
        self.geometry.lines.iter().zip(&self.raster).fold(Rect::NOTHING, |all, (line, raster)| {
            line.paint_spans.iter().fold(all, |all, span| {
                let source = raster.bounds.intersect(Rect::from_x_y_ranges(
                    span.source.start..=span.source.end,
                    f32::NEG_INFINITY..=f32::INFINITY,
                ));
                all.union(source.translate(vec2(span.offset, 0.0)))
            })
        })
    }

    pub fn paint(&self, painter: &Painter, origin: Pos2, opacity: f32) -> Result<(), String> {
        let pixels = self.pixels;
        let clip = painter.clip_rect().translate(-origin.to_vec2());
        for (index, line) in self.geometry.lines.iter().enumerate() {
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
                let left = (source_clip.min.x.max(bounds.min.x) * pixels / 2048.0).floor() as i32;
                let right = (source_clip.max.x.min(bounds.max.x) * pixels / 2048.0).floor() as i32;
                for tile in left..=right {
                    let (handle, offset, size) = self.tile(painter.ctx(), index, tile, pixels)?;
                    let whole = Rect::from_min_size(origin + offset + vec2(span.offset, 0.0), size);
                    let destination = whole.intersect(
                        source_clip.translate(origin.to_vec2() + vec2(span.offset, 0.0)),
                    );
                    if !destination.is_positive() {
                        continue;
                    }
                    let uv = Rect::from_min_max(
                        ((destination.min - whole.min) / size).to_pos2(),
                        ((destination.max - whole.min) / size).to_pos2(),
                    );
                    painter.image(
                        handle.id(),
                        destination,
                        uv,
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
        let css = super::super::fonts::font_css(&self.font.family, self.font.size * pixels);
        let mut cache = self.tiles.lock().expect("font texture cache");
        let RasterLine { height, baseline, .. } = self.raster[index];
        if let Some(handle) = cache.get(&(index, tile)) {
            return Ok((
                handle.clone(),
                vec2(tile as f32 * 2048.0 / pixels, line.baseline - baseline / pixels),
                vec2(2048.0 / pixels, height as f32 / pixels),
            ));
        }
        let image = with_probe(|probe| {
            probe.canvas.set_width(2048);
            probe.canvas.set_height(height);
            probe.context.set_font(&css);
            probe.context.set_text_align("left");
            probe.context.set_text_baseline("alphabetic");
            let [red, green, blue, alpha] = self.color.to_srgba_unmultiplied();
            probe.context.set_fill_style_str(&format!(
                "rgba({},{},{},{})",
                red,
                green,
                blue,
                f32::from(alpha) / 255.0
            ));
            probe
                .context
                .fill_text(&line.raster_text, -f64::from(tile) * 2048.0, f64::from(baseline))
                .map_err(|e| format!("font run paint: {e:?}"))?;
            let data = probe
                .context
                .get_image_data(0.0, 0.0, 2048.0, f64::from(height))
                .map_err(|e| format!("font run bitmap: {e:?}"))?
                .data();
            Ok(ColorImage::from_rgba_unmultiplied([2048, height as usize], &data.0))
        })?;
        let handle = ctx.load_texture("native font run", image, egui::TextureOptions::LINEAR);
        if cache.len() >= 64 {
            let first = *cache.keys().next().expect("nonempty font tile cache");
            cache.remove(&first);
        }
        cache.insert((index, tile), handle.clone());
        Ok((
            handle,
            vec2(tile as f32 * 2048.0 / pixels, line.baseline - baseline / pixels),
            vec2(2048.0 / pixels, height as f32 / pixels),
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
        line_height: options.line_height.map(f32::to_bits),
        tab_size: options.tab_size.map(f32::to_bits),
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
    let raster = with_probe(|probe| {
        let css = super::super::fonts::font_css(&font.family, font.size * pixels);
        probe.context.set_font(&css);
        geometry
            .lines
            .iter()
            .map(|line| {
                let metrics = probe
                    .context
                    .measure_text(&line.raster_text)
                    .map_err(|e| format!("font ink measurement: {e:?}"))?;
                let ascent = metrics.actual_bounding_box_ascent().ceil() as f32;
                let descent = metrics.actual_bounding_box_descent().ceil() as f32;
                Ok(RasterLine {
                    height: (ascent + descent + 2.0).max(1.0) as u32,
                    baseline: ascent + 1.0,
                    bounds: Rect::from_min_max(
                        pos2(
                            -(metrics.actual_bounding_box_left().ceil() as f32 + 1.0) / pixels,
                            line.baseline - (ascent + 1.0) / pixels,
                        ),
                        pos2(
                            (metrics.actual_bounding_box_right().ceil() as f32 + 1.0) / pixels,
                            line.baseline + (descent + 1.0) / pixels,
                        ),
                    ),
                })
            })
            .collect::<Result<Vec<_>, String>>()
    })?;
    let run = Arc::new(NativeFontRun {
        font,
        color,
        geometry,
        pixels,
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
