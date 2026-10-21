//! Native beam and web geometry submitted through the shared triangle renderer.

use solomon_sim::Vec2;
use solomon_sim::enemies::actor_presentation::{EnemyArtAtlas, EnemyArtBlend, EnemySpriteLayer};
use solomon_sim::enemies::lightning_presentation::native_mage_lightning_pulse_plan;
use solomon_sim::enemies::presentation::NativeEnemyProjectileStreak;
use solomon_sim::enemies::web_presentation::EnemyWebMesh;
use solomon_sim::enemies::world::EnemyWorld;
use solomon_sim::float::{fround, hypot};
use solomon_sim::painter::PainterRegistration;
use solomon_sim::spells::air::{NativeAirCoronaPlan, NativeAirMesh, NativeAirRibbonLayer};
use solomon_sim::spells::presentation::{SpellAtlas, SpellBlend};

use super::art::{blend, color, project};
use super::{EnemyDraw, EnemyDrawGroup, EnemyPainter, EnemyRoot, EnemyView, EnemyViewFrame};
use crate::courtyard::Camera;
use crate::native_mesh::{NativeMeshVertex, clip_native_mesh};
use crate::render::{BlendMode, Renderer, Vertex};

impl EnemyView {
    pub(super) fn draw_mesh(&self, renderer: &mut Renderer, camera: &Camera, draw: &EnemyDraw) {
        match draw {
            EnemyDraw::AtlasMesh { atlas, entry, triangles, alpha, tint, blend: mode } => {
                let (texture, record) = self.art.sprite(*atlas, *entry);
                let tint = color(*tint, *alpha, [1.0; 3]);
                let triangles: Vec<_> = triangles
                    .iter()
                    .map(|triangle| {
                        triangle.map(|vertex| Vertex {
                            position: project(camera, Vec2::ZERO, vertex.position),
                            texel: [
                                record.x as f32 + vertex.uv.x as f32 * record.width as f32,
                                record.y as f32 + vertex.uv.y as f32 * record.height as f32,
                            ],
                            tint,
                        })
                    })
                    .collect();
                renderer.draw_triangles(texture, &triangles, blend(*mode));
            }
            EnemyDraw::Web(mesh) => {
                let triangles: Vec<_> = mesh
                    .indices
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|indices| {
                        indices.map(|index| {
                            let index = index as usize;
                            Vertex {
                                position: project(camera, Vec2::ZERO, mesh.vertices[index]),
                                texel: [0.0, 0.0],
                                tint: unpack_color(mesh.colors[index]),
                            }
                        })
                    })
                    .collect();
                renderer.draw_solid_triangles(&triangles, BlendMode::Normal);
            }
            EnemyDraw::MouthBeam { vertices, uv_offset, start_alpha, end_alpha } => {
                let uv = [
                    Vec2::new(0.0, *uv_offset),
                    Vec2::new(1.0, *uv_offset),
                    Vec2::new(0.0, uv_offset + 1.0),
                    Vec2::new(1.0, uv_offset + 1.0),
                ];
                let vertices = [0, 1, 2, 3].map(|index| {
                    let alpha = if index < 2 { *start_alpha } else { *end_alpha };
                    // The source stores these channels through nativePackedColor.
                    let alpha = (alpha.clamp(0.0, 1.0) * 255.0).trunc() / 255.0;
                    Vertex {
                        position: project(camera, Vec2::ZERO, vertices[index]),
                        texel: [
                            uv[index].x as f32 * self.green_plasma.width,
                            uv[index].y as f32 * self.green_plasma.height,
                        ],
                        tint: [1.0, 1.0, 1.0, alpha as f32],
                    }
                });
                renderer.draw_triangles(
                    &self.green_plasma,
                    &[
                        [vertices[0], vertices[1], vertices[2]],
                        [vertices[1], vertices[3], vertices[2]],
                    ],
                    BlendMode::Add,
                );
            }
            EnemyDraw::Sprite { .. }
            | EnemyDraw::Segment(_)
            | EnemyDraw::Underlay(_)
            | EnemyDraw::Gradient(_) => {
                unreachable!("ordinary enemy draws are submitted by the sprite compositor")
            }
        }
    }

    pub(super) fn append_lightning_body(
        &mut self,
        position: Vec2,
        layers: &[NativeAirRibbonLayer],
        registration: PainterRegistration,
        split: bool,
    ) {
        let mode = if layers.first().is_some_and(|layer| layer.texture_record == 64) {
            EnemyArtBlend::Normal
        } else {
            EnemyArtBlend::Add
        };
        if !split {
            let mut group = EnemyDrawGroup::at(position);
            for layer in layers {
                append_ribbon(&mut group, layer, position, None, mode);
            }
            self.push_root(registration, position.y, 0.0, None, group);
            return;
        }
        let mut minimum = f64::INFINITY;
        let mut maximum = f64::NEG_INFINITY;
        for layer in layers {
            for point in &layer.mesh.vertices {
                minimum = minimum.min(point.y);
                maximum = maximum.max(point.y);
            }
            if let Some(branch) = &layer.branch {
                for point in &branch.mesh.vertices {
                    minimum = minimum.min(point.y);
                    maximum = maximum.max(point.y);
                }
            }
        }
        let mut owner = registration.layer(position.y + minimum, 0.0);
        owner.visible = false;
        self.roots.push(EnemyRoot {
            painter: EnemyPainter { layer: owner, region_light_point: None },
            group: EnemyDrawGroup::at(position),
        });
        let count = ((maximum - minimum) / 25.0).ceil() as usize;
        for band in 0..count {
            let top = minimum + band as f64 * 25.0;
            let bottom = top + 25.0;
            let mut group = EnemyDrawGroup::at(position);
            for layer in layers {
                append_ribbon(&mut group, layer, position, Some((top, bottom)), mode);
            }
            let mut layer = registration.layer(position.y + bottom, 0.0);
            layer.is_insertion = true;
            self.roots.push(EnemyRoot {
                painter: EnemyPainter { layer, region_light_point: None },
                group,
            });
        }
    }

    pub(super) fn prepare_mage_lightning(
        &mut self,
        world: &EnemyWorld,
        frame: &EnemyViewFrame<'_>,
    ) {
        for pulse in &world.mage_lightning_pulses {
            let Some(plan) = native_mage_lightning_pulse_plan(pulse, frame.tick) else { continue };
            if let Some(layers) = plan.body {
                let mut group = EnemyDrawGroup::at(plan.source);
                for layer in &layers {
                    append_ribbon(&mut group, layer, plan.source, None, EnemyArtBlend::Add);
                }
                self.push_root(pulse.painter_registrations[0], plan.midpoint.y, 0.0, None, group);
            }
            if let Some(corona) = plan.source_corona {
                let group = corona_group(corona, plan.source);
                self.push_root(pulse.painter_registrations[1], plan.source.y, 0.0, None, group);
            }
            if let Some(target_id) = &plan.contact_target_player_id {
                if let Some(target) = frame.players.iter().find(|player| player.id == target_id) {
                    self.late_world.push(corona_group(plan.contact, target.position));
                }
            } else {
                let y = plan.contact.center.y;
                let group = corona_group(plan.contact, Vec2::ZERO);
                self.push_root(pulse.painter_registrations[2], y, 0.0, None, group);
            }
        }
    }
}

fn corona_group(corona: NativeAirCoronaPlan, origin: Vec2) -> EnemyDrawGroup {
    let mut group =
        EnemyDrawGroup::at(Vec2::new(origin.x + corona.center.x, origin.y + corona.center.y));
    for draw in corona.draws {
        let layer = EnemySpriteLayer {
            alpha: draw.alpha * corona.alpha,
            blend: match draw.blend {
                SpellBlend::Normal => EnemyArtBlend::Normal,
                SpellBlend::Add => EnemyArtBlend::Add,
            },
            offset: draw.offset,
            rotation_radians: draw.rotation,
            scale: draw.scale.x,
            scale_y: Some(draw.scale.y),
            tint: draw.tint,
            ..EnemySpriteLayer::new(
                match draw.atlas {
                    SpellAtlas::BadGuys => EnemyArtAtlas::BadGuys,
                    SpellAtlas::DeadHawg => EnemyArtAtlas::DeadHawg,
                },
                draw.record,
                "mage-lightning-corona",
            )
        };
        group.draws.push(EnemyDraw::Sprite { layer, region_tint: false });
    }
    group
}

fn append_ribbon(
    group: &mut EnemyDrawGroup,
    layer: &NativeAirRibbonLayer,
    origin: Vec2,
    bounds: Option<(f64, f64)>,
    blend: EnemyArtBlend,
) {
    group.draws.push(EnemyDraw::AtlasMesh {
        atlas: EnemyArtAtlas::BadGuys,
        entry: layer.texture_record,
        triangles: mesh_triangles(&layer.mesh, origin, bounds),
        alpha: layer.alpha,
        tint: layer.tint,
        blend,
    });
    if let Some(branch) = &layer.branch {
        group.draws.push(EnemyDraw::AtlasMesh {
            atlas: EnemyArtAtlas::BadGuys,
            entry: branch.texture_record,
            triangles: mesh_triangles(&branch.mesh, origin, bounds),
            alpha: layer.alpha,
            tint: layer.tint,
            blend,
        });
    }
}

fn mesh_triangles(
    mesh: &NativeAirMesh,
    origin: Vec2,
    bounds: Option<(f64, f64)>,
) -> Vec<[NativeMeshVertex; 3]> {
    if let Some((top, bottom)) = bounds {
        return clip_native_mesh(mesh, origin, top, bottom);
    }
    mesh.indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|indices| {
            indices.map(|index| {
                let index = index as usize;
                let point = mesh.vertices[index];
                NativeMeshVertex {
                    position: Vec2::new(origin.x + point.x, origin.y + point.y),
                    uv: mesh.uvs[index],
                    color: [1.0; 4],
                }
            })
        })
        .collect()
}

pub(super) fn arrow_streak(streak: NativeEnemyProjectileStreak, origin: Vec2) -> EnemyDraw {
    let dx = streak.end.x - streak.start.x;
    let dy = streak.end.y - streak.start.y;
    let length = hypot(dx, dy);
    let half = if length == 0.0 { 0.0 } else { streak.width / (2.0 * length) };
    let normal = Vec2::new(-dy * half, dx * half);
    let points = [
        Vec2::new(streak.start.x - normal.x, streak.start.y - normal.y),
        Vec2::new(streak.start.x + normal.x, streak.start.y + normal.y),
        Vec2::new(streak.end.x - normal.x, streak.end.y - normal.y),
        Vec2::new(streak.end.x + normal.x, streak.end.y + normal.y),
    ];
    let alpha = (streak.alpha * 255.0).round() as u32;
    EnemyDraw::Web(EnemyWebMesh {
        vertices: points
            .map(|point| Vec2::new(origin.x + fround(point.x), origin.y + fround(point.y)))
            .to_vec(),
        colors: vec![0x808080 | alpha << 24, 0x808080 | alpha << 24, 0x808080, 0x808080],
        indices: vec![0, 1, 2, 2, 1, 3],
    })
}

fn unpack_color(color: u32) -> [f32; 4] {
    [
        (color & 255) as f32 / 255.0,
        ((color >> 8) & 255) as f32 / 255.0,
        ((color >> 16) & 255) as f32 / 255.0,
        ((color >> 24) & 255) as f32 / 255.0,
    ]
}
