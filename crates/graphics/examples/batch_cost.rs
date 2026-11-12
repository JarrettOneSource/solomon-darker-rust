//! Measure complete atlas batches through the real browser renderer.
//! Time repeated `batch_cost_frame` calls after initialization and warmup.

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::cell::RefCell;

    use solomon_graphics::{BlendMode, Quad, Renderer, Texture};
    use wasm_bindgen::prelude::*;
    use web_sys::WebGl2RenderingContext as Gl;

    struct BatchCost {
        renderer: Renderer,
        texture: Texture,
        gl: Gl,
    }

    thread_local! {
        static PROBE: RefCell<Option<BatchCost>> = const { RefCell::new(None) };
    }

    #[wasm_bindgen]
    pub fn batch_cost_init(gl: Gl) -> Result<(), JsValue> {
        batch_cost_dispose();
        let renderer = Renderer::new(gl.clone()).map_err(|error| JsValue::from_str(&error))?;
        let mut pixels = Vec::with_capacity(64 * 64 * 4);
        for y in 0..64 {
            for x in 0..64 {
                pixels.extend_from_slice(&[(x * 4) as u8, (y * 4) as u8, 120, 180]);
            }
        }
        let texture =
            renderer.upload_rgba(64, 64, &pixels).map_err(|error| JsValue::from_str(&error))?;
        PROBE.with_borrow_mut(|probe| *probe = Some(BatchCost { renderer, texture, gl }));
        Ok(())
    }

    #[wasm_bindgen]
    pub fn batch_cost_frame(records: bool, count: u32) -> Result<(), JsValue> {
        PROBE.with_borrow_mut(|probe| {
            let probe = probe.as_mut().ok_or_else(|| JsValue::from_str("Batch probe absent"))?;
            probe.renderer.begin_frame(512, 512, [512.0; 2]);
            for index in 0..count {
                let quad = Quad {
                    source: [((index % 8) * 8) as f32, (((index / 8) % 8) * 8) as f32, 8.0, 8.0],
                    destination: [
                        ((index % 64) * 8) as f32 + 0.2,
                        (((index / 64) % 64) * 8) as f32 + 0.3,
                        8.0,
                        8.0,
                    ],
                    tint: [1.0; 4],
                    blend: BlendMode::Normal,
                };
                if records {
                    probe.renderer.draw_clamped(&probe.texture, quad);
                } else {
                    probe.renderer.draw(&probe.texture, quad);
                }
            }
            probe.renderer.flush();
            probe.gl.finish();
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub fn batch_cost_dispose() {
        PROBE.with_borrow_mut(|probe| {
            if let Some(probe) = probe.take() {
                probe.renderer.destroy_texture(&probe.texture);
            }
        });
    }
}
