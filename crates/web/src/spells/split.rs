//! Deferred native ZAnimSplit owners and clipped Air/channel painter bands.

use solomon_sim::Vec2;
use solomon_sim::painter::{PainterRegistration, native_zanim_split_band_height};

use super::{SpellDraw, SpellPainter, SpellRoot};

#[allow(clippy::too_many_arguments)] // One split owner's bounds, material and painter registration.
pub(super) fn append_split_bands(
    roots: &mut Vec<SpellRoot>,
    registration: PainterRegistration,
    origin: Vec2,
    [minimum, maximum]: [f64; 2],
    enhanced_effects: bool,
    region_light_point: Option<Vec2>,
    overlay_owner_id: Option<String>,
    draws: impl Fn(f64, f64) -> Vec<SpellDraw>,
) {
    let mut owner = registration.layer(origin.y + minimum, 0.0);
    owner.visible = false;
    roots.push(SpellRoot {
        painter: SpellPainter {
            layer: owner,
            region_light_point,
            overlay_owner_id,
            view_id: 0,
            view_creation_order: 0,
        },
        draws: Vec::new(),
    });
    let height = native_zanim_split_band_height(enhanced_effects);
    let count = ((maximum - minimum) / height).ceil() as usize;
    for band in 0..count {
        let top = minimum + band as f64 * height;
        let bottom = top + height;
        let mut layer = registration.layer(origin.y + bottom, 0.0);
        layer.is_insertion = true;
        roots.push(SpellRoot {
            painter: SpellPainter {
                layer,
                region_light_point,
                overlay_owner_id: None,
                view_id: 0,
                view_creation_order: 0,
            },
            draws: draws(top, bottom),
        });
    }
}

#[cfg(test)]
mod tests {
    use solomon_sim::boneyard::presentation::{BoneyardPainterEntry, boneyard_painter_order};
    use solomon_sim::hub::painter::{PainterLane, PainterLayer, painter_row};
    use solomon_sim::painter::WorldManagerLane;
    use solomon_sim::spells::air::NativeAirMesh;
    use solomon_sim::spells::weld::NativeWeldMeshUvSpace;

    use crate::native_mesh::clip_native_mesh;

    use super::*;

    fn emitted_beam(enhanced_effects: bool) -> Vec<SpellRoot> {
        // Independent 100-unit vertical strip, wide enough to cross both sides
        // of the native 10,000-unit horizontal clip. UVs span the entire strip.
        let mesh = NativeAirMesh {
            vertices: vec![
                Vec2::new(-120.0, 0.0),
                Vec2::new(10_120.0, 0.0),
                Vec2::new(-120.0, 100.0),
                Vec2::new(10_120.0, 100.0),
            ],
            uvs: vec![Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0), Vec2::new(1.0, 1.0)],
            indices: vec![0, 1, 2, 1, 3, 2],
        };
        let origin = Vec2::new(100.0, 200.0);
        let mut roots = Vec::new();
        append_split_bands(
            &mut roots,
            PainterRegistration {
                manager_lane: WorldManagerLane::Transient,
                registration_ordinal: 7,
            },
            origin,
            [0.0, 100.0],
            enhanced_effects,
            None,
            Some("wizard".into()),
            |top, bottom| {
                vec![SpellDraw::Mesh {
                    record: 44,
                    triangles: clip_native_mesh(&mesh, origin, top, bottom),
                    alpha: 1.0,
                    tint: 0x8a_ffff,
                    uv_space: NativeWeldMeshUvSpace::Record,
                }]
            },
        );
        roots
    }

    fn assert_emission(
        roots: &[SpellRoot],
        bands: &[[f64; 2]],
        expected_rows: &[i64],
        expected_order: &[usize],
    ) {
        assert_eq!(roots.len(), bands.len() + 1, "one invisible owner plus the native bands");
        let owner = &roots[0];
        assert!(!owner.painter.layer.visible);
        assert!(!owner.painter.layer.is_insertion);
        assert_eq!(owner.painter.layer.world_y, 200.0);
        assert_eq!(owner.painter.overlay_owner_id.as_deref(), Some("wizard"));
        assert!(owner.draws.is_empty());
        let mut entries = vec![BoneyardPainterEntry {
            draw_id: 100,
            layer: PainterLayer {
                visible: true,
                is_insertion: false,
                manager_lane: PainterLane::Scenery,
                registration_ordinal: 4,
                world_y: 237.0,
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
        }
        assert_eq!(boneyard_painter_order(&entries, 200.0), expected_order);
        assert_eq!(
            roots[1..]
                .iter()
                .map(|root| painter_row(root.painter.layer.world_y, 0.0, 200.0))
                .collect::<Vec<_>>(),
            expected_rows,
        );
        for (root, [top, bottom]) in roots[1..].iter().zip(bands) {
            assert!(root.painter.layer.visible && root.painter.layer.is_insertion);
            assert_eq!(root.painter.layer.registration_ordinal, 7);
            assert_eq!(root.painter.layer.world_y, *bottom, "clip bottom owns the painter row");
            assert_eq!(root.painter.overlay_owner_id, None);
            let SpellDraw::Mesh { triangles, record, .. } = &root.draws[0] else {
                panic!("beam mesh");
            };
            assert_eq!(*record, 44);
            let vertices: Vec<_> = triangles.iter().flatten().collect();
            let bounds = vertices.iter().fold(
                [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY],
                |[left, upper, right, lower], vertex| {
                    [
                        left.min(vertex.position.x),
                        upper.min(vertex.position.y),
                        right.max(vertex.position.x),
                        lower.max(vertex.position.y),
                    ]
                },
            );
            assert_eq!(bounds, [0.0, *top, 10_000.0, *bottom]);
            for vertex in vertices {
                assert!((vertex.uv.x - (vertex.position.x + 20.0) / 10_240.0).abs() < 1e-12);
                assert!((vertex.uv.y - (vertex.position.y - 200.0) / 100.0).abs() < 1e-12);
                assert_eq!(vertex.color, [1.0; 4]);
            }
        }
    }

    #[test]
    fn enhanced_beam_bands_keep_the_upper_slice_behind_crossing_scenery() {
        let roots = emitted_beam(true);
        assert_emission(
            &roots,
            &[[200.0, 225.0], [225.0, 250.0], [250.0, 275.0], [275.0, 300.0]],
            &[12, 25, 37, 50],
            &[1, 100, 2, 3, 4],
        );
    }

    #[test]
    fn standard_beam_bands_use_fifty_unit_rows_at_the_same_scenery_crossing() {
        let roots = emitted_beam(false);
        assert_emission(&roots, &[[200.0, 250.0], [250.0, 300.0]], &[25, 50], &[100, 1, 2]);
    }
}
