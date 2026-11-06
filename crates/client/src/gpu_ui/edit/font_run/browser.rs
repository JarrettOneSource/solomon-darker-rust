//! Browser-only native font measurement and text rasterization.
use super::{FontRunOptions, RasterLine, TILE_CONTENT, TILE_WIDTH};
use crate::gpu_ui::edit::geometry::{Caret, Cluster, Geometry, Line};
use egui::{ColorImage, FontId, Rect, pos2, vec2};
use unicode_segmentation::UnicodeSegmentation;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, Document, Element, HtmlCanvasElement, SvgTextContentElement,
};

pub(super) struct FontProbe {
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

    pub(super) fn reference(&self) -> Result<Rect, String> {
        self.text.set_text_content(Some("|ÉqÅM"));
        let node = self.text.first_child().ok_or("no font reference text")?;
        let range =
            self.document.create_range().map_err(|e| format!("font reference range: {e:?}"))?;
        range
            .select_node_contents(&node)
            .map_err(|e| format!("font reference selection: {e:?}"))?;
        let reference = from_dom(&range.get_bounding_client_rect());
        let origin = self.root.get_bounding_client_rect();
        let reference = reference.translate(-vec2(origin.x() as f32, origin.y() as f32));
        if reference.height() <= 0.0 || !reference.is_finite() {
            return Err("native font metrics are not ready".into());
        }
        Ok(reference)
    }

    pub(super) fn word_boundaries(&self, text: &str) -> Result<Vec<usize>, String> {
        let mut boundaries = Vec::new();
        let complex = |character: char| {
            unicode_linebreak::break_property(character as u32)
                == unicode_linebreak::BreakClass::ComplexContext
        };
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
            let byte = crate::gpu_ui::edit::model::byte_index(text, segment.index() as usize);
            if text[..byte].chars().next_back().is_some_and(complex)
                || text[byte..].chars().next().is_some_and(complex)
            {
                boundaries.push(byte);
            }
        }
        Ok(boundaries)
    }

    fn set_font(&self, css: &str, spacing: f32) -> Result<(), String> {
        self.text
            .set_attribute(
                "style",
                &format!("font:{css};white-space:pre;letter-spacing:{spacing}px"),
            )
            .map_err(|e| format!("font style: {e:?}"))?;
        self.set_canvas_font(css, spacing)
    }

    fn set_canvas_font(&self, css: &str, spacing: f32) -> Result<(), String> {
        self.context.set_font(css);
        self.context.set_text_align("left");
        self.context.set_text_baseline("alphabetic");
        let property = wasm_bindgen::JsValue::from_str("letterSpacing");
        let supported = js_sys::Reflect::has(self.context.as_ref(), &property)
            .map_err(|error| format!("native font spacing capability: {error:?}"))?;
        if supported {
            if !js_sys::Reflect::set(
                self.context.as_ref(),
                &property,
                &format!("{spacing}px").into(),
            )
            .map_err(|error| format!("native font spacing: {error:?}"))?
            {
                return Err("native font spacing is not writable".into());
            }
        } else if spacing != 0.0 {
            return Err("this browser does not support native font letter spacing".into());
        }
        Ok(())
    }

    pub(super) fn width(&mut self, text: &str, tab_size: f32) -> Result<f32, String> {
        self.text.set_text_content(Some(&text.replace('\t', "\u{1f}")));
        Ok(self.text.get_computed_text_length()
            + self.tab_adjustments(text, tab_size).last().map_or(0.0, |adjustment| adjustment.2))
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

    pub(super) fn line(
        &mut self,
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
        Ok(super::apply_tab_stops(
            Line {
                start,
                end: start + units,
                text: text.to_owned(),
                raster_text,
                paint_spans: Vec::new(),
                top,
                baseline,
                height: metrics.height(),
                width: self.text.get_computed_text_length(),
                carets,
                clusters,
            },
            &adjustments,
        ))
    }
}

fn from_dom(rect: &web_sys::DomRect) -> Rect {
    Rect::from_min_size(
        pos2(rect.x() as f32, rect.y() as f32),
        vec2(rect.width() as f32, rect.height() as f32),
    )
}

fn with_probe<T>(f: impl FnOnce(&mut FontProbe) -> Result<T, String>) -> Result<T, String> {
    PROBE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(FontProbe::new()?);
        }
        let probe = slot.as_mut().expect("font probe was initialized");
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

pub(super) fn raster_geometry(
    font: &FontId,
    geometry: &Geometry,
    pixels: f32,
    options: FontRunOptions,
) -> Result<Vec<RasterLine>, String> {
    with_probe(|probe| {
        let css = crate::gpu_ui::fonts::font_css(&font.family, font.size * pixels);
        probe.set_canvas_font(&css, options.letter_spacing.unwrap_or(0.0) * pixels)?;
        geometry
            .lines
            .iter()
            .map(|line| {
                let metrics = probe
                    .context
                    .measure_text(&line.raster_text)
                    .map_err(|e| format!("font ink measurement: {e:?}"))?;
                // SVG's default miter limit4 can extend two full stroke widths.
                let stroke_bleed =
                    options.stroke.map_or(0.0, |stroke| stroke.width * pixels * 2.0).ceil();
                let ascent = metrics.actual_bounding_box_ascent().ceil() as f32 + stroke_bleed;
                let descent = metrics.actual_bounding_box_descent().ceil() as f32 + stroke_bleed;
                Ok(RasterLine {
                    height: (ascent + descent + 2.0).max(1.0) as u32,
                    baseline: ascent + 1.0,
                    bounds: Rect::from_min_max(
                        pos2(
                            -(metrics.actual_bounding_box_left().ceil() as f32
                                + stroke_bleed
                                + 1.0)
                                / pixels,
                            line.baseline - (ascent + 1.0) / pixels,
                        ),
                        pos2(
                            (metrics.actual_bounding_box_right().ceil() as f32
                                + stroke_bleed
                                + 1.0)
                                / pixels,
                            line.baseline + (descent + 1.0) / pixels,
                        ),
                    ),
                })
            })
            .collect::<Result<Vec<_>, String>>()
    })
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
    let css = crate::gpu_ui::fonts::font_css(&font.family, font.size * pixels);
    let RasterLine { height, baseline, .. } = raster;
    with_probe(|probe| {
        probe.canvas.set_width(TILE_WIDTH);
        probe.canvas.set_height(height);
        probe.set_canvas_font(&css, letter_spacing * pixels)?;
        let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
        probe.context.set_fill_style_str(&format!(
            "rgba({},{},{},{})",
            red,
            green,
            blue,
            f32::from(alpha) / 255.0
        ));
        let x = 1.0 - f64::from(tile) * f64::from(TILE_CONTENT);
        if let Some(stroke) = stroke {
            let [red, green, blue, alpha] = stroke.color.to_srgba_unmultiplied();
            probe.context.set_stroke_style_str(&format!(
                "rgba({red},{green},{blue},{})",
                f32::from(alpha) / 255.0
            ));
            probe.context.set_line_width(f64::from(stroke.width * pixels));
            probe.context.set_line_join("miter");
            probe.context.set_miter_limit(4.0);
            probe
                .context
                .stroke_text(&line.raster_text, x, f64::from(baseline))
                .map_err(|error| format!("font run stroke: {error:?}"))?;
        }
        probe
            .context
            .fill_text(&line.raster_text, x, f64::from(baseline))
            .map_err(|e| format!("font run paint: {e:?}"))?;
        let data = probe
            .context
            .get_image_data(0.0, 0.0, f64::from(TILE_WIDTH), f64::from(height))
            .map_err(|e| format!("font run bitmap: {e:?}"))?
            .data();
        Ok(ColorImage::from_rgba_unmultiplied([TILE_WIDTH as usize, height as usize], &data.0))
    })
}

pub(super) fn with_layout_probe<T>(
    font: &FontId,
    spacing: f32,
    run: impl FnOnce(&mut FontProbe) -> Result<T, String>,
) -> Result<T, String> {
    with_probe(|probe| {
        probe.set_font(&crate::gpu_ui::fonts::font_css(&font.family, font.size), spacing)?;
        run(probe)
    })
}
