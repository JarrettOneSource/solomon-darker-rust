//! Low-level secondary spell submission: affine transforms, registered atlas texels
//! and packed vertex colors.

use solomon_sim::Vec2;
use solomon_sim::float;
use solomon_sim::presentation::multiply_tints;
use solomon_sim::secondary::presentation::*;
use solomon_sim::spells::air::{AirCoronaPlan, AirMesh};
use solomon_sim::spells::weld::FlameLashBounds;

use super::SecondaryView;
use crate::beam_mesh::clip_to_rect;
use crate::courtyard::Camera;
use solomon_graphics::{BlendMode, Renderer, Texture, TextureColorMode, Vertex};

/// How to draw a render-target texture: center, scale, alpha, blend, tint and color mode.
pub(super) struct TargetDraw {
    pub center: Vec2,
    pub scale: f64,
    pub alpha: f64,
    pub blend: BlendMode,
    pub tint: [f32; 3],
    pub color_mode: TextureColorMode,
}

/// A sprite's texture, source rectangle, logical size and registered origin; `texels`
/// marks atlas records, whose UVs are inset from the rectangle's edges.
pub(super) struct SpriteRecord {
    pub texture: Texture,
    pub source: [f32; 4],
    pub width: f64,
    pub height: f64,
    pub origin: Vec2,
    pub texels: bool,
}

impl SecondaryView {
    pub(super) fn record(&self, atlas: SecondaryAtlas, entry: usize) -> SpriteRecord {
        if atlas == SecondaryAtlas::Clothes {
            let texture =
                &self.clothes[entry.checked_sub(1).expect("Clothes secondary entry is 1 or 2")];
            return SpriteRecord {
                texture: texture.clone(),
                source: [0.0, 0.0, texture.width, texture.height],
                width: f64::from(texture.width),
                height: f64::from(texture.height),
                origin: Vec2::ZERO,
                texels: false,
            };
        }
        let atlas = match atlas {
            SecondaryAtlas::BadGuys => &self.bad_guys,
            SecondaryAtlas::DeadHawg => &self.dead_hawg,
            SecondaryAtlas::Demon => &self.demon,
            SecondaryAtlas::Golem => &self.golem,
            SecondaryAtlas::Clothes => unreachable!(),
        };
        let record = &atlas.records[entry];
        SpriteRecord {
            texture: atlas.texture.clamped_to(record.atlas_rect()),
            source: [record.x as f32, record.y as f32, record.width as f32, record.height as f32],
            width: f64::from(record.width),
            height: f64::from(record.height),
            origin: Vec2::new(f64::from(record.center_x), f64::from(record.center_y)),
            texels: true,
        }
    }

    /// Shared low-level stock submission for secondary and Dampen projectile layers.
    /// The caller keeps the domain transform in its canonical draw plan.
    pub fn draw_sprite(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        root: Vec2,
        draw: &SecondarySpriteDraw,
        region_tint: [f32; 3],
    ) {
        let mode = match draw.color_mode {
            Some(SecondaryColorMode::AlphaMask) => TextureColorMode::WhiteAlphaFilter,
            _ => TextureColorMode::Texture,
        };
        self.draw_sprite_with_color_mode(renderer, camera, root, draw, region_tint, mode);
    }

    /// Stock geometry shared with Goodie and enemy diffuse-hit copies. The diffuse
    /// pass (`AlphaMask`) and Storm's white filter (`WhiteAlphaFilter`) are distinct.
    pub fn draw_sprite_with_color_mode(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        root: Vec2,
        draw: &SecondarySpriteDraw,
        region_tint: [f32; 3],
        mode: TextureColorMode,
    ) {
        if draw.alpha <= 0.0 {
            return;
        }
        let (record, vertices) = self.sprite_vertices(camera, root, draw, region_tint);
        if let Some([x, y, width, height]) = draw.clip {
            let clip = camera.project(root.x + x, root.y + y, width, height);
            renderer.draw_triangles_with_color_mode(
                &record.texture,
                &clipped_sprite_triangles(vertices, clip),
                blend(draw.blend),
                mode,
            );
            return;
        }
        let triangles =
            [[vertices[0], vertices[1], vertices[2]], [vertices[0], vertices[2], vertices[3]]];
        renderer.draw_triangles_with_color_mode(
            &record.texture,
            &triangles,
            blend(draw.blend),
            mode,
        );
    }

    pub(super) fn sprite_vertices(
        &self,
        camera: &Camera,
        root: Vec2,
        draw: &SecondarySpriteDraw,
        tint: [f32; 3],
    ) -> (SpriteRecord, [Vertex; 4]) {
        let record = self.record(draw.atlas, draw.entry);
        let left = -record.width / 2.0 + record.origin.x;
        let top = -record.height / 2.0 + record.origin.y;
        let bounds = [
            (left, top),
            (left + record.width, top),
            (left + record.width, top + record.height),
            (left, top + record.height),
        ];
        let (sine, cosine) = float::sin_cos(draw.rotation_radians);
        let texels = texels(&record);
        let vertices = std::array::from_fn(|index| {
            let (x, y) = bounds[index];
            // Rotation comes before scale, so unequal scales stretch along world axes.
            let transformed_x = (cosine * x - sine * y) * draw.scale_x;
            let transformed_y = (sine * x + cosine * y) * draw.scale_y;
            let projected = camera.project(
                root.x + draw.offset.x + transformed_x,
                root.y + draw.offset.y + transformed_y,
                0.0,
                0.0,
            );
            Vertex {
                position: [projected[0], projected[1]],
                texel: texels[index],
                tint: rgba(draw.tint, draw.alpha, tint),
            }
        });
        (record, vertices)
    }

    pub(super) fn draw_actor(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        index: usize,
        tint: [f32; 3],
    ) {
        let actor = &self.actors[index];
        let plan = &actor.plan;
        if let Some(composite) = &plan.storm_composite {
            self.draw_target(
                renderer,
                camera,
                &self.storm_targets[&actor.id].texture,
                TargetDraw {
                    center: Vec2::new(
                        plan.root.x + composite.offset.x,
                        plan.root.y + composite.offset.y,
                    ),
                    scale: composite.scale,
                    alpha: 1.0,
                    blend: BlendMode::Normal,
                    tint,
                    color_mode: TextureColorMode::Texture,
                },
            );
        }
        if let Some(lightning) = &actor.lightning {
            if let Some(layers) = &lightning.body {
                for layer in layers {
                    self.draw_air_mesh(
                        renderer,
                        camera,
                        plan.root,
                        &layer.mesh,
                        layer.texture_record,
                        rgba(layer.tint, layer.alpha, tint),
                    );
                    if let Some(branch) = &layer.branch {
                        self.draw_air_mesh(
                            renderer,
                            camera,
                            plan.root,
                            &branch.mesh,
                            branch.texture_record,
                            rgba(layer.tint, layer.alpha, tint),
                        );
                    }
                }
            }
            if let Some(corona) = &lightning.source_corona {
                self.draw_air_corona(renderer, camera, plan.root, corona);
            }
            self.draw_air_corona(renderer, camera, plan.root, &lightning.contact_corona);
        }
        // Equal Z indices draw in creation order: mesh, gradients, quad. Sprites
        // begin after all mesh and quad indices.
        for index in 0..plan.meshes.len().max(plan.quads.len()).max(1) {
            if let Some(mesh) = plan.meshes.get(index) {
                self.draw_mesh(renderer, camera, plan.root, mesh, tint);
            }
            if index == 0 {
                for gradient in &plan.gradients {
                    self.draw_gradient(renderer, camera, plan.root, gradient);
                }
            }
            if let Some(quad) = plan.quads.get(index) {
                self.draw_quad(renderer, camera, plan.root, quad, tint);
            }
        }
        for draw in &plan.draws {
            self.draw_sprite(renderer, camera, plan.root, draw, tint);
        }
    }

    fn draw_gradient(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        root: Vec2,
        draw: &SecondaryGradientDraw,
    ) {
        let left = root.x + draw.top_left.x;
        let top = root.y + draw.top_left.y;
        let points = [
            (left, top),
            (left + draw.width, top),
            (left, top + draw.height),
            (left + draw.width, top + draw.height),
        ];
        let vertices = std::array::from_fn(|index| {
            let projected = camera.project(points[index].0, points[index].1, 0.0, 0.0);
            let color = if index < 2 {
                rgba(draw.top_color, draw.top_alpha, [1.0; 3])
            } else {
                rgba(draw.bottom_color, draw.bottom_alpha, [1.0; 3])
            };
            Vertex { position: [projected[0], projected[1]], texel: [0.0, 0.0], tint: color }
        });
        renderer.draw_solid_triangles(&quad_triangles(vertices), BlendMode::Normal);
    }

    fn draw_mesh(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        root: Vec2,
        draw: &SecondaryMeshDraw,
        tint: [f32; 3],
    ) {
        let group = rgba(draw.tint, draw.alpha, tint);
        let mut vertices = Vec::with_capacity(draw.vertices.len() / 2);
        for index in 0..draw.vertices.len() / 2 {
            let projected = camera.project(
                root.x + draw.vertices[index * 2],
                root.y + draw.vertices[index * 2 + 1],
                0.0,
                0.0,
            );
            let packed = draw.vertex_colors[index];
            let color = std::array::from_fn(|channel| {
                let vertex = ((packed >> (channel * 8)) & 255) as f32;
                (vertex * group[channel]).trunc() / 255.0
            });
            vertices.push(Vertex {
                position: [projected[0], projected[1]],
                texel: [
                    draw.uvs[index * 2] as f32 * self.ether_plane.width,
                    draw.uvs[index * 2 + 1] as f32 * self.ether_plane.height,
                ],
                tint: color,
            });
        }
        let triangles = indexed_triangles(&vertices, &draw.indices);
        renderer.draw_triangles(&self.ether_plane, &triangles, blend(draw.blend));
    }

    fn draw_quad(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        root: Vec2,
        draw: &SecondaryQuadDraw,
        tint: [f32; 3],
    ) {
        let record = draw.atlas.zip(draw.entry).map(|(atlas, entry)| self.record(atlas, entry));
        let uv = record.as_ref().map(texels).unwrap_or([[0.0, 0.0]; 4]);
        // Quad vertices are ordered TL, TR, BL, BR, while registered sprite UVs are clockwise.
        let uv = [uv[0], uv[1], uv[3], uv[2]];
        let vertices = std::array::from_fn(|index| {
            let projected = camera.project(
                root.x + draw.vertices[index * 2],
                root.y + draw.vertices[index * 2 + 1],
                0.0,
                0.0,
            );
            Vertex {
                position: [projected[0], projected[1]],
                texel: uv[index],
                tint: rgba(draw.tint, draw.alpha, tint),
            }
        });
        let triangles = quad_triangles(vertices);
        if let Some(record) = record {
            renderer.draw_triangles(&record.texture, &triangles, blend(draw.blend));
        } else {
            renderer.draw_solid_triangles(&triangles, blend(draw.blend));
        }
    }

    fn draw_air_mesh(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        root: Vec2,
        mesh: &AirMesh,
        entry: usize,
        tint: [f32; 4],
    ) {
        let record = self.record(SecondaryAtlas::BadGuys, entry);
        let [top_left, _, bottom_right, _] = texels(&record);
        let vertices = mesh
            .vertices
            .iter()
            .zip(&mesh.uvs)
            .map(|(point, uv)| {
                let projected = camera.project(root.x + point.x, root.y + point.y, 0.0, 0.0);
                Vertex {
                    position: [projected[0], projected[1]],
                    texel: [
                        top_left[0] + uv.x as f32 * (bottom_right[0] - top_left[0]),
                        top_left[1] + uv.y as f32 * (bottom_right[1] - top_left[1]),
                    ],
                    tint,
                }
            })
            .collect::<Vec<_>>();
        renderer.draw_triangles(
            &record.texture,
            &indexed_triangles(&vertices, &mesh.indices),
            BlendMode::Add,
        );
    }

    fn draw_air_corona(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        root: Vec2,
        corona: &AirCoronaPlan,
    ) {
        let root = Vec2::new(root.x + corona.center.x, root.y + corona.center.y);
        for sprite in &corona.draws {
            let draw = SecondarySpriteDraw {
                alpha: sprite.alpha * corona.alpha,
                atlas: SecondaryAtlas::BadGuys,
                blend: SecondaryBlend::Add,
                color_mode: None,
                clip: None,
                entry: sprite.record,
                offset: sprite.offset,
                role: "storm-lightning-corona".into(),
                rotation_radians: sprite.rotation,
                scale_x: sprite.scale.x,
                scale_y: sprite.scale.y,
                tint: sprite.tint,
            };
            self.draw_sprite(renderer, camera, root, &draw, [1.0; 3]);
        }
    }

    pub(super) fn draw_target(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        texture: &Texture,
        draw: TargetDraw,
    ) {
        let TargetDraw { center, scale, alpha, blend, tint, color_mode } = draw;
        let width = f64::from(texture.width) * scale;
        let height = f64::from(texture.height) * scale;
        let corners = [
            (center.x - width / 2.0, center.y - height / 2.0),
            (center.x + width / 2.0, center.y - height / 2.0),
            (center.x + width / 2.0, center.y + height / 2.0),
            (center.x - width / 2.0, center.y + height / 2.0),
        ];
        let positions = corners.map(|(x, y)| {
            let point = camera.project(x, y, 0.0, 0.0);
            [point[0], point[1]]
        });
        // FBO texels have the opposite Y direction from the uploaded stock PNGs.
        renderer.draw_quad_with_color_mode(
            texture,
            [0.0, texture.height, texture.width, -texture.height],
            positions,
            [rgba(0xffffff, alpha, tint); 4],
            blend,
            color_mode,
        );
    }
}

pub(super) fn rgba(tint: u32, alpha: f64, light: [f32; 3]) -> [f32; 4] {
    let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
    let light = byte(light[0]) << 16 | byte(light[1]) << 8 | byte(light[2]);
    let color = multiply_tints(tint, light);
    [
        ((color >> 16) & 255) as f32 / 255.0,
        ((color >> 8) & 255) as f32 / 255.0,
        (color & 255) as f32 / 255.0,
        (alpha.clamp(0.0, 1.0) * 255.0).trunc() as f32 / 255.0,
    ]
}

pub(super) fn texels(record: &SpriteRecord) -> [[f32; 2]; 4] {
    let [x, y, width, height] = record.source;
    // Atlas records start half a texel in and end a quarter texel past their far edge.
    let (left, top, right, bottom) = if record.texels {
        (x + 0.5, y + 0.5, x + width + 0.25, y + height + 0.25)
    } else {
        (x, y, x + width, y + height)
    };
    [[left, top], [right, top], [right, bottom], [left, bottom]]
}

fn quad_triangles(vertices: [Vertex; 4]) -> [[Vertex; 3]; 2] {
    [[vertices[0], vertices[1], vertices[2]], [vertices[1], vertices[2], vertices[3]]]
}
fn indexed_triangles(vertices: &[Vertex], indices: &[u32]) -> Vec<[Vertex; 3]> {
    let (triangles, remainder) = indices.as_chunks::<3>();
    assert!(remainder.is_empty(), "native mesh indices form triangles");
    triangles.iter().map(|triangle| triangle.map(|index| vertices[index as usize])).collect()
}

fn blend(value: SecondaryBlend) -> BlendMode {
    match value {
        SecondaryBlend::Normal => BlendMode::Normal,
        SecondaryBlend::Add => BlendMode::Add,
    }
}

/// Sprite tint is uniform. The existing Region clipper retains position and UV
/// interpolation while the sprite keeps that tint and the enclosing GPU state.
fn clipped_sprite_triangles(vertices: [Vertex; 4], clip: [f32; 4]) -> Vec<[Vertex; 3]> {
    let mesh = AirMesh {
        indices: vec![0, 1, 2, 0, 2, 3],
        vertices: vertices
            .iter()
            .map(|vertex| Vec2::new(f64::from(vertex.position[0]), f64::from(vertex.position[1])))
            .collect(),
        uvs: vertices
            .iter()
            .map(|vertex| Vec2::new(f64::from(vertex.texel[0]), f64::from(vertex.texel[1])))
            .collect(),
    };
    clip_to_rect(
        &mesh,
        Vec2::ZERO,
        FlameLashBounds {
            x: f64::from(clip[0]),
            y: f64::from(clip[1]),
            width: f64::from(clip[2]),
            height: f64::from(clip[3]),
        },
        None,
    )
    .into_iter()
    .map(|triangle| {
        triangle.map(|vertex| Vertex {
            position: [vertex.position.x as f32, vertex.position.y as f32],
            texel: [vertex.uv.x as f32, vertex.uv.y as f32],
            tint: vertices[0].tint,
        })
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_clip_preserves_uvs_for_rotated_and_mirrored_geometry() {
        for rotated in [false, true] {
            let positions = if rotated {
                [[20.0, -20.0], [20.0, 20.0], [-20.0, 20.0], [-20.0, -20.0]]
            } else {
                [[20.0, -20.0], [-20.0, -20.0], [-20.0, 20.0], [20.0, 20.0]]
            };
            let texels = [[0.0, 0.0], [40.0, 0.0], [40.0, 40.0], [0.0, 40.0]];
            let tint = [0.2, 0.4, 0.6, 0.25];
            let vertices = std::array::from_fn(|index| Vertex {
                position: positions[index],
                texel: texels[index],
                tint,
            });
            let triangles = clipped_sprite_triangles(vertices, [-10.0, -5.0, 20.0, 10.0]);
            assert!(!triangles.is_empty());
            for vertex in triangles.iter().flatten() {
                let [x, y] = vertex.position;
                assert!((-10.0..=10.0).contains(&x) && (-5.0..=5.0).contains(&y));
                let expected = if rotated { [y + 20.0, 20.0 - x] } else { [20.0 - x, y + 20.0] };
                assert_eq!(vertex.texel, expected);
                assert_eq!(vertex.tint, tint);
            }
            assert!(clipped_sprite_triangles(vertices, [100.0, 100.0, 10.0, 10.0]).is_empty());
        }
    }
}
