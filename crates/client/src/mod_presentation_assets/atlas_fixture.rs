//! Adversarial authored sheet through the actual cached-frame and wearable owners.
use super::*;
use solomon_graphics::{RenderTarget, TextureSampling};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use web_sys::WebGl2RenderingContext as Gl;

const WIDTH: usize = 4080;
const HEIGHT: usize = 170;
const OUTPUT: usize = 256;

struct Fixture {
    gl: Gl,
    renderer: Renderer,
    assets: ModPresentationAssets,
    sprite: ModSpriteTexture,
    reference: ModSpriteTexture,
    wearable: Option<ModSpriteTexture>,
    wearable_reference: ModSpriteTexture,
    output: RenderTarget,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.assets.destroy(&mut self.renderer);
        self.renderer.destroy_texture(&self.reference.texture);
        self.renderer.destroy_texture(&self.wearable_reference.texture);
        self.renderer.destroy_render_target(&self.output);
    }
}

thread_local! {
    static FIXTURE: RefCell<Option<Fixture>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
pub fn mod_atlas_fixture_init(gl: Gl, case: &str) -> Result<(), JsValue> {
    let create = || -> Result<Fixture, String> {
        let renderer = Renderer::new(gl.clone())?;
        let mut pixels = [255, 0, 255, 255].repeat(WIDTH * HEIGHT);
        for y in 0..8 {
            for x in 0..16 {
                let color = if x >= 8 {
                    [0, 0, 255, 255]
                } else if (x + y) % 2 == 0 {
                    [255, 0, 0, 255]
                } else {
                    [0, 255, 0, 255]
                };
                let start = (y * WIDTH + x) * 4;
                pixels[start..start + 4].copy_from_slice(&color);
            }
        }
        let (x, y, width, height, reference_pixels) = match case {
            "integer" => {
                let mut crop = Vec::new();
                for y in 0..8 {
                    crop.extend_from_slice(&pixels[y * WIDTH * 4..y * WIDTH * 4 + 32]);
                }
                (0.0, 0.0, 8.0, 8.0, crop)
            }
            "half-span" => (0.25, 0.25, 0.5, 0.5, vec![255, 0, 0, 255]),
            "half-span-between-texels" => (0.75, 0.75, 0.5, 0.5, vec![128, 128, 0, 255]),
            "fractional-origin" => (0.5, 0.5, 1.0, 1.0, vec![128, 128, 0, 255]),
            _ => return Err(format!("Unknown authored frame case {case}")),
        };
        let frame = ModSpriteFrame {
            x,
            y,
            width,
            height,
            logical_width: width,
            logical_height: height,
            content_width: width,
            content_height: height,
            center_offset_x: 0.0,
            center_offset_y: 0.0,
        };
        let source = renderer.upload_rgba(WIDTH as u32, HEIGHT as u32, &pixels)?;
        let mut assets = ModPresentationAssets {
            images: BTreeMap::new(),
            bases: BTreeMap::from([(asset_key("fixture", "shared.png"), source)]),
            frames: BTreeMap::new(),
            wearables: BTreeMap::new(),
            destroyed: false,
        };
        let sprite = assets.sprite_frame("fixture", "fixture", "shared.png", frame)?;
        let reference_size = if case == "integer" { 8 } else { 1 };
        let reference = ModSpriteTexture {
            texture: renderer.upload_rgba(reference_size, reference_size, &reference_pixels)?,
            frame: ModSpriteFrame { x: 0.0, y: 0.0, ..frame },
            sampling: TextureSampling::Texture,
        };
        let mut cell = Vec::new();
        for y in 0..170 {
            cell.extend_from_slice(&pixels[y * WIDTH * 4..y * WIDTH * 4 + 170 * 4]);
        }
        let wearable_reference = ModSpriteTexture {
            texture: renderer.upload_rgba(170, 170, &cell)?,
            frame: mod_wearable_cell(0, 0),
            sampling: TextureSampling::Nearest,
        };
        let output = renderer.create_render_target(OUTPUT as u32)?;
        Ok(Fixture {
            gl: gl.clone(),
            renderer,
            assets,
            sprite,
            reference,
            wearable: None,
            wearable_reference,
            output,
        })
    };
    let fixture = create().map_err(|e| JsValue::from_str(&e))?;
    FIXTURE.with(|slot| *slot.borrow_mut() = Some(fixture));
    Ok(())
}

#[wasm_bindgen]
pub fn mod_atlas_fixture_prepare_wearable() -> Result<(), JsValue> {
    FIXTURE.with(|slot| {
        let mut fixture = slot.borrow_mut();
        let fixture =
            fixture.as_mut().ok_or_else(|| JsValue::from_str("Authored fixture is absent"))?;
        let frames = fixture
            .assets
            .actor_sheet("fixture", "shared.png", ModWearableSlot::Hat)
            .map_err(|e| JsValue::from_str(&e))?;
        fixture.wearable = Some(frames[0][0].clone());
        Ok(())
    })
}

#[wasm_bindgen]
pub fn mod_atlas_fixture_render(
    reference: bool,
    wearable: bool,
) -> Result<js_sys::Uint8Array, JsValue> {
    FIXTURE.with(|slot| {
        let mut fixture = slot.borrow_mut();
        let fixture =
            fixture.as_mut().ok_or_else(|| JsValue::from_str("Authored fixture is absent"))?;
        let sprite = match (reference, wearable) {
            (true, true) => &fixture.wearable_reference,
            (true, false) => &fixture.reference,
            (false, true) => fixture
                .wearable
                .as_ref()
                .ok_or_else(|| JsValue::from_str("Wearable was not prepared"))?,
            (false, false) => &fixture.sprite,
        };
        let draw = ModSpriteDraw {
            center: Vec2::new(107.3, 96.7),
            scale: Vec2::new(60.0 / sprite.frame.width, 60.0 / sprite.frame.height),
            anchor: Vec2::new(0.5, 0.5),
            rotation_radians: 0.125,
            tint: [1.0; 4],
            blend: BlendMode::Normal,
        };
        let mut pixels = vec![0; OUTPUT * OUTPUT * 4];
        let mut read = Ok(());
        fixture.renderer.render_to_color(
            &fixture.output,
            [OUTPUT as f32; 2],
            [0.1, 0.2, 0.3, 1.0],
            |renderer| {
                if reference {
                    let sampling =
                        if wearable { TextureSampling::Nearest } else { TextureSampling::Texture };
                    renderer.draw_sampled(sampling, |renderer| {
                        let frame = sprite.frame;
                        renderer.draw_quad(
                            &sprite.texture,
                            [
                                frame.x as f32,
                                frame.y as f32,
                                frame.width as f32,
                                frame.height as f32,
                            ],
                            mod_sprite_corners(
                                frame,
                                draw.center,
                                draw.scale,
                                draw.anchor,
                                draw.rotation_radians,
                            )
                            .map(|point| [point.x as f32, point.y as f32]),
                            [draw.tint; 4],
                            draw.blend,
                        );
                    });
                } else {
                    sprite.draw(renderer, &draw);
                }
                renderer.flush();
                read = fixture.gl.read_pixels_with_opt_u8_array(
                    0,
                    0,
                    OUTPUT as i32,
                    OUTPUT as i32,
                    Gl::RGBA,
                    Gl::UNSIGNED_BYTE,
                    Some(&mut pixels),
                );
            },
        );
        read?;
        if fixture.gl.get_error() != Gl::NO_ERROR {
            return Err(JsValue::from_str("Authored frame WebGL error"));
        }
        Ok(js_sys::Uint8Array::from(pixels.as_slice()))
    })
}

#[wasm_bindgen]
pub fn mod_atlas_fixture_dispose() {
    FIXTURE.with(|slot| drop(slot.borrow_mut().take()));
}
