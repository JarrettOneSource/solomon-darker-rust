//! Source native affine transforms, registered atlas texels and packed vertex colors.

use solomon_sim::Vec2;
use solomon_sim::float;
use solomon_sim::presentation::multiply_native_tints;
use solomon_sim::secondary::presentation::*;
use solomon_sim::spells::air::{NativeAirCoronaPlan, NativeAirMesh};

use super::SecondaryView;
use crate::courtyard::Camera;
use crate::render::{BlendMode, Renderer, Texture, TextureColorMode, Vertex};

pub(super) struct TargetDraw {
    pub center: Vec2,
    pub scale: f64,
    pub alpha: f64,
    pub blend: BlendMode,
    pub tint: [f32; 3],
    pub color_mode: TextureColorMode,
}

pub(super) struct SpriteRecord<'a> {
    pub texture: &'a Texture,
    pub source: [f32; 4],
    pub width: f64,
    pub height: f64,
    pub origin: Vec2,
    pub native_texels: bool,
}

impl SecondaryView {
    pub(super) fn record(&self, atlas: NativeSecondaryAtlas, entry: usize) -> SpriteRecord<'_> {
        if atlas == NativeSecondaryAtlas::Clothes {
            let texture =
                &self.clothes[entry.checked_sub(1).expect("Clothes secondary entry is 1 or 2")];
            return SpriteRecord {
                texture,
                source: [0.0, 0.0, texture.width, texture.height],
                width: f64::from(texture.width),
                height: f64::from(texture.height),
                origin: Vec2::ZERO,
                native_texels: false,
            };
        }
        let atlas = match atlas {
            NativeSecondaryAtlas::BadGuys => &self.bad_guys,
            NativeSecondaryAtlas::DeadHawg => &self.dead_hawg,
            NativeSecondaryAtlas::Golem => &self.golem,
            NativeSecondaryAtlas::Clothes => unreachable!(),
        };
        let record = &atlas.records[entry];
        SpriteRecord {
            texture: &atlas.texture,
            source: [record.x as f32, record.y as f32, record.width as f32, record.height as f32],
            width: f64::from(record.width),
            height: f64::from(record.height),
            origin: Vec2::new(f64::from(record.center_x), f64::from(record.center_y)),
            native_texels: true,
        }
    }

    /// Shared low-level stock submission for secondary and Dampen projectile layers.
    /// The caller keeps the domain transform in its canonical draw plan.
    pub fn draw_sprite(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        root: Vec2,
        draw: &NativeSecondarySpriteDraw,
        region_tint: [f32; 3],
    ) {
        let mode = match draw.color_mode {
            Some(NativeSecondaryColorMode::AlphaMask) => TextureColorMode::WhiteAlphaFilter,
            _ => TextureColorMode::Texture,
        };
        self.draw_sprite_with_color_mode(renderer, camera, root, draw, region_tint, mode);
    }

    /// Stock geometry shared with Goodie and enemy diffuse-hit copies. The native
    /// diffuse pass and Storm's white ColorMatrixFilter are distinct operations.
    pub fn draw_sprite_with_color_mode(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        root: Vec2,
        draw: &NativeSecondarySpriteDraw,
        region_tint: [f32; 3],
        mode: TextureColorMode,
    ) {
        if draw.alpha <= 0.0 {
            return;
        }
        let (record, vertices) = self.sprite_vertices(camera, root, draw, region_tint);
        let triangles =
            [[vertices[0], vertices[1], vertices[2]], [vertices[0], vertices[2], vertices[3]]];
        renderer.draw_triangles_with_color_mode(
            record.texture,
            &triangles,
            blend(draw.blend),
            mode,
        );
    }

    pub(super) fn sprite_vertices(
        &self,
        camera: &Camera,
        root: Vec2,
        draw: &NativeSecondarySpriteDraw,
        tint: [f32; 3],
    ) -> (SpriteRecord<'_>, [Vertex; 4]) {
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
            // `writeNativeRotationThenScaleMatrix`: unequal scales follow rotation.
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
        // Equal Z indices preserve the source container's creation order: mesh,
        // gradients, quad. Sprites begin after all mesh and quad indices.
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
        draw: &NativeSecondaryGradientDraw,
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
        draw: &NativeSecondaryMeshDraw,
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
        draw: &NativeSecondaryQuadDraw,
        tint: [f32; 3],
    ) {
        let record = draw.atlas.zip(draw.entry).map(|(atlas, entry)| self.record(atlas, entry));
        let uv = record.as_ref().map(texels).unwrap_or([[0.0, 0.0]; 4]);
        // Source mesh order is TL, TR, BL, BR, while registered sprite UVs are clockwise.
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
            renderer.draw_triangles(record.texture, &triangles, blend(draw.blend));
        } else {
            renderer.draw_solid_triangles(&triangles, blend(draw.blend));
        }
    }

    fn draw_air_mesh(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        root: Vec2,
        mesh: &NativeAirMesh,
        entry: usize,
        tint: [f32; 4],
    ) {
        let record = self.record(NativeSecondaryAtlas::BadGuys, entry);
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
            record.texture,
            &indexed_triangles(&vertices, &mesh.indices),
            BlendMode::Add,
        );
    }

    fn draw_air_corona(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        root: Vec2,
        corona: &NativeAirCoronaPlan,
    ) {
        let root = Vec2::new(root.x + corona.center.x, root.y + corona.center.y);
        for sprite in &corona.draws {
            let draw = NativeSecondarySpriteDraw {
                alpha: sprite.alpha * corona.alpha,
                atlas: NativeSecondaryAtlas::BadGuys,
                blend: NativeSecondaryBlend::Add,
                color_mode: None,
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
    let color = multiply_native_tints(tint, light);
    [
        ((color >> 16) & 255) as f32 / 255.0,
        ((color >> 8) & 255) as f32 / 255.0,
        (color & 255) as f32 / 255.0,
        (alpha.clamp(0.0, 1.0) * 255.0).trunc() as f32 / 255.0,
    ]
}

pub(super) fn texels(record: &SpriteRecord<'_>) -> [[f32; 2]; 4] {
    let [x, y, width, height] = record.source;
    // `nativeSpriteRecordUvs`: retained half-texel start and quarter-texel end.
    let (left, top, right, bottom) = if record.native_texels {
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

fn blend(value: NativeSecondaryBlend) -> BlendMode {
    match value {
        NativeSecondaryBlend::Normal => BlendMode::Normal,
        NativeSecondaryBlend::Add => BlendMode::Add,
    }
}
