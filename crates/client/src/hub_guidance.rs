//! College onboarding from Courtyard::Present (0x0051EB60): the world callout
//! follows the foreground, while UI 88 arrows project into the screen-space pass.

use solomon_graphics::{BlendMode, Quad, Renderer, Texture};
use solomon_sim::Vec2;
use solomon_sim::hub::interaction::{
    HUB_WALK_TO_TALK_TEXT, HubNpcOnboardingPlan, hub_npc_directional_hint_frame,
    hub_npc_onboarding_plan,
};
use solomon_sim::hud::HudRect;
use solomon_sim::ui_text::{
    UiFontName, UiTextLayout, UiTextSpec, layout_ui_text, ui_font, ui_glyph_ink_bounds,
};

use crate::beam_mesh::{sprite_frame_texel_rect, sprite_texel_rect};
use crate::courtyard::Camera;
use crate::spells::{StockAtlas, load_atlas};

struct Arrow {
    record: usize,
    corners: [[f32; 2]; 4],
    visible: bool,
    bounds: HudRect,
}

struct Callout {
    arrow: Arrow,
    text: Vec2,
    scale: f64,
}

pub struct HubGuidanceView {
    ui: StockAtlas,
    font: Texture,
    text: UiTextLayout,
    callout: Option<Callout>,
    arrows: Vec<Arrow>,
    bounds: Vec<HudRect>,
}

impl HubGuidanceView {
    pub async fn load(renderer: &Renderer) -> Result<Self, String> {
        let ui = load_atlas(renderer, "UI").await?;
        let font = renderer
            .upload(&solomon_graphics::assets::load_image("assets/stock/Fonts.png").await?)?;
        let line_height = ui_font(UiFontName::Medium).metrics[2];
        let text = layout_ui_text(&UiTextSpec {
            line_height: Some(line_height),
            ..UiTextSpec::new(UiFontName::Medium, HUB_WALK_TO_TALK_TEXT, 0.0, -line_height / 2.0)
        });
        Ok(Self { ui, font, text, callout: None, arrows: Vec::new(), bounds: Vec::new() })
    }

    pub fn clear(&mut self) {
        self.callout = None;
        self.arrows.clear();
        self.bounds.clear();
    }

    /// Both drawing and Party reservations use the same submitted geometry.
    pub fn prepare(&mut self, help: &[bool; 10], tick: u64, camera: &Camera, viewport: Vec2) {
        self.clear();
        // The active arrows reserve their space even during their dark blink phase.
        for plan in hub_npc_onboarding_plan(help, 41, None) {
            match plan {
                HubNpcOnboardingPlan::WalkToTalk {
                    target,
                    arrow_offset,
                    arrow_record,
                    arrow_rotation_degrees,
                    text_offset,
                    ..
                } => {
                    let actor = target.definition(false).geometry.position;
                    let arrow = self.arrow(
                        arrow_record,
                        project(
                            Vec2::new(actor.x + arrow_offset.x, actor.y + arrow_offset.y),
                            camera,
                        ),
                        camera.scale,
                        arrow_rotation_degrees.to_radians(),
                        true,
                    );
                    let text = project(
                        Vec2::new(actor.x + text_offset.x, actor.y + text_offset.y),
                        camera,
                    );
                    let mut bounds = arrow.bounds;
                    for glyph in &self.text.glyphs {
                        let ink = ui_glyph_ink_bounds(glyph);
                        // The native black outline samples radii 1 and 3 around each line.
                        let glyph_bounds = HudRect::new(
                            text.x + (ink.left - 3.0) * camera.scale,
                            text.y + (ink.top - 3.0) * camera.scale,
                            (ink.width + 6.0) * camera.scale,
                            (ink.height + 6.0) * camera.scale,
                        );
                        bounds = union(bounds, glyph_bounds);
                    }
                    self.bounds.push(bounds);
                    self.callout = Some(Callout { arrow, text, scale: camera.scale });
                }
                HubNpcOnboardingPlan::Directional { target, offset, record } => {
                    let actor = target.definition(false).geometry.position;
                    let target = project(Vec2::new(actor.x + offset.x, actor.y + offset.y), camera);
                    let (center, rotation) = hub_npc_directional_hint_frame(target, viewport);
                    let arrow = self.arrow(record, center, 1.0, rotation, tick % 80 > 40);
                    self.bounds.push(arrow.bounds);
                    self.arrows.push(arrow);
                }
            }
        }
    }

    pub fn screen_bounds(&self, display_scale: f64) -> Vec<HudRect> {
        self.bounds
            .iter()
            .map(|bounds| {
                HudRect::new(
                    bounds.x * display_scale,
                    bounds.y * display_scale,
                    bounds.width * display_scale,
                    bounds.height * display_scale,
                )
            })
            .collect()
    }

    pub fn draw_world(&self, renderer: &mut Renderer) {
        let Some(callout) = &self.callout else { return };
        self.draw_arrow(renderer, &callout.arrow);
        for radius in [1.0, 3.0] {
            for degrees in (0..360).step_by(20) {
                let radians = f64::from(degrees).to_radians();
                self.draw_text(
                    renderer,
                    callout,
                    Vec2::new(radians.cos() * radius, radians.sin() * radius),
                    [0.0, 0.0, 0.0, 1.0],
                );
            }
        }
        self.draw_text(renderer, callout, Vec2::ZERO, [0.85, 0.73, 0.44, 1.0]);
    }

    pub fn draw_screen(&self, renderer: &mut Renderer) {
        for arrow in &self.arrows {
            if arrow.visible {
                self.draw_arrow(renderer, arrow);
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn apply_diagnostics(
        &self,
        canvas: &web_sys::HtmlCanvasElement,
        display_scale: f64,
    ) -> Result<(), String> {
        canvas
            .set_attribute(
                "data-npc-walk-to-talk-visible",
                if self.callout.is_some() { "true" } else { "false" },
            )
            .map_err(|_| "Could not report College callout visibility")?;
        canvas
            .set_attribute(
                "data-npc-directional-hint-count",
                &self.arrows.iter().filter(|arrow| arrow.visible).count().to_string(),
            )
            .map_err(|_| "Could not report College hint visibility")?;
        let bounds: Vec<_> = self.screen_bounds(display_scale).iter().map(|bounds| {
            serde_json::json!({"x":bounds.x,"y":bounds.y,"width":bounds.width,"height":bounds.height})
        }).collect();
        canvas
            .set_attribute(
                "data-npc-guidance-bounds",
                &serde_json::to_string(&bounds).map_err(|error| error.to_string())?,
            )
            .map_err(|_| "Could not report College hint geometry".into())
    }

    fn arrow(
        &self,
        record: usize,
        center: Vec2,
        scale: f64,
        rotation: f64,
        visible: bool,
    ) -> Arrow {
        let sprite = &self.ui.records[record];
        let (corners, bounds) = arrow_geometry(sprite, center, scale, rotation);
        Arrow { record, corners, visible, bounds }
    }

    fn draw_arrow(&self, renderer: &mut Renderer, arrow: &Arrow) {
        let source = sprite_texel_rect(&self.ui.records[arrow.record]);
        let record = &self.ui.records[arrow.record];
        let texture = self.ui.texture.clamped_to([
            record.x as f32,
            record.y as f32,
            record.width as f32,
            record.height as f32,
        ]);
        renderer.draw_quad(&texture, source, arrow.corners, [[1.0; 4]; 4], BlendMode::Normal);
    }

    fn draw_text(&self, renderer: &mut Renderer, callout: &Callout, offset: Vec2, tint: [f32; 4]) {
        for glyph in &self.text.glyphs {
            let ink = ui_glyph_ink_bounds(glyph);
            renderer.draw(
                &self.font.clamped_to(glyph.glyph.frame.map(|value| value as f32)),
                Quad {
                    source: sprite_frame_texel_rect(glyph.glyph.frame),
                    destination: [
                        (callout.text.x + (ink.left + offset.x) * callout.scale) as f32,
                        (callout.text.y + (ink.top + offset.y) * callout.scale) as f32,
                        (ink.width * callout.scale) as f32,
                        (ink.height * callout.scale) as f32,
                    ],
                    tint,
                    blend: BlendMode::Normal,
                },
            );
        }
    }
}

fn arrow_geometry(
    sprite: &solomon_assets::SpriteRecord,
    center: Vec2,
    scale: f64,
    rotation: f64,
) -> ([[f32; 2]; 4], HudRect) {
    let (sin, cos) = rotation.sin_cos();
    let project_rect = |left: f64, top: f64, width: f64, height: f64| {
        [[left, top], [left + width, top], [left + width, top + height], [left, top + height]].map(
            |[x, y]| {
                [
                    (center.x + (x * cos - y * sin) * scale) as f32,
                    (center.y + (x * sin + y * cos) * scale) as f32,
                ]
            },
        )
    };
    let left = -f64::from(sprite.logical_width) / 2.0;
    let top = -f64::from(sprite.logical_height) / 2.0;
    let [registered_x, registered_y] = sprite.registered_origin();
    let corners = project_rect(
        left + f64::from(registered_x),
        top + f64::from(registered_y),
        f64::from(sprite.width),
        f64::from(sprite.height),
    );
    // UI 88 registers a 16x21 crop inside a 116x128 logical box. Reserve that
    // authored box, as the maintained Pixi sprite does, without stretching its ink.
    let bounds = corners_bounds(project_rect(
        left,
        top,
        f64::from(sprite.logical_width),
        f64::from(sprite.logical_height),
    ));
    (corners, bounds)
}

fn project(point: Vec2, camera: &Camera) -> Vec2 {
    Vec2::new(
        (point.x - camera.origin.x) * camera.scale,
        (point.y - camera.origin.y) * camera.scale,
    )
}

fn corners_bounds(corners: [[f32; 2]; 4]) -> HudRect {
    let left = corners.iter().map(|point| f64::from(point[0])).fold(f64::INFINITY, f64::min);
    let top = corners.iter().map(|point| f64::from(point[1])).fold(f64::INFINITY, f64::min);
    let right = corners.iter().map(|point| f64::from(point[0])).fold(f64::NEG_INFINITY, f64::max);
    let bottom = corners.iter().map(|point| f64::from(point[1])).fold(f64::NEG_INFINITY, f64::max);
    HudRect::new(left, top, right - left, bottom - top)
}

fn union(first: HudRect, second: HudRect) -> HudRect {
    let left = first.x.min(second.x);
    let top = first.y.min(second.y);
    HudRect::new(
        left,
        top,
        (first.x + first.width).max(second.x + second.width) - left,
        (first.y + first.height).max(second.y + second.height) - top,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directional_arrow_keeps_its_authored_registration_and_logical_box() {
        let records =
            solomon_assets::parse_bundle(include_bytes!("../../../web/assets/stock/UI.bundle"))
                .expect("stock UI bundle");
        let (corners, bounds) = arrow_geometry(&records[88], Vec2::new(300.0, 200.0), 1.0, 0.0);
        assert_eq!(corners, [[291.0, 190.0], [307.0, 190.0], [307.0, 211.0], [291.0, 211.0]]);
        assert_eq!(bounds, HudRect::new(242.0, 136.0, 116.0, 128.0));
    }
}
