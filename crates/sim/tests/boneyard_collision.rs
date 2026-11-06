use solomon_sim::Vec2;
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::collision::{
    BoneyardCollisionWorld, CollisionCircle, CollisionPolygon, CollisionSegment,
    can_place_boneyard_body, can_place_boneyard_body_ignoring, first_boneyard_path_block_progress,
    resolve_boneyard_movement, resolve_boneyard_movement_ignoring,
};

#[test]
fn ignored_scenery_matches_removing_only_its_owned_shapes() {
    let circles = vec![CollisionCircle {
        center: Vec2::new(-4.0, 0.0),
        radius: 3.0,
        line_mask: 0,
        source_id: Some("grave-circle".into()),
    }];
    let polygons = vec![CollisionPolygon {
        points: vec![
            Vec2::new(-2.0, -1.0),
            Vec2::new(7.0, -1.0),
            Vec2::new(7.0, 2.0),
            Vec2::new(-2.0, 2.0),
        ],
        line_mask: 0,
        source_id: Some("grave-polygon".into()),
    }];
    let segments = vec![
        CollisionSegment {
            start: Vec2::new(2.0, -8.0),
            end: Vec2::new(2.0, 8.0),
            radius: 1.0,
            line_mask: 0,
            source_id: Some("grave-segment".into()),
        },
        CollisionSegment {
            start: Vec2::new(9.0, -8.0),
            end: Vec2::new(9.0, 8.0),
            radius: 0.0,
            line_mask: 0,
            source_id: None,
        },
    ];
    let world = BoneyardCollisionWorld::from_primitives(
        circles.clone(),
        polygons.clone(),
        segments.clone(),
    );
    let bounds = BoneyardBounds::new(-20.0, -20.0, 40.0, 40.0);
    for selection in 0..8 {
        let mut ignored: Vec<_> = ["grave-circle", "grave-polygon", "grave-segment"]
            .into_iter()
            .enumerate()
            .filter_map(|(index, id)| (selection & (1 << index) != 0).then_some(id))
            .collect();
        if selection == 7 {
            ignored.extend(["unknown-source", "grave-circle", "grave-circle"]);
        }
        let retained = |id: &Option<String>| id.as_deref().is_none_or(|id| !ignored.contains(&id));
        let filtered = BoneyardCollisionWorld::from_primitives(
            circles.iter().filter(|shape| retained(&shape.source_id)).cloned().collect(),
            polygons.iter().filter(|shape| retained(&shape.source_id)).cloned().collect(),
            segments.iter().filter(|shape| retained(&shape.source_id)).cloned().collect(),
        );
        for radius in [0.0, 1.0, 3.0] {
            for x in [-12.0, -5.0, 0.0, 2.0, 8.0, 12.0] {
                for y in [-8.0, 0.0, 8.0] {
                    let start = Vec2::new(-12.0, -8.0);
                    let end = Vec2::new(x, y);
                    assert_eq!(
                        can_place_boneyard_body_ignoring(end, bounds, &world, radius, &ignored),
                        can_place_boneyard_body(end, bounds, &filtered, radius),
                    );
                    assert_eq!(
                        first_boneyard_path_block_progress(
                            start, end, bounds, &world, radius, &ignored,
                        ),
                        first_boneyard_path_block_progress(
                            start,
                            end,
                            bounds,
                            &filtered,
                            radius,
                            &[],
                        ),
                    );
                    assert_eq!(
                        resolve_boneyard_movement_ignoring(
                            start, end, bounds, &world, radius, &ignored,
                        ),
                        resolve_boneyard_movement(start, end, bounds, &filtered, radius),
                    );
                }
            }
        }
    }
}
