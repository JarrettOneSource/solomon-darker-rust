//! Run with --features browser-fixtures, followed by an output directory.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), String> {
    use glow::HasContext;
    use solomon_desktop::window::NativeWindow;
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, EventLoop},
        window::WindowId,
    };

    struct Probe {
        output: std::path::PathBuf,
        result: Option<Result<(), String>>,
    }
    impl ApplicationHandler for Probe {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            self.result = Some((|| {
                let window =
                    NativeWindow::open(event_loop, "Wizard material regression", [256, 256])?;
                // SAFETY: the renderer is dropped before its window and current context.
                let mut renderer = unsafe { window.renderer()? };
                let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web");
                solomon_graphics::assets::configure(solomon_graphics::assets::NativeAssets::new(
                    root,
                )?);
                renderer.begin_frame(256, 256, [256.0; 2]);
                renderer.set_native_world_pixel_centers(true);
                let mut read_pixels = |renderer: &mut solomon_graphics::Renderer| {
                    let gl = renderer.graphics_context();
                    let mut pixels = vec![0; 256 * 256 * 4];
                    // SAFETY: the live current context writes the initialized RGBA slice.
                    unsafe {
                        gl.read_pixels(
                            0,
                            0,
                            256,
                            256,
                            glow::RGBA,
                            glow::UNSIGNED_BYTE,
                            glow::PixelPackData::Slice(Some(&mut pixels)),
                        );
                    }
                    pixels
                };
                let cases = futures_lite::future::block_on(
                    solomon_web::wizard::material_probe::run(&mut renderer, &mut read_pixels),
                )?;
                std::fs::create_dir_all(&self.output).map_err(|error| error.to_string())?;
                let mut receipts = Vec::new();
                for case in &cases {
                    for (label, pixels) in
                        [("actual", &case.actual), ("reference", &case.reference)]
                    {
                        let mut image = image::RgbaImage::from_raw(256, 256, pixels.clone())
                            .ok_or("probe extent")?;
                        image::imageops::flip_vertical_in_place(&mut image);
                        image
                            .save(self.output.join(format!("{}-{label}.png", case.name)))
                            .map_err(|error| error.to_string())?;
                    }
                    receipts.push(serde_json::json!({"name":case.name,"differentChannels":case.different_channels,
                        "maximumDifference":case.maximum_difference}));
                }
                std::fs::write(
                    self.output.join("receipt.json"),
                    serde_json::to_vec_pretty(&receipts).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                renderer.end_frame();
                if let Some(case) = cases.iter().find(|case| case.maximum_difference > 1) {
                    return Err(format!(
                        "{}: actual wizard differs from recovered Stoneskin composition: {} channels, max {}",
                        case.name, case.different_channels, case.maximum_difference
                    ));
                }
                println!("{} actual WizardView material compositions pass", cases.len());
                Ok(())
            })());
            event_loop.exit();
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }
    let output = std::env::args_os().nth(1).ok_or("Pass an evidence output directory")?.into();
    let mut probe = Probe { output, result: None };
    EventLoop::new()
        .map_err(|error| error.to_string())?
        .run_app(&mut probe)
        .map_err(|error| error.to_string())?;
    probe.result.ok_or("Native material probe did not run")?
}

#[cfg(target_arch = "wasm32")]
fn main() {}
