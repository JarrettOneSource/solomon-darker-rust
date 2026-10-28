//! Actual native-context qualification of the shared shader and texture pipeline.

#[cfg(not(target_arch = "wasm32"))]
#[path = "../../graphics/examples/support/atlas_crop.rs"]
mod atlas_crop;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), String> {
    use glow::HasContext;
    use solomon_desktop::window::NativeWindow;
    use solomon_graphics::{BlendMode, Quad, Renderer, Texture};
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, EventLoop},
        window::WindowId,
    };

    struct Probe {
        renderer: Option<Renderer>,
        texture: Option<Texture>,
        window: Option<NativeWindow>,
        output: std::path::PathBuf,
        error: Option<String>,
        finished: bool,
    }
    impl ApplicationHandler for Probe {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let result = (|| {
                let window = NativeWindow::open(
                    event_loop,
                    "Solomon native graphics qualification",
                    [960, 600],
                )?;
                // SAFETY: Probe drops its renderer before its owning window on every exit.
                let renderer = unsafe { window.renderer()? };
                let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web");
                let assets = solomon_graphics::assets::NativeAssets::new(root)?;
                let bytes =
                    futures_lite::future::block_on(assets.fetch("assets/stock/College.png"))?;
                let image = solomon_graphics::assets::decode_image("College.png", &bytes)?;
                let texture = renderer.upload(&image)?;
                window.window.set_visible(true);
                window.window.request_redraw();
                self.renderer = Some(renderer);
                self.texture = Some(texture);
                self.window = Some(window);
                Ok::<_, String>(())
            })();
            if let Err(error) = result {
                self.error = Some(error);
                event_loop.exit();
            }
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            if self.finished || !matches!(event, WindowEvent::RedrawRequested) {
                return;
            }
            let result = (|| {
                let window = self.window.as_ref().ok_or("Native window absent")?;
                let renderer = self.renderer.as_mut().ok_or("Shared renderer absent")?;
                let crop: serde_json::Value = serde_json::from_str(&atlas_crop::probe(renderer)?)
                    .map_err(|error| error.to_string())?;
                std::fs::create_dir_all(&self.output).map_err(|error| error.to_string())?;
                std::fs::write(
                    self.output.join("atlas-crops.json"),
                    serde_json::to_vec_pretty(&crop).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                for case in crop.as_array().ok_or("Atlas crop cases are absent")? {
                    if case["maximumDifference"]
                        .as_u64()
                        .ok_or("Atlas crop difference is absent")?
                        > 1
                        || case["restored"] != true
                    {
                        return Err(format!("Isolated atlas crop differs: {case}"));
                    }
                }
                let texture = self.texture.as_ref().ok_or("Stock texture absent")?;
                let size = window.window.inner_size();
                renderer.begin_frame(size.width as i32, size.height as i32, [960.0, 600.0]);
                renderer.draw(
                    texture,
                    Quad {
                        source: [0.0, 0.0, texture.width, texture.height],
                        destination: [0.0, 0.0, 960.0, 600.0],
                        tint: [1.0; 4],
                        blend: BlendMode::Normal,
                    },
                );
                renderer.draw_solid([0.0, 0.0, 96.0, 96.0], [1.0, 0.0, 0.0, 1.0]);
                renderer.end_frame();
                let gl = renderer.graphics_context();
                let mut pixels = vec![0_u8; size.width as usize * size.height as usize * 4];
                // SAFETY: this live current context writes exactly the initialized readback slice.
                let (version, device, error) = unsafe {
                    gl.read_pixels(
                        0,
                        0,
                        size.width as i32,
                        size.height as i32,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelPackData::Slice(Some(&mut pixels)),
                    );
                    (
                        gl.get_parameter_string(glow::VERSION),
                        gl.get_parameter_string(glow::RENDERER),
                        gl.get_error(),
                    )
                };
                if error != glow::NO_ERROR {
                    return Err(format!("Native readback failed: 0x{error:04x}"));
                }
                let index = ((size.height - 10) * size.width + 10) as usize * 4;
                if pixels[index..index + 3] != [255, 0, 0] {
                    return Err("Shared shader did not draw the expected opaque red quad".into());
                }
                let mut image = image::RgbaImage::from_raw(size.width, size.height, pixels)
                    .ok_or("Native readback extent")?;
                image::imageops::flip_vertical_in_place(&mut image);
                std::fs::create_dir_all(&self.output).map_err(|error| error.to_string())?;
                image
                    .save(self.output.join("native-graphics.png"))
                    .map_err(|error| error.to_string())?;
                std::fs::write(self.output.join("native-graphics.json"), serde_json::to_vec_pretty(&serde_json::json!({"version":version,"renderer":device,"size":[size.width,size.height],"stockTexture":[texture.width,texture.height],"redQuad":true,"graphicsError":error})).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
                window.present()?;
                Ok::<_, String>(())
            })();
            if let Err(error) = result {
                self.error = Some(error);
            }
            self.finished = true;
            if let (Some(renderer), Some(texture)) = (&self.renderer, self.texture.take()) {
                renderer.destroy_texture(&texture);
            }
            self.renderer.take();
            self.window.take();
            event_loop.exit();
        }
    }
    let output = std::env::args_os().nth(1).ok_or("Pass an evidence output directory")?.into();
    let mut probe =
        Probe { renderer: None, texture: None, window: None, output, error: None, finished: false };
    EventLoop::new()
        .map_err(|error| error.to_string())?
        .run_app(&mut probe)
        .map_err(|error| error.to_string())?;
    probe.error.map_or(Ok(()), Err)
}

#[cfg(target_arch = "wasm32")]
fn main() {}
