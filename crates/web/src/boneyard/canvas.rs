//! The two actual Canvas2D scenery consumers from `editor/render.ts`:
//! post-road Terrain tiles and the generated three-stroke Wall image.

use super::{BoundsDocument, PreparedImage};
use js_sys::{Function, Object, Reflect};
use solomon_sim::Vec2;
use solomon_sim::boneyard::{BoneyardBounds, BoneyardFence, BoneyardScene};
use std::collections::{BTreeMap, BTreeSet};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

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
            let (canvas, context) = canvas(width as u32, height as u32)?;
            context.set_image_smoothing_enabled(false);
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
                let pattern = context
                    .create_pattern_with_html_image_element(image, "repeat")
                    .map_err(error)?
                    .ok_or("Could not create the native terrain pattern")?;
                // web-sys still declares the obsolete SVGMatrix overload. The browser
                // accepts the current DOMMatrix2DInit contract used by Source.
                let transform = Object::new();
                for (name, value) in [
                    ("a", 1.0),
                    ("b", 0.0),
                    ("c", 0.0),
                    ("d", 1.0),
                    ("e", -center.x + width / 2.0),
                    ("f", -center.y + height / 2.0),
                ] {
                    Reflect::set(&transform, &JsValue::from_str(name), &JsValue::from_f64(value))
                        .map_err(error)?;
                }
                let set_transform: Function =
                    Reflect::get(pattern.as_ref(), &JsValue::from_str("setTransform"))
                        .map_err(error)?
                        .dyn_into()
                        .map_err(error)?;
                set_transform.call1(pattern.as_ref(), &transform).map_err(error)?;
                let width = if style == 0 { 96.0 } else { 64.0 };
                context.save();
                context.set_line_join("round");
                context.set_line_cap("round");
                context.set_stroke_style_str("rgba(0,0,0,0.5)");
                context.set_line_width(width + 5.0);
                stroke(&context, points, center, canvas.width() as f64, canvas.height() as f64);
                context.set_stroke_style_canvas_pattern(&pattern);
                context.set_line_width(width);
                stroke(&context, points, center, canvas.width() as f64, canvas.height() as f64);
                if style == 0 {
                    context.set_stroke_style_str("rgba(65,227,255,0.12)");
                    context.set_line_width(width * 0.45);
                    stroke(&context, points, center, canvas.width() as f64, canvas.height() as f64);
                }
                context.restore();
            }
            if let Some(paint) = consume(canvas, context, x, y)? {
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
    let (canvas, context) = canvas(width.max(1.0) as u32, height.max(1.0) as u32)?;
    context.set_image_smoothing_enabled(true);
    context.set_line_cap("butt");
    let center = Vec2::new(x + width / 2.0, y + height / 2.0);
    for (color, width) in [("#101014", 26.0), ("#3a3a42", 20.0), ("#55555e", 8.0)] {
        context.set_stroke_style_str(color);
        context.set_line_width(width);
        stroke(&context, &fence.points[..2], center, canvas.width() as f64, canvas.height() as f64);
    }
    consume(canvas, context, x, y)
}

fn canvas(
    width: u32,
    height: u32,
) -> Result<(HtmlCanvasElement, CanvasRenderingContext2d), String> {
    let document =
        web_sys::window().and_then(|window| window.document()).ok_or("No canvas document")?;
    let canvas: HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(error)?
        .dyn_into()
        .map_err(|value: web_sys::Element| error(value.into()))?;
    canvas.set_width(width);
    canvas.set_height(height);
    let context: CanvasRenderingContext2d = canvas
        .get_context("2d")
        .map_err(error)?
        .ok_or("Boneyard scenery requires Canvas2D")?
        .dyn_into()
        .map_err(|value: js_sys::Object| error(value.into()))?;
    Ok((canvas, context))
}

fn stroke(
    context: &CanvasRenderingContext2d,
    points: &[Vec2],
    center: Vec2,
    width: f64,
    height: f64,
) {
    context.begin_path();
    for (index, point) in points.iter().enumerate() {
        let x = point.x - center.x + width / 2.0;
        let y = point.y - center.y + height / 2.0;
        if index == 0 {
            context.move_to(x, y);
        } else {
            context.line_to(x, y);
        }
    }
    context.stroke();
}

fn consume(
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    x: f64,
    y: f64,
) -> Result<Option<PaintedCanvas>, String> {
    let width = canvas.width() as usize;
    let height = canvas.height() as usize;
    let pixels =
        context.get_image_data(0.0, 0.0, width as f64, height as f64).map_err(error)?.data().0;
    // Source crops only nonzero alpha; transparent RGB stays untouched inside
    // the retained rectangle, and the scratch canvas is released immediately.
    canvas.set_width(0);
    canvas.set_height(0);
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

fn error(value: JsValue) -> String {
    format!("Native scenery canvas: {value:?}")
}

/// Match Source's one-frame yield between native static tiles and painter batches.
pub(super) async fn next_frame() -> Result<(), String> {
    let window = web_sys::window().ok_or("No scenery animation frame owner")?;
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        if let Err(value) = window.request_animation_frame(&resolve) {
            let _ = reject.call1(&JsValue::UNDEFINED, &value);
        }
    });
    wasm_bindgen_futures::JsFuture::from(promise).await.map_err(error)?;
    Ok(())
}
