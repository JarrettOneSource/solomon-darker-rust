//! The articulated material captures in `player-harden-view.ts`,
//! `player-webbed-view.ts` and `native-player-diffuse-capture.ts`.

use solomon_sim::enemies::webbed::native_cocoon_tilt;
use solomon_sim::float;
use solomon_sim::secondary::presentation::{
    NATIVE_PLAYER_MAGIC_SHIELD_ENTRY, NATIVE_PLAYER_MAGIC_SHIELD_OFFSET_Y,
};
use solomon_sim::spells::presentation::{NativeSpellSprite, NativeSpellSpritePlan, SpellBlend};

use super::*;
use crate::native_mesh::native_sprite_texel_rect;

impl WizardView {
    pub(super) fn draw_living_layers(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        wizard: &HubWizard,
        tick: f64,
        spells: &SpellView,
        pass: WizardDrawPass,
    ) {
        self.draw_actor(renderer, camera, wizard, tick, spells, pass);
        let state = &self.player_presentation;
        if state.hit_alpha > 0.0 {
            renderer.draw_diffuse_with_opacity(state.hit_alpha as f32, |renderer| {
                self.draw_actor(renderer, camera, wizard, tick, spells, WizardDrawPass::Hit);
            });
        }
        if let Some(shield) = state.magic_shield.filter(|shield| shield.visible) {
            let record = &self.records[NATIVE_PLAYER_MAGIC_SHIELD_ENTRY];
            let mut tint = tint_rgba(shield.tint);
            tint[3] = packed_alpha(shield.alpha);
            draw_centered_texture(
                renderer,
                camera,
                &self.atlas,
                native_sprite_texel_rect(record),
                Vec2::new(
                    wizard.position.x + f64::from(record.center_x) * shield.scale,
                    wizard.position.y
                        + NATIVE_PLAYER_MAGIC_SHIELD_OFFSET_Y
                        + f64::from(record.center_y) * shield.scale,
                ),
                Vec2::new(
                    f64::from(record.width) * shield.scale,
                    f64::from(record.height) * shield.scale,
                ),
                0.0,
                tint,
                BlendMode::Add,
            );
        }
    }

    pub(super) fn draw_material_capture(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        wizard: &HubWizard,
        tick: f64,
        spells: &SpellView,
    ) {
        let state = &self.player_presentation;
        if state.harden_coating <= 0.0 && state.webbed.is_none() {
            return;
        }
        let mut captured_wizard = wizard.clone();
        captured_wizard.position = Vec2::ZERO;
        let capture_camera = Camera { origin: Vec2::new(-128.0, -153.0), scale: 1.0 };
        renderer.render_diffuse_to(&self.diffuse_capture, [256.0; 2], |renderer| {
            self.draw_living_layers(
                renderer,
                &capture_camera,
                &captured_wizard,
                tick,
                spells,
                WizardDrawPass::Diffuse,
            );
        });
        if state.harden_coating > 0.0 {
            let offsets = wizard_draw_plan(
                wizard.heading_index,
                wizard.walk_cycle_primary,
                wizard.gait_degrees,
            );
            renderer.render_onto(&self.diffuse_capture, [256.0; 2], |renderer| {
                let record = &self.records[1];
                draw_centered_texture(
                    renderer,
                    &Camera { origin: Vec2::ZERO, scale: 1.0 },
                    &self.atlas,
                    native_sprite_texel_rect(record),
                    Vec2::new(128.0 + offsets.head_offset.x, 128.0 + offsets.head_offset.y),
                    Vec2::new(f64::from(record.width), f64::from(record.height)),
                    wizard.heading_index as f64 * 24.0 * std::f64::consts::PI / 180.0,
                    [1.0; 4],
                    BlendMode::Multiply,
                );
            });
            for _ in 0..3 {
                self.draw_capture(
                    renderer,
                    camera,
                    wizard.position,
                    f64::from(1.12_f32),
                    fround(state.harden_coating * f64::from(0.7_f32)),
                    0x3fbfff,
                );
            }
            self.draw_harden_orb(renderer, camera, wizard, tick, spells);
            return;
        }
        let web = state.webbed.expect("a material capture has Harden or Webbed ownership");
        for index in 0..3 {
            if index as f64 >= web.severity {
                continue;
            }
            self.draw_capture(
                renderer,
                camera,
                wizard.position,
                fround(1.0 + f64::from(0.05_f32) * index as f64),
                (web.severity - index as f64).clamp(0.0, 1.0),
                0xffffff,
            );
        }
        if web.severity >= 3.0 {
            let atlas = spells.dead_hawg();
            let record = &atlas.records[29];
            let tilt = native_cocoon_tilt(wizard.heading_index as f64 * 15.0);
            let height = f64::from(record.height);
            let left = -f64::from(record.width) * 0.5 + f64::from(record.center_x);
            let top = -height * 0.5 + f64::from(record.center_y);
            let bottom = top + height;
            let corners = [
                [left, top],
                [left + f64::from(record.width), top],
                [left + f64::from(record.width), bottom],
                [left, bottom],
            ]
            .map(|[x, y]| {
                let point = camera.project(
                    wizard.position.x + x - tilt.x / height * y + tilt.x * bottom / height,
                    wizard.position.y + (1.0 - tilt.y / height) * y - 20.0
                        + tilt.y * bottom / height,
                    0.0,
                    0.0,
                );
                [point[0], point[1]]
            });
            renderer.draw_quad(
                &atlas.texture,
                native_sprite_texel_rect(record),
                corners,
                [[1.0; 4]; 4],
                BlendMode::Normal,
            );
            if web.hit_pulse > 0.0 {
                let alpha = packed_alpha(web.hit_pulse);
                renderer.draw_quad(
                    &atlas.texture,
                    native_sprite_texel_rect(record),
                    corners,
                    [[1.0, 0.0, 0.0, alpha]; 4],
                    BlendMode::Normal,
                );
            }
        }
    }

    fn draw_capture(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        position: Vec2,
        scale: f64,
        alpha: f64,
        tint: u32,
    ) {
        let mut color = tint_rgba(tint);
        color[3] = packed_alpha(alpha);
        renderer.draw(
            &self.diffuse_capture.texture,
            Quad {
                // Render-target rows are inverted relative to uploaded PNGs.
                source: [0.0, 256.0, 256.0, -256.0],
                destination: camera.project(
                    position.x - 128.0 * scale,
                    position.y - 25.0 - 128.0 * scale,
                    256.0 * scale,
                    256.0 * scale,
                ),
                tint: color,
                blend: BlendMode::Add,
            },
        );
    }

    fn draw_harden_orb(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        wizard: &HubWizard,
        tick: f64,
        spells: &SpellView,
    ) {
        if wizard.primary_cast.selected_primary_id.is_none()
            || self
                .equipment
                .appearance
                .weapon
                .as_ref()
                .is_none_or(|weapon| weapon.kind() != NativePlayerWeaponKind::Staff)
        {
            return;
        }
        let state = &self.player_presentation;
        let element =
            selected_cast_element(wizard.primary_cast.selected_primary_id, self.equipment.element);
        let poses = player_character_attachment_poses(
            &wizard.primary_cast,
            element,
            state.staff_action_pose,
            state.secondary_cast,
        );
        let offset = player_staff_attachment_offset(wizard.heading_index, poses.staff);
        let gait =
            wizard_draw_plan(wizard.heading_index, wizard.walk_cycle_primary, wizard.gait_degrees);
        let position = Vec2::new(
            wizard.position.x + offset.x + gait.front_attachment_offset.x,
            wizard.position.y + offset.y + gait.front_attachment_offset.y,
        );
        self.draw_damage_x4(
            renderer,
            camera,
            position,
            tick,
            player_equipped_element_effect_scale(state.overlay_effect_phase),
            spells,
        );
        spells.draw_element_vfx(
            renderer,
            camera,
            position,
            &native_selected_primary_element_vfx_plan(
                if state.plane_active {
                    80
                } else {
                    wizard.primary_cast.selected_primary_id.unwrap()
                },
                tick,
                player_equipped_element_effect_scale(state.overlay_effect_phase),
            ),
        );
    }

    pub(super) fn draw_damage_x4(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        position: Vec2,
        tick: f64,
        scale: f64,
        spells: &SpellView,
    ) {
        let mut plan = particle_plan(position);
        for layer in native_damage_x4_vfx_plan(
            self.player_presentation.damage_x4_ticks_remaining,
            tick,
            scale,
        ) {
            plan.draws.push(NativeSpellSprite {
                alpha: layer.alpha,
                blend: SpellBlend::Add,
                rotation: layer.rotation_degrees * std::f64::consts::PI / 180.0,
                tint: 0xd8ba70,
                ..NativeSpellSprite::new(7, layer.scale)
            });
        }
        spells.draw_sprite_plan(renderer, camera, &plan);
    }

    pub(super) fn hit_tint(&self, tint: u32) -> u32 {
        let red_factor =
            if self.player_presentation.complex_lighting { f64::from(0.65_f32) } else { 1.0 };
        ((f64::from((tint >> 16) & 0xff) * red_factor).trunc() as u32) << 16
    }
}

fn particle_plan(position: Vec2) -> NativeSpellSpritePlan {
    NativeSpellSpritePlan {
        position,
        world_y: position.y,
        sort_bias: 0.0,
        region_light_point: None,
        draws: Vec::new(),
    }
}

fn packed_alpha(alpha: f64) -> f32 {
    ((alpha.clamp(0.0, 1.0) * 255.0).trunc() / 255.0) as f32
}

#[allow(clippy::too_many_arguments)] // One authored sprite has a source, transform and material.
fn draw_centered_texture(
    renderer: &mut Renderer,
    camera: &Camera,
    texture: &Texture,
    source: [f32; 4],
    position: Vec2,
    size: Vec2,
    rotation: f64,
    tint: [f32; 4],
    blend: BlendMode,
) {
    let (sin, cos) = float::sin_cos(rotation);
    let corners = [
        [-size.x * 0.5, -size.y * 0.5],
        [size.x * 0.5, -size.y * 0.5],
        [size.x * 0.5, size.y * 0.5],
        [-size.x * 0.5, size.y * 0.5],
    ]
    .map(|[x, y]| {
        let point = camera.project(
            position.x + x * cos - y * sin,
            position.y + x * sin + y * cos,
            0.0,
            0.0,
        );
        [point[0], point[1]]
    });
    renderer.draw_quad(texture, source, corners, [tint; 4], blend);
}
