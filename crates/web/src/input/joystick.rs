//! Source TouchJoystick's actual GPU circle, knob and physical input geometry.
//! Saved rotation changes its art; the canonical knob/input kernels retain screen aim.
use super::*;
use egui::{Color32, Pos2, Rect, Stroke, Vec2 as EVec2};
use solomon_sim::mobile_ui::{
    MobileUiElementId, MobileUiSize, mobile_ui_joystick_input_radius,
    mobile_ui_joystick_knob_offset,
};

pub(super) fn layer_id() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Middle, egui::Id::unique("gameplay-joysticks"))
}

#[derive(Clone, Copy)]
pub(super) struct JoystickGeometry {
    pub center: Pos2,
    pub size: f32,
    pub rotation: f32,
    pub scale: f32,
    pub display_scale: f32,
}
impl JoystickGeometry {
    pub fn bounds(self) -> Rect {
        Rect::from_center_size(self.center, EVec2::splat(self.size))
    }
    pub fn input_radius(self) -> f64 {
        let expansion = self.rotation.cos().abs() + self.rotation.sin().abs();
        mobile_ui_joystick_input_radius(
            MobileUiSize {
                width: (self.size * expansion) as f64,
                height: (self.size * expansion) as f64,
            },
            self.rotation.to_degrees() as f64,
        )
    }
}
fn geometry(state: &GameplayInputState, lane: JoystickLane) -> JoystickGeometry {
    let (width, height, percent) = state.viewport;
    let display_scale = solomon_sim::viewport::game_viewport(Vec2::new(width as f64, height as f64))
        .display_scale as f32;
    let root = display_scale * percent as f32 / 100.0;
    let id = if lane == JoystickLane::Movement {
        MobileUiElementId::LeftJoystick
    } else {
        MobileUiElementId::RightJoystick
    };
    let transform = state.mobile_layout.layout.get(id);
    let scale = root * if state.mobile_layout.customized { transform.scale as f32 } else { 1.0 };
    let center = if state.mobile_layout.customized {
        egui::pos2(
            width as f32 * transform.x as f32 / 100.0,
            height as f32 * transform.y as f32 / 100.0,
        )
    } else {
        egui::pos2(
            if lane == JoystickLane::Movement {
                (48.0 + 118.75) * root
            } else {
                width as f32 - (48.0 + 118.75) * root
            },
            height as f32 - (56.0 + 118.75) * root,
        )
    };
    JoystickGeometry {
        center,
        size: 237.5 * scale,
        scale,
        display_scale,
        rotation: if state.mobile_layout.customized {
            transform.rotation.to_radians() as f32
        } else {
            0.0
        },
    }
}

pub(super) fn paint(input: &MovementInput, frame: &UiFrame<'_>) -> Result<(), String> {
    let coarse = input
        .window
        .match_media("(hover:none) and (pointer:coarse)")
        .map_err(|_| "joystick pointer query")?
        .is_some_and(|query| query.matches());
    let (mut lanes, pointers) = {
        let mut state = input.state.borrow_mut();
        if !coarse || state.blocked {
            let pointers = [state.pointer.take(), state.primary_pointer.take()]
                .into_iter()
                .flatten()
                .collect();
            state.touch = Vec2::ZERO;
            state.touch_primary = Vec2::ZERO;
            state.joystick_geometry = [None, None];
            (Vec::new(), pointers)
        } else {
            let mut lanes = Vec::new();
            for (index, lane) in [(0, JoystickLane::Movement), (1, JoystickLane::Primary)] {
                if lane == JoystickLane::Primary && !state.primary_enabled {
                    state.joystick_geometry[index] = None;
                    continue;
                }
                let geometry = geometry(&state, lane);
                state.joystick_geometry[index] = Some(geometry);
                lanes.push((
                    lane,
                    geometry,
                    if lane == JoystickLane::Movement { state.touch } else { state.touch_primary },
                ));
            }
            (lanes, Vec::new())
        }
    };
    release_pointers(&input.canvas, pointers);
    if lanes.is_empty() {
        return Ok(());
    }
    egui::Area::new(layer_id().id)
        .order(layer_id().order)
        .fade_in(false)
        .background_interactable(false)
        .fixed_pos(Pos2::ZERO)
        .constrain(false)
        .show(frame.ctx, |ui| -> Result<(), String> {
            for (lane, geometry, movement) in lanes.drain(..) {
                let id = if lane == JoystickLane::Movement {
                    "hud:joystick:movement"
                } else {
                    "hud:joystick:primary"
                };
                let label = if lane == JoystickLane::Movement {
                    "Movement joystick"
                } else {
                    "Primary attack joystick"
                };
                let response = ui.interact(
                    geometry.bounds(),
                    egui::Id::unique(id),
                    egui::Sense::DRAG,
                );
                let p = ui.painter();
                let r = geometry.size / 2.0;
                let mut shadow = egui::epaint::RectShape::filled(
                    geometry.bounds().translate(egui::emath::Rot2::from_angle(geometry.rotation) * egui::vec2(0.0, 4.0 * geometry.scale)),
                    r,
                    Color32::from_black_alpha(112),
                );
                shadow.blur_width = 14.0 * geometry.scale;
                p.add(shadow);
                radial(
                    p,
                    geometry.center,
                    r,
                    EVec2::ZERO,
                    r * std::f32::consts::SQRT_2,
                    &[
                        (0.0, Color32::from_rgba_unmultiplied(13, 18, 24, 31)),
                        (0.34, Color32::from_rgba_unmultiplied(13, 18, 24, 31)),
                        (0.36, Color32::from_rgba_unmultiplied(9, 13, 18, 122)),
                        (1.0, Color32::from_rgba_unmultiplied(9, 13, 18, 122)),
                    ],
                );
                p.circle_stroke(
                    geometry.center,
                    r,
                    Stroke::new(
                        3.0 * geometry.scale,
                        if movement != Vec2::ZERO {
                            Color32::from_rgb(229, 204, 131)
                        } else {
                            Color32::from_rgba_unmultiplied(226, 216, 181, 122)
                        },
                    ),
                );
                let rotation = egui::emath::Rot2::from_angle(geometry.rotation);
                let offset = mobile_ui_joystick_knob_offset(
                    movement,
                    MobileUiSize { width: geometry.size as f64, height: geometry.size as f64 },
                    geometry.rotation.to_degrees() as f64,
                );
                let center = geometry.center + rotation * egui::vec2(offset.x as f32, offset.y as f32);
                // CSS's farthest-corner radius at 40% 35% of the 100px knob.
                let gradient_extent = 60.0_f32.hypot(65.0) * geometry.scale;
                let knob_radius = 50.0 * geometry.scale;
                radial(
                    p,
                    center,
                    knob_radius,
                    rotation * egui::vec2(-10.0, -15.0) * geometry.scale,
                    gradient_extent,
                    &[
                        (0.0, Color32::from_rgb(109, 113, 107)),
                        (0.68, Color32::from_rgb(37, 43, 44)),
                        (1.0, Color32::from_rgb(17, 17, 17)),
                    ],
                );
                p.circle_stroke(
                    center,
                    knob_radius,
                    Stroke::new(2.0 * geometry.scale, Color32::from_rgba_unmultiplied(232, 220, 181, 158)),
                );
                let text = if lane == JoystickLane::Movement { "MOVE" } else { "AIM / CAST" };
                // Source cancels only the native frame's display scale in its
                // font size; player UI/custom sizing still scales the label.
                let font_size = 11.0 * geometry.scale / geometry.display_scale;
                let opacity = if movement != Vec2::ZERO { 0.45 } else { 1.0 };
                let anchor = geometry.center
                    + rotation * egui::vec2(0.0, geometry.size / 2.0 - (24.0 + 3.0) * geometry.scale);
                let galley = p.layout_job(egui::text::LayoutJob::simple_format(
                    text.into(),
                    egui::TextFormat {
                        font_id: egui::FontId::new(font_size, egui::FontFamily::Name("Cinzel Bold".into())),
                        extra_letter_spacing: 0.06 * font_size,
                        line_height: Some(font_size),
                        color: Color32::from_rgb(217, 210, 188),
                        ..Default::default()
                    },
                ));
                let origin = anchor - rotation * egui::vec2(galley.size().x / 2.0, galley.size().y);
                let shape = egui::epaint::TextShape::new(origin, galley, Color32::from_rgb(217, 210, 188))
                    .with_angle(geometry.rotation);
                let label_bounds = shape.visual_bounding_rect();
                frame.paint_group(
                    p,
                    crate::gpu_ui::GroupEffect {
                        opacity,
                        shadows: vec![crate::gpu_ui::ImageShadow {
                            offset: rotation * egui::vec2(0.0, geometry.scale),
                            // CSS text-shadow's 3px blur radius is sigma 1.5px.
                            blur: 1.5 * geometry.scale,
                            color: Color32::BLACK,
                        }],
                    },
                    |_, painter| {
                        painter.add(shape);
                        Ok(())
                    },
                )?;
                let label_id = format!("{id}:label");
                frame.record_rect(&label_id, text, "text", label_bounds, false);
                frame.record_value(
                    &label_id,
                    &serde_json::json!({"fontFamily":"Cinzel Bold","fontSize":font_size,
                        "lineHeight":font_size,"letterSpacing":0.06*font_size,"opacity":opacity}).to_string(),
                    None,
                );
                frame.record_ellipse(id, label, "joystick", &response);
                frame.record_value(
                    id,
                    &serde_json::json!({"movement":[movement.x,movement.y],"rotation":geometry.rotation,"size":geometry.size}).to_string(),
                    Some(movement != Vec2::ZERO),
                );
            }
            Ok(())
        }).inner?;
    Ok(())
}
/// The two authored CSS radial gradients become ordinary coloured GPU triangles.
fn radial(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    focus: EVec2,
    extent: f32,
    stops: &[(f32, Color32)],
) {
    let sample = |point: Pos2| {
        let t = (point - center - focus).length() / extent;
        for pair in stops.windows(2) {
            if t <= pair[1].0 {
                let u = ((t - pair[0].0) / (pair[1].0 - pair[0].0)).clamp(0.0, 1.0);
                let a = pair[0].1.to_array();
                let b = pair[1].1.to_array();
                let color = std::array::from_fn::<_, 4, _>(|index| {
                    (a[index] as f32 + (b[index] as f32 - a[index] as f32) * u).round() as u8
                });
                return Color32::from_rgba_premultiplied(color[0], color[1], color[2], color[3]);
            }
        }
        stops.last().expect("authored gradient has colours").1
    };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, sample(center));
    for ring in 1..=6 {
        for index in 0..64 {
            let angle = index as f32 * std::f32::consts::TAU / 64.0;
            let point = center + egui::vec2(angle.cos(), angle.sin()) * radius * ring as f32 / 6.0;
            mesh.colored_vertex(point, sample(point));
        }
    }
    for index in 0..64u32 {
        mesh.add_triangle(0, 1 + index, 1 + (index + 1) % 64);
    }
    for ring in 1..6u32 {
        let inner = 1 + (ring - 1) * 64;
        let outer = inner + 64;
        for index in 0..64 {
            let next = (index + 1) % 64;
            mesh.add_triangle(inner + index, outer + index, outer + next);
            mesh.add_triangle(inner + index, outer + next, inner + next);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}
