//! Source `loader-render-contract.ts`, `loader-renderer.ts` and `NativeLoader.tsx`.
//! The stock four-image splash is decoded and drawn before main WASM is requested.

#![cfg(target_arch = "wasm32")]

use solomon_graphics::{BlendMode, Quad, Renderer, Texture, TextureColorMode, assets};
use solomon_sim::native_ui_text::{
    NativeUiFontName, NativeUiTextSpec, layout_native_ui_text, measure_native_ui_text,
    native_ui_font, native_ui_glyph_ink_bounds,
};
use std::cell::{Cell, RefCell};
use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlCanvasElement, HtmlImageElement, KeyboardEvent, Window};

const ART: [(&str, [f32; 4]); 4] = [
    ("loader-logo.png", [41.0, 13.0, 388.0, 227.0]),
    ("loader-url.png", [119.0, 251.0, 244.0, 18.0]),
    ("loader-frame.png", [125.0, 263.0, 230.0, 54.0]),
    ("loader-fill.png", [144.0, 282.0, 192.0, 18.0]),
];

#[derive(Default)]
struct Progress {
    stage: String,
    item: String,
    completed: u32,
    total: u32,
    error: Option<String>,
}
impl Progress {
    fn ratio(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (f64::from(self.completed) / f64::from(self.total)).clamp(0.0, 1.0)
        }
    }
    fn percentage(&self) -> u32 {
        if self.completed >= self.total && self.total > 0 {
            100
        } else {
            (self.ratio() * 100.0).floor() as u32
        }
    }
}

struct Loader {
    window: Window,
    canvas: HtmlCanvasElement,
    renderer: Renderer,
    art: Vec<Texture>,
    font: Option<Texture>,
    progress: Progress,
    frame: u64,
    meshes: usize,
    triangles: usize,
    viewport: [f32; 2],
    logical: [f32; 2],
    scale: f32,
    keyboard: Closure<dyn FnMut(KeyboardEvent)>,
    widgets: Vec<serde_json::Value>,
    reader: solomon_graphics::announcements::ScreenReader,
    last_announced: String,
}
struct Animation {
    callback: Closure<dyn FnMut(f64)>,
    request: Option<i32>,
}
thread_local! {
    static LOADER:RefCell<Option<Loader>>=const{RefCell::new(None)};
    static ANIMATION:RefCell<Option<Animation>>=const{RefCell::new(None)};
    // Native announcements can synchronously query the bridge while render owns
    // its mutable borrow. Activity belongs to the lifecycle, not that borrow.
    static ACTIVE:Cell<bool>=const{Cell::new(false)};
    static PAINTED_STATE:RefCell<String>=const{RefCell::new(String::new())};
}

#[wasm_bindgen]
pub fn active() -> bool {
    ACTIVE.with(Cell::get)
}

#[wasm_bindgen]
pub fn gpu_ui_state() -> String {
    PAINTED_STATE.with(|state| {
        let state = state.borrow();
        if state.is_empty() {
            serde_json::json!({"schemaVersion":1,"surface":"hidden","ready":true,"loading":null,"error":null,"widgets":[],"draw":{"meshes":0,"triangles":0,"textures":0}}).to_string()
        } else {
            state.clone()
        }
    })
}

#[wasm_bindgen]
pub fn progress(stage: &str, current_item: &str, completed: u32, total: u32) {
    LOADER.with(|slot| {
        if let Some(loader) = slot.borrow_mut().as_mut() {
            loader.progress.stage = stage.to_owned();
            loader.progress.item = current_item.to_owned();
            loader.progress.completed = completed;
            loader.progress.total = total;
        }
    });
}

#[wasm_bindgen]
pub fn error(message: &str) {
    LOADER.with(|slot| {
        if let Some(loader) = slot.borrow_mut().as_mut() {
            loader.progress.error = Some(message.to_owned());
            if let Err(render_error) = loader.render() {
                loader.progress.error = Some(format!("{message}\n{render_error}"));
            }
        }
    });
}

#[wasm_bindgen]
pub fn finish() {
    ACTIVE.with(|active| active.set(false));
    PAINTED_STATE.with(|state| state.borrow_mut().clear());
    ANIMATION.with(|slot| {
        if let Some(animation) = slot.borrow_mut().take()
            && let Some(request) = animation.request
            && let Some(window) = web_sys::window()
        {
            let _ = window.cancel_animation_frame(request);
        }
    });
    LOADER.with(|slot| {
        slot.borrow_mut().take();
    });
}

#[wasm_bindgen]
pub async fn boot_loader() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let result = boot().await;
    if let Err(message) = &result {
        error(message);
    }
    result.map_err(JsValue::from)
}

async fn boot() -> Result<(), String> {
    let window = web_sys::window().ok_or("no loader window")?;
    let document = window.document().ok_or("no loader document")?;
    let canvas: HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(|e| format!("loader canvas: {e:?}"))?
        .dyn_into()
        .map_err(|_| "loader surface is not a canvas")?;
    for (name, value) in [
        ("data-gpu-loader", ""),
        ("aria-label", "Loading Solomon Dark"),
        (
            "style",
            "position:fixed;inset:0;width:100vw;height:100vh;display:block;z-index:2147483647;touch-action:none",
        ),
    ] {
        canvas.set_attribute(name, value).map_err(|e| format!("loader canvas placement: {e:?}"))?;
    }
    document
        .body()
        .ok_or("no loader body")?
        .append_child(&canvas)
        .map_err(|e| format!("loader attachment: {e:?}"))?;
    let renderer = Renderer::for_canvas(&canvas)?;
    let reader = solomon_graphics::announcements::ScreenReader::attach_loader(&document)?;
    let keyboard = Closure::<dyn FnMut(KeyboardEvent)>::new(|event: KeyboardEvent| {
        if !event.ctrl_key() && !event.meta_key() && !event.alt_key() {
            event.prevent_default();
            event.stop_immediate_propagation();
        }
    });
    window
        .add_event_listener_with_callback_and_bool(
            "keydown",
            keyboard.as_ref().unchecked_ref(),
            true,
        )
        .map_err(|e| format!("loader input ownership: {e:?}"))?;
    LOADER.with(|slot| {
        *slot.borrow_mut() = Some(Loader {
            window: window.clone(),
            canvas,
            renderer,
            art: Vec::new(),
            font: None,
            progress: Progress { stage: "Loading Solomon Dark".to_owned(), ..Default::default() },
            frame: 0,
            meshes: 0,
            triangles: 0,
            viewport: [0.0; 2],
            logical: [1600.0, 900.0],
            scale: 1.0,
            keyboard,
            widgets: Vec::new(),
            reader,
            last_announced: String::new(),
        })
    });
    ACTIVE.with(|active| active.set(true));
    LOADER.with(|slot| slot.borrow_mut().as_mut().expect("loader is installed").render())?;
    start_animation()?;
    let bytes =
        js_sys::Uint8Array::from(include_bytes!("../../../web/assets/stock/Fonts.png").as_slice());
    let sequence = js_sys::Array::of1(&bytes);
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("image/png");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&sequence, &options)
        .map_err(|e| format!("loader font bitmap: {e:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| format!("loader font URL: {e:?}"))?;
    let font = assets::load_image(&url).await;
    web_sys::Url::revoke_object_url(&url).map_err(|e| format!("loader font URL cleanup: {e:?}"))?;
    let font = font?;
    LOADER.with(|slot| {
        let mut slot = slot.borrow_mut();
        let loader = slot.as_mut().ok_or("loader retired before font decode")?;
        loader.font = Some(loader.renderer.upload(&font)?);
        Ok::<_, String>(())
    })?;
    let promises = js_sys::Array::new();
    for (name, _) in ART {
        let path = format!("assets/stock/{name}");
        promises.push(&wasm_bindgen_futures::future_to_promise(async move {
            assets::load_image(&path).await.map(JsValue::from).map_err(JsValue::from)
        }));
    }
    let images: js_sys::Array = JsFuture::from(js_sys::Promise::all(&promises))
        .await
        .map_err(|e| format!("loading splash artwork: {e:?}"))?
        .dyn_into()
        .map_err(|_| "splash decoding did not return images")?;
    LOADER.with(|slot| {
        let mut slot = slot.borrow_mut();
        let loader = slot.as_mut().ok_or("loader retired before splash decode")?;
        for image in images {
            let image: HtmlImageElement =
                image.dyn_into().map_err(|_| "decoded splash item is not an image")?;
            loader.art.push(loader.renderer.upload(&image)?);
        }
        loader.render()?;
        Ok::<_, String>(())
    })?;
    // The explicit render above flushes the first GPU frame. A hidden editor
    // document must continue module loading and disposal without waiting for rAF.
    Ok(())
}

fn start_animation() -> Result<(), String> {
    let callback = Closure::<dyn FnMut(f64)>::new(|_| {
        ANIMATION.with(|slot| {
            if let Some(animation) = slot.borrow_mut().as_mut() {
                animation.request = None;
            }
        });
        let rendered = LOADER.with(|slot| match slot.borrow_mut().as_mut() {
            Some(loader) => loader.render(),
            None => Ok(()),
        });
        if let Err(message) = rendered {
            error(&message);
        }
        if active()
            && let Err(message) = next_frame()
        {
            error(&message);
        }
    });
    ANIMATION.with(|slot| *slot.borrow_mut() = Some(Animation { callback, request: None }));
    next_frame()
}
fn next_frame() -> Result<(), String> {
    ANIMATION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let animation = slot.as_mut().ok_or("loader animation is retired")?;
        animation.request = Some(
            web_sys::window()
                .ok_or("no loader animation window")?
                .request_animation_frame(animation.callback.as_ref().unchecked_ref())
                .map_err(|e| format!("loader animation: {e:?}"))?,
        );
        Ok(())
    })
}

impl Loader {
    fn render(&mut self) -> Result<(), String> {
        let width = self
            .window
            .inner_width()
            .map_err(|e| format!("loader width: {e:?}"))?
            .as_f64()
            .ok_or("loader width is not a number")? as f32;
        let height = self
            .window
            .inner_height()
            .map_err(|e| format!("loader height: {e:?}"))?
            .as_f64()
            .ok_or("loader height is not a number")? as f32;
        let (width, height) =
            if width > 0.0 && height > 0.0 { (width, height) } else { (1600.0, 900.0) };
        self.viewport = [width, height];
        self.scale = (width / 1600.0).min(height / 900.0);
        self.logical = [width / self.scale, height / self.scale];
        let density = self.window.device_pixel_ratio() as f32;
        let physical = [(width * density).round() as u32, (height * density).round() as u32];
        if self.canvas.width() != physical[0] {
            self.canvas.set_width(physical[0]);
        }
        if self.canvas.height() != physical[1] {
            self.canvas.set_height(physical[1]);
        }
        self.renderer.begin_frame(physical[0] as i32, physical[1] as i32, self.logical);
        self.renderer.draw_solid(
            [0.0, 0.0, self.logical[0], self.logical[1]],
            [0.0, 0.0, 84.0 / 255.0, 1.0],
        );
        self.meshes = 1;
        self.triangles = 2;
        self.widgets.clear();
        let origin = [(self.logical[0] - 480.0) / 2.0, (self.logical[1] - 320.0) / 2.0];
        for index in 0..self.art.len() {
            let texture = &self.art[index];
            let mut bounds = ART[index].1;
            let mut source = [0.0, 0.0, texture.width, texture.height];
            if index == 3 {
                let ratio = self.progress.ratio() as f32;
                bounds[2] *= ratio;
                source[2] *= ratio;
            }
            if bounds[2] > 0.0 {
                bounds[0] += origin[0];
                bounds[1] += origin[1];
                self.renderer.draw(
                    texture,
                    Quad { source, destination: bounds, tint: [1.0; 4], blend: BlendMode::Normal },
                );
                self.meshes += 1;
                self.triangles += 2;
                let (id, label) = [
                    ("loader:logo", "Solomon Dark logo"),
                    ("loader:url", "Solomon Dark website"),
                    ("loader:frame", "Loading progress frame"),
                    ("loader:fill", "Completed loading work"),
                ][index];
                self.record(id, label, "img", bounds);
            }
        }
        if self.font.is_some() {
            let size = (width * 0.0105).clamp(11.52, 16.0) / self.scale;
            let top = self.logical[1] / 2.0 + self.logical[0] * 0.11;
            self.text(
                "loader:status",
                &format!("{}  {}%", stage_label(&self.progress.stage), self.progress.percentage()),
                top + size,
                size,
                0xffffff,
                None,
            );
            if !self.progress.item.is_empty() {
                let item = fit_text(
                    &format!("Current item: {}", self.progress.item),
                    size * 0.82,
                    (width * 0.82).min(672.0) / self.scale,
                );
                self.text(
                    "loader:current-item",
                    &item,
                    top + size * 2.5,
                    size * 0.82,
                    0xb8c9ef,
                    None,
                );
            }
            if self.progress.total > 0 {
                self.text(
                    "loader:count",
                    &format!("{} / {} items ready", self.progress.completed, self.progress.total),
                    top + size * 3.8,
                    size * 0.75,
                    0x91a8d7,
                    None,
                );
            }
            if let Some(message) = self.progress.error.clone() {
                self.text(
                    "loader:error",
                    &message,
                    self.logical[1] * 0.87,
                    size,
                    0xead8aa,
                    Some(self.logical[0] * 0.8),
                );
            }
        }
        self.renderer.flush();
        self.frame += 1;
        // Publish one submitted frame atomically. Completion callbacks may update
        // progress before the next paint; its fill geometry must stay consistent.
        PAINTED_STATE.with(|state| {
            *state.borrow_mut() = serde_json::json!({
                "schemaVersion":1,"surface":"loading","ready":false,"frame":self.frame,
                "loading":{"stage":self.progress.stage,"completed":self.progress.completed,
                    "total":self.progress.total,"currentItem":self.progress.item,"progress":self.progress.ratio()},
                "error":self.progress.error,
                "canvas":{"width":self.viewport[0],"height":self.viewport[1],
                    "pixelsPerPoint":f64::from(self.canvas.width())/f64::from(self.viewport[0])},
                "capture":{"pointer":true,"keyboard":true,"text":false},
                "draw":{"meshes":self.meshes,"triangles":self.triangles,
                    "textures":self.art.len()+usize::from(self.font.is_some())},
                "splashImagesReady":self.art.len(),"widgets":self.widgets,
            }).to_string();
        });
        let caption = self.progress.error.clone().unwrap_or_else(|| {
            format!("{} {}%", stage_label(&self.progress.stage), self.progress.percentage())
        });
        if caption != self.last_announced {
            self.reader.announce(&caption)?;
            self.last_announced = caption;
        }
        Ok(())
    }
    fn text(
        &mut self,
        id: &str,
        text: &str,
        baseline: f32,
        size: f32,
        tint: u32,
        max_width: Option<f32>,
    ) {
        let font_name = NativeUiFontName::Body;
        let mut spec = NativeUiTextSpec::new(
            font_name,
            text,
            f64::from(self.logical[0] / 2.0),
            f64::from(baseline),
        );
        spec.scale = f64::from(size) / native_ui_font(font_name).metrics[0];
        spec.tint = tint;
        spec.max_width = max_width.map(f64::from);
        let layout = layout_native_ui_text(&spec);
        let Some(texture) = self.font.as_ref() else {
            return;
        };
        let mut bounds = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
        for glyph in &layout.glyphs {
            let ink = native_ui_glyph_ink_bounds(glyph);
            let [x, y, w, h] = glyph.glyph.frame;
            let left = ink.left as f32;
            let top = ink.top as f32;
            let right = left + ink.width as f32;
            let bottom = top + ink.height as f32;
            bounds = [
                bounds[0].min(left),
                bounds[1].min(top),
                bounds[2].max(right),
                bounds[3].max(bottom),
            ];
            self.renderer.draw_quad_with_color_mode(
                texture,
                [x as f32, y as f32, w as f32, h as f32],
                [[left, top], [right, top], [right, bottom], [left, bottom]],
                [[
                    f32::from((tint >> 16) as u8) / 255.0,
                    f32::from((tint >> 8) as u8) / 255.0,
                    f32::from(tint as u8) / 255.0,
                    1.0,
                ]; 4],
                BlendMode::Normal,
                TextureColorMode::AlphaMask,
            );
            self.meshes += 1;
            self.triangles += 2;
        }
        if bounds[0].is_finite() {
            self.record(
                id,
                text,
                if id == "loader:error" { "alert" } else { "status" },
                [bounds[0], bounds[1], bounds[2] - bounds[0], bounds[3] - bounds[1]],
            );
        }
    }

    fn record(&mut self, id: &str, label: &str, role: &str, bounds: [f32; 4]) {
        let paint = bounds.map(|value| value * self.scale);
        let left = paint[0].max(0.0);
        let top = paint[1].max(0.0);
        let width = (paint[0] + paint[2]).min(self.viewport[0]) - left;
        let height = (paint[1] + paint[3]).min(self.viewport[1]) - top;
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        self.widgets.push(serde_json::json!({
            "id": id, "label": label, "role": role,
            "rect": [left,top,width,height],
            "paintRect": paint,
            "enabled": true, "focused": false,
        }));
    }
}

fn fit_text(text: &str, size: f32, width: f32) -> String {
    let scale = f64::from(size) / native_ui_font(NativeUiFontName::Body).metrics[0];
    if measure_native_ui_text(text, NativeUiFontName::Body, scale) <= f64::from(width) {
        return text.to_owned();
    }
    let mut shortened = text.to_owned();
    while !shortened.is_empty()
        && measure_native_ui_text(&format!("{shortened}..."), NativeUiFontName::Body, scale)
            > f64::from(width)
    {
        shortened.pop();
    }
    shortened + "..."
}

fn stage_label(stage: &str) -> &str {
    match stage {
        "assets" => "Loading stock artwork",
        "fonts" => "Loading fonts",
        "interface" => "Preparing the interface",
        "audio" => "Loading music and sound",
        "runtime" => "Preparing the game",
        "account" => "Loading your wizard",
        "ready" => "Ready",
        other => other,
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        let _ = self.window.remove_event_listener_with_callback_and_bool(
            "keydown",
            self.keyboard.as_ref().unchecked_ref(),
            true,
        );
        for texture in &self.art {
            self.renderer.destroy_texture(texture);
        }
        if let Some(font) = &self.font {
            self.renderer.destroy_texture(font);
        }
        self.canvas.remove();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bridge_activity_is_queryable_during_a_mutable_render_state_borrow() {
        LOADER.with(|slot| {
            let _render = slot.borrow_mut();
            assert!(!active());
            assert!(gpu_ui_state().contains("\"surface\":\"hidden\""));
        });
    }

    #[test]
    fn progress_is_actual_bounded_work_with_unknown_code_totals() {
        let mut progress = Progress::default();
        assert_eq!(progress.ratio(), 0.0);
        assert_eq!(progress.percentage(), 0);
        progress.total = 3;
        progress.completed = 1;
        assert_eq!(progress.percentage(), 33);
        progress.completed = 3;
        assert_eq!(progress.percentage(), 100);
    }
    #[test]
    fn stock_loader_geometry_keeps_the_recovered_horizontal_art() {
        assert_eq!(ART[0].1, [41.0, 13.0, 388.0, 227.0]);
        assert_eq!(ART[3].1, [144.0, 282.0, 192.0, 18.0]);
    }
}
