use std::sync::Arc;

use solomon_sim::Vec2;
use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::kurbo::{Affine, BezPath, Cap, Join, Stroke};
use vello_cpu::peniko::{Extend, ImageQuality};
use vello_cpu::{Image, ImageSource, Pixmap, RenderContext, Resources};

use super::Paint;

pub struct Raster(RenderContext);

impl Raster {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let width = u16::try_from(width).map_err(|_| "Scenery image is too wide")?;
        let height = u16::try_from(height).map_err(|_| "Scenery image is too tall")?;
        if width == 0 || height == 0 {
            return Err("Scenery image must have positive dimensions".into());
        }
        Ok(Self(RenderContext::new(width, height)))
    }

    pub fn stroke(
        &mut self,
        points: &[Vec2],
        width: f64,
        round: bool,
        paint: Paint<'_>,
    ) -> Result<(), String> {
        let mut path = BezPath::new();
        for (index, point) in points.iter().enumerate() {
            if index == 0 {
                path.move_to((point.x, point.y));
            } else {
                path.line_to((point.x, point.y));
            }
        }
        let stroke = Stroke::new(width)
            .with_caps(if round { Cap::Round } else { Cap::Butt })
            .with_join(if round { Join::Round } else { Join::Miter });
        self.0.set_stroke(stroke);
        self.0.reset_paint_transform();
        match paint {
            Paint::Color { rgb: [r, g, b], alpha } => {
                self.0.set_paint(AlphaColor::<Srgb>::new([
                    f32::from(r) / 255.0,
                    f32::from(g) / 255.0,
                    f32::from(b) / 255.0,
                    alpha as f32,
                ]));
            }
            Paint::RepeatingImage { image, origin } => {
                let width =
                    u16::try_from(image.width()).map_err(|_| "Terrain image is too wide")?;
                let height =
                    u16::try_from(image.height()).map_err(|_| "Terrain image is too tall")?;
                let pixels = image
                    .rgba()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|[r, g, b, a]| {
                        AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a).premultiply().to_rgba8()
                    })
                    .collect();
                let pixmap = Pixmap::from_parts(pixels, width, height);
                self.0.set_paint(
                    Image {
                        image: ImageSource::Pixmap(Arc::new(pixmap)),
                        sampler: Default::default(),
                    }
                    .with_extend(Extend::Repeat)
                    .with_quality(ImageQuality::Low),
                );
                self.0.set_paint_transform(Affine::translate((origin.x, origin.y)));
            }
        }
        self.0.stroke_path(&path);
        Ok(())
    }

    /// Straight RGBA matches Canvas2D image data and the scenery upload contract.
    pub fn into_rgba(mut self) -> Result<Vec<u8>, String> {
        self.0.flush();
        let mut pixels = Pixmap::new(self.0.width(), self.0.height());
        self.0.render(&mut pixels, &mut Resources::new());
        Ok(pixels
            .data()
            .iter()
            .flat_map(|pixel| {
                let straight = |channel: u8| {
                    if pixel.a == 0 {
                        0
                    } else {
                        ((u32::from(channel) * 255 + u32::from(pixel.a) / 2) / u32::from(pixel.a))
                            .min(255) as u8
                    }
                };
                [straight(pixel.r), straight(pixel.g), straight(pixel.b), pixel.a]
            })
            .collect())
    }
}
