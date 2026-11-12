//! Development-only readback of the actual UiFrame -> typed nodes -> GPU painter
//! program. Normal packages expose no fixture or pixel-readback entry point.
use super::*;
use solomon_graphics::{RenderTarget, Texture};
use wasm_bindgen::prelude::*;
use web_sys::WebGl2RenderingContext as Gl;

struct Fixture {
    gl: Gl,
    renderer: Renderer,
    painter: painter::GpuPainter,
    images: painter::Images,
    ui: StockAtlas,
    atlas: Texture,
    output: RenderTarget,
    ctx: Context,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.renderer.destroy_texture(&self.atlas);
        self.renderer.destroy_render_target(&self.output);
    }
}

thread_local! {
    static FIXTURE: RefCell<Option<Fixture>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
pub async fn unforge_fixture_init(gl: Gl) -> Result<(), JsValue> {
    let renderer = Renderer::new(gl.clone()).map_err(|error| JsValue::from_str(&error))?;
    let ui = StockAtlas::load("UI").await.map_err(|error| JsValue::from_str(&error))?;
    let image = solomon_graphics::assets::load_image(ui.path())
        .await
        .map_err(|error| JsValue::from_str(&error))?;
    let atlas = renderer.upload(&image).map_err(|error| JsValue::from_str(&error))?;
    let images = painter::Images::default();
    images.register(ui.path(), &atlas, &renderer.graphics_context());
    let output = renderer.create_render_target(256).map_err(|error| JsValue::from_str(&error))?;
    let painter = painter::GpuPainter::new(&renderer);
    FIXTURE.with(|fixture| {
        *fixture.borrow_mut() = Some(Fixture {
            gl,
            renderer,
            painter,
            images,
            ui,
            atlas,
            output,
            ctx: Context::default(),
        })
    });
    Ok(())
}

#[wasm_bindgen]
pub fn render_unforge_fixture(
    tick: i32,
    reveal: f32,
    capture: bool,
    grouped: bool,
) -> Result<js_sys::Uint8Array, JsValue> {
    FIXTURE.with(|fixture| {
        let mut fixture = fixture.borrow_mut();
        let fixture = fixture
            .as_mut()
            .ok_or_else(|| JsValue::from_str("Unforge fixture is not initialized"))?;
        render(fixture, tick, reveal, capture, grouped)
            .map(|pixels| js_sys::Uint8Array::from(pixels.as_slice()))
            .map_err(|error| JsValue::from_str(&error))
    })
}

#[wasm_bindgen]
pub fn unforge_fixture_dispose() {
    FIXTURE.with(|fixture| drop(fixture.borrow_mut().take()));
}

fn render(
    fixture: &mut Fixture,
    tick: i32,
    reveal: f32,
    capture: bool,
    grouped: bool,
) -> Result<Vec<u8>, String> {
    let Fixture { gl, renderer, painter, images, ui, output, ctx, .. } = fixture;
    images.begin_frame();
    let widgets = RefCell::new(BTreeMap::new());
    let previous_widgets = BTreeMap::new();
    let announcements = RefCell::new(Vec::new());
    let input_scope = RefCell::new(None);
    let retired_images = RefCell::new(Vec::new());
    let editors = RefCell::new(Vec::new());
    let editor_errors = RefCell::new(Vec::new());
    let mut recorded = Ok(());
    let frame_output = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(256.0, 256.0))),
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
            let paint = |frame: &UiFrame<'_>, painter: &Painter| {
                frame.paint_unforge_target(
                    painter,
                    ui,
                    f64::from(tick) * 10.0,
                    reveal,
                    vec2(-1344.0, -644.0),
                )
            };
            let layer = ctx.layer_painter(egui::LayerId::background());
            recorded = if grouped {
                frame.paint_group(&layer, GroupEffect::default(), paint)
            } else {
                paint(&frame, &layer)
            };
            layer.rect_filled(
                Rect::from_min_max(pos2(10.0, 10.0), pos2(30.0, 30.0)),
                0.0,
                Color32::from_rgb(18, 52, 86),
            );
        },
    );
    recorded?;
    let primitives = ctx.tessellate(frame_output.shapes, frame_output.pixels_per_point);
    let mut drawn = Ok((0, 0));
    let mut pixels = Vec::new();
    let mut read = Ok(());
    renderer.render_to_color(output, [256.0; 2], [0.0; 4], |renderer| {
        drawn = painter.paint(
            renderer,
            [256; 2],
            [256.0; 2],
            images,
            primitives,
            frame_output.textures_delta,
        );
        renderer.flush();
        read = read_pixels(gl).map(|value| pixels = value);
    });
    drawn?;
    read?;
    if capture {
        let target =
            painter.unforge_capture().ok_or("The actual Unforge operation submitted no target")?;
        let mut captured = Ok(Vec::new());
        renderer.render_onto(&target, [256.0; 2], |_| captured = read_pixels(gl));
        pixels = captured?;
    } else {
        renderer.begin_frame(256, 256, [256.0; 2]);
        renderer.draw_quad_with_color_mode(
            &output.texture,
            [0.0, 256.0, 256.0, -256.0],
            [[0.0, 0.0], [256.0, 0.0], [256.0, 256.0], [0.0, 256.0]],
            [[1.0; 4]; 4],
            BlendMode::Normal,
            solomon_graphics::TextureColorMode::Premultiplied,
        );
        renderer.flush();
    }
    if gl.get_error() != Gl::NO_ERROR {
        return Err("Unforge fixture produced a WebGL error".to_owned());
    }
    Ok(pixels)
}

fn read_pixels(gl: &Gl) -> Result<Vec<u8>, String> {
    let mut bottom_up = vec![0; 256 * 256 * 4];
    gl.read_pixels_with_opt_u8_array(
        0,
        0,
        256,
        256,
        Gl::RGBA,
        Gl::UNSIGNED_BYTE,
        Some(&mut bottom_up),
    )
    .map_err(|error| format!("Unforge readback: {error:?}"))?;
    let mut top_down = vec![0; bottom_up.len()];
    for y in 0..256 {
        top_down[y * 1024..(y + 1) * 1024]
            .copy_from_slice(&bottom_up[(255 - y) * 1024..(256 - y) * 1024]);
    }
    Ok(top_down)
}
