//! Staff proc and Pike-break stock sprites share the secondary atlas submission.

use solomon_sim::Vec2;
use solomon_sim::secondary::presentation::{SecondaryAtlas, SecondarySpriteDraw};
use solomon_sim::spells::staff::presentation::{
    player_staff_pike_break_draws, player_staff_vfx_render_plan,
};
use solomon_sim::spells::staff::{PlayerStaffPikeBreakVfx, PlayerStaffTransient, PlayerStaffVfx};

use super::{SecondaryPainter, SecondaryView};
use crate::courtyard::Camera;
use solomon_graphics::Renderer;

impl SecondaryView {
    /// Joins visible children directly from PrimarySpellTransient::Staff. Action
    /// and contact owners have no painter of their own; their emitted VFX own the rows.
    #[must_use]
    pub fn staff_painter(state: &PlayerStaffTransient) -> Option<SecondaryPainter> {
        let position = match state {
            PlayerStaffTransient::Vfx(state) => player_staff_vfx_render_plan(state).position,
            PlayerStaffTransient::PikeBreak(state) => state.position,
            PlayerStaffTransient::Action(_)
            | PlayerStaffTransient::Contact(_)
            | PlayerStaffTransient::ContactKnockback(_)
            | PlayerStaffTransient::Knockback(_) => return None,
        };
        let registration = state
            .common()
            .painter_registrations
            .first()
            .expect("visible staff effect has its native painter registration");
        Some(SecondaryPainter {
            layer: registration.layer(position.y, 0.0),
            region_light_point: None,
        })
    }

    pub fn draw_staff_transient(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        state: &PlayerStaffTransient,
        world_tint: u32,
    ) {
        match state {
            PlayerStaffTransient::Vfx(state) => {
                self.draw_staff_vfx(renderer, camera, state, world_tint)
            }
            PlayerStaffTransient::PikeBreak(state) => {
                self.draw_staff_pike_break(renderer, camera, state, world_tint)
            }
            // These owners perform combat work and have no visual root.
            PlayerStaffTransient::Action(_)
            | PlayerStaffTransient::Contact(_)
            | PlayerStaffTransient::ContactKnockback(_)
            | PlayerStaffTransient::Knockback(_) => {}
        }
    }

    pub fn draw_staff_vfx(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        state: &PlayerStaffVfx,
        world_tint: u32,
    ) {
        let plan = player_staff_vfx_render_plan(state);
        let draw = SecondarySpriteDraw {
            alpha: plan.alpha,
            atlas: SecondaryAtlas::BadGuys,
            blend: plan.blend,
            color_mode: None,
            clip: None,
            entry: plan.entry,
            offset: Vec2::ZERO,
            role: "player-staff-vfx".into(),
            rotation_radians: plan.rotation_radians,
            scale_x: plan.scale,
            scale_y: plan.scale,
            tint: plan.tint.unwrap_or(world_tint),
        };
        self.draw_sprite(renderer, camera, plan.position, &draw, [1.0; 3]);
    }

    pub fn draw_staff_pike_break(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        state: &PlayerStaffPikeBreakVfx,
        world_tint: u32,
    ) {
        for plan in player_staff_pike_break_draws(state) {
            let draw = SecondarySpriteDraw {
                alpha: plan.alpha,
                atlas: SecondaryAtlas::BadGuys,
                blend: plan.blend,
                color_mode: None,
                clip: None,
                entry: plan.entry,
                offset: plan.offset,
                role: plan.role,
                rotation_radians: plan.rotation_radians,
                scale_x: plan.scale_x,
                scale_y: plan.scale_y,
                tint: plan.tint.unwrap_or(world_tint),
            };
            self.draw_sprite(renderer, camera, state.position, &draw, [1.0; 3]);
        }
    }
}
