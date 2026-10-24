//! Rust screen-space interaction and painting over the existing WebGL2 game loop.
//!
//! Stock bundle geometry and bitmap text remain the canonical native UI primitives.
//! egui owns layout, focus, text editing and tessellation; this module owns the browser
//! input/text adapters and the renderer's GPU resource lifetime. No visual DOM is created.

mod accessibility;
mod batches;
mod edit;
mod effects;
mod fonts;
mod geometry;
mod input;
#[cfg(test)]
mod layer_tests;
mod material;
mod painter;
mod stock;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use egui::{Color32, Context, Painter, Pos2, Rect, Response, Vec2, pos2, vec2};
use serde::Serialize;
use web_sys::{Document, HtmlCanvasElement};

use solomon_graphics::{BlendMode, ColorFilter, Renderer};

pub use edit::{
    EditorRules, FontRunOptions, NativeEditOutput, NativeFontRun, NativeFontStroke,
    NativeLabelStyle, NativeTextEdit, TextSelection, font_run, font_run_with_options, shaped_label,
    shaped_label_with_effect, text_edit_focused,
};
pub use effects::GroupEffect;
pub use geometry::{clip_polygon_ellipse, css_bezier};
pub use input::{BrowserKey, canvas_point, key_from_browser};
pub use painter::clip_scissor;
pub use solomon_graphics::TextureSampling;
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

/// Source `drop-shadow`: offsets and Gaussian sigma in the current UI coordinates.
#[derive(Clone, Copy, Debug)]
pub struct ImageShadow {
    pub offset: Vec2,
    pub blur: f32,
    pub color: Color32,
}

/// Shared painting seam. Feature controllers retain typed state and actions.
pub struct UiFrame<'a> {
    pub ctx: &'a Context,
    images: &'a painter::Images,
    widgets: &'a RefCell<BTreeMap<String, WidgetGeometry>>,
    previous_widgets: &'a BTreeMap<String, WidgetGeometry>,
    key_codes: &'a [BrowserKey],
    blend: BlendMode,
    color_filter: ColorFilter,
    announcements: &'a RefCell<Vec<accessibility::Announcement>>,
    rotation: egui::emath::Rot2,
    translation: Vec2,
    input_scope: &'a RefCell<Option<(egui::LayerId, String)>>,
    retired_images: &'a RefCell<Vec<String>>,
    focus_from_keyboard: bool,
    editor_receipts: &'a [edit::model::Receipt],
    editors: &'a RefCell<Vec<edit::EditorDescriptor>>,
    editor_errors: &'a RefCell<Vec<String>>,
    native_click_count: Option<u32>,
}

impl UiFrame<'_> {
    /// Native browser double-click count, gated by this real widget's pointer response.
    pub fn double_clicked(&self, response: &Response) -> bool {
        response.enabled()
            && response.clicked_by(egui::PointerButton::Primary)
            && self.native_click_count == Some(2)
    }
    pub(crate) fn fail(&self, error: String) {
        self.editor_errors.borrow_mut().push(error);
    }
    /// Captures only this Rust GPU geometry, then applies filters/shadows/opacity.
    pub fn paint_group(
        &self,
        painter: &Painter,
        effect: GroupEffect,
        paint: impl FnOnce(&UiFrame<'_>, &Painter) -> Result<(), String>,
    ) -> Result<(), String> {
        self.paint_group_output(painter, effect, solomon_graphics::ShadowOutput::Source, paint)
    }

    /// Source outer box-shadow: paint an opaque rounded GPU caster, never its pixels.
    pub fn paint_outer_shadow(
        &self,
        painter: &Painter,
        shadow: ImageShadow,
        caster: impl FnOnce(&Painter) -> Result<(), String>,
    ) -> Result<(), String> {
        self.paint_box_shadow(painter, shadow, solomon_graphics::ShadowOutput::Outer, caster)
    }

    /// Positive CSS spread changes only the caster; its original interior stays clear.
    pub fn paint_outer_shadow_spread(
        &self,
        painter: &Painter,
        shadow: ImageShadow,
        expanded_caster: impl FnOnce(&Painter) -> Result<(), String>,
        original_caster: impl FnOnce(&Painter) -> Result<(), String>,
    ) -> Result<(), String> {
        self.paint_group_output(
            painter,
            GroupEffect { shadows: vec![shadow], ..Default::default() },
            solomon_graphics::ShadowOutput::Outer,
            |_, caster| {
                expanded_caster(caster)?;
                caster.add(egui::Shape::Callback(egui::epaint::PaintCallback {
                    rect: caster.clip_rect(),
                    callback: std::sync::Arc::new(effects::Boundary::Knockout),
                }));
                original_caster(caster)
            },
        )
    }

    /// Source inset box-shadow: the opaque caster is the rounded padding box.
    pub fn paint_inner_shadow(
        &self,
        painter: &Painter,
        shadow: ImageShadow,
        caster: impl FnOnce(&Painter) -> Result<(), String>,
    ) -> Result<(), String> {
        self.paint_box_shadow(painter, shadow, solomon_graphics::ShadowOutput::Inner, caster)
    }

    fn paint_box_shadow(
        &self,
        painter: &Painter,
        shadow: ImageShadow,
        output: solomon_graphics::ShadowOutput,
        caster: impl FnOnce(&Painter) -> Result<(), String>,
    ) -> Result<(), String> {
        self.paint_group_output(
            painter,
            GroupEffect { shadows: vec![shadow], ..Default::default() },
            output,
            |_, painter| caster(painter),
        )
    }

    fn paint_group_output(
        &self,
        painter: &Painter,
        mut effect: GroupEffect,
        output: solomon_graphics::ShadowOutput,
        paint: impl FnOnce(&UiFrame<'_>, &Painter) -> Result<(), String>,
    ) -> Result<(), String> {
        assert!(effect.opacity.is_finite() && (0.0..=1.0).contains(&effect.opacity));
        let layer = self.ctx.layer_transform_to_global(painter.layer_id()).unwrap_or_default();
        let scale = layer.scaling.abs() * (self.rotation * Vec2::X).length();
        for shadow in &mut effect.shadows {
            assert!(shadow.blur.is_finite() && shadow.blur >= 0.0 && shadow.offset.is_finite());
            shadow.offset = layer.scaling * (self.rotation * shadow.offset);
            shadow.blur *= scale;
        }
        effect.opacity *= painter.opacity();
        let style = effects::Style { effect, filter: self.color_filter, blend: self.blend, output };
        let marker = |boundary| {
            egui::Shape::Callback(egui::epaint::PaintCallback {
                rect: painter.clip_rect(),
                callback: std::sync::Arc::new(boundary),
            })
        };
        // Egui drops shapes at opacity0. Keep data markers so reset child opacity
        // cannot release ungrouped geometry from a fully transparent parent.
        let mut boundaries = painter.clone();
        boundaries.set_opacity(1.0);
        boundaries.add(marker(effects::Boundary::Begin(style)));
        let mut group_painter = painter.clone();
        group_painter.set_clip_rect(Rect::EVERYTHING);
        group_painter.set_opacity(1.0);
        let mut frame = self.with_blend(BlendMode::Normal);
        frame.color_filter = ColorFilter::default();
        let result = paint(&frame, &group_painter);
        boundaries.add(marker(effects::Boundary::End));
        result
    }

    pub fn focus_visible(&self, response: &Response) -> bool {
        response.has_focus() && (self.focus_from_keyboard || text_edit_focused(self.ctx))
    }

    /// Retires a dynamic image after every mesh from this frame has been submitted.
    pub fn forget_image(&self, path: &str) {
        self.retired_images.borrow_mut().push(path.to_owned());
    }
    /// The highest real layer owns controller navigation, using the same ordering
    /// as painting, hit testing and modals. No scope leaves the gamepad with gameplay.
    pub fn input_scope(&self, scope: &str, layer: egui::LayerId) {
        let mut current = self.input_scope.borrow_mut();
        if current
            .as_ref()
            .is_none_or(|(previous, _)| !self.ctx.compare_layer_order(layer, *previous).is_lt())
        {
            *current = Some((layer, scope.to_owned()));
        }
    }
    /// Current Source status owner, or accepted receipt/event identity for distinct messages.
    pub fn announce(&self, owner: &str, text: &str) {
        self.announcements
            .borrow_mut()
            .push(accessibility::Announcement { owner: owner.to_owned(), text: text.to_owned() });
    }
    pub fn with_blend(&self, blend: BlendMode) -> UiFrame<'_> {
        UiFrame {
            ctx: self.ctx,
            images: self.images,
            widgets: self.widgets,
            previous_widgets: self.previous_widgets,
            key_codes: self.key_codes,
            blend,
            color_filter: self.color_filter,
            announcements: self.announcements,
            rotation: self.rotation,
            translation: self.translation,
            input_scope: self.input_scope,
            retired_images: self.retired_images,
            focus_from_keyboard: self.focus_from_keyboard,
            editor_receipts: self.editor_receipts,
            editors: self.editors,
            editor_errors: self.editor_errors,
            native_click_count: self.native_click_count,
        }
    }
    /// Sets the image RGB gain for this view; stock bitmap text inherits it.
    pub fn with_brightness(&self, brightness: f32) -> UiFrame<'_> {
        assert!(brightness.is_finite() && brightness >= 0.0);
        let mut frame = self.with_blend(self.blend);
        frame.color_filter.brightness = brightness;
        frame
    }

    /// Uses Source CSS saturation before brightness, preserving image alpha.
    pub fn with_saturation(&self, saturation: f32) -> UiFrame<'_> {
        assert!(saturation.is_finite() && saturation >= 0.0);
        let mut frame = self.with_blend(self.blend);
        frame.color_filter.saturation = saturation;
        frame
    }

    /// Applies this view's Source colour filter to direct egui text/solid paint.
    pub fn filtered_color(&self, color: Color32) -> Color32 {
        if self.color_filter == ColorFilter::default() {
            return color;
        }
        let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
        let [red, green, blue] = self
            .color_filter
            .rgb([red, green, blue].map(|channel| f32::from(channel) / 255.0))
            .map(|channel| (channel * 255.0).round() as u8);
        Color32::from_rgba_unmultiplied(red, green, blue, alpha)
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
    #[must_use]
    pub fn key_codes(&self) -> Vec<BrowserKey> {
        self.ctx.input(|state| {
            self.key_codes
                .iter()
                .filter(|native| {
                    if !native.pressed {
                        return true;
                    }
                    let Some(mapped) = input::key_from_browser(&native.key) else {
                        return true;
                    };
                    // Logical keys and physical codes describe the same native
                    // dispatch. A consumer must not leave its code replayable.
                    state.events.iter().any(|event| {
                        matches!(
                            event,
                            egui::Event::Key { key, pressed: true, modifiers, .. }
                                if *key == mapped
                                    && modifiers.alt == native.alt
                                    && modifiers.ctrl == native.ctrl
                                    && modifiers.shift == native.shift
                                    && modifiers.mac_cmd == native.meta
                        )
                    })
                })
                .cloned()
                .collect()
        })
    }
    pub fn image(
        &self,
        painter: &Painter,
        path: &str,
        destination: Rect,
        uv: Option<Rect>,
        tint: Color32,
    ) -> bool {
        self.image_sampled(painter, path, destination, uv, tint, TextureSampling::Texture)
    }

    /// Per-draw pixelated sampling leaves an admitted world texture unchanged.
    pub fn image_sampled(
        &self,
        painter: &Painter,
        path: &str,
        destination: Rect,
        uv: Option<Rect>,
        tint: Color32,
        sampling: TextureSampling,
    ) -> bool {
        let Some(texture) = self.images.sampled_texture_id(
            path,
            self.blend,
            false,
            self.color_filter,
            None,
            sampling,
        ) else {
            return false;
        };
        self.paint_texture_quad(
            painter,
            texture,
            [
                destination.left_top(),
                destination.right_top(),
                destination.right_bottom(),
                destination.left_bottom(),
            ],
            uv.unwrap_or(Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0))),
            tint,
        );
        true
    }

    pub fn image_size(&self, path: &str) -> Option<Vec2> {
        self.images.size(path)
    }

    pub fn managed_image(
        &self,
        painter: &Painter,
        texture: egui::TextureId,
        destination: Rect,
        uv: Option<Rect>,
        tint: Color32,
    ) {
        let egui::TextureId::Managed(id) = texture else {
            panic!("managed image requires an egui TextureHandle")
        };
        let texture = self.images.managed_texture_id(id, self.blend, self.color_filter);
        self.paint_texture_quad(
            painter,
            texture,
            [
                destination.left_top(),
                destination.right_top(),
                destination.right_bottom(),
                destination.left_bottom(),
            ],
            uv.unwrap_or(Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0))),
            tint,
        );
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

    /// Composites the blurred alpha and source together before applying tint alpha.
    #[allow(clippy::too_many_arguments)]
    pub fn atlas_image_shadowed(
        &self,
        painter: &Painter,
        path: &str,
        source: Rect,
        destination: Rect,
        shadow: ImageShadow,
        tint: Color32,
    ) -> bool {
        assert!(shadow.blur.is_finite() && shadow.blur >= 0.0);
        assert!(shadow.offset.is_finite());
        if !destination.is_positive() {
            return true;
        }
        let layer = self.ctx.layer_transform_to_global(painter.layer_id()).unwrap_or_default();
        let physical_sigma = shadow.blur
            * layer.scaling.abs()
            * (self.rotation * Vec2::X).length()
            * self.ctx.pixels_per_point();
        if physical_sigma > 3.0 {
            if !self.images.ready(path) {
                self.images.request(path);
                return false;
            }
            let [red, green, blue, alpha] = tint.to_srgba_unmultiplied();
            self.with_blend(BlendMode::Normal)
                .paint_group(
                    painter,
                    GroupEffect { opacity: f32::from(alpha) / 255.0, shadows: vec![shadow] },
                    |frame, painter| {
                        frame.atlas_image(
                            painter,
                            path,
                            source,
                            destination,
                            Color32::from_rgb(red, green, blue),
                        );
                        Ok(())
                    },
                )
                .expect("an image shadow capture only records GPU geometry");
            return true;
        }
        let Some(size) = self.image_size(path) else {
            self.images.request(path);
            return false;
        };
        let uv = Rect::from_min_max(
            (source.min.to_vec2() / size).to_pos2(),
            (source.max.to_vec2() / size).to_pos2(),
        );
        let (destination, uv, shadow) = geometry::shadow_geometry(uv, destination, shadow);
        let Some(texture_id) =
            self.images.texture_id(path, BlendMode::Normal, false, self.color_filter, Some(shadow))
        else {
            return false;
        };
        self.paint_texture_quad(
            painter,
            texture_id,
            [
                destination.left_top(),
                destination.right_top(),
                destination.right_bottom(),
                destination.left_bottom(),
            ],
            uv,
            tint,
        );
        true
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
        let Some(texture_id) = self.images.texture_id(path, blend, false, self.color_filter, None)
        else {
            return false;
        };
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
        let Some(texture_id) = self.images.texture_id(path, blend, mask, self.color_filter, None)
        else {
            return false;
        };
        self.paint_texture_quad(painter, texture_id, positions, uv, tint);
        true
    }

    fn paint_texture_quad(
        &self,
        painter: &Painter,
        texture_id: egui::TextureId,
        positions: [Pos2; 4],
        uv: Rect,
        tint: Color32,
    ) {
        let mut mesh = egui::Mesh::with_texture(texture_id);
        let uvs = [uv.left_top(), uv.right_top(), uv.right_bottom(), uv.left_bottom()];
        mesh.vertices.extend(positions.into_iter().zip(uvs).map(|(pos, uv)| {
            egui::epaint::Vertex { pos: self.paint_position(pos), uv, color: tint }
        }));
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
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
            let region = widget.hit_region();
            self.ctx.set_widget_hit_test(
                response.id,
                std::sync::Arc::new(move |pos| region.contains(pos)),
            );
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
            let transform =
                self.ctx.layer_transform_to_global(response.layer_id).unwrap_or_default();
            let center = transform * center;
            let size = size * transform.scaling.abs();
            widget.hit_shape = HitShape::RotatedRect {
                center: [center.x, center.y],
                size: [size.x, size.y],
                radians,
            };
            let region = widget.hit_region();
            self.ctx.set_widget_hit_test(
                response.id,
                std::sync::Arc::new(move |pos| region.contains(pos)),
            );
        }
    }

    /// Geometry of a painted control whose real gestures are owned outside egui.
    pub fn record_painted_rect(
        &self,
        painter: &Painter,
        id: &str,
        label: &str,
        role: &str,
        rect: Rect,
        enabled: bool,
    ) {
        let transform = self.ctx.layer_transform_to_global(painter.layer_id()).unwrap_or_default();
        self.record_clipped_rect(
            id,
            label,
            role,
            transform * rect,
            transform * painter.clip_rect(),
            enabled,
        );
        if let Some(widget) = self.widgets.borrow_mut().get_mut(id) {
            widget.layer_id = Some(painter.layer_id());
        }
    }

    pub fn record_rect(&self, id: &str, label: &str, role: &str, rect: Rect, enabled: bool) {
        self.record_clipped_rect(
            id,
            label,
            role,
            rect,
            self.ctx.input(|input| input.content_rect()),
            enabled,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_clipped_rect(
        &self,
        id: &str,
        label: &str,
        role: &str,
        paint_rect: Rect,
        clip_rect: Rect,
        enabled: bool,
    ) {
        let paint_bounds = paint_rect.into();
        let rect =
            paint_rect.intersect(clip_rect).intersect(self.ctx.input(|input| input.content_rect()));
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
    input_diagnostics: UiDiagnostics,
    drawn_diagnostics: UiDiagnostics,
    deferred_textures: egui::TexturesDelta,
    deferred_retired_images: Vec<String>,
    deferred_keyboard_activations: BTreeSet<String>,
    focus_from_keyboard: bool,
}

/// An input transaction whose final shapes are submitted after the world draw.
#[must_use = "finish_frame or finish_native_dispatch must retain managed texture updates and retirements"]
pub struct UiRun {
    pending: VecDeque<batches::InputPass>,
    layout_input: egui::RawInput,
    viewport: [f32; 2],
    physical: [i32; 2],
    pixels_per_point: f32,
    output: Option<egui::FullOutput>,
    textures: egui::TexturesDelta,
    retired_images: Vec<String>,
    keyboard_activations: BTreeSet<String>,
    failure: Option<String>,
    capture: UiCapture,
}

impl GpuUi {
    pub(crate) fn startup_resources() -> Vec<crate::runtime_ui::StartupResource> {
        let mut resources = fonts::startup_resources();
        resources.extend(
            include_str!("gpu_ui/stock-images.txt")
                .lines()
                .filter(|path| !path.is_empty())
                .map(|path| crate::runtime_ui::StartupResource::new("interface", path)),
        );
        resources
    }

    pub async fn load(
        document: &Document,
        canvas: &HtmlCanvasElement,
        renderer: &Renderer,
    ) -> Result<Self, String> {
        let ctx = Context::default();
        // A controller emits actions once per game frame, including egui sizing frames.
        ctx.options_mut(|options| {
            options.max_passes = std::num::NonZeroUsize::new(1).expect("one UI pass");
            options.input_options.raw_point_scroll = true;
        });
        let fonts = fonts::load(document, &ctx).await?;
        let images = painter::Images::default();
        images.load_stock(renderer).await?;
        let input = match input::BrowserInput::attach(document, canvas, &ctx) {
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
            input_diagnostics: UiDiagnostics::default(),
            drawn_diagnostics: UiDiagnostics::default(),
            deferred_textures: egui::TexturesDelta::default(),
            deferred_retired_images: Vec::new(),
            deferred_keyboard_activations: BTreeSet::new(),
            focus_from_keyboard: true,
        })
    }

    pub fn context(&self) -> &Context {
        &self.ctx
    }

    pub fn set_native_dispatch_handler(
        &mut self,
        handler: impl Fn() -> Result<(), String> + 'static,
    ) {
        self.input.set_native_dispatch_handler(handler);
    }

    /// Commit prior native UI events before the next native editing/clipboard
    /// dispatch. This performs no controller poll, simulation or GPU submission.
    pub fn begin_native_dispatch(&mut self, time_seconds: f64) -> Result<Option<UiRun>, String> {
        let viewport = self.input_diagnostics.viewport;
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return Ok(None);
        }
        let pixels_per_point = self.input_diagnostics.pixels_per_point;
        let Some(pending) = self.input.take_native(viewport, pixels_per_point, time_seconds)?
        else {
            return Ok(None);
        };
        Ok(Some(self.input_run(viewport, pixels_per_point, pending)))
    }

    /// A native-only transaction owns the same managed texture lifecycle as a
    /// drawn frame. Keep its deltas and real activations until the next submit.
    pub fn finish_native_dispatch(
        &mut self,
        mut run: UiRun,
        result: Result<(), String>,
    ) -> Result<(), String> {
        self.deferred_textures.append(run.textures);
        self.deferred_retired_images.append(&mut run.retired_images);
        self.deferred_keyboard_activations = run.keyboard_activations;
        run.failure.map_or(result, Err).inspect_err(|error| {
            self.input_diagnostics.error = Some(error.clone());
            self.drawn_diagnostics.error = Some(error.clone());
        })
    }

    pub fn run(
        &mut self,
        renderer: &mut Renderer,
        viewport: [f32; 2],
        time_seconds: f64,
        mut paint: impl FnMut(&mut UiFrame<'_>) -> Result<(), String>,
    ) -> Result<UiCapture, String> {
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return Ok(UiCapture::default());
        }
        let mut run = self.begin_frame(renderer, viewport, time_seconds)?;
        let result = (|| {
            while self.paint_next(&mut run, &mut paint)? {}
            Ok(())
        })();
        self.finish_frame(renderer, run, result)
    }

    pub fn begin_frame(
        &mut self,
        renderer: &mut Renderer,
        viewport: [f32; 2],
        time_seconds: f64,
    ) -> Result<UiRun, String> {
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return Err("GPU UI needs a drawable viewport".to_owned());
        }
        renderer.flush();
        self.images.upload_ready(renderer)?;
        self.images.begin_frame();
        let physical = [self.canvas.width() as i32, self.canvas.height() as i32];
        let pixels_per_point = physical[0] as f32 / viewport[0];
        let pending = self.input.take(viewport, pixels_per_point, time_seconds)?;
        self.input_diagnostics.frame += 1;
        Ok(self.input_run(viewport, pixels_per_point, pending))
    }

    fn input_run(
        &mut self,
        viewport: [f32; 2],
        pixels_per_point: f32,
        pending: Vec<batches::InputPass>,
    ) -> UiRun {
        let mut layout_input =
            pending.first().expect("each frame has an input/layout pass").raw.clone();
        layout_input.events.clear();
        UiRun {
            pending: pending.into(),
            layout_input,
            viewport,
            physical: [self.canvas.width() as i32, self.canvas.height() as i32],
            pixels_per_point,
            output: None,
            textures: std::mem::take(&mut self.deferred_textures),
            retired_images: std::mem::take(&mut self.deferred_retired_images),
            keyboard_activations: std::mem::take(&mut self.deferred_keyboard_activations),
            failure: None,
            capture: UiCapture::default(),
        }
    }

    /// Returns after one input/layout commit, releasing all controller borrows.
    pub fn paint_next(
        &mut self,
        run: &mut UiRun,
        mut paint: impl FnMut(&mut UiFrame<'_>) -> Result<(), String>,
    ) -> Result<bool, String> {
        let Some(pass) = run.pending.pop_front() else { return Ok(false) };
        self.paint_pass(run, pass, &mut paint)?;
        Ok(true)
    }

    /// Refreshes models after simulation/typed actions without replaying input.
    pub fn paint_layout(
        &mut self,
        run: &mut UiRun,
        mut paint: impl FnMut(&mut UiFrame<'_>) -> Result<(), String>,
    ) -> Result<(), String> {
        let raw = run.layout_input.clone();
        self.paint_pass(
            run,
            batches::InputPass { raw, codes: Vec::new(), edits: Vec::new(), click_count: None },
            &mut paint,
        )
    }

    fn paint_pass(
        &mut self,
        run: &mut UiRun,
        pass: batches::InputPass,
        paint: &mut impl FnMut(&mut UiFrame<'_>) -> Result<(), String>,
    ) -> Result<(), String> {
        if let Some(error) = &run.failure {
            return Err(error.clone());
        }
        let result = self.prepare_pass(run, pass, paint);
        if let Err(error) = &result {
            run.failure = Some(error.clone());
        }
        result
    }

    fn prepare_pass(
        &mut self,
        run: &mut UiRun,
        pass: batches::InputPass,
        paint: &mut impl FnMut(&mut UiFrame<'_>) -> Result<(), String>,
    ) -> Result<(), String> {
        self.focus_from_keyboard = focus_modality(self.focus_from_keyboard, &pass.raw.events);
        focus_pointer_text(&self.ctx, &self.widgets, &pass.raw.events);
        let widgets = RefCell::new(BTreeMap::new());
        let announcements = RefCell::new(Vec::new());
        let input_scope = RefCell::new(None);
        let retired_images = RefCell::new(Vec::new());
        let editors = RefCell::new(Vec::new());
        let editor_errors = RefCell::new(Vec::new());
        let mut result = Ok(());
        edit::begin_native_focus(&self.ctx, &pass.codes);
        let mut output = self.ctx.run_pass(pass.raw, |ctx| {
            result = paint(&mut UiFrame {
                ctx,
                images: &self.images,
                widgets: &widgets,
                previous_widgets: &self.widgets,
                key_codes: &pass.codes,
                blend: BlendMode::Normal,
                color_filter: ColorFilter::default(),
                announcements: &announcements,
                rotation: egui::emath::Rot2::IDENTITY,
                translation: Vec2::ZERO,
                input_scope: &input_scope,
                retired_images: &retired_images,
                focus_from_keyboard: self.focus_from_keyboard,
                editor_receipts: &pass.edits,
                editors: &editors,
                editor_errors: &editor_errors,
                native_click_count: pass.click_count,
            });
        });
        edit::finish_native_focus(&self.ctx, &pass.codes);
        run.textures.append(std::mem::take(&mut output.textures_delta));
        run.retired_images.extend(retired_images.into_inner());
        result?;
        if let Some(error) = editor_errors.into_inner().into_iter().next() {
            return Err(error);
        }
        let editors = editors.into_inner();
        edit::set_focus(&self.ctx, &editors);
        self.input.set_editors(editors, pass.edits.last().map(|receipt| receipt.sequence))?;
        self.widgets = widgets.into_inner();
        rank_widgets(&self.ctx, &mut self.widgets);
        if let Some(widget) = self.widgets.values().find(|widget| widget.duplicate) {
            let error = format!("GPU UI widget id {} was painted more than once", widget.id);
            return Err(error);
        }
        run.keyboard_activations.retain(|id| self.widgets.contains_key(id));
        for widget in self.widgets.values_mut() {
            if widget.keyboard_activated {
                run.keyboard_activations.insert(widget.id.clone());
            }
            widget.keyboard_activated |= run.keyboard_activations.contains(&widget.id);
        }
        self.input.set_widgets(&self.widgets);
        let input_scope = input_scope.into_inner().map(|(_, scope)| scope);
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
            keyboard: self.ctx.egui_wants_keyboard_input() || text_edit_focused(&self.ctx),
            text: text_edit_focused(&self.ctx),
        };
        self.input.apply_output(&output.platform_output, capture)?;
        self.accessibility.update(
            &output.platform_output,
            &announcements.into_inner(),
            &self.widgets,
        )?;
        if let Some(error) = fonts::take_error() {
            return Err(error);
        }
        self.input_diagnostics.viewport = run.viewport;
        self.input_diagnostics.pixels_per_point = run.pixels_per_point;
        self.input_diagnostics.capture = capture;
        self.input_diagnostics.input_scope = input_scope;
        self.input_diagnostics.widgets = self.widgets.values().cloned().collect();
        run.capture = capture;
        run.output = Some(output);
        Ok(())
    }

    /// Always finish, passing controller/action failures here so texture updates
    /// and deferred image retirement complete even when remaining input is skipped.
    pub fn finish_frame(
        &mut self,
        renderer: &mut Renderer,
        mut run: UiRun,
        result: Result<(), String>,
    ) -> Result<UiCapture, String> {
        let mut failure = run.failure.take().or_else(|| result.err());
        let primitives = if failure.is_none() {
            if let Some(output) = run.output.take() {
                self.ctx.tessellate(output.shapes, output.pixels_per_point)
            } else {
                failure = Some("GPU UI frame was not prepared".to_owned());
                Vec::new()
            }
        } else {
            Vec::new()
        };
        let painted = self.painter.paint(
            renderer,
            run.physical,
            run.viewport,
            &self.images,
            primitives,
            run.textures,
        );
        for path in run.retired_images {
            self.images.forget(&path, &self.painter.gl);
        }
        if let Some(error) = failure {
            let error = if let Err(cleanup_error) = painted {
                format!("{error}; GPU UI texture cleanup: {cleanup_error}")
            } else {
                error
            };
            self.input_diagnostics.error = Some(error.clone());
            self.drawn_diagnostics.error = Some(error.clone());
            return Err(error);
        }
        let (meshes, triangles) = painted.inspect_err(|error| {
            self.input_diagnostics.error = Some(error.clone());
            self.drawn_diagnostics.error = Some(error.clone());
        })?;
        self.input_diagnostics.meshes = meshes;
        self.input_diagnostics.triangles = triangles;
        self.input_diagnostics.textures =
            self.painter.texture_count() + self.images.texture_count();
        self.input_diagnostics.borrowed_images = self.images.borrowed_count();
        self.input_diagnostics.static_images_ready = self.images.static_images_ready();
        self.input_diagnostics.static_images_total = self.images.static_image_count();
        self.input_diagnostics.error = None;
        self.drawn_diagnostics = self.input_diagnostics.clone();
        Ok(run.capture)
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
    pub fn diagnostics(&self) -> &UiDiagnostics {
        &self.drawn_diagnostics
    }
    /// Actual latest UI ownership for input admission, including native-only
    /// commits. Public diagnostics remain the last completed GPU submission.
    pub fn input_diagnostics(&self) -> &UiDiagnostics {
        &self.input_diagnostics
    }
}

fn rank_widgets(ctx: &Context, widgets: &mut BTreeMap<String, WidgetGeometry>) {
    let mut ordered: Vec<_> = widgets.values_mut().collect();
    ordered.sort_by(|a, b| {
        ctx.compare_layer_order(
            a.layer_id.unwrap_or_else(egui::LayerId::background),
            b.layer_id.unwrap_or_else(egui::LayerId::background),
        )
        .then_with(|| a.z.cmp(&b.z))
    });
    for (z, widget) in ordered.into_iter().enumerate() {
        widget.z = z;
    }
}

fn focus_modality(mut keyboard: bool, events: &[egui::Event]) -> bool {
    for event in events {
        match event {
            egui::Event::PointerButton { pressed: true, .. }
            | egui::Event::Touch { phase: egui::TouchPhase::Start, .. } => keyboard = false,
            egui::Event::Key { pressed: true, .. } | egui::Event::Text(_) => keyboard = true,
            _ => {}
        }
    }
    keyboard
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
        let Some(id) = ctx.pointer_widget_at(*pos) else { continue };
        if widgets.values().any(|widget| {
            widget.egui_id == Some(id)
                && widget.enabled
                && widget.is_interactive()
                && matches!(widget.role.as_str(), "textbox" | "password" | "input" | "text-entry")
                && widget.contains(*pos)
        }) {
            // The canonical native query includes same-layer occlusion and modals.
            // Focus only that actual editor before its first queued text receipt.
            ctx.memory_mut(|memory| memory.request_focus(id));
        }
    }
}

impl Drop for GpuUi {
    fn drop(&mut self) {
        self.images.destroy(&self.painter.gl);
    }
}
