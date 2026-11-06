//! Native beam and web geometry submitted through the shared triangle renderer.

use solomon_sim::Vec2;
use solomon_sim::enemies::actor_presentation::{EnemyArtAtlas, EnemyArtBlend, EnemySpriteLayer};
use solomon_sim::enemies::lightning_presentation::mage_lightning_pulse_plan;
use solomon_sim::enemies::presentation::EnemyProjectileStreak;
use solomon_sim::enemies::web_presentation::EnemyWebMesh;
use solomon_sim::enemies::world::EnemyWorld;
use solomon_sim::float::{fround, hypot};
use solomon_sim::painter::{PainterRegistration, zanim_split_band_height};
use solomon_sim::spells::air::{AirCoronaPlan, AirMesh, AirRibbonLayer};
use solomon_sim::spells::presentation::{SpellAtlas, SpellBlend};

use super::art::{blend, color, project};
use super::{EnemyDraw, EnemyDrawGroup, EnemyPainter, EnemyRoot, EnemyView, EnemyViewFrame};
use crate::beam_mesh::{MeshVertex, clip_to_bands};
use crate::courtyard::Camera;
use solomon_graphics::{BlendMode, Renderer, Vertex};

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
                    // These channels are stored as packed bytes, so truncate to 1/255 steps.
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
        roots: &mut Vec<EnemyRoot>,
        position: Vec2,
        layers: &[AirRibbonLayer],
        registration: PainterRegistration,
        enhanced_effects: bool,
    ) {
        let mode = if layers.first().is_some_and(|layer| layer.texture_record == 64) {
            EnemyArtBlend::Normal
        } else {
            EnemyArtBlend::Add
        };
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
        roots.push(EnemyRoot {
            painter: EnemyPainter { layer: owner, region_light_point: None },
            group: EnemyDrawGroup::at(position),
        });
        let height = zanim_split_band_height(enhanced_effects);
        let count = ((maximum - minimum) / height).ceil() as usize;
        for band in 0..count {
            let top = minimum + band as f64 * height;
            let bottom = top + height;
            let mut group = EnemyDrawGroup::at(position);
            for layer in layers {
                append_ribbon(&mut group, layer, position, Some((top, bottom)), mode);
            }
            let mut layer = registration.layer(position.y + bottom, 0.0);
            layer.is_insertion = true;
            roots.push(EnemyRoot {
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
            let Some(plan) = mage_lightning_pulse_plan(pulse, frame.tick) else { continue };
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

fn corona_group(corona: AirCoronaPlan, origin: Vec2) -> EnemyDrawGroup {
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
    layer: &AirRibbonLayer,
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
    mesh: &AirMesh,
    origin: Vec2,
    bounds: Option<(f64, f64)>,
) -> Vec<[MeshVertex; 3]> {
    if let Some((top, bottom)) = bounds {
        return clip_to_bands(mesh, origin, top, bottom);
    }
    mesh.indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|indices| {
            indices.map(|index| {
                let index = index as usize;
                let point = mesh.vertices[index];
                MeshVertex {
                    position: Vec2::new(origin.x + point.x, origin.y + point.y),
                    uv: mesh.uvs[index],
                    color: [1.0; 4],
                }
            })
        })
        .collect()
}

pub(super) fn arrow_streak(streak: EnemyProjectileStreak, origin: Vec2) -> EnemyDraw {
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

#[cfg(test)]
mod tests {
    use solomon_sim::boneyard::presentation::{BoneyardPainterEntry, boneyard_painter_order};
    use solomon_sim::enemies::boss_spell::{BossSpell, BossSpellKind, BossSpellOwner};
    use solomon_sim::enemies::lightning_presentation::dark_lightning_body;
    use solomon_sim::hub::painter::{PainterLane, PainterLayer};
    use solomon_sim::painter::WorldManagerLane;

    use super::*;

    #[test]
    fn faculty_live_split_quality_changes_the_scenery_crossing_and_keeps_born_ribbons() {
        for born_enhanced in [true, false] {
            let spell = BossSpell {
                owner: BossSpellOwner {
                    age_ticks: 0,
                    damage: 0.0,
                    id: 7,
                    owner_actor_id: 1,
                    painter_registration: PainterRegistration {
                        manager_lane: WorldManagerLane::Transient,
                        registration_ordinal: 17,
                    },
                    position: Vec2::new(1000.0, 2000.0),
                    spawn_tick: 31,
                },
                kind: BossSpellKind::Blightning {
                    endpoint: Vec2::new(1100.0, 1600.0),
                    midpoint: Vec2::new(1050.0, 1800.0),
                    enhanced_effects: born_enhanced,
                },
            };
            let layers = dark_lightning_body(&spell);
            let original = layers.clone();
            let mut enhanced_content = None;
            for live_enhanced in [true, false, true] {
                let mut roots = Vec::new();
                EnemyView::append_lightning_body(
                    &mut roots,
                    spell.owner.position,
                    &layers,
                    spell.owner.painter_registration,
                    live_enhanced,
                );
                let owner_y = roots[0].painter.layer.world_y;
                let height = if live_enhanced { 25.0 } else { 50.0 };
                assert_eq!(
                    roots[1].painter.layer.world_y - owner_y,
                    height,
                    "born={born_enhanced}, live={live_enhanced}"
                );
                let mut entries = vec![BoneyardPainterEntry {
                    draw_id: usize::MAX,
                    layer: PainterLayer {
                        visible: true,
                        is_insertion: false,
                        manager_lane: PainterLane::Scenery,
                        registration_ordinal: 4,
                        world_y: owner_y + 37.0,
                        sort_bias: 0.0,
                    },
                    proxy: None,
                }];
                for (index, root) in roots.iter().enumerate() {
                    entries.push(BoneyardPainterEntry {
                        draw_id: index,
                        layer: root.painter.layer,
                        proxy: None,
                    });
                    assert_eq!(root.painter.layer.registration_ordinal, 17);
                    assert_eq!(root.painter.layer.manager_lane, PainterLane::Transient);
                }
                let mut expected: Vec<_> = (1..roots.len()).collect();
                expected.insert(usize::from(live_enhanced), usize::MAX);
                assert_eq!(boneyard_painter_order(&entries, owner_y), expected);
                let mut content = Vec::new();
                for root in &roots[1..] {
                    assert!(root.painter.layer.visible && root.painter.layer.is_insertion);
                    let mut source = layers.iter().flat_map(|layer| {
                        std::iter::once((layer.texture_record, layer.alpha, layer.tint)).chain(
                            layer
                                .branch
                                .iter()
                                .map(|branch| (branch.texture_record, layer.alpha, layer.tint)),
                        )
                    });
                    for draw in &root.group.draws {
                        let EnemyDraw::AtlasMesh { entry, alpha, tint, blend, triangles, .. } =
                            draw
                        else {
                            panic!("Faculty ribbon")
                        };
                        assert_eq!((*entry, *alpha, *tint), source.next().unwrap());
                        assert_eq!(*blend, EnemyArtBlend::Normal);
                        for triangle in triangles {
                            content.push(
                                triangle.map(|vertex| (vertex.position, vertex.uv, vertex.color)),
                            );
                            for vertex in triangle {
                                assert!(vertex.position.y <= root.painter.layer.world_y + 1e-9);
                                assert!(
                                    vertex.position.y >= root.painter.layer.world_y - height - 1e-9
                                );
                                assert_eq!(vertex.color, [1.0; 4]);
                            }
                        }
                    }
                    assert!(source.next().is_none());
                }
                if live_enhanced {
                    if let Some(first) = &enhanced_content {
                        assert_eq!(&content, first);
                    } else {
                        enhanced_content = Some(content);
                    }
                }
                assert_eq!(layers, original, "live quality must not reroll Faculty ribbons");
            }
        }
    }
}
