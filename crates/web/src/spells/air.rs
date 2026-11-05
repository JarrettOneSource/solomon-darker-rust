//! Air's immutable born ribbons composed into the live native split queue.

use solomon_sim::spells::air::NativeAirRibbonLayer;
use solomon_sim::spells::model::PrimarySpellAirTransient;
use solomon_sim::spells::weld::NativeWeldMeshUvSpace;

use super::split::append_split_bands;
use super::{SpellDraw, SpellRoot};
use crate::native_mesh::clip_native_mesh;

pub(super) fn append_air_body(
    roots: &mut Vec<SpellRoot>,
    spell: &PrimarySpellAirTransient,
    layers: &[NativeAirRibbonLayer],
    enhanced_effects: bool,
) {
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
    append_split_bands(
        roots,
        spell.common.painter_registrations[0],
        spell.origin,
        [minimum, maximum],
        enhanced_effects,
        None,
        None,
        |top, bottom| {
            let mut draws = Vec::new();
            for layer in layers {
                draws.push(SpellDraw::Mesh {
                    record: layer.texture_record,
                    triangles: clip_native_mesh(&layer.mesh, spell.origin, top, bottom),
                    alpha: layer.alpha,
                    tint: layer.tint,
                    uv_space: NativeWeldMeshUvSpace::Record,
                });
                if let Some(branch) = &layer.branch {
                    draws.push(SpellDraw::Mesh {
                        record: branch.texture_record,
                        triangles: clip_native_mesh(&branch.mesh, spell.origin, top, bottom),
                        alpha: layer.alpha,
                        tint: layer.tint,
                        uv_space: NativeWeldMeshUvSpace::Record,
                    });
                }
            }
            draws
        },
    );
}

#[cfg(test)]
mod tests {
    use solomon_sim::Vec2;
    use solomon_sim::boneyard::presentation::{BoneyardPainterEntry, boneyard_painter_order};
    use solomon_sim::hub::painter::{PainterLane, PainterLayer};
    use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
    use solomon_sim::spells::air::{NativeAirLightningInput, build_native_air_lightning_body};
    use solomon_sim::spells::model::PrimarySpellTransientBase;

    use super::*;

    #[test]
    fn air_live_split_quality_changes_the_scenery_crossing_and_keeps_born_ribbons() {
        for born_enhanced in [true, false] {
            for (chained, underpowered) in [(false, false), (true, false), (false, true)] {
                let registration = PainterRegistration {
                    manager_lane: WorldManagerLane::Transient,
                    registration_ordinal: 7,
                };
                let spell = PrimarySpellAirTransient {
                    common: PrimarySpellTransientBase {
                        age_ticks: 0,
                        id: 17,
                        light_registration: None,
                        owner_id: "wizard".into(),
                        painter_registrations: vec![registration],
                        world_key: "boneyard:test".into(),
                    },
                    chained,
                    enhanced_effects: born_enhanced,
                    birth_tick: 31,
                    direction: Vec2::new(0.0, -1.0),
                    endpoint: Vec2::new(1100.0, 1600.0),
                    hurricane_charge: 0.0,
                    midpoint: Vec2::new(1050.0, 1800.0),
                    origin: Vec2::new(1000.0, 2000.0),
                    target_id: None,
                    underpowered,
                    variant: 0,
                };
                let body = build_native_air_lightning_body(NativeAirLightningInput {
                    chained,
                    enhanced_effects: born_enhanced,
                    age_ticks: 0,
                    birth_tick: spell.birth_tick,
                    endpoint: Vec2::new(100.0, -400.0),
                    id: spell.common.id,
                    midpoint: Vec2::new(50.0, -200.0),
                    underpowered,
                })
                .expect("age-zero Air owns its born body");
                let original = body.clone();
                let mut enhanced_content = None;
                for live_enhanced in [true, false, true] {
                    let mut roots = Vec::new();
                    append_air_body(&mut roots, &spell, &body, live_enhanced);
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
                        assert_eq!(root.painter.layer.registration_ordinal, 7);
                        assert_eq!(root.painter.layer.manager_lane, PainterLane::Transient);
                    }
                    let mut expected: Vec<_> = (1..roots.len()).collect();
                    expected.insert(usize::from(live_enhanced), usize::MAX);
                    assert_eq!(boneyard_painter_order(&entries, owner_y), expected);

                    let mut content = Vec::new();
                    for root in &roots[1..] {
                        assert!(root.painter.layer.visible && root.painter.layer.is_insertion);
                        let mut source =
                            body.iter().flat_map(|layer| {
                                std::iter::once((layer.texture_record, layer.alpha, layer.tint))
                                    .chain(layer.branch.iter().map(|branch| {
                                        (branch.texture_record, layer.alpha, layer.tint)
                                    }))
                            });
                        for draw in &root.draws {
                            let SpellDraw::Mesh { record, alpha, tint, triangles, uv_space } = draw
                            else {
                                panic!("Air body mesh")
                            };
                            assert_eq!((*record, *alpha, *tint), source.next().unwrap());
                            assert_eq!(*uv_space, NativeWeldMeshUvSpace::Record);
                            for triangle in triangles {
                                content.push(
                                    triangle
                                        .map(|vertex| (vertex.position, vertex.uv, vertex.color)),
                                );
                                for vertex in triangle {
                                    assert!(vertex.position.y <= root.painter.layer.world_y + 1e-9);
                                    assert!(
                                        vertex.position.y
                                            >= root.painter.layer.world_y - height - 1e-9
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
                    assert_eq!(body, original, "live quality must not alter born geometry");
                    assert_eq!(spell.enhanced_effects, born_enhanced);
                }
            }
        }
    }
}
