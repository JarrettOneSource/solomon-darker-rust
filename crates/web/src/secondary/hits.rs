//! `NativeLeviathanCompositeView::setPuppetHit`: diffuse copies of Main sprites
//! and the appendage target, with the source's separate capture/output alpha.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use solomon_sim::Vec2;
use solomon_sim::presentation::multiply_native_tints;
use solomon_sim::secondary::NativeSecondaryActorKind;
use solomon_sim::secondary::presentation::{
    NativeSecondaryBlend, NativeSecondarySpriteDraw, native_leviathan_composite_plan,
};

use super::draws::TargetDraw;
use super::{SecondaryPuppetHit, SecondaryView};
use crate::courtyard::Camera;
use solomon_graphics::{BlendMode, Renderer, TextureColorMode, Vertex};

impl SecondaryView {
    /// Call after prepare(), before painter submission. Values come from the
    /// canonical world feedback map; only Leviathan has this composite hit pass.
    pub fn prepare_puppet_hits(
        &mut self,
        renderer: &mut Renderer,
        hits: &BTreeMap<u64, SecondaryPuppetHit>,
    ) -> Result<(), String> {
        self.puppet_hits.clear();
        for actor in &self.actors {
            if actor.kind != NativeSecondaryActorKind::Leviathan {
                continue;
            }
            let Some(&hit) = hits.get(&actor.id) else {
                continue;
            };
            self.puppet_hits.insert(actor.id, hit);
            if let Entry::Vacant(target) = self.hit_targets.entry(actor.id) {
                renderer.flush();
                target.insert(renderer.create_render_target(256)?);
            }
        }
        for (&id, &hit) in &self.puppet_hits {
            let parent = self.actors.iter().find(|actor| actor.id == id).unwrap();
            let camera = Camera {
                origin: Vec2::new(parent.plan.root.x - 128.0, parent.plan.root.y - 128.0),
                scale: 1.0,
            };
            renderer.render_to_color(&self.hit_targets[&id], [256.0; 2], [0.0; 4], |renderer| {
                for &child in &self.composite_children[&id] {
                    let actor = &self.actors[child];
                    for draw in &actor.plan.draws {
                        let draw = hit_sprite(draw, hit);
                        self.draw_hit_sprite(renderer, &camera, actor.plan.root, &draw, [1.0; 3]);
                    }
                }
                let composite = native_leviathan_composite_plan(parent.scale);
                self.draw_leviathan_mask(
                    renderer,
                    &camera,
                    parent.plan.root,
                    composite.mask,
                    Some(hit),
                );
                let clear = composite.clear;
                renderer.draw_solid_triangles(
                    &clear_triangles(clear.x, clear.y, clear.width, clear.height),
                    BlendMode::Multiply,
                );
            });
        }
        Ok(())
    }

    pub(super) fn draw_leviathan_hit(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        index: usize,
        tint: [f32; 3],
    ) {
        let actor = &self.actors[index];
        let Some(&hit) = self.puppet_hits.get(&actor.id) else {
            return;
        };
        for draw in &actor.plan.draws {
            let draw = hit_sprite(draw, hit);
            self.draw_hit_sprite(renderer, camera, actor.plan.root, &draw, tint);
        }
        let hit_tint = [
            ((hit.tint >> 16) & 255) as f32 / 255.0 * tint[0],
            ((hit.tint >> 8) & 255) as f32 / 255.0 * tint[1],
            (hit.tint & 255) as f32 / 255.0 * tint[2],
        ];
        for output in native_leviathan_composite_plan(actor.scale).outputs {
            self.draw_target(
                renderer,
                camera,
                &self.hit_targets[&actor.id].texture,
                TargetDraw {
                    center: Vec2::new(actor.plan.root.x - 0.5, actor.plan.root.y - 0.5),
                    scale: 1.0,
                    alpha: output.alpha * hit.alpha,
                    blend: if output.blend == NativeSecondaryBlend::Add {
                        BlendMode::Add
                    } else {
                        BlendMode::Normal
                    },
                    tint: hit_tint,
                    color_mode: TextureColorMode::AlphaMask,
                },
            );
        }
    }

    fn draw_hit_sprite(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        root: Vec2,
        draw: &NativeSecondarySpriteDraw,
        tint: [f32; 3],
    ) {
        let (record, vertices) = self.sprite_vertices(camera, root, draw, tint);
        let blend = if draw.blend == NativeSecondaryBlend::Add {
            BlendMode::Add
        } else {
            BlendMode::Normal
        };
        renderer.draw_triangles_with_color_mode(
            record.texture,
            &[[vertices[0], vertices[1], vertices[2]], [vertices[0], vertices[2], vertices[3]]],
            blend,
            TextureColorMode::AlphaMask,
        );
    }
}

fn hit_sprite(
    draw: &NativeSecondarySpriteDraw,
    hit: SecondaryPuppetHit,
) -> NativeSecondarySpriteDraw {
    let mut draw = draw.clone();
    draw.alpha *= hit.alpha;
    draw.tint = multiply_native_tints(draw.tint, hit.tint);
    draw
}

pub(super) fn clear_triangles(x: f64, y: f64, width: f64, height: f64) -> [[Vertex; 3]; 2] {
    let points = [[x, y], [x + width, y], [x, y + height], [x + width, y + height]];
    let vertices = points.map(|point| Vertex {
        position: [point[0] as f32, point[1] as f32],
        texel: [0.0; 2],
        // Source's lower clear Sprite uses transparent Texture.EMPTY. Multiply
        // must clear both RGB and alpha, including the diffuse-hit target.
        tint: [0.0; 4],
    });
    [[vertices[0], vertices[1], vertices[2]], [vertices[1], vertices[2], vertices[3]]]
}
