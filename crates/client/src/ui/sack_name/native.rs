use solomon_graphics::text::{FontSystem, ShapedText, TextStyle};
use unicode_segmentation::UnicodeSegmentation;

use super::{Metrics, TextLayout};

pub(super) struct FontCanvas {
    size: f64,
}

impl FontCanvas {
    pub(super) fn new(size: f64) -> Result<Self, String> {
        Ok(Self { size })
    }

    fn shape(&self, fonts: &mut FontSystem, text: &str) -> Result<ShapedText, String> {
        fonts.shape(
            text,
            TextStyle {
                families: "Arial,Helvetica,sans-serif",
                size: self.size as f32,
                weight: 400.0,
                italic: false,
                letter_spacing: 0.0,
                line_height: Some(self.size as f32),
            },
        )
    }

    pub(super) fn metrics(&self, text: &str) -> Result<Metrics, String> {
        crate::gpu_ui::with_fonts(|fonts| {
            let text = self.shape(fonts, text)?;
            let baseline =
                text.layout.lines().next().map_or(0.0, |line| f64::from(line.metrics().baseline));
            Ok(Metrics {
                advance: f64::from(text.layout.full_width()),
                left: -text.ink.x0,
                right: text.ink.x1,
                ascent: baseline - text.ink.y0,
                descent: text.ink.y1 - baseline,
            })
        })
    }

    pub(super) fn paint(
        &self,
        lines: &[String],
        tint: u32,
        layout: TextLayout,
    ) -> Result<egui::ColorImage, String> {
        let width = u16::try_from(layout.width).map_err(|_| "Sack name is too wide")?;
        let height = u16::try_from(layout.height).map_err(|_| "Sack name is too tall")?;
        crate::gpu_ui::with_fonts(|fonts| {
            let text = FontCanvas { size: layout.line_height }.shape(fonts, &lines.join("\n"))?;
            let baseline =
                text.layout.lines().next().map_or(0.0, |line| f64::from(line.metrics().baseline));
            let bitmap = fonts.rasterize(
                &text,
                [width, height],
                [0.0, layout.baseline - baseline],
                [(tint >> 16) as u8, (tint >> 8) as u8, tint as u8, 255],
                None,
            );
            Ok(egui::ColorImage::from_rgba_premultiplied(
                [width as usize, height as usize],
                &bitmap.rgba,
            ))
        })
    }
}

pub(super) fn graphemes(text: &str) -> Result<Vec<String>, String> {
    Ok(text.graphemes(true).map(str::to_owned).collect())
}
