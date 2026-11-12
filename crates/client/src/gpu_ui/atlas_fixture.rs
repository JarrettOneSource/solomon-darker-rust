//! Development-only pixel comparison through the actual UI material and painter.
//! The supplied RGBA bytes are decoded from stock PNGs without premultiplication.
use super::*;
use solomon_graphics::{RenderTarget, Texture};
use wasm_bindgen::prelude::*;
use web_sys::WebGl2RenderingContext as Gl;

struct Fixture {
    gl: Gl,
    renderer: Renderer,
    painter: painter::GpuPainter,
    images: painter::Images,
    atlas: StockAtlas,
    references: [StockAtlas; 2],
    records: [usize; 2],
    textures: Vec<Texture>,
    output: RenderTarget,
    ctx: Context,
    authored: Option<Rect>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for texture in &self.textures {
            self.renderer.destroy_texture(texture);
        }
        self.renderer.destroy_render_target(&self.output);
    }
}
thread_local! {
    static FIXTURE: RefCell<Option<Fixture>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
pub async fn ui_atlas_fixture_init(
    gl: Gl,
    name: &str,
    pixels: &[u8],
    record: usize,
) -> Result<(), JsValue> {
    let name = match name {
        "UI" => "UI",
        "Create" => "Create",
        _ => return Err(JsValue::from_str("UI atlas fixture supports UI and Create pages")),
    };
    let atlas = StockAtlas::load(name).await.map_err(|e| JsValue::from_str(&e))?;
    if pixels.len() != 1024 * 1024 * 4 {
        return Err(JsValue::from_str("Expected one exact 1024-square stock RGBA page"));
    }
    let records = [record, if name == "UI" { if record == 37 { 3 } else { 37 } } else { 13 }];
    let renderer = Renderer::new(gl.clone()).map_err(|e| JsValue::from_str(&e))?;
    let images = painter::Images::default();
    let mut textures =
        vec![renderer.upload_rgba(1024, 1024, pixels).map_err(|e| JsValue::from_str(&e))?];
    images.register(atlas.path(), &textures[0], &renderer.graphics_context());
    let mut references = Vec::new();
    for (index, record) in records.iter().enumerate() {
        let record = atlas.record(*record).map_err(|e| JsValue::from_str(&e))?;
        let mut crop = Vec::with_capacity((record.width * record.height * 4) as usize);
        for y in record.y..record.y + record.height {
            let start = ((y * 1024 + record.x) * 4) as usize;
            crop.extend_from_slice(&pixels[start..start + record.width as usize * 4]);
        }
        let texture = renderer
            .upload_rgba(record.width, record.height, &crop)
            .map_err(|e| JsValue::from_str(&e))?;
        let path = format!("ui-atlas-reference:{index}");
        images.register(&path, &texture, &renderer.graphics_context());
        references.push(
            StockAtlas::from_image(&path, vec2(record.width as f32, record.height as f32))
                .map_err(|e| JsValue::from_str(&e))?,
        );
        textures.push(texture);
    }
    let output = renderer.create_render_target(512).map_err(|e| JsValue::from_str(&e))?;
    let painter = painter::GpuPainter::new(&renderer);
    FIXTURE.with(|slot| {
        *slot.borrow_mut() = Some(Fixture {
            gl,
            renderer,
            painter,
            images,
            atlas,
            references: references
                .try_into()
                .unwrap_or_else(|_| unreachable!("two source records")),
            records,
            textures,
            output,
            ctx: Context::default(),
            authored: None,
        });
    });
    Ok(())
}

#[wasm_bindgen]
pub fn ui_authored_fixture_init(gl: Gl, case: &str) -> Result<(), JsValue> {
    let create = || -> Result<Fixture, String> {
        let renderer = Renderer::new(gl.clone())?;
        let pixels: Vec<u8> =
            (0..16 * 16)
                .flat_map(|i| {
                    if (i % 16 + i / 16) % 2 == 0 { [255, 0, 0, 255] } else { [0, 255, 0, 255] }
                })
                .collect();
        let (source, color) = match case {
            "half-span" => {
                (Rect::from_min_size(pos2(0.25, 0.25), vec2(0.5, 0.5)), [255, 0, 0, 255])
            }
            "half-span-between-texels" => {
                (Rect::from_min_size(pos2(0.75, 0.75), vec2(0.5, 0.5)), [128, 128, 0, 255])
            }
            "fractional-origin" => {
                (Rect::from_min_size(pos2(0.5, 0.5), vec2(1.0, 1.0)), [128, 128, 0, 255])
            }
            _ => return Err(format!("Unknown authored UI case {case}")),
        };
        let textures =
            vec![renderer.upload_rgba(16, 16, &pixels)?, renderer.upload_rgba(1, 1, &color)?];
        let images = painter::Images::default();
        images.register("authored:source", &textures[0], &renderer.graphics_context());
        images.register("authored:reference", &textures[1], &renderer.graphics_context());
        let atlas = StockAtlas::from_image("authored:source", vec2(16.0, 16.0))?;
        let references = [
            StockAtlas::from_image("authored:reference", Vec2::splat(1.0))?,
            StockAtlas::from_image("authored:reference", Vec2::splat(1.0))?,
        ];
        let output = renderer.create_render_target(512)?;
        let painter = painter::GpuPainter::new(&renderer);
        Ok(Fixture {
            gl: gl.clone(),
            renderer,
            painter,
            images,
            atlas,
            references,
            records: [0, 0],
            textures,
            output,
            ctx: Context::default(),
            authored: Some(source),
        })
    };
    let fixture = create().map_err(|e| JsValue::from_str(&e))?;
    FIXTURE.with(|slot| *slot.borrow_mut() = Some(fixture));
    Ok(())
}

#[wasm_bindgen]
pub fn ui_atlas_fixture_render(
    method: &str,
    reference: bool,
) -> Result<js_sys::Uint8Array, JsValue> {
    FIXTURE.with(|slot| {
        let mut fixture = slot.borrow_mut();
        let fixture =
            fixture.as_mut().ok_or_else(|| JsValue::from_str("UI atlas fixture is absent"))?;
        render(fixture, method, reference)
            .map(|bytes| js_sys::Uint8Array::from(bytes.as_slice()))
            .map_err(|e| JsValue::from_str(&e))
    })
}

#[wasm_bindgen]
pub fn ui_atlas_fixture_dispose() {
    FIXTURE.with(|slot| drop(slot.borrow_mut().take()));
}

fn render(fixture: &mut Fixture, method: &str, reference: bool) -> Result<Vec<u8>, String> {
    let Fixture {
        gl,
        renderer,
        painter,
        images,
        atlas,
        references,
        records,
        output,
        ctx,
        authored,
        ..
    } = fixture;
    images.begin_frame();
    let widgets = RefCell::new(BTreeMap::new());
    let previous_widgets = BTreeMap::new();
    let announcements = RefCell::new(Vec::new());
    let input_scope = RefCell::new(None);
    let retired_images = RefCell::new(Vec::new());
    let editors = RefCell::new(Vec::new());
    let editor_errors = RefCell::new(Vec::new());
    let mut painted: Result<(), String> = Ok(());
    let frame_output = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(512.0, 512.0))),
            ..Default::default()
        },
        |ctx| {
            let frame = UiFrame {
                ctx,
                images,
                widgets: &widgets,
                previous_widgets: &previous_widgets,
                key_codes: &[],
                blend: BlendMode::Normal,
                color_filter: ColorFilter::default(),
                image_bounds: None,
                announcements: &announcements,
                rotation: egui::emath::Rot2::IDENTITY,
                translation: Vec2::ZERO,
                input_scope: &input_scope,
                retired_images: &retired_images,
                focus_from_keyboard: false,
                editor_receipts: &[],
                editors: &editors,
                editor_errors: &editor_errors,
                native_click_count: None,
            };
            let layer = ctx.layer_painter(egui::LayerId::background());
            painted = (|| {
                if let Some(source) = authored {
                    let path = if reference { references[0].path() } else { atlas.path() };
                    let source = if reference {
                        Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0))
                    } else {
                        *source
                    };
                    let destination = Rect::from_min_size(pos2(63.3, 35.7), vec2(60.0, 60.0));
                    let ready =
                        match method {
                            "raw" => {
                                frame.atlas_image(&layer, path, source, destination, Color32::WHITE)
                            }
                            "rotated" => frame
                                .with_rotation(destination.center(), 0.125)
                                .atlas_image(&layer, path, source, destination, Color32::WHITE),
                            "shadow3" | "shadow4" => frame.atlas_image_shadowed(
                                &layer,
                                path,
                                source,
                                destination,
                                ImageShadow {
                                    offset: vec2(2.0, 1.0),
                                    blur: if method == "shadow3" { 3.0 } else { 4.0 },
                                    color: Color32::from_black_alpha(150),
                                },
                                Color32::WHITE,
                            ),
                            _ => return Err(format!("Unknown authored UI method {method}")),
                        };
                    return if ready {
                        Ok(())
                    } else {
                        Err("Authored UI image is not ready".into())
                    };
                }
                let (image, index) =
                    if reference { (&references[0], 0) } else { (&*atlas, records[0]) };
                let record = image.record(index)?;
                let size = vec2(record.width as f32, record.height as f32);
                let scale = 1.7_f32.min(340.0 / size.max_elem());
                let destination = Rect::from_min_size(pos2(63.3, 35.7), size * scale);
                let paint = |frame: &UiFrame<'_>, layer: &Painter| -> Result<(), String> {
                    match method {
                        "raw" | "group" => {
                            image.paint_raw(frame, layer, index, destination, Color32::WHITE)
                        }
                        "rotated" => image.paint_raw(
                            &frame.with_rotation(destination.center(), 0.17),
                            layer,
                            index,
                            destination,
                            Color32::WHITE,
                        ),
                        "slice" => image.paint_slice(
                            frame,
                            layer,
                            index,
                            Rect::from_min_max(
                                pos2(0.25, 0.25),
                                (size - Vec2::splat(0.25)).to_pos2(),
                            ),
                            destination,
                            Color32::WHITE,
                        ),
                        "frame" => image.paint_frame(
                            frame,
                            layer,
                            index,
                            destination,
                            12.0,
                            Color32::WHITE,
                        ),
                        "shadow3" | "shadow4" => {
                            let source =
                                Rect::from_min_size(pos2(record.x as f32, record.y as f32), size);
                            let ready = frame.atlas_image_shadowed(
                                layer,
                                image.path(),
                                source,
                                destination,
                                ImageShadow {
                                    offset: vec2(2.0, 1.0),
                                    blur: if method == "shadow3" { 3.0 } else { 4.0 },
                                    color: Color32::from_black_alpha(150),
                                },
                                Color32::WHITE,
                            );
                            if ready { Ok(()) } else { Err("UI shadow image is not ready".into()) }
                        }
                        _ => Err(format!("Unknown UI atlas case {method}")),
                    }
                };
                if method == "group" {
                    frame.paint_group(
                        &layer,
                        GroupEffect { opacity: 0.7, ..Default::default() },
                        paint,
                    )?;
                } else {
                    paint(&frame, &layer)?;
                }
                let (second, index) =
                    if reference { (&references[1], 0) } else { (&*atlas, records[1]) };
                let record = second.record(index)?;
                second.paint_raw(
                    &frame,
                    &layer,
                    index,
                    Rect::from_min_size(
                        pos2(23.2, 414.7),
                        vec2(record.width as f32, record.height as f32) * 0.3,
                    ),
                    Color32::WHITE,
                )?;
                // An unbounded whole-page draw after two different records proves source scope retirement.
                frame.image(
                    &layer,
                    atlas.path(),
                    Rect::from_min_size(pos2(470.0, 470.0), vec2(32.0, 32.0)),
                    None,
                    Color32::WHITE,
                );
                Ok(())
            })();
        },
    );
    painted?;
    let primitives = ctx.tessellate(frame_output.shapes, frame_output.pixels_per_point);
    let mut result = Ok((0, 0));
    let mut bytes = vec![0; 512 * 512 * 4];
    let mut read = Ok(());
    renderer.render_to_color(output, [512.0; 2], [0.1, 0.2, 0.3, 1.0], |renderer| {
        result = painter.paint(
            renderer,
            [512; 2],
            [512.0; 2],
            images,
            primitives,
            frame_output.textures_delta,
        );
        renderer.flush();
        read = gl
            .read_pixels_with_opt_u8_array(
                0,
                0,
                512,
                512,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE,
                Some(&mut bytes),
            )
            .map_err(|e| format!("UI atlas readback: {e:?}"));
    });
    result?;
    read?;
    if gl.get_error() != Gl::NO_ERROR {
        return Err("UI atlas fixture produced a WebGL error".into());
    }
    Ok(bytes)
}
