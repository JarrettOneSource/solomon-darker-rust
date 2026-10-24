//! Stock/remote image lifetime and egui mesh submission through the world renderer.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use egui::{ClippedPrimitive, Rect, TextureId, Vec2, vec2};
use web_sys::{HtmlImageElement, WebGl2RenderingContext};

use solomon_graphics::assets;
use solomon_graphics::{AlphaShadow, ColorFilter};
use solomon_graphics::{BlendMode, Renderer, Texture, TextureColorMode, TextureSampling, Vertex};

use super::material::{Material, Materials, TextureSource};

enum ImageState {
    Loading,
    Decoded(HtmlImageElement),
    Ready(Texture),
    Failed(String),
}

struct ImageEntry {
    id: u64,
    state: ImageState,
    size: Option<Vec2>,
    owned: bool,
}

#[derive(Default)]
struct ImageCache {
    next_id: u64,
    images: BTreeMap<String, ImageEntry>,
    paths: BTreeMap<u64, String>,
    materials: Materials,
}

#[derive(Clone, Default)]
pub(super) struct Images(Rc<RefCell<ImageCache>>);

fn path_key(path: &str) -> String {
    if path.starts_with("assets/") || path.starts_with('/') || path.contains(':') {
        path.to_owned()
    } else {
        format!("assets/stock/{path}")
    }
}

impl Images {
    pub fn begin_frame(&self) {
        self.0.borrow_mut().materials.clear();
    }

    pub fn register(&self, path: &str, texture: &Texture, gl: &WebGl2RenderingContext) {
        self.forget(path, gl);
        let path = path_key(path);
        let mut cache = self.0.borrow_mut();
        cache.next_id += 1;
        let id = cache.next_id;
        cache.paths.insert(id, path.clone());
        cache.images.insert(
            path,
            ImageEntry {
                id,
                state: ImageState::Ready(texture.clone()),
                size: Some(vec2(texture.width, texture.height)),
                owned: false,
            },
        );
    }
    pub fn borrowed_count(&self) -> usize {
        self.0.borrow().images.values().filter(|entry| !entry.owned).count()
    }
    pub fn borrowed_paths(&self) -> Vec<String> {
        self.0
            .borrow()
            .images
            .iter()
            .filter(|(_, entry)| !entry.owned)
            .map(|(path, _)| path.clone())
            .collect()
    }
    pub fn cached_texture(&self, path: &str) -> Option<Texture> {
        match &self.0.borrow().images.get(&path_key(path))?.state {
            ImageState::Ready(texture) => Some(texture.clone()),
            _ => None,
        }
    }
    pub fn static_image_count(&self) -> usize {
        include_str!("stock-images.txt").lines().filter(|line| !line.is_empty()).count()
    }
    pub fn static_images_ready(&self) -> usize {
        let cache = self.0.borrow();
        include_str!("stock-images.txt")
            .lines()
            .filter(|path| {
                cache
                    .images
                    .get(*path)
                    .is_some_and(|entry| matches!(entry.state, ImageState::Ready(_)))
            })
            .count()
    }
    pub fn request(&self, path: &str) {
        let path = path_key(path);
        let request_id = {
            let mut cache = self.0.borrow_mut();
            if cache.images.contains_key(&path) {
                return;
            }
            cache.next_id += 1;
            let id = cache.next_id;
            cache.paths.insert(id, path.clone());
            cache.images.insert(
                path.clone(),
                ImageEntry { id, state: ImageState::Loading, size: None, owned: true },
            );
            id
        };
        let images = self.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let decoded = assets::load_image(&path).await;
            let mut cache = images.0.borrow_mut();
            let Some(entry) = cache.images.get_mut(&path) else { return };
            if entry.id != request_id {
                return;
            }
            match decoded {
                Ok(image) => {
                    entry.size =
                        Some(vec2(image.natural_width() as f32, image.natural_height() as f32));
                    entry.state = ImageState::Decoded(image);
                }
                Err(error) => entry.state = ImageState::Failed(error),
            }
        });
    }

    pub async fn load_stock(&self, renderer: &Renderer) -> Result<(), String> {
        crate::runtime_ui::enter_phase("interface")?;
        // These are presentation inputs, not world/death sprite banks. The inventory is
        // tracked beside the painter so missing files fail readiness before a menu opens.
        let paths = include_str!("stock-images.txt").lines().filter(|path| !path.is_empty());
        let promises = js_sys::Array::new();
        for path in paths {
            let images = self.clone();
            let path = path.to_owned();
            promises.push(&wasm_bindgen_futures::future_to_promise(async move {
                let image = assets::load_image(&path).await.map_err(wasm_bindgen::JsValue::from)?;
                let mut cache = images.0.borrow_mut();
                cache.next_id += 1;
                let id = cache.next_id;
                let size = vec2(image.natural_width() as f32, image.natural_height() as f32);
                cache.paths.insert(id, path.clone());
                cache.images.insert(
                    path.clone(),
                    ImageEntry {
                        id,
                        state: ImageState::Decoded(image),
                        size: Some(size),
                        owned: true,
                    },
                );
                drop(cache);
                crate::runtime_ui::completed(&path).map_err(wasm_bindgen::JsValue::from)?;
                Ok(wasm_bindgen::JsValue::UNDEFINED)
            }));
        }
        wasm_bindgen_futures::JsFuture::from(js_sys::Promise::all(&promises))
            .await
            .map_err(|error| format!("GPU UI stock art readiness: {error:?}"))?;
        if let Err(error) = self.upload_decoded(renderer) {
            self.destroy(&renderer.graphics_context());
            return Err(error);
        }
        Ok(())
    }

    fn upload_decoded(&self, renderer: &Renderer) -> Result<(), String> {
        for entry in self.0.borrow_mut().images.values_mut() {
            if let ImageState::Decoded(image) = &entry.state {
                let texture = renderer.upload(image)?;
                entry.state = ImageState::Ready(texture);
            }
        }
        Ok(())
    }

    pub fn upload_ready(&self, renderer: &mut Renderer) -> Result<(), String> {
        renderer.flush();
        self.upload_decoded(renderer)
    }

    pub fn size(&self, path: &str) -> Option<Vec2> {
        self.0.borrow().images.get(&path_key(path)).and_then(|entry| entry.size)
    }

    pub fn error(&self, path: &str) -> Option<String> {
        match &self.0.borrow().images.get(&path_key(path))?.state {
            ImageState::Failed(error) => Some(error.clone()),
            _ => None,
        }
    }

    pub fn texture_id(
        &self,
        path: &str,
        blend: BlendMode,
        mask: bool,
        filter: ColorFilter,
        shadow: Option<AlphaShadow>,
    ) -> Option<TextureId> {
        self.sampled_texture_id(path, blend, mask, filter, shadow, TextureSampling::Texture)
    }

    pub fn sampled_texture_id(
        &self,
        path: &str,
        blend: BlendMode,
        mask: bool,
        filter: ColorFilter,
        shadow: Option<AlphaShadow>,
        sampling: TextureSampling,
    ) -> Option<TextureId> {
        self.request(path);
        let mut cache = self.0.borrow_mut();
        let entry = cache.images.get(&path_key(path))?;
        if !matches!(entry.state, ImageState::Ready(_)) {
            return None;
        }
        let mut material = Material::new(TextureSource::Image(entry.id), blend, mask, filter)
            .with_sampling(sampling);
        if let Some(shadow) = shadow {
            material = material.with_shadow(shadow);
        }
        Some(cache.materials.texture_id(material))
    }

    pub fn managed_texture_id(&self, id: u64, blend: BlendMode, filter: ColorFilter) -> TextureId {
        self.0.borrow_mut().materials.texture_id(Material::new(
            TextureSource::Managed(id),
            blend,
            false,
            filter,
        ))
    }

    fn material(&self, id: u64) -> Option<Material> {
        self.0.borrow().materials.get(id)
    }

    fn texture(&self, id: u64) -> Option<(Texture, bool)> {
        let cache = self.0.borrow();
        let entry = cache.images.get(cache.paths.get(&id)?)?;
        match &entry.state {
            ImageState::Ready(texture) => Some((texture.clone(), entry.owned)),
            _ => None,
        }
    }

    pub fn texture_count(&self) -> usize {
        self.0
            .borrow()
            .images
            .values()
            .filter(|entry| matches!(entry.state, ImageState::Ready(_)))
            .count()
    }

    pub fn forget(&self, path: &str, gl: &WebGl2RenderingContext) {
        let mut cache = self.0.borrow_mut();
        if let Some(entry) = cache.images.remove(&path_key(path)) {
            cache.paths.remove(&entry.id);
            if entry.owned
                && let ImageState::Ready(texture) = entry.state
            {
                Renderer::delete_ui_texture(gl, &texture);
            }
        }
    }

    pub fn destroy(&self, gl: &WebGl2RenderingContext) {
        let mut cache = self.0.borrow_mut();
        for entry in cache.images.values() {
            if entry.owned
                && let ImageState::Ready(texture) = &entry.state
            {
                Renderer::delete_ui_texture(gl, texture);
            }
        }
        cache.images.clear();
        cache.paths.clear();
        cache.materials.clear();
    }
}

/// Converts a CSS-pixel clip to a conservative, bottom-origin WebGL scissor.
pub fn clip_scissor(rect: Rect, physical: [i32; 2], logical: [f32; 2]) -> Option<[i32; 4]> {
    let scale = vec2(physical[0] as f32 / logical[0], physical[1] as f32 / logical[1]);
    let left = (rect.min.x * scale.x).floor().clamp(0.0, physical[0] as f32) as i32;
    let top = (rect.min.y * scale.y).floor().clamp(0.0, physical[1] as f32) as i32;
    let right = (rect.max.x * scale.x).ceil().clamp(0.0, physical[0] as f32) as i32;
    let bottom = (rect.max.y * scale.y).ceil().clamp(0.0, physical[1] as f32) as i32;
    (right > left && bottom > top).then_some([
        left,
        physical[1] - bottom,
        right - left,
        bottom - top,
    ])
}

mod groups;

pub(super) struct GpuPainter {
    pub gl: WebGl2RenderingContext,
    textures: BTreeMap<TextureId, Texture>,
    groups: groups::Targets,
}

impl GpuPainter {
    pub fn new(renderer: &Renderer) -> Self {
        Self {
            gl: renderer.graphics_context(),
            textures: BTreeMap::new(),
            groups: groups::Targets::default(),
        }
    }

    pub fn texture_count(&self) -> usize {
        self.textures.len() + self.groups.texture_count()
    }

    pub fn paint(
        &mut self,
        renderer: &mut Renderer,
        physical: [i32; 2],
        logical: [f32; 2],
        images: &Images,
        primitives: Vec<ClippedPrimitive>,
        mut delta: egui::TexturesDelta,
    ) -> Result<(usize, usize), String> {
        renderer.flush();
        let updates = std::mem::take(&mut delta.set);
        let frees = std::mem::take(&mut delta.free);
        for (id, updates) in updates {
            for update in updates {
                let egui::ImageData::Color(image) = update.image;
                let pixels: Vec<u8> =
                    image.pixels.iter().flat_map(|pixel| pixel.to_srgba_unmultiplied()).collect();
                if let Some(origin) = update.pos {
                    let texture =
                        self.textures.get(&id).ok_or("GPU UI patched a missing texture")?;
                    renderer.update_rgba(texture, origin, image.size, &pixels)?;
                } else {
                    let texture = renderer.upload_rgba(
                        image.width() as u32,
                        image.height() as u32,
                        &pixels,
                    )?;
                    if let Some(previous) = self.textures.insert(id, texture) {
                        renderer.destroy_texture(&previous);
                    }
                }
                let texture =
                    self.textures.get(&id).ok_or("GPU UI texture upload was not retained")?;
                renderer.set_texture_sampling(
                    texture,
                    update.options.magnification == egui::TextureFilter::Nearest,
                );
            }
        }
        let mut counts = (0, 0);
        let result = super::effects::parse(primitives).and_then(|nodes| {
            renderer.draw_screen_space(physical, logical, |renderer| {
                counts = self.draw_nodes(
                    renderer,
                    images,
                    &nodes,
                    groups::View::screen(physical, logical),
                    0,
                )?;
                Ok(())
            })
        });
        for id in frees {
            if let Some(texture) = self.textures.remove(&id) {
                renderer.destroy_texture(&texture);
            }
        }
        result.map(|()| counts)
    }
    fn draw_mesh(
        &self,
        renderer: &mut Renderer,
        images: &Images,
        mesh: &egui::Mesh,
        clip: Rect,
        view: groups::View,
    ) -> Result<(usize, usize), String> {
        let clip = clip.intersect(view.clip).translate(-view.origin);
        let Some(clip) = clip_scissor(clip, view.physical, view.logical) else { return Ok((0, 0)) };
        if mesh.indices.is_empty() {
            return Ok((0, 0));
        }
        if !mesh.is_valid() {
            return Err("GPU UI produced an invalid mesh".to_owned());
        }
        let (texture, blend, mode, sampling, filter, shadow, override_sampling) =
            match mesh.texture_id {
                TextureId::Managed(_) => (
                    self.textures
                        .get(&mesh.texture_id)
                        .cloned()
                        .ok_or("GPU UI mesh references a missing managed texture")?,
                    BlendMode::Normal,
                    TextureColorMode::Texture,
                    None,
                    ColorFilter::default(),
                    None,
                    TextureSampling::Texture,
                ),
                TextureId::User(id) => {
                    let material =
                        images.material(id).ok_or("GPU UI mesh references a retired material")?;
                    let (texture, owned) = match material.source {
                        TextureSource::Image(id) => {
                            images.texture(id).ok_or("GPU UI mesh references a retired image")?
                        }
                        TextureSource::Managed(id) => (
                            self.textures
                                .get(&TextureId::Managed(id))
                                .cloned()
                                .ok_or("GPU UI mesh references a missing managed texture")?,
                            false,
                        ),
                    };
                    let mask = material.mask;
                    (
                        texture,
                        material.blend(),
                        if mask { TextureColorMode::AlphaMask } else { TextureColorMode::Texture },
                        owned.then_some(mask),
                        material.filter(),
                        material.shadow(),
                        material.sampling,
                    )
                }
            };
        if let Some(nearest) = sampling {
            renderer.set_texture_sampling(&texture, nearest);
        }
        let vertices: Vec<Vertex> = mesh
            .vertices
            .iter()
            .map(|vertex| {
                let color = vertex.color.to_srgba_unmultiplied();
                Vertex {
                    position: [vertex.pos.x - view.origin.x, vertex.pos.y - view.origin.y],
                    texel: [vertex.uv.x * texture.width, vertex.uv.y * texture.height],
                    tint: color.map(|value| f32::from(value) / 255.0),
                }
            })
            .collect();
        let mesh_triangles: Vec<[Vertex; 3]> = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|indices| {
                [
                    vertices[indices[0] as usize],
                    vertices[indices[1] as usize],
                    vertices[indices[2] as usize],
                ]
            })
            .collect();
        renderer.draw_sampled(override_sampling, |renderer| {
            renderer.draw_clipped(clip, 1.0, |renderer| {
                let draw = |renderer: &mut Renderer| {
                    renderer.draw_triangles_with_color_mode(&texture, &mesh_triangles, blend, mode);
                };
                let filtered = |renderer: &mut Renderer| {
                    if filter == ColorFilter::default() {
                        draw(renderer);
                    } else {
                        renderer.draw_color_filtered(filter, draw);
                    }
                };
                if let Some(shadow) = shadow {
                    renderer.draw_image_shadow(shadow, filtered);
                } else {
                    filtered(renderer);
                }
            })
        });
        Ok((1, mesh_triangles.len()))
    }
}

impl Drop for GpuPainter {
    fn drop(&mut self) {
        self.groups.destroy(&self.gl);
        for texture in self.textures.values() {
            Renderer::delete_ui_texture(&self.gl, texture);
        }
    }
}
