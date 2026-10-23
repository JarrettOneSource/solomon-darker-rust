//! The source's 256-square Storm and Leviathan render-target passes.

use std::collections::BTreeMap;

use solomon_sim::Vec2;
use solomon_sim::secondary::presentation::*;

use super::draws::TargetDraw;
use super::hits::clear_triangles;
use super::{SecondaryPuppetHit, SecondaryView};
use crate::courtyard::Camera;
use solomon_graphics::{BlendMode, Renderer, TextureColorMode};

impl SecondaryView {
    pub(super) fn update_composites(
        &self,
        renderer: &mut Renderer,
        indexes: &BTreeMap<u64, usize>,
    ) {
        let storm_camera = Camera { origin: Vec2::new(-128.0, -128.0), scale: 1.0 };
        for actor in &self.actors {
            let Some(plan) = &actor.plan.storm_composite else {
                continue;
            };
            renderer.render_to_color(
                &self.storm_targets[&actor.id],
                [256.0; 2],
                [0.0; 4],
                |renderer| {
                    for draw in &plan.draws {
                        self.draw_sprite(renderer, &storm_camera, Vec2::ZERO, draw, [1.0; 3]);
                    }
                },
            );
        }
        for (&parent_id, children) in &self.composite_children {
            let parent = &self.actors[indexes[&parent_id]];
            let camera = Camera {
                origin: Vec2::new(parent.plan.root.x - 128.0, parent.plan.root.y - 128.0),
                scale: 1.0,
            };
            let composite = native_leviathan_composite_plan(parent.scale);
            renderer.render_to_color(
                &self.leviathan_targets[&parent_id],
                [256.0; 2],
                [0.0; 4],
                |renderer| {
                    for &child in children {
                        self.draw_actor(renderer, &camera, child, [1.0; 3]);
                    }
                    self.draw_leviathan_mask(
                        renderer,
                        &camera,
                        parent.plan.root,
                        composite.mask,
                        None,
                    );
                    let clear = composite.clear;
                    renderer.draw_solid_triangles(
                        &clear_triangles(clear.x, clear.y, clear.width, clear.height),
                        BlendMode::Multiply,
                    );
                },
            );
        }
    }

    /// Current Source uses a geometric rectangle mask, so this multiply draw
    /// runs on the appendage target itself. It is not the old filtered Sprite mask.
    pub(super) fn draw_leviathan_mask(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        root: Vec2,
        mask: NativeLeviathanCompositeMask,
        hit: Option<SecondaryPuppetHit>,
    ) {
        let draw = NativeSecondarySpriteDraw {
            alpha: hit.map_or(1.0, |hit| hit.alpha),
            atlas: NativeSecondaryAtlas::BadGuys,
            blend: NativeSecondaryBlend::Normal,
            color_mode: None,
            entry: mask.entry,
            offset: Vec2::ZERO,
            role: "leviathan-appendage-lower-mask".into(),
            rotation_radians: 0.0,
            scale_x: mask.scale,
            scale_y: mask.scale,
            tint: hit.map_or(0xffffff, |hit| hit.tint),
        };
        let (record, vertices) = self.sprite_vertices(camera, root, &draw, [1.0; 3]);
        let triangles =
            [[vertices[0], vertices[1], vertices[2]], [vertices[0], vertices[2], vertices[3]]];
        // The target is 256 square. GL scissor starts at its bottom-left;
        // Source's geometric clip starts at (0, clipTop) from the top-left.
        let clip = [
            0,
            (NATIVE_LEVIATHAN_RENDER_TARGET_SIZE - mask.clip_top - mask.clip_size) as i32,
            mask.clip_size as i32,
            mask.clip_size as i32,
        ];
        let color_mode =
            if hit.is_some() { TextureColorMode::AlphaMask } else { TextureColorMode::Texture };
        renderer.draw_clipped(clip, 1.0, |renderer| {
            renderer.draw_triangles_with_color_mode(
                record.texture,
                &triangles,
                BlendMode::Multiply,
                color_mode,
            );
        });
    }

    pub(super) fn draw_leviathan_outputs(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        index: usize,
        tint: [f32; 3],
    ) {
        let actor = &self.actors[index];
        let texture = &self.leviathan_targets[&actor.id].texture;
        for output in native_leviathan_composite_plan(actor.scale).outputs {
            self.draw_target(
                renderer,
                camera,
                texture,
                TargetDraw {
                    // Source's output Sprite anchor is .5 + .5 / targetSize.
                    center: Vec2::new(actor.plan.root.x - 0.5, actor.plan.root.y - 0.5),
                    scale: 1.0,
                    alpha: output.alpha,
                    blend: if output.blend == NativeSecondaryBlend::Add {
                        BlendMode::Add
                    } else {
                        BlendMode::Normal
                    },
                    tint,
                    color_mode: TextureColorMode::Texture,
                },
            );
        }
    }
}
