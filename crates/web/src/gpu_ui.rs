//! Rust screen-space interaction and painting over the existing WebGL2 game loop.
//!
//! Stock bundle geometry and bitmap text remain the canonical native UI primitives.
//! egui owns layout, focus, text editing and tessellation; this module owns the browser
//! input/text adapters and the renderer's GPU resource lifetime. No visual DOM is created.

mod accessibility;
mod fonts;
mod geometry;
mod input;
mod painter;
mod stock;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::BTreeMap;

use egui::{Color32, Context, Painter, Pos2, Rect, Response, Vec2, pos2, vec2};
use serde::Serialize;
use web_sys::{Document, HtmlCanvasElement};

use solomon_graphics::{BlendMode, Renderer};

pub use geometry::clip_polygon_ellipse;
pub use input::{BrowserKey, canvas_point, key_from_browser};
pub use painter::clip_scissor;
pub use stock::{StockAtlas, message_frame, message_frame_scaled, native_text, paint_text_layout};

pub fn cooldown_sector_points(remaining: f64, capacity: f64) -> Vec<Pos2> {
    solomon_sim::hud::quickbar::native_cooldown_sector_points(remaining, capacity)
        .into_iter()
        .map(|point| pos2(point.x as f32, point.y as f32))
        .collect()
}

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

impl WidgetBounds {
    pub fn rect(self) -> Rect {
        Rect::from_min_size(pos2(self.x, self.y), vec2(self.width, self.height))
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HitShape {
    #[default]
    Rect,
    Ellipse,
    RotatedRect {
        center: [f32; 2],
        size: [f32; 2],
        radians: f32,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct HitRegion {
    pub bounds: WidgetBounds,
    pub paint_bounds: WidgetBounds,
    pub shape: HitShape,
}
impl HitRegion {
    pub fn contains(self, point: Pos2) -> bool {
        if !self.bounds.rect().contains(point) {
            return false;
        }
        match self.shape {
            HitShape::Rect => true,
            HitShape::Ellipse => {
                let rect = self.paint_bounds.rect();
                rect.is_positive()
                    && ((point - rect.center()) / (rect.size() / 2.0)).length_sq() <= 1.0
            }
            HitShape::RotatedRect { center, size, radians } => {
                let point =
                    egui::emath::Rot2::from_angle(-radians) * (point - pos2(center[0], center[1]));
                point.x.abs() <= size[0] / 2.0 && point.y.abs() <= size[1] / 2.0
            }
        }
    }
}

/// A visible real widget, read by diagnostics and Tutorial anchors only.
#[derive(Clone, Debug, Serialize)]
pub struct WidgetGeometry {
    pub id: String,
    pub label: String,
    pub role: String,
    pub bounds: WidgetBounds,
    pub paint_bounds: WidgetBounds,
    pub hit_shape: HitShape,
    pub enabled: bool,
    pub focused: bool,
    pub modal: bool,
    pub z: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    #[serde(skip)]
    egui_id: Option<egui::Id>,
    #[serde(skip)]
    layer_id: Option<egui::LayerId>,
    #[serde(skip)]
    duplicate: bool,
    #[serde(skip)]
    activated: bool,
    #[serde(skip)]
    interactive: bool,
    #[serde(skip)]
    keyboard_activated: bool,
}

impl WidgetGeometry {
    pub fn hit_region(&self) -> HitRegion {
        HitRegion { bounds: self.bounds, paint_bounds: self.paint_bounds, shape: self.hit_shape }
    }
    pub fn contains(&self, point: Pos2) -> bool {
        self.hit_region().contains(point)
    }
    pub fn is_interactive(&self) -> bool {
        self.interactive
    }
    pub fn activated_by_keyboard(&self) -> bool {
        self.keyboard_activated
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct UiDiagnostics {
    pub frame: u64,
    pub viewport: [f32; 2],
    pub pixels_per_point: f32,
    pub meshes: usize,
    pub triangles: usize,
    pub textures: usize,
    pub borrowed_images: usize,
    pub static_images_ready: usize,
    pub static_images_total: usize,
    pub error: Option<String>,
    pub capture: UiCapture,
    pub input_scope: Option<String>,
    pub widgets: Vec<WidgetGeometry>,
}

/// Shared painting seam. Feature controllers retain typed state and actions.
pub struct UiFrame<'a> {
    pub ctx: &'a Context,
    images: &'a painter::Images,
    widgets: &'a RefCell<BTreeMap<String, WidgetGeometry>>,
    previous_widgets: &'a BTreeMap<String, WidgetGeometry>,
    key_codes: &'a [BrowserKey],
    blend: BlendMode,
    announcements: &'a RefCell<Vec<String>>,
    rotation: egui::emath::Rot2,
    translation: Vec2,
    input_scope: &'a RefCell<Option<String>>,
    retired_images: &'a RefCell<Vec<String>>,
}

impl UiFrame<'_> {
    /// Retires a dynamic image after every mesh from this frame has been submitted.
    pub fn forget_image(&self, path: &str) {
        self.retired_images.borrow_mut().push(path.to_owned());
    }
    /// The last foreground owner painted this frame owns controller navigation.
    /// Frames without an owner leave the gamepad with gameplay.
    pub fn input_scope(&self, scope: &str) {
        *self.input_scope.borrow_mut() = Some(scope.to_owned());
    }
    pub fn announce(&self, text: &str) {
        self.announcements.borrow_mut().push(text.to_owned());
    }
    pub fn with_blend(&self, blend: BlendMode) -> UiFrame<'_> {
        UiFrame {
            ctx: self.ctx,
            images: self.images,
            widgets: self.widgets,
            previous_widgets: self.previous_widgets,
            key_codes: self.key_codes,
            blend,
            announcements: self.announcements,
            rotation: self.rotation,
            translation: self.translation,
            input_scope: self.input_scope,
            retired_images: self.retired_images,
        }
    }
    pub fn with_rotation(&self, pivot: Pos2, radians: f32) -> UiFrame<'_> {
        let local = egui::emath::Rot2::from_angle(radians);
        let mut frame = self.with_blend(self.blend);
        frame.rotation = self.rotation * local;
        frame.translation =
            self.translation + self.rotation * (pivot.to_vec2() - local * pivot.to_vec2());
        frame
    }

    fn with_scale(&self, origin: Pos2, scale: f32) -> UiFrame<'_> {
        let mut frame = self.with_blend(self.blend);
        frame.rotation = scale * self.rotation;
        frame.translation = self.translation + self.rotation * (origin.to_vec2() * (1.0 - scale));
        frame
    }

    fn paint_position(&self, point: Pos2) -> Pos2 {
        (self.rotation * point.to_vec2() + self.translation).to_pos2()
    }
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
        self.paint_quad(painter, path, positions, uv, tint, self.blend, false)
    }

    pub fn image_triangle(
        &self,
        painter: &Painter,
        path: &str,
        positions: [Pos2; 3],
        uv: [Pos2; 3],
        tint: Color32,
        blend: BlendMode,
    ) -> bool {
        let Some(texture_id) = self.images.texture_id(path, blend, false) else { return false };
        let mut mesh = egui::Mesh::with_texture(texture_id);
        mesh.vertices.extend(positions.into_iter().zip(uv).map(|(pos, uv)| egui::epaint::Vertex {
            pos: self.paint_position(pos),
            uv,
            color: tint,
        }));
        mesh.indices.extend_from_slice(&[0, 1, 2]);
        painter.add(egui::Shape::mesh(mesh));
        true
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

    pub fn image_quad_masked(
        &self,
        painter: &Painter,
        path: &str,
        positions: [Pos2; 4],
        uv: Rect,
        tint: Color32,
        blend: BlendMode,
    ) -> bool {
        self.paint_quad(painter, path, positions, uv, tint, blend, true)
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
        mesh.vertices.extend(positions.into_iter().zip(uvs).map(|(pos, uv)| {
            egui::epaint::Vertex { pos: self.paint_position(pos), uv, color: tint }
        }));
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
        true
    }

    pub fn record(&self, id: &str, label: &str, role: &str, response: &Response) {
        if !response.visible() {
            return;
        }
        let rect = self
            .ctx
            .layer_transform_to_global(response.layer_id)
            .map_or(response.interact_rect, |transform| transform * response.interact_rect);
        self.record_rect(id, label, role, rect, response.enabled());
        if let Some(widget) = self.widgets.borrow_mut().get_mut(id) {
            widget.paint_bounds = self
                .ctx
                .layer_transform_to_global(response.layer_id)
                .map_or(response.rect, |transform| transform * response.rect)
                .into();
            widget.focused = response.has_focus();
            widget.modal = self.ctx.memory(|memory| memory.top_modal_layer().is_some());
            widget.egui_id = Some(response.id);
            widget.layer_id = Some(response.layer_id);
            widget.activated = response.clicked();
            widget.interactive = response.sense.interactive();
            widget.keyboard_activated = response.clicked()
                && !self
                    .ctx
                    .input(|input| input.pointer.button_released(egui::PointerButton::Primary));
        }
    }

    pub fn record_ellipse(&self, id: &str, label: &str, role: &str, response: &Response) {
        self.record(id, label, role, response);
        if let Some(widget) = self.widgets.borrow_mut().get_mut(id) {
            widget.hit_shape = HitShape::Ellipse;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_rotated_rect(
        &self,
        id: &str,
        label: &str,
        role: &str,
        response: &Response,
        center: Pos2,
        size: Vec2,
        radians: f32,
    ) {
        self.record(id, label, role, response);
        if let Some(widget) = self.widgets.borrow_mut().get_mut(id) {
            widget.hit_shape = HitShape::RotatedRect {
                center: [center.x, center.y],
                size: [size.x, size.y],
                radians,
            };
        }
    }

    pub fn record_rect(&self, id: &str, label: &str, role: &str, rect: Rect, enabled: bool) {
        let paint_bounds = rect.into();
        let rect = rect.intersect(self.ctx.input(|input| input.content_rect()));
        if !rect.is_positive() || !rect.is_finite() {
            return;
        }
        let mut widgets = self.widgets.borrow_mut();
        if let Some(existing) = widgets.get_mut(id) {
            existing.duplicate = true;
            return;
        }
        let z = widgets.len();
        widgets.insert(
            id.to_owned(),
            WidgetGeometry {
                id: id.to_owned(),
                label: label.to_owned(),
                role: role.to_owned(),
                bounds: rect.into(),
                paint_bounds,
                hit_shape: HitShape::Rect,
                enabled,
                focused: false,
                modal: false,
                z,
                value: None,
                selected: None,
                egui_id: None,
                layer_id: None,
                duplicate: false,
                activated: false,
                interactive: matches!(
                    role,
                    "button"
                        | "textbox"
                        | "password"
                        | "input"
                        | "text-entry"
                        | "slider"
                        | "checkbox"
                        | "switch"
                        | "combobox"
                        | "radio"
                        | "tab"
                        | "menuitem"
                ),
                keyboard_activated: false,
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
    accessibility: accessibility::Accessibility,
    widgets: BTreeMap<String, WidgetGeometry>,
    diagnostics: UiDiagnostics,
}

impl GpuUi {
    pub(crate) fn startup_asset_count() -> usize {
        fonts::asset_count()
            + include_str!("gpu_ui/stock-images.txt")
                .lines()
                .filter(|path| !path.is_empty())
                .count()
    }

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
        let input = match input::BrowserInput::attach(document, canvas) {
            Ok(input) => input,
            Err(error) => {
                images.destroy(&renderer.graphics_context());
                return Err(error);
            }
        };
        let painter = painter::GpuPainter::new(renderer);
        let accessibility = match accessibility::Accessibility::attach(document) {
            Ok(accessibility) => accessibility,
            Err(error) => {
                images.destroy(&renderer.graphics_context());
                return Err(error);
            }
        };
        Ok(Self {
            ctx,
            canvas: canvas.clone(),
            input,
            images,
            painter,
            _fonts: fonts,
            accessibility,
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
        focus_pointer_text(&self.ctx, &self.widgets, &raw.events);
        let key_codes = self.input.take_key_codes();
        let widgets = RefCell::new(BTreeMap::new());
        let announcements = RefCell::new(Vec::new());
        let input_scope = RefCell::new(None);
        let retired_images = RefCell::new(Vec::new());
        let mut output = self.ctx.run_pass(raw, |ctx| {
            paint(&mut UiFrame {
                ctx,
                images: &self.images,
                widgets: &widgets,
                previous_widgets: &self.widgets,
                key_codes: &key_codes,
                blend: BlendMode::Normal,
                announcements: &announcements,
                rotation: egui::emath::Rot2::IDENTITY,
                translation: Vec2::ZERO,
                input_scope: &input_scope,
                retired_images: &retired_images,
            });
        });
        self.widgets = widgets.into_inner();
        if let Some(widget) = self.widgets.values().find(|widget| widget.duplicate) {
            let error = format!("GPU UI widget id {} was painted more than once", widget.id);
            self.diagnostics.error = Some(error.clone());
            output.textures_delta.clear();
            return Err(error);
        }
        self.input.set_widgets(&self.widgets);
        let input_scope = input_scope.into_inner();
        self.input.set_scope(input_scope.as_deref());
        let pointer = self.ctx.input(|input| input.pointer.interact_pos());
        let capture = UiCapture {
            pointer: input_scope.is_some()
                || self.input.owns_pointer()
                || pointer.is_some_and(|point| {
                    self.widgets
                        .values()
                        .any(|widget| widget.is_interactive() && widget.contains(point))
                }),
            keyboard: self.ctx.egui_wants_keyboard_input(),
            text: output.platform_output.ime.is_some(),
        };
        if let Err(error) = self.input.apply_output(&output.platform_output, capture) {
            output.textures_delta.clear();
            return Err(error);
        }
        if let Err(error) = self.accessibility.update(
            &output.platform_output,
            &announcements.into_inner(),
            &self.widgets,
        ) {
            output.textures_delta.clear();
            return Err(error);
        }
        if let Some(error) = fonts::take_error() {
            output.textures_delta.clear();
            return Err(error);
        }
        let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        let painted = self.painter.paint(
            renderer,
            physical,
            viewport,
            &self.images,
            primitives,
            output.textures_delta,
        );
        for path in retired_images.into_inner() {
            self.images.forget(&path, &self.painter.gl);
        }
        let (meshes, triangles) = painted?;
        self.diagnostics = UiDiagnostics {
            frame: self.diagnostics.frame + 1,
            viewport,
            pixels_per_point,
            meshes,
            triangles,
            textures: self.painter.texture_count() + self.images.texture_count(),
            borrowed_images: self.images.borrowed_count(),
            static_images_ready: self.images.static_images_ready(),
            static_images_total: self.images.static_image_count(),
            error: None,
            capture,
            input_scope,
            widgets: self.widgets.values().cloned().collect(),
        };
        Ok(capture)
    }

    pub fn forget_image(&mut self, path: &str) {
        self.images.forget(path, &self.painter.gl);
    }
    pub fn image_texture(&self, path: &str) -> Option<solomon_graphics::Texture> {
        self.images.cached_texture(path)
    }
    /// Shares a decoded world-owned texture; cache retirement never deletes it.
    pub fn register_image(&mut self, path: &str, texture: &solomon_graphics::Texture) {
        self.images.register(path, texture, &self.painter.gl);
    }
    pub fn borrowed_image_paths(&self) -> Vec<String> {
        self.images.borrowed_paths()
    }
    pub fn take_key_codes(&mut self) -> Vec<BrowserKey> {
        self.input.take_key_codes()
    }
    pub fn diagnostics(&self) -> &UiDiagnostics {
        &self.diagnostics
    }
}

fn focus_pointer_text(
    ctx: &Context,
    widgets: &BTreeMap<String, WidgetGeometry>,
    events: &[egui::Event],
) {
    for event in events {
        let egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            ..
        } = event
        else {
            continue;
        };
        let layer = ctx.layer_id_at(*pos);
        for widget in widgets.values().rev() {
            if !widget.enabled
                || !matches!(widget.role.as_str(), "textbox" | "password" | "input" | "text-entry")
                || widget.layer_id != layer
            {
                continue;
            }
            let rect = Rect::from_min_size(
                pos2(widget.bounds.x, widget.bounds.y),
                vec2(widget.bounds.width, widget.bounds.height),
            );
            if rect.contains(*pos)
                && let Some(id) = widget.egui_id
            {
                // egui0.37 lays out TextEdit content before its pointer response.
                // Focus the actual hit target before its first queued text event.
                ctx.memory_mut(|memory| memory.request_focus(id));
                break;
            }
        }
    }
}

impl Drop for GpuUi {
    fn drop(&mut self) {
        self.images.destroy(&self.painter.gl);
    }
}
