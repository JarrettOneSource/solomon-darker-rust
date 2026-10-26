use js_sys::{Function, Object, Reflect};
use solomon_sim::Vec2;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use super::Paint;

pub struct Raster {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
}

impl Raster {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let document =
            web_sys::window().and_then(|window| window.document()).ok_or("No canvas document")?;
        let canvas: HtmlCanvasElement = document
            .create_element("canvas")
            .map_err(error)?
            .dyn_into()
            .map_err(|_| "Scenery canvas type")?;
        canvas.set_width(width);
        canvas.set_height(height);
        let context: CanvasRenderingContext2d = canvas
            .get_context("2d")
            .map_err(error)?
            .ok_or("Boneyard scenery requires Canvas2D")?
            .dyn_into()
            .map_err(|_| "Scenery context type")?;
        Ok(Self { canvas, context })
    }

    pub fn stroke(
        &mut self,
        points: &[Vec2],
        width: f64,
        round: bool,
        paint: Paint<'_>,
    ) -> Result<(), String> {
        self.context.set_line_cap(if round { "round" } else { "butt" });
        self.context.set_line_join(if round { "round" } else { "miter" });
        self.context.set_line_width(width);
        match paint {
            Paint::Color { rgb: [r, g, b], alpha } => {
                self.context.set_stroke_style_str(&format!("rgba({r},{g},{b},{alpha})"));
            }
            Paint::RepeatingImage { image, origin } => {
                self.context.set_image_smoothing_enabled(false);
                let pattern = self
                    .context
                    .create_pattern_with_html_image_element(image, "repeat")
                    .map_err(error)?
                    .ok_or("Could not create the terrain pattern")?;
                // web-sys still declares the obsolete SVGMatrix overload.
                let transform = Object::new();
                for (name, value) in [
                    ("a", 1.0),
                    ("b", 0.0),
                    ("c", 0.0),
                    ("d", 1.0),
                    ("e", origin.x),
                    ("f", origin.y),
                ] {
                    Reflect::set(&transform, &name.into(), &JsValue::from_f64(value))
                        .map_err(error)?;
                }
                let set: Function = Reflect::get(pattern.as_ref(), &"setTransform".into())
                    .map_err(error)?
                    .dyn_into()
                    .map_err(error)?;
                set.call1(pattern.as_ref(), &transform).map_err(error)?;
                self.context.set_stroke_style_canvas_pattern(&pattern);
            }
        }
        self.context.begin_path();
        for (index, point) in points.iter().enumerate() {
            if index == 0 {
                self.context.move_to(point.x, point.y);
            } else {
                self.context.line_to(point.x, point.y);
            }
        }
        self.context.stroke();
        Ok(())
    }

    pub fn into_rgba(self) -> Result<Vec<u8>, String> {
        self.context
            .get_image_data(
                0.0,
                0.0,
                f64::from(self.canvas.width()),
                f64::from(self.canvas.height()),
            )
            .map(|pixels| pixels.data().0)
            .map_err(error)
    }
}

impl Drop for Raster {
    fn drop(&mut self) {
        self.canvas.set_width(0);
        self.canvas.set_height(0);
    }
}

fn error(value: JsValue) -> String {
    format!("Scenery raster: {value:?}")
}
