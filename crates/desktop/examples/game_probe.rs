//! Interactive qualification of the actual shared Game with an owned native authority.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), String> {
    use glow::HasContext;
    use solomon_desktop::{authority::LocalAuthority, window::NativeWindow};
    use solomon_web::{
        Game,
        desktop::{
            DesktopControls,
            launcher::{Build, DesktopState},
        },
        platform::{ClientTasks, Surface},
    };
    use std::{cell::RefCell, path::PathBuf, rc::Rc};
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, EventLoop},
        window::WindowId,
    };
    type AccessibilityEvent = egui_winit::accesskit_winit::Event;

    struct Probe {
        game: Option<Rc<RefCell<Game>>>,
        tasks: Option<ClientTasks>,
        authority: Option<LocalAuthority>,
        controls: Option<DesktopControls>,
        window: Option<NativeWindow>,
        proxy: winit::event_loop::EventLoopProxy<AccessibilityEvent>,
        assets: PathBuf,
        output: PathBuf,
        revision: String,
        frames: u32,
        close_after: u32,
        closing: bool,
        finished: bool,
        error: Option<String>,
    }
    impl Probe {
        fn capture(&self) -> Result<(), String> {
            let window = self.window.as_ref().ok_or("Native game window absent")?;
            let game = self.game.as_ref().ok_or("Native game absent")?.borrow();
            window.make_current()?;
            let size = window.window.inner_size();
            let mut rgba = vec![0; size.width as usize * size.height as usize * 4];
            let gl = window.graphics_context();
            // SAFETY: the live window owns the current context and the full output framebuffer.
            let (error, device) = unsafe {
                gl.read_buffer(glow::FRONT);
                gl.read_pixels(
                    0,
                    0,
                    size.width as i32,
                    size.height as i32,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelPackData::Slice(Some(&mut rgba)),
                );
                gl.read_buffer(glow::BACK);
                (gl.get_error(), gl.get_parameter_string(glow::RENDERER))
            };
            if error != glow::NO_ERROR {
                return Err(format!("Native game readback error: {error}"));
            }
            let mut bitmap = image::RgbaImage::from_raw(size.width, size.height, rgba)
                .ok_or("Game readback extent")?;
            image::imageops::flip_vertical_in_place(&mut bitmap);
            std::fs::create_dir_all(&self.output).map_err(|error| error.to_string())?;
            bitmap.save(self.output.join("native-game.png")).map_err(|error| error.to_string())?;
            std::fs::write(self.output.join("native-game.json"), serde_json::to_vec_pretty(&serde_json::json!({
                "renderer":device,"graphicsError":error,"ui":game.gpu_ui_snapshot(),"wizard":game.wizard_snapshot(),"frames":self.frames,"closing":self.closing,
            })).map_err(|error| error.to_string())?).map_err(|error| error.to_string())
        }

        fn stop(&mut self, event_loop: &ActiveEventLoop) {
            self.finished = true;
            self.tasks.take();
            self.game.take();
            if let Some(mut authority) = self.authority.take()
                && let Err(error) = authority.close()
            {
                self.error.get_or_insert(error);
            }
            self.controls.take();
            self.window.take();
            event_loop.exit();
        }

        fn request_close(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
            if self.closing {
                return Ok(());
            }
            self.capture()?;
            self.closing = self
                .game
                .as_ref()
                .ok_or("Native game absent")?
                .borrow_mut()
                .begin_desktop_close()?;
            if !self.closing {
                self.stop(event_loop);
            }
            Ok(())
        }
    }

    impl ApplicationHandler<AccessibilityEvent> for Probe {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_some() || self.finished {
                return;
            }
            let result = (|| {
                solomon_graphics::assets::configure(solomon_graphics::assets::NativeAssets::new(
                    &self.assets,
                )?);
                solomon_web::configure_native_storage(self.output.join("profile"))?;
                self.tasks = Some(ClientTasks::install());
                let authority = LocalAuthority::start(self.assets.clone(), self.revision.clone())?;
                let endpoint = authority.endpoint();
                solomon_web::platform::configure_native_location(
                    &endpoint.url.replacen("ws:", "http:", 1),
                    None,
                )?;
                self.authority = Some(authority);
                let controls = DesktopControls::install(DesktopState {
                    build: Build { revision: self.revision.clone() },
                    addresses: Vec::new(),
                    update: None,
                    update_error: None,
                });
                self.controls = Some(controls.clone());
                let window =
                    NativeWindow::open(event_loop, "Solomon Darker — native game", [1360, 850])?;
                // SAFETY: all client tasks and Game GPU owners retire before this window.
                let renderer = unsafe { window.renderer()? };
                self.window = Some(window);
                let surface = Surface::from_window(self.window.as_ref().unwrap().window.clone());
                let game = futures_lite::future::block_on(Game::load_native(
                    surface, renderer, endpoint, controls,
                ))?;
                {
                    let mut game = game.borrow_mut();
                    let ui = game.native_ui();
                    ui.init_accesskit(event_loop, self.proxy.clone());
                    ui.context().enable_accesskit();
                }
                self.game = Some(game);
                let window = self.window.as_ref().unwrap();
                window.window.set_visible(true);
                window.window.request_redraw();
                Ok::<_, String>(())
            })();
            if let Err(error) = result {
                self.error = Some(error);
                self.stop(event_loop);
            }
        }

        fn user_event(&mut self, _: &ActiveEventLoop, event: AccessibilityEvent) {
            if self.finished
                || self.window.as_ref().is_none_or(|window| window.window.id() != event.window_id)
            {
                return;
            }
            let Some(game) = &self.game else { return };
            let mut game = game.borrow_mut();
            match event.window_event {
                egui_winit::accesskit_winit::WindowEvent::InitialTreeRequested => {
                    game.native_ui().context().enable_accesskit()
                }
                egui_winit::accesskit_winit::WindowEvent::ActionRequested(action) => {
                    if let Err(error) =
                        game.accesskit_action(action, solomon_web::platform::monotonic_ms())
                    {
                        self.error = Some(error);
                    }
                }
                egui_winit::accesskit_winit::WindowEvent::AccessibilityDeactivated => {}
            }
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            if self.finished {
                return;
            }
            let result = (|| {
                let now = solomon_web::platform::monotonic_ms();
                let Some(game) = self.game.as_ref().cloned() else { return Ok(()) };
                game.borrow_mut().window_event(&event, now)?;
                if let WindowEvent::Resized(size) = event
                    && let Some(window) = &self.window
                {
                    window.resize(size.width, size.height);
                }
                if matches!(event, WindowEvent::RedrawRequested) {
                    let window = self.window.as_ref().ok_or("Native game window absent")?;
                    window.make_current()?;
                    if let Some(tasks) = &self.tasks {
                        tasks.tick();
                    }
                    game.borrow_mut().frame(now)?;
                    Game::run_pending_work(&game);
                    window.present()?;
                    self.frames += 1;
                    if self.frames == 5 || self.frames.is_multiple_of(60) {
                        self.capture()?;
                    }
                    if self.closing && game.borrow_mut().poll_desktop_close(now)? {
                        drop(game);
                        self.capture()?;
                        self.stop(event_loop);
                        return Ok(());
                    }
                    if self.close_after > 0 && self.frames >= self.close_after {
                        drop(game);
                        self.request_close(event_loop)?;
                    }
                    if let Some(window) = &self.window {
                        window.window.request_redraw();
                    }
                } else if matches!(event, WindowEvent::CloseRequested) {
                    // Release the local Game clone before context teardown.
                    drop(game);
                    self.request_close(event_loop)?;
                }
                Ok::<_, String>(())
            })();
            if let Err(error) = result {
                self.error = Some(error);
                self.stop(event_loop);
            }
        }
    }

    let mut args = std::env::args().skip(1);
    let assets =
        PathBuf::from(args.next().ok_or("game_probe ASSETS OUTPUT REVISION [CLOSE_AFTER_FRAMES]")?);
    let output = PathBuf::from(args.next().ok_or("missing output directory")?);
    let revision = args.next().ok_or("missing full source revision")?;
    let close_after = args
        .next()
        .map(|value| value.parse::<u32>())
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or(0);
    let event_loop = EventLoop::<AccessibilityEvent>::with_user_event()
        .build()
        .map_err(|error| error.to_string())?;
    let mut probe = Probe {
        game: None,
        tasks: None,
        authority: None,
        controls: None,
        window: None,
        proxy: event_loop.create_proxy(),
        assets,
        output,
        revision,
        frames: 0,
        close_after,
        closing: false,
        finished: false,
        error: None,
    };
    event_loop.run_app(&mut probe).map_err(|error| error.to_string())?;
    if let Some(error) = probe.error { Err(error) } else { Ok(()) }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
