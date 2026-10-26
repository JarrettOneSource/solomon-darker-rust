//! The generated scenery consumers from `editor/render.ts`:
//! post-road Terrain tiles and the generated three-stroke Wall image.

use super::{BoundsDocument, PreparedImage};
pub(super) use crate::platform::next_frame;
use solomon_graphics::raster::{Paint, Raster};
use solomon_sim::Vec2;
use solomon_sim::boneyard::{BoneyardBounds, BoneyardFence, BoneyardScene};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct PaintedCanvas {
    pub bounds: BoundsDocument,
    pub image: PreparedImage,
}

pub(super) async fn terrain_tiles(
    scene: &BoneyardScene,
    images: &BTreeMap<String, PreparedImage>,
    retired: &BTreeSet<String>,
) -> Result<Vec<(usize, PaintedCanvas)>, String> {
    if scene.terrain.is_empty() {
        return Ok(Vec::new());
    }
    let bounds = scene.bounds;
    let mut output = Vec::new();
    let mut index = 0;
    let mut y = bounds.y - 256.0;
    while y < bounds.y + bounds.height + 256.0 {
        let mut x = bounds.x - 256.0;
        while x < bounds.x + bounds.width + 256.0 {
            let width = 1024.0_f64.min(bounds.x + bounds.width + 256.0 - x).ceil();
            let height = 1024.0_f64.min(bounds.y + bounds.height + 256.0 - y).ceil();
            let mut raster = Raster::new(width as u32, height as u32)?;
            let center = Vec2::new(x + width / 2.0, y + height / 2.0);
            for terrain in &scene.terrain {
                if retired.contains(&format!("terrain:{}", terrain.eid)) {
                    continue;
                }
                let Some(points) = terrain.points.as_ref().filter(|points| points.len() >= 2)
                else {
                    continue;
                };
                let view = BoneyardBounds::new(x - 256.0, y - 256.0, width + 512.0, height + 512.0);
                let extent = BoundsDocument::points(points);
                if !extent.overlaps_inclusive(view) {
                    continue;
                }
                let style =
                    terrain.style.map(|value| value as usize).or(terrain.entry).unwrap_or(0).min(1);
                let image = match &images[if style == 0 { "river.png" } else { "rise.png" }] {
                    PreparedImage::Image(image) => image,
                    PreparedImage::Pixels { .. } => {
                        unreachable!("surface catalog textures are decoded images")
                    }
                };
                let points: Vec<_> = points
                    .iter()
                    .map(|point| {
                        Vec2::new(
                            point.x - center.x + width / 2.0,
                            point.y - center.y + height / 2.0,
                        )
                    })
                    .collect();
                let origin = Vec2::new(-center.x + width / 2.0, -center.y + height / 2.0);
                let stroke_width = if style == 0 { 96.0 } else { 64.0 };
                raster.stroke(
                    &points,
                    stroke_width + 5.0,
                    true,
                    Paint::Color { rgb: [0, 0, 0], alpha: 0.5 },
                )?;
                raster.stroke(
                    &points,
                    stroke_width,
                    true,
                    Paint::RepeatingImage { image, origin },
                )?;
                if style == 0 {
                    raster.stroke(
                        &points,
                        stroke_width * 0.45,
                        true,
                        Paint::Color { rgb: [65, 227, 255], alpha: 0.12 },
                    )?;
                }
            }
            if let Some(paint) = consume(raster, width as usize, height as usize, x, y)? {
                output.push((index, paint));
            }
            index += 1;
            next_frame().await?;
            x += 1024.0;
        }
        y += 1024.0;
    }
    Ok(output)
}

pub(super) fn wall(fence: &BoneyardFence) -> Result<Option<PaintedCanvas>, String> {
    if fence.points.len() < 2 || fence.points[0] == fence.points[1] {
        return Ok(None);
    }
    let extent = BoundsDocument::points(&fence.points);
    let x = (extent.x - 256.0).floor();
    let y = (extent.y - 256.0).floor();
    let width = (extent.x + extent.width + 256.0).ceil() - x;
    let height = (extent.y + extent.height + 256.0).ceil() - y;
    let width = width.max(1.0) as usize;
    let height = height.max(1.0) as usize;
    let mut raster = Raster::new(width as u32, height as u32)?;
    let center = Vec2::new(x + width as f64 / 2.0, y + height as f64 / 2.0);
    let points: Vec<_> = fence.points[..2]
        .iter()
        .map(|point| {
            Vec2::new(
                point.x - center.x + width as f64 / 2.0,
                point.y - center.y + height as f64 / 2.0,
            )
        })
        .collect();
    for (rgb, width) in [([16, 16, 20], 26.0), ([58, 58, 66], 20.0), ([85, 85, 94], 8.0)] {
        raster.stroke(&points, width, false, Paint::Color { rgb, alpha: 1.0 })?;
    }
    consume(raster, width, height, x, y)
}

fn consume(
    raster: Raster,
    width: usize,
    height: usize,
    x: f64,
    y: f64,
) -> Result<Option<PaintedCanvas>, String> {
    let pixels = raster.into_rgba()?;
    // Source crops only nonzero alpha; transparent RGB stays untouched inside
    // the retained rectangle, and the scratch canvas is released immediately.
    let mut left = width;
    let mut top = height;
    let mut right = 0;
    let mut bottom = 0;
    let mut occupied = false;
    for row in 0..height {
        for column in 0..width {
            if pixels[(row * width + column) * 4 + 3] == 0 {
                continue;
            }
            occupied = true;
            left = left.min(column);
            top = top.min(row);
            right = right.max(column);
            bottom = bottom.max(row);
        }
    }
    if !occupied {
        return Ok(None);
    }
    let crop_width = right - left + 1;
    let crop_height = bottom - top + 1;
    let mut cropped = Vec::with_capacity(crop_width * crop_height * 4);
    for row in top..=bottom {
        let start = (row * width + left) * 4;
        cropped.extend_from_slice(&pixels[start..start + crop_width * 4]);
    }
    Ok(Some(PaintedCanvas {
        bounds: BoundsDocument {
            x: x + left as f64,
            y: y + top as f64,
            width: crop_width as f64,
            height: crop_height as f64,
        },
        image: PreparedImage::Pixels {
            width: crop_width as u32,
            height: crop_height as u32,
            pixels: cropped,
        },
    }))
}
