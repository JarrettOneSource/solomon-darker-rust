//! Rust screen-space interaction and painting over the existing WebGL2 game loop.
//!
//! Stock bundle geometry and bitmap text remain the canonical native UI primitives.
//! egui owns layout, focus, text editing and tessellation; this module owns the browser
//! input/text adapters and the renderer's GPU resource lifetime. No visual DOM is created.

mod fonts;
mod input;
mod painter;
mod stock;

use std::cell::RefCell;
use std::collections::BTreeMap;

use egui::{Color32, Context, Painter, Pos2, Rect, Response, Vec2, pos2, vec2};
use serde::Serialize;
use web_sys::{Document, HtmlCanvasElement};

use solomon_graphics::{BlendMode, Renderer};

pub use input::{BrowserKey, canvas_point, key_from_browser};
pub use painter::clip_scissor;
pub use stock::{StockAtlas, message_frame, native_text, paint_text_layout};

/// UI ownership sampled by the game before it admits a world input frame.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct UiCapture {
    pub pointer: bool,
    pub keyboard: bool,
    pub text: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct WidgetBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<Rect> for WidgetBounds {
    fn from(rect: Rect) -> Self {
        Self { x: rect.min.x, y: rect.min.y, width: rect.width(), height: rect.height() }
    }
}

/// A visible real widget, read by diagnostics and Tutorial anchors only.
#[derive(Clone, Debug, Serialize)]
pub struct WidgetGeometry {
    pub id: String,
    pub label: String,
    pub role: String,
    pub bounds: WidgetBounds,
    pub enabled: bool,
    pub focused: bool,
    pub modal: bool,
    pub z: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct UiDiagnostics {
    pub viewport: [f32; 2],
    pub pixels_per_point: f32,
    pub meshes: usize,
    pub triangles: usize,
    pub textures: usize,
    pub static_images_ready: usize,
    pub static_images_total: usize,
    pub error: Option<String>,
    pub capture: UiCapture,
    pub widgets: Vec<WidgetGeometry>,
}

/// Shared painting seam. Feature controllers retain typed state and actions.
pub struct UiFrame<'a> {
    pub ctx: &'a Context,
    images: &'a painter::Images,
    widgets: &'a RefCell<BTreeMap<String, WidgetGeometry>>,
    previous_widgets: &'a BTreeMap<String, WidgetGeometry>,
    key_codes: &'a [BrowserKey],
}

impl UiFrame<'_> {
    pub fn key_codes(&self) -> &[BrowserKey] {
        self.key_codes
    }
    pub fn image(
        &self,
        painter: &Painter,
        path: &str,
        destination: Rect,
        uv: Option<Rect>,
        tint: Color32,
    ) -> bool {
        self.image_quad(
            painter,
            path,
            [
                destination.left_top(),
                destination.right_top(),
                destination.right_bottom(),
                destination.left_bottom(),
            ],
            uv.unwrap_or(Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0))),
            tint,
        )
    }

    pub fn image_size(&self, path: &str) -> Option<Vec2> {
        self.images.size(path)
    }

    pub fn image_error(&self, path: &str) -> Option<String> {
        self.images.error(path)
    }

    pub fn atlas_image(
        &self,
        painter: &Painter,
        path: &str,
        source: Rect,
        destination: Rect,
        tint: Color32,
    ) -> bool {
        let Some(size) = self.image_size(path) else {
            self.images.request(path);
            return false;
        };
        self.image(
            painter,
            path,
            destination,
            Some(Rect::from_min_max(
                (source.min.to_vec2() / size).to_pos2(),
                (source.max.to_vec2() / size).to_pos2(),
            )),
            tint,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn image_rotated(
        &self,
        painter: &Painter,
        path: &str,
        destination: Rect,
        uv: Option<Rect>,
        rotation: f32,
        pivot: Pos2,
        tint: Color32,
    ) -> bool {
        let rotation = egui::emath::Rot2::from_angle(rotation);
        let positions = [
            destination.left_top(),
            destination.right_top(),
            destination.right_bottom(),
            destination.left_bottom(),
        ]
        .map(|position| pivot + rotation * (position - pivot));
        self.image_quad(
            painter,
            path,
            positions,
            uv.unwrap_or(Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0))),
            tint,
        )
    }

    pub fn image_quad(
        &self,
        painter: &Painter,
        path: &str,
        positions: [Pos2; 4],
        uv: Rect,
        tint: Color32,
    ) -> bool {
        self.paint_quad(painter, path, positions, uv, tint, BlendMode::Normal, false)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn image_quad_blended(
        &self,
        painter: &Painter,
        path: &str,
        positions: [Pos2; 4],
        uv: Rect,
        tint: Color32,
        blend: BlendMode,
    ) -> bool {
        self.paint_quad(painter, path, positions, uv, tint, blend, false)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_quad(
        &self,
        painter: &Painter,
        path: &str,
        positions: [Pos2; 4],
        uv: Rect,
        tint: Color32,
        blend: BlendMode,
        mask: bool,
    ) -> bool {
        let Some(texture_id) = self.images.texture_id(path, blend, mask) else { return false };
        let mut mesh = egui::Mesh::with_texture(texture_id);
        let uvs = [uv.left_top(), uv.right_top(), uv.right_bottom(), uv.left_bottom()];
        mesh.vertices.extend(
            positions.into_iter().zip(uvs).map(|(pos, uv)| egui::epaint::Vertex {
                pos,
                uv,
                color: tint,
            }),
        );
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
        true
    }

    pub fn record(&self, id: &str, label: &str, role: &str, response: &Response) {
        let rect = self
            .ctx
            .layer_transform_to_global(response.layer_id)
            .map_or(response.rect, |transform| transform * response.rect);
        self.record_rect(id, label, role, rect, response.enabled());
        if let Some(widget) = self.widgets.borrow_mut().get_mut(id) {
            widget.focused = response.has_focus();
            widget.modal = self.ctx.memory(|memory| memory.top_modal_layer().is_some());
        }
    }

    pub fn record_rect(&self, id: &str, label: &str, role: &str, rect: Rect, enabled: bool) {
        let rect = rect.intersect(self.ctx.input(|input| input.content_rect()));
        if !rect.is_positive() || !rect.is_finite() {
            return;
        }
        let mut widgets = self.widgets.borrow_mut();
        let z = widgets.len();
        widgets.insert(
            id.to_owned(),
            WidgetGeometry {
                id: id.to_owned(),
                label: label.to_owned(),
                role: role.to_owned(),
                bounds: rect.into(),
                enabled,
                focused: false,
                modal: false,
                z,
                value: None,
                selected: None,
            },
        );
    }

    pub fn record_value(&self, id: &str, value: &str, selected: Option<bool>) {
        if let Some(widget) = self.widgets.borrow_mut().get_mut(id) {
            widget.value = Some(value.to_owned());
            widget.selected = selected;
        }
    }

    pub fn widget_rect(&self, id: &str) -> Option<Rect> {
        let widgets = self.widgets.borrow();
        let widget = widgets.get(id).or_else(|| self.previous_widgets.get(id))?;
        Some(Rect::from_min_size(
            pos2(widget.bounds.x, widget.bounds.y),
            vec2(widget.bounds.width, widget.bounds.height),
        ))
    }
}

pub struct GpuUi {
    ctx: Context,
    canvas: HtmlCanvasElement,
    input: input::BrowserInput,
    images: painter::Images,
    painter: painter::GpuPainter,
    _fonts: fonts::BrowserFonts,
    widgets: BTreeMap<String, WidgetGeometry>,
    diagnostics: UiDiagnostics,
}

impl GpuUi {
    pub async fn load(
        document: &Document,
        canvas: &HtmlCanvasElement,
        renderer: &Renderer,
    ) -> Result<Self, String> {
        let ctx = Context::default();
        // A controller emits actions once per game frame, including egui sizing frames.
        ctx.options_mut(|options| {
            options.max_passes = std::num::NonZeroUsize::new(1).expect("one UI pass")
        });
        let fonts = fonts::load(document, &ctx).await?;
        let images = painter::Images::default();
        images.load_stock(renderer).await?;
        let input = input::BrowserInput::attach(document, canvas)?;
        let painter = painter::GpuPainter::new(renderer);
        Ok(Self {
            ctx,
            canvas: canvas.clone(),
            input,
            images,
            painter,
            _fonts: fonts,
            widgets: BTreeMap::new(),
            diagnostics: UiDiagnostics::default(),
        })
    }

    pub fn context(&self) -> &Context {
        &self.ctx
    }

    pub fn run(
        &mut self,
        renderer: &mut Renderer,
        viewport: [f32; 2],
        time_seconds: f64,
        mut paint: impl FnMut(&mut UiFrame<'_>),
    ) -> Result<UiCapture, String> {
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return Ok(UiCapture::default());
        }
        renderer.flush();
        self.images.upload_ready(renderer)?;
        let physical = [self.canvas.width() as i32, self.canvas.height() as i32];
        let pixels_per_point = physical[0] as f32 / viewport[0];
        let raw = self.input.take(viewport, pixels_per_point, time_seconds)?;
        let key_codes = self.input.take_key_codes();
        let widgets = RefCell::new(BTreeMap::new());
        let output = self.ctx.run_pass(raw, |ctx| {
            paint(&mut UiFrame {
                ctx,
                images: &self.images,
                widgets: &widgets,
                previous_widgets: &self.widgets,
                key_codes: &key_codes,
            });
        });
        let capture = UiCapture {
            pointer: self.ctx.egui_wants_pointer_input(),
            keyboard: self.ctx.egui_wants_keyboard_input(),
            text: output.platform_output.ime.is_some(),
        };
        self.widgets = widgets.into_inner();
        self.input.set_text_rects(&self.widgets);
        self.input.apply_output(&output.platform_output, capture)?;
        if let Some(error) = fonts::take_error() {
            return Err(error);
        }
        let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        let (meshes, triangles) = self.painter.paint(
            renderer,
            physical,
            viewport,
            &self.images,
            primitives,
            output.textures_delta,
        )?;
        self.diagnostics = UiDiagnostics {
            viewport,
            pixels_per_point,
            meshes,
            triangles,
            textures: self.painter.texture_count() + self.images.texture_count(),
            static_images_ready: self.images.static_image_count(),
            static_images_total: self.images.static_image_count(),
            error: None,
            capture,
            widgets: self.widgets.values().cloned().collect(),
        };
        Ok(capture)
    }

    pub fn forget_image(&mut self, path: &str) {
        self.images.forget(path, &self.painter.gl);
    }
    pub fn take_key_codes(&mut self) -> Vec<BrowserKey> {
        self.input.take_key_codes()
    }
    pub fn diagnostics(&self) -> &UiDiagnostics {
        &self.diagnostics
    }
}

impl Drop for GpuUi {
    fn drop(&mut self) {
        self.images.destroy(&self.painter.gl);
    }
}
