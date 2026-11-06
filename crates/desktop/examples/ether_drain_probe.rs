//! Native SecondaryView comparison with independent stock-record/crop submission.
//! Run with an output directory; this is composed rendering, not a live-game journey.

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use glow::HasContext;
    use solomon_client::Camera;
    use solomon_client::enemies::EnemyView;
    use solomon_client::secondary::SecondaryView;
    use solomon_client::spells::{SpellView, StockAtlas};
    use solomon_desktop::window::NativeWindow;
    use solomon_graphics::{BlendMode, Renderer, Vertex};
    use solomon_sim::Vec2;
    use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
    use solomon_sim::secondary::ether_drain::{
        EtherDrainAnimation as Animation, EtherDrainAtlas as Atlas, EtherDrainState,
    };
    use solomon_sim::secondary::{SecondaryActorKind, SecondaryActorState};
    use std::path::{Path, PathBuf};
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::window::WindowId;

    const SIZE: [u32; 2] = [640, 480];
    const ROOT: Vec2 = Vec2::new(320.0, 280.0);

    struct Case {
        name: String,
        animations: Vec<Animation>,
        quality: bool,
        pixel_centers: bool,
        retain_image: bool,
    }

    fn actor(animations: Vec<Animation>) -> SecondaryActorState {
        let mut actor = SecondaryActorState::new(
            1,
            SecondaryActorKind::EtherDrain,
            "caster".into(),
            "boneyard:probe".into(),
            Some(74),
        );
        actor.position = ROOT;
        actor.alpha = 0.25;
        actor.scale = 1.0;
        actor.rotation_radians = 0.4;
        actor.painter_registrations = vec![PainterRegistration {
            manager_lane: WorldManagerLane::Transient,
            registration_ordinal: 1,
        }];
        actor.ether_drain = Some(EtherDrainState { animations, ..Default::default() });
        actor
    }

    fn captured(atlas: Atlas, entry: usize, body_y_offset: u32, alpha: f64) -> Animation {
        Animation::Captured { alpha, atlas, entry, tint: 0xffffff, body_y_offset }
    }

    fn cases() -> Vec<Case> {
        let mut result = Vec::new();
        let mut banks = Vec::new();
        for base in [1477, 1495, 1513, 1531, 1549, 1567] {
            banks.push((Atlas::BadGuys, base, 23));
        }
        for base in [2293, 2311, 2329] {
            banks.push((Atlas::BadGuys, base, 0));
        }
        banks.push((Atlas::Demon, 80, 0));
        for (atlas, base, body_y) in banks {
            for entry in base..base + 18 {
                result.push(Case {
                    name: format!("{atlas:?}-{entry}-clipped"),
                    animations: vec![captured(atlas, entry, body_y, 0.8)],
                    quality: false,
                    pixel_centers: false,
                    retain_image: entry == base && [1477, 2293, 80].contains(&base),
                });
            }
            for quality in [false, true] {
                for pixel_centers in [false, true] {
                    for alpha in [1.25, 1.0, 0.8, 0.05] {
                        if !quality && !pixel_centers && alpha == 0.8 {
                            continue;
                        }
                        result.push(Case {
                            name: format!(
                                "{atlas:?}-{base}-alpha{alpha}-q{quality}-p{pixel_centers}"
                            ),
                            animations: vec![captured(atlas, base, body_y, alpha)],
                            quality,
                            pixel_centers,
                            retain_image: quality
                                && pixel_centers
                                && alpha == 1.25
                                && [1477, 2293, 80].contains(&base),
                        });
                    }
                }
            }
        }
        for variant in 0..3 {
            for quality in [false, true] {
                for pixel_centers in [false, true] {
                    result.push(Case {
                        name: format!("debris{variant}-q{quality}-p{pixel_centers}"),
                        animations: vec![Animation::Debris {
                            position: Vec2::new(ROOT.x + 45.0, ROOT.y - 35.0),
                            direction: Vec2::new(1.0, 0.0),
                            remaining_distance: 30.0,
                            speed: 1.0,
                            oscillation_degrees: 0.0,
                            rotation_degrees: 90.0,
                            variant,
                        }],
                        quality,
                        pixel_centers,
                        retain_image: variant == 0 && quality && pixel_centers,
                    });
                }
            }
        }
        // Reusing the same view must retire the previous private draws on each transition.
        for pixel_centers in [false, true] {
            result.push(Case {
                name: format!("mixed-p{pixel_centers}"),
                animations: vec![
                    Animation::Debris {
                        position: Vec2::new(ROOT.x - 35.0, ROOT.y - 50.0),
                        direction: Vec2::new(1.0, 0.0),
                        remaining_distance: 30.0,
                        speed: 1.0,
                        oscillation_degrees: 0.0,
                        rotation_degrees: 90.0,
                        variant: 2,
                    },
                    captured(Atlas::Demon, 80, 0, 0.8),
                ],
                quality: true,
                pixel_centers,
                retain_image: pixel_centers,
            });
            result.push(Case {
                name: format!("retired-p{pixel_centers}"),
                animations: vec![],
                quality: true,
                pixel_centers,
                retain_image: pixel_centers,
            });
        }
        result
    }

    /// Independent axis-aligned crop oracle: interpolate the record UV rectangle
    /// directly rather than using the production triangle clipper or draw plan.
    fn record(
        renderer: &mut Renderer,
        atlas: &StockAtlas,
        entry: usize,
        offset: Vec2,
        tint: u32,
        clip: bool,
        quarter_turn: bool,
    ) {
        let cell = &atlas.records[entry];
        let width = f64::from(cell.width);
        let height = f64::from(cell.height);
        let left = -width / 2.0 + f64::from(cell.center_x) + offset.x;
        let top = -height / 2.0 + f64::from(cell.center_y) + offset.y;
        let bounds = if clip {
            [left.max(-100.0), top.max(-110.0), (left + width).min(100.0), (top + height).min(0.0)]
        } else {
            [left, top, left + width, top + height]
        };
        if bounds[2] <= bounds[0] || bounds[3] <= bounds[1] {
            return;
        }
        let points = [
            (bounds[0], bounds[1]),
            (bounds[2], bounds[1]),
            (bounds[2], bounds[3]),
            (bounds[0], bounds[3]),
        ];
        let vertices: [Vertex; 4] = std::array::from_fn(|index| {
            let (x, y) = points[index];
            let (px, py) = if quarter_turn {
                (offset.x - (y - offset.y), offset.y + (x - offset.x))
            } else {
                (x, y)
            };
            Vertex {
                position: [(ROOT.x + px) as f32, (ROOT.y + py) as f32],
                texel: [
                    (f64::from(cell.x) + 0.5 + (x - left) / width * (width - 0.25)) as f32,
                    (f64::from(cell.y) + 0.5 + (y - top) / height * (height - 0.25)) as f32,
                ],
                tint: [
                    ((tint >> 16) & 255) as f32 / 255.0,
                    ((tint >> 8) & 255) as f32 / 255.0,
                    (tint & 255) as f32 / 255.0,
                    63.0 / 255.0,
                ],
            }
        });
        renderer.draw_triangles(
            &atlas.texture,
            &[[vertices[0], vertices[1], vertices[2]], [vertices[0], vertices[2], vertices[3]]],
            BlendMode::Normal,
        );
    }

    fn reference(
        renderer: &mut Renderer,
        animations: &[Animation],
        bad: &StockAtlas,
        dead: &StockAtlas,
        demon: &StockAtlas,
    ) {
        for animation in animations {
            match *animation {
                Animation::Debris { position, variant, .. } => {
                    record(
                        renderer,
                        dead,
                        177 + variant as usize,
                        Vec2::new(position.x - ROOT.x, position.y - ROOT.y),
                        0xffffff,
                        false,
                        true,
                    );
                }
                Animation::Captured { alpha, atlas, entry, tint, body_y_offset } => {
                    if alpha < 1.0 {
                        record(renderer, bad, 9, Vec2::ZERO, 0xffffff, false, false);
                    }
                    record(
                        renderer,
                        if atlas == Atlas::Demon { demon } else { bad },
                        entry,
                        Vec2::new(
                            0.0,
                            f64::from(body_y_offset) - 10.0 + 35.0 * (1.0 - alpha).max(0.0),
                        ),
                        tint,
                        alpha < 1.0,
                        false,
                    );
                    if alpha < 1.0 {
                        record(renderer, bad, 8, Vec2::new(0.0, -1.0), 0xffffff, false, false);
                    }
                }
            }
        }
    }

    fn pixels(renderer: &Renderer) -> Result<Vec<u8>, String> {
        let mut pixels = vec![0; SIZE[0] as usize * SIZE[1] as usize * 4];
        let gl = renderer.graphics_context();
        // SAFETY: the current probe context owns this framebuffer and complete readback slice.
        unsafe {
            gl.read_pixels(
                0,
                0,
                SIZE[0] as i32,
                SIZE[1] as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut pixels)),
            );
            let error = gl.get_error();
            if error != glow::NO_ERROR {
                return Err(format!("GPU readback: {error}"));
            }
        }
        Ok(pixels)
    }

    fn image(path: &Path, pixels: Vec<u8>) -> Result<(), String> {
        let mut image =
            image::RgbaImage::from_raw(SIZE[0], SIZE[1], pixels).ok_or("image extent")?;
        image::imageops::flip_vertical_in_place(&mut image);
        image.save(path).map_err(|error| error.to_string())
    }

    fn qualify(event_loop: &ActiveEventLoop, output: &Path) -> Result<(), String> {
        let window = NativeWindow::open(event_loop, "Ether Drain private presentation", SIZE)?;
        // SAFETY: all views and the renderer drop before their owning native window.
        let mut renderer = unsafe { window.renderer()? };
        solomon_graphics::assets::configure(solomon_graphics::assets::NativeAssets::new(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web"),
        )?);
        let spells = futures_lite::future::block_on(SpellView::load(&renderer))?;
        let bad = spells.bad_guys();
        let dead = spells.dead_hawg();
        let enemies = futures_lite::future::block_on(EnemyView::load(
            &renderer,
            bad.records.clone(),
            bad.texture.clone(),
            dead.records.clone(),
            dead.texture.clone(),
        ))?;
        let demon = enemies.demon_atlas();
        let mut view =
            futures_lite::future::block_on(SecondaryView::load(&renderer, &spells, demon.clone()))?;
        let target = renderer.create_render_target_size(SIZE)?;
        let camera = Camera { origin: Vec2::ZERO, scale: 1.0 };
        renderer.begin_frame(SIZE[0] as i32, SIZE[1] as i32, [SIZE[0] as f32, SIZE[1] as f32]);
        std::fs::create_dir_all(output).map_err(|error| error.to_string())?;
        let mut results = Vec::new();
        for case in cases() {
            renderer.set_world_pixel_centers(case.pixel_centers);
            let mut captures = Vec::new();
            let mut baseline = Vec::new();
            for expected in [false, true] {
                let actor = actor(if expected { vec![] } else { case.animations.clone() });
                let mut result = Ok(Vec::new());
                renderer.render_to_color(
                    &target,
                    [SIZE[0] as f32, SIZE[1] as f32],
                    [0.12, 0.12, 0.12, 1.0],
                    |renderer| {
                        result = (|| {
                            view.prepare(
                                renderer,
                                std::iter::once(&actor),
                                "boneyard:probe",
                                Some(42.0),
                                None,
                                &|_| 1.0,
                                case.quality,
                            )?;
                            for (index, _) in view.painters() {
                                view.draw(renderer, &camera, index, [1.0; 3]);
                            }
                            if expected {
                                renderer.flush();
                                baseline = pixels(renderer)?;
                                reference(renderer, &case.animations, bad, dead, &demon);
                            }
                            renderer.flush();
                            pixels(renderer)
                        })();
                    },
                );
                captures.push(result?);
            }
            let maximum =
                captures[0].iter().zip(&captures[1]).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
            let different = captures[0]
                .as_chunks::<4>()
                .0
                .iter()
                .zip(captures[1].as_chunks::<4>().0)
                .filter(|(a, b)| a != b)
                .count();
            let visible_private_pixels = captures[1]
                .as_chunks::<4>()
                .0
                .iter()
                .zip(baseline.as_chunks::<4>().0)
                .filter(|(a, b)| a != b)
                .count();
            if (visible_private_pixels == 0) != case.animations.is_empty() {
                return Err(format!("{}: independent private art visibility", case.name));
            }
            if case.retain_image {
                image(&output.join(format!("{}-actual.png", case.name)), captures[0].clone())?;
                image(&output.join(format!("{}-expected.png", case.name)), captures[1].clone())?;
            }
            results.push(serde_json::json!({"name":case.name,"maximumDifference":maximum,
                "differentPixels":different,"visiblePrivatePixels":visible_private_pixels,
                "quality":case.quality,"nativePixelCenters":case.pixel_centers}));
        }
        let gl = renderer.graphics_context();
        // SAFETY: the live current context supports renderer identity queries.
        let device = unsafe { gl.get_parameter_string(glow::RENDERER) };
        let failed =
            results.iter().filter(|case| case["maximumDifference"].as_u64().unwrap() > 1).count();
        std::fs::write(output.join("comparison.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "renderer":device,"cases":results,"qualification":"Independent stock-record crop oracle; one 8-bit channel rounding level permitted. Not live-game acceptance.",
        })).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
        if failed > 0 {
            return Err(format!("{failed} composed cases exceed one channel level"));
        }
        Ok(())
    }

    pub fn run() -> Result<(), String> {
        struct Probe {
            output: PathBuf,
            error: Option<String>,
        }
        impl ApplicationHandler for Probe {
            fn resumed(&mut self, event_loop: &ActiveEventLoop) {
                self.error = qualify(event_loop, &self.output).err();
                event_loop.exit();
            }
            fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
        }
        let mut probe = Probe {
            output: std::env::args_os().nth(1).ok_or("output directory required")?.into(),
            error: None,
        };
        EventLoop::new()
            .map_err(|error| error.to_string())?
            .run_app(&mut probe)
            .map_err(|error| error.to_string())?;
        probe.error.map_or(Ok(()), Err)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), String> {
    native::run()
}
#[cfg(target_arch = "wasm32")]
fn main() {}
