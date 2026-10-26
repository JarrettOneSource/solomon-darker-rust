//! Native font loading, Unicode shaping and text-only CPU rasterization.
//! The existing GPU painter owns the resulting tiles. Parley preserves mixed
//! direction text and cluster geometry; Vello shares the existing font stack.

pub use parley;
pub use vello_cpu::kurbo::Rect;

use parley::layout::{Alignment, AlignmentOptions, PositionedLayoutItem};
use parley::style::{FontStyle, FontWeight, LineHeight};
use parley::{FontContext, Layout, LayoutContext, StyleProperty};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use vello_cpu::kurbo::{Affine, Diagonal2, Stroke};
use vello_cpu::{Glyph, Pixmap, RenderContext, Resources};

pub struct TextStyle<'a> {
    /// A CSS family list; registered stock faces precede system fallbacks.
    pub families: &'a str,
    pub size: f32,
    pub weight: f32,
    pub italic: bool,
    pub letter_spacing: f32,
    pub line_height: Option<f32>,
}

#[derive(Default)]
pub struct FontSystem {
    fonts: FontContext,
    layout: LayoutContext<()>,
    resources: Resources,
}

pub struct ShapedText {
    /// Native layout is also the source for cursor and selection geometry.
    pub layout: Layout<()>,
    pub ink: Rect,
}

pub struct TextBitmap {
    pub width: u16,
    pub height: u16,
    /// RGBA in premultiplied sRGB, ready for an egui managed texture.
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy)]
pub struct TextStroke {
    pub width: f32,
    pub color: [u8; 4],
}

impl FontSystem {
    pub fn register_woff2(&mut self, bytes: &[u8]) -> Result<(), String> {
        let font =
            wuff::decompress_woff2(bytes).map_err(|error| format!("WOFF2 font: {error:?}"))?;
        if self.fonts.collection.register_fonts(font.into(), None).is_empty() {
            return Err("Decoded font contains no usable face".into());
        }
        Ok(())
    }

    pub fn shape(&mut self, text: &str, style: TextStyle<'_>) -> Result<ShapedText, String> {
        if !style.size.is_finite() || style.size <= 0.0 {
            return Err("Font size must be finite and positive".into());
        }
        // Script fallbacks may choose a Latin face without common symbols. The
        // platform's mathematical font supplies checks/arrows after the requested faces.
        let families = format!("{}, math", style.families);
        let mut builder = self.layout.ranged_builder(&mut self.fonts, text, 1.0, false);
        builder.push_default(StyleProperty::FontFamily(families.as_str().into()));
        builder.push_default(StyleProperty::FontSize(style.size));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(style.weight)));
        builder.push_default(StyleProperty::FontStyle(if style.italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        }));
        builder.push_default(StyleProperty::LetterSpacing(style.letter_spacing));
        if let Some(height) = style.line_height {
            builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(height)));
        }
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        // The source text containers own alignment; runs retain visual order
        // within a left-origin line even when their paragraph is right-to-left.
        layout.align(Alignment::Left, AlignmentOptions::default());
        let ink = ink_bounds(&layout)?;
        Ok(ShapedText { layout, ink })
    }

    pub fn rasterize(
        &mut self,
        text: &ShapedText,
        size: [u16; 2],
        origin: [f64; 2],
        color: [u8; 4],
        stroke: Option<TextStroke>,
    ) -> TextBitmap {
        let [width, height] = size;
        let mut context = RenderContext::new(width, height);
        context.set_transform(Affine::translate((origin[0], origin[1])));
        if let Some(stroke) = stroke {
            context.set_paint(vello_cpu::color::AlphaColor::<vello_cpu::color::Srgb>::from_rgba8(
                stroke.color[0],
                stroke.color[1],
                stroke.color[2],
                stroke.color[3],
            ));
            context.set_stroke(Stroke::new(f64::from(stroke.width)).with_miter_limit(4.0));
            draw(&mut context, &mut self.resources, &text.layout, true);
        }
        context.set_paint(vello_cpu::color::AlphaColor::<vello_cpu::color::Srgb>::from_rgba8(
            color[0], color[1], color[2], color[3],
        ));
        draw(&mut context, &mut self.resources, &text.layout, false);
        context.flush();
        let mut bitmap = Pixmap::new(width, height);
        context.render(&mut bitmap, &mut self.resources);
        let rgba =
            bitmap.data().iter().flat_map(|pixel| [pixel.r, pixel.g, pixel.b, pixel.a]).collect();
        TextBitmap { width, height, rgba }
    }
}

fn draw(context: &mut RenderContext, resources: &mut Resources, layout: &Layout<()>, stroke: bool) {
    for line in layout.lines() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyphs) = item else { continue };
            let run = glyphs.run();
            let mut painter = context
                .glyph_run(resources, run.font())
                .font_size(run.font_size())
                .normalized_coords(run.normalized_coords())
                .hint(true);
            if let Some(angle) = run.synthesis().skew() {
                // Glifo applies this after flipping font coordinates into Y-down layout space.
                painter = painter
                    .glyph_transform(Affine::skew(-f64::from(angle).to_radians().tan(), 0.0));
            }
            if run.synthesis().embolden() {
                painter = painter.font_embolden(glifo::FontEmbolden::new(Diagonal2::new(
                    f64::from(run.font_size()) / 48.0,
                    f64::from(run.font_size()) / 48.0,
                )));
            }
            let glyphs = glyphs.positioned_glyphs().map(|glyph| Glyph {
                id: glyph.id,
                x: glyph.x,
                y: glyph.y,
            });
            if stroke {
                painter.stroke_glyphs(glyphs);
            } else {
                painter.fill_glyphs(glyphs);
            }
        }
    }
}

fn ink_bounds(layout: &Layout<()>) -> Result<Rect, String> {
    let mut ink: Option<Rect> = None;
    for line in layout.lines() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyphs) = item else { continue };
            let run = glyphs.run();
            let font = skrifa::FontRef::from_index(run.font().data.as_ref(), run.font().index)
                .map_err(|error| format!("Native font metrics: {error}"))?;
            let coordinates: Vec<_> =
                run.normalized_coords().iter().copied().map(NormalizedCoord::from_bits).collect();
            let metrics =
                font.glyph_metrics(Size::new(run.font_size()), LocationRef::new(&coordinates));
            for glyph in glyphs.positioned_glyphs() {
                let Some(bounds) = metrics.bounds(skrifa::GlyphId::new(glyph.id)) else { continue };
                let mut rect = Rect::new(
                    f64::from(glyph.x + bounds.x_min),
                    f64::from(glyph.y - bounds.y_max),
                    f64::from(glyph.x + bounds.x_max),
                    f64::from(glyph.y - bounds.y_min),
                );
                if let Some(angle) = run.synthesis().skew() {
                    let local = Rect::new(
                        f64::from(bounds.x_min),
                        -f64::from(bounds.y_max),
                        f64::from(bounds.x_max),
                        -f64::from(bounds.y_min),
                    );
                    rect = (Affine::translate((f64::from(glyph.x), f64::from(glyph.y)))
                        * Affine::skew(-f64::from(angle).to_radians().tan(), 0.0))
                    .transform_rect_bbox(local);
                }
                if run.synthesis().embolden() {
                    rect = rect.inflate(
                        f64::from(run.font_size()) / 48.0,
                        f64::from(run.font_size()) / 48.0,
                    );
                }
                ink = Some(ink.map_or(rect, |current| current.union(rect)));
            }
        }
    }
    Ok(ink.unwrap_or(Rect::ZERO))
}
