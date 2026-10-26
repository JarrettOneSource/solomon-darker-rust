//! Actual OS window qualification of the shared GPU UI, fonts and text transport.
//! Usage: ui_probe <packaged web root> <receipt directory> [capture frame count].

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), String> {
    use glow::HasContext;
    use solomon_desktop::window::NativeWindow;
    use solomon_graphics::Renderer;
    use solomon_web::gpu_ui::{
        FontRunOptions, GpuUi, NativeFontStroke, NativeTextEdit, font_run_with_options,
    };
    use solomon_web::platform::{ClientTasks, Surface};
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, EventLoop},
        window::WindowId,
    };

    struct Operations {
        surface: Surface,
        picker: solomon_web::platform::files::FilePicker,
        fullscreen: solomon_web::fullscreen::FullscreenControl,
        events: Rc<RefCell<Vec<serde_json::Value>>>,
    }
    impl Operations {
        fn new(surface: Surface) -> Result<Self, String> {
            let mut fullscreen = solomon_web::fullscreen::FullscreenControl::attach(&surface)?;
            fullscreen.update(solomon_web::fullscreen::FullscreenPlacement::Title)?;
            Ok(Self {
                picker: solomon_web::platform::files::FilePicker::new(
                    &surface,
                    &["json", "txt"],
                    true,
                )?,
                surface,
                fullscreen,
                events: Rc::new(RefCell::new(Vec::new())),
            })
        }
        fn poll(&mut self) {
            for files in self.picker.take() {
                let events = self.events.clone();
                solomon_web::platform::spawn_local(async move {
                    for file in files {
                        let result = file.read(1024 * 1024).await;
                        events.borrow_mut().push(match result {
                            Ok(bytes) => serde_json::json!({"selected":file.name(), "bytes":bytes.len(), "utf8":String::from_utf8(bytes).ok()}),
                            Err(error) => serde_json::json!({"fileError":error}),
                        });
                    }
                });
            }
        }
        fn paint(
            &mut self,
            frame: &solomon_web::gpu_ui::UiFrame<'_>,
            ui: &mut egui::Ui,
            origin: egui::Pos2,
        ) -> Result<(), String> {
            for (index, (id, label)) in
                [("pick", "Open file"), ("save", "Save file"), ("copy", "Copy fixture")]
                    .into_iter()
                    .enumerate()
            {
                let response = ui.put(
                    egui::Rect::from_min_size(
                        origin + egui::vec2(index as f32 * 190.0, 0.0),
                        egui::vec2(180.0, 38.0),
                    ),
                    egui::Button::new(label),
                );
                frame.record(&format!("probe/{id}"), label, "button", &response);
                if response.clicked() {
                    match id {
                        "pick" => self.picker.choose()?,
                        "save" => {
                            let surface = self.surface.clone();
                            let events = self.events.clone();
                            solomon_web::platform::spawn_local(async move {
                                let result = solomon_web::platform::files::save_file(
                                    &surface,
                                    "native-platform-check.json",
                                    br#"{"native":true,"save":"complete"}"#.to_vec(),
                                    "application/json",
                                )
                                .await;
                                events.borrow_mut().push(serde_json::json!({"saved":result}));
                            });
                        }
                        "copy" => {
                            let request = solomon_web::platform::copy_text("Native clipboard ✓");
                            let events = self.events.clone();
                            solomon_web::platform::spawn_local(async move {
                                let result = match request {
                                    Ok(request) => request.await,
                                    Err(error) => Err(error),
                                };
                                events.borrow_mut().push(serde_json::json!({"copied":result}));
                            });
                        }
                        _ => unreachable!(),
                    }
                }
            }
            if let Some(last) = self.events.borrow().last() {
                ui.put(
                    egui::Rect::from_min_size(
                        origin + egui::vec2(0.0, 48.0),
                        egui::vec2(850.0, 42.0),
                    ),
                    egui::Label::new(last.to_string()),
                );
            }
            Ok(())
        }
    }
    struct Probe {
        operations: Option<Operations>,
        audio: Option<solomon_web::audio::GameAudio>,
        check_audio: bool,
        audio_trace: Vec<serde_json::Value>,
        ui: Option<GpuUi>,
        renderer: Option<Renderer>,
        tasks: Option<ClientTasks>,
        window: Option<NativeWindow>,
        assets: PathBuf,
        output: PathBuf,
        frames: u32,
        capture_after: u32,
        text: String,
        clicks: u32,
        proxy: winit::event_loop::EventLoopProxy<egui_winit::accesskit_winit::Event>,
        error: Option<String>,
        finished: bool,
    }
    impl Probe {
        fn draw(&mut self) -> Result<(), String> {
            if let Some(audio) = &self.audio {
                use solomon_sim::audio::{AUDIO_CATALOG, AudioKind, AudioOptions, MusicScene};
                let loop_cue = AUDIO_CATALOG
                    .iter()
                    .find(|cue| cue.kind == AudioKind::Loop)
                    .expect("stock loop")
                    .cue;
                match self.frames {
                    10 => {
                        audio.play_after(
                            AUDIO_CATALOG
                                .iter()
                                .find(|cue| cue.kind == AudioKind::Sound)
                                .expect("stock sound")
                                .cue,
                            AudioOptions::default(),
                            50.0,
                        )?;
                    }
                    20 => audio.start_loop(
                        loop_cue,
                        "native-audio-probe",
                        AudioOptions { volume: 0.2, ..Default::default() },
                    )?,
                    30 => audio.set_narration_mix(0.3),
                    40 => audio.set_scene(MusicScene::Hub)?,
                    50 => audio.stop_loops_for_owner("native-audio-probe")?,
                    60 => audio.set_music_paused(true)?,
                    _ => {}
                }
                audio.update()?;
                if self.frames.is_multiple_of(10) {
                    self.audio_trace
                        .push(serde_json::json!({"frame": self.frames, "audio": audio.snapshot()}));
                }
            }
            self.operations.as_mut().ok_or("Native operations absent")?.poll();
            let window = self.window.as_ref().ok_or("Native UI window absent")?;
            let renderer = self.renderer.as_mut().ok_or("Native UI renderer absent")?;
            let ui = self.ui.as_mut().ok_or("Native UI absent")?;
            let size = window.window.inner_size();
            let logical = size.to_logical::<f32>(window.window.scale_factor());
            let viewport = [logical.width, logical.height];
            renderer.begin_frame(size.width as i32, size.height as i32, viewport);
            renderer.draw_solid([0.0, 0.0, viewport[0], viewport[1]], [0.06, 0.04, 0.08, 1.0]);
            renderer.end_frame();
            ui.run(renderer, viewport, solomon_web::platform::monotonic_ms() / 1000.0, |frame| {
                Self::paint(
                    frame,
                    viewport,
                    &mut self.text,
                    &mut self.clicks,
                    self.frames == 0,
                    self.operations.as_mut().ok_or("Native operations absent")?,
                )
            })?;
            self.frames += 1;
            window.present()
        }
        fn paint(
            frame: &mut solomon_web::gpu_ui::UiFrame<'_>,
            viewport: [f32; 2],
            text: &mut String,
            clicks: &mut u32,
            first: bool,
            operations: &mut Operations,
        ) -> Result<(), String> {
            let context = frame.ctx.clone();
            egui::Area::new(egui::Id::unique("native-ui-probe"))
                .fixed_pos(egui::pos2(36.0, 32.0))
                .fade_in(false)
                .show(&context, |ui| {
                    ui.set_min_size(egui::vec2(viewport[0] - 72.0, viewport[1] - 64.0));
                    let mut y = 0.0;
                    for (text, family, size, italic_stroke) in [
                        ("Solomon Darker", "Cinzel Bold", 42.0, false),
                        ("Shared Rust UI · AV ffi e\u{301}", "Alegreya", 28.0, false),
                        ("Italic overhang and stroke", "Alegreya Bold Italic", 30.0, true),
                        ("Wizard مرحبا עולם", "Alegreya", 28.0, false),
                    ] {
                        let run = font_run_with_options(
                            frame.ctx,
                            egui::FontId::new(size, egui::FontFamily::Name(family.into())),
                            egui::Color32::from_rgb(244, 223, 174),
                            text,
                            FontRunOptions {
                                stroke: italic_stroke.then_some(NativeFontStroke {
                                    width: 1.0,
                                    color: egui::Color32::from_rgb(96, 24, 90),
                                }),
                                ..Default::default()
                            },
                        );
                        match run {
                            Ok(run) => {
                                run.paint(
                                    frame,
                                    ui.painter(),
                                    ui.min_rect().min + egui::vec2(0.0, y),
                                    1.0,
                                )?;
                                y += run.size().y + 12.0;
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    let field = egui::Rect::from_min_size(
                        ui.min_rect().min + egui::vec2(0.0, y + 12.0),
                        egui::vec2(viewport[0] - 72.0, 220.0),
                    );
                    NativeTextEdit::multiline(text, "probe/editor", "Native text input")
                        .font(egui::FontId::new(
                            22.0,
                            egui::FontFamily::Name("JetBrains Mono".into()),
                        ))
                        .background(egui::Color32::from_rgb(34, 28, 40))
                        .tab_size(4.0)
                        .request_focus(first)
                        .show(frame, ui, field);
                    let button = ui.put(
                        egui::Rect::from_min_size(
                            field.left_bottom() + egui::vec2(0.0, 20.0),
                            egui::vec2(180.0, 42.0),
                        ),
                        egui::Button::new("Clear text"),
                    );
                    frame.record("probe/clear", "Clear text", "button", &button);
                    if button.clicked() {
                        text.clear();
                        *clicks += 1;
                    }
                    operations.paint(frame, ui, field.left_bottom() + egui::vec2(0.0, 72.0))?;
                    Ok(())
                })
                .inner?;
            operations.fullscreen.paint(frame)
        }
        fn dispatch(&mut self) -> Result<(), String> {
            let Some(ui) = self.ui.as_mut() else { return Ok(()) };
            let Some(mut run) =
                ui.begin_native_dispatch(solomon_web::platform::monotonic_ms() / 1000.0)?
            else {
                return Ok(());
            };
            let window = self.window.as_ref().ok_or("Native UI window absent")?;
            let size = window.window.inner_size().to_logical::<f32>(window.window.scale_factor());
            let mut paint = |frame: &mut solomon_web::gpu_ui::UiFrame<'_>| {
                Self::paint(
                    frame,
                    [size.width, size.height],
                    &mut self.text,
                    &mut self.clicks,
                    false,
                    self.operations.as_mut().ok_or("Native operations absent")?,
                )
            };
            let result = (|| {
                while ui.paint_next(&mut run, &mut paint)? {}
                ui.paint_layout(&mut run, &mut paint)
            })();
            ui.finish_native_dispatch(run, result)
        }
        fn capture(&self) -> Result<(), String> {
            let window = self.window.as_ref().ok_or("Native UI window absent")?;
            let renderer = self.renderer.as_ref().ok_or("Native UI renderer absent")?;
            let size = window.window.inner_size();
            let mut rgba = vec![0_u8; size.width as usize * size.height as usize * 4];
            let gl = renderer.graphics_context();
            // SAFETY: the owning NativeWindow is alive/current and the output slice covers its framebuffer.
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
                return Err(format!("Native UI readback error: {error}"));
            }
            let mut bitmap = image::RgbaImage::from_raw(size.width, size.height, rgba)
                .ok_or("UI readback extent")?;
            image::imageops::flip_vertical_in_place(&mut bitmap);
            std::fs::create_dir_all(&self.output).map_err(|error| error.to_string())?;
            bitmap.save(self.output.join("native-ui.png")).map_err(|error| error.to_string())?;
            let snapshot =
                self.ui.as_ref().ok_or("Native UI absent")?.snapshot("native-ui-probe", true, None);
            std::fs::write(self.output.join("native-ui.json"), serde_json::to_vec_pretty(&serde_json::json!({
                "renderer":device,"graphicsError":error,"ui":snapshot,"text":self.text,"clicks":self.clicks,"audio":self.audio.as_ref().map(|audio|audio.snapshot()),"audioTrace":self.audio_trace,"operations":self.operations.as_ref().map(|operations|operations.events.borrow().clone()),"fullscreen":window.window.fullscreen().is_some(),
            })).map_err(|error| error.to_string())?).map_err(|error| error.to_string())
        }
        fn close(&mut self, event_loop: &ActiveEventLoop) {
            self.finished = true;
            self.tasks.take();
            self.audio.take();
            self.operations.take();
            self.ui.take();
            self.renderer.take();
            self.window.take();
            event_loop.exit();
        }
    }
    impl ApplicationHandler<egui_winit::accesskit_winit::Event> for Probe {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let result = (|| {
                let window =
                    NativeWindow::open(event_loop, "Solomon native UI qualification", [1000, 740])?;
                // SAFETY: Probe closes pending work and UI/renderer before NativeWindow on every exit.
                let renderer = unsafe { window.renderer()? };
                solomon_graphics::assets::configure(solomon_graphics::assets::NativeAssets::new(
                    &self.assets,
                )?);
                self.tasks = Some(ClientTasks::install());
                let mut ui = futures_lite::future::block_on(GpuUi::load(
                    &Surface::from_window(window.window.clone()),
                    &renderer,
                ))?;
                ui.init_accesskit(event_loop, self.proxy.clone());
                ui.context().enable_accesskit();
                if self.check_audio {
                    let audio =
                        futures_lite::future::block_on(solomon_web::audio::GameAudio::load(
                            &Surface::from_window(window.window.clone()),
                        ))?;
                    audio.set_volumes(20, 15);
                    audio.set_scene(solomon_sim::audio::MusicScene::Title)?;
                    self.audio = Some(audio);
                }
                self.operations =
                    Some(Operations::new(Surface::from_window(window.window.clone()))?);
                window.window.set_visible(true);
                window.window.request_redraw();
                self.ui = Some(ui);
                self.renderer = Some(renderer);
                self.window = Some(window);
                Ok::<_, String>(())
            })();
            if let Err(error) = result {
                self.error = Some(error);
                self.close(event_loop);
            }
        }
        fn user_event(&mut self, _: &ActiveEventLoop, event: egui_winit::accesskit_winit::Event) {
            if let Err(error) = self.dispatch() {
                self.error = Some(error);
                return;
            }
            if let Some(ui) = &mut self.ui {
                match event.window_event {
                    egui_winit::accesskit_winit::WindowEvent::InitialTreeRequested => {
                        ui.context().enable_accesskit()
                    }
                    egui_winit::accesskit_winit::WindowEvent::ActionRequested(action) => {
                        if let Err(error) = ui.accesskit_action(action) {
                            self.error = Some(error);
                        }
                    }
                    egui_winit::accesskit_winit::WindowEvent::AccessibilityDeactivated => {}
                }
            }
        }
        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            if self.finished {
                return;
            }
            let result = (|| {
                self.dispatch()?;
                if let Some(ui) = &mut self.ui {
                    ui.window_event(&event)?;
                }
                if let WindowEvent::Resized(size) = event
                    && let Some(window) = &self.window
                {
                    window.resize(size.width, size.height);
                }
                if matches!(event, WindowEvent::RedrawRequested) {
                    if let Some(tasks) = &self.tasks {
                        tasks.tick();
                    }
                    self.draw()?;
                    if self.capture_after != 0 && self.frames >= self.capture_after {
                        self.capture()?;
                        self.close(event_loop);
                    } else if let Some(window) = &self.window {
                        window.window.request_redraw();
                    }
                } else if matches!(event, WindowEvent::CloseRequested) {
                    self.capture()?;
                    self.close(event_loop);
                }
                Ok::<_, String>(())
            })();
            if let Err(error) = result {
                self.error = Some(error);
                self.close(event_loop);
            }
        }
    }
    let mut args = std::env::args_os().skip(1);
    let assets = PathBuf::from(args.next().ok_or("Pass the packaged web asset root")?);
    let output = PathBuf::from(args.next().ok_or("Pass a receipt directory")?);
    let capture_after = args.next().map_or(Ok(30), |value| {
        value.to_string_lossy().parse::<u32>().map_err(|error| error.to_string())
    })?;
    let check_audio = args.next().is_some_and(|value| value == "audio");
    let event_loop = EventLoop::<egui_winit::accesskit_winit::Event>::with_user_event()
        .build()
        .map_err(|error| error.to_string())?;
    let mut probe = Probe {
        operations: None,
        audio: None,
        check_audio,
        audio_trace: Vec::new(),
        ui: None,
        renderer: None,
        tasks: None,
        window: None,
        assets,
        output,
        frames: 0,
        capture_after,
        text: "Native clipboard, IME and selection.\n\tAV ffi e\u{301} 😀\nWizard مرحبا".into(),
        clicks: 0,
        proxy: event_loop.create_proxy(),
        error: None,
        finished: false,
    };
    event_loop.run_app(&mut probe).map_err(|error| error.to_string())?;
    probe.error.map_or(Ok(()), Err)
}
