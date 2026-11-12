//! Native target-query and tracking replays, against recordings from a pinned build of
//! the Website client.
use serde::Deserialize;
use solomon_sim::Vec2;
use solomon_sim::spells::targeting::*;

#[derive(Clone, Copy, Deserialize)]
struct Point {
    x: f64,
    y: f64,
}

impl From<Point> for Vec2 {
    fn from(point: Point) -> Self {
        Self::new(point.x, point.y)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TargetRecord {
    id: String,
    position: Point,
    active: bool,
    actor_flags: u32,
    attachment: Point,
    body_radius: f64,
    cell_binding_order: usize,
    #[serde(rename = "nativePriority")]
    priority: i32,
    pending_remove: bool,
    query_lane: String,
    registration_order: usize,
    grid_position: Option<Point>,
    kind: String,
}

impl From<TargetRecord> for PrimarySpellTarget {
    fn from(record: TargetRecord) -> Self {
        Self {
            id: record.id,
            position: record.position.into(),
            active: record.active,
            actor_flags: record.actor_flags,
            attachment: record.attachment.into(),
            body_radius: record.body_radius,
            cell_binding_order: record.cell_binding_order,
            priority: record.priority,
            pending_remove: record.pending_remove,
            query_lane: match record.query_lane.as_str() {
                "grid" => PrimarySpellQueryLane::Grid,
                "transient" => PrimarySpellQueryLane::Transient,
                other => panic!("unknown lane {other}"),
            },
            registration_order: record.registration_order,
            grid_position: record.grid_position.map(Into::into),
            heading_degrees: None,
            kind: match record.kind.as_str() {
                "enemy" => PrimarySpellTargetKind::Enemy,
                "scenery" => PrimarySpellTargetKind::Scenery,
                other => panic!("unknown target kind {other}"),
            },
        }
    }
}

#[derive(Deserialize)]
struct PointSample {
    position: [f64; 2],
    point: Option<String>,
    fire: Option<String>,
}
#[derive(Deserialize)]
struct RangeSample {
    reach: f64,
    ids: Vec<String>,
}
#[derive(Deserialize)]
struct AirSample {
    reach: f64,
    id: Option<String>,
}
#[derive(Deserialize)]
struct PolygonSample {
    vertices: Vec<[f64; 2]>,
    ids: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HomingInput {
    heading_degrees: f64,
    movement_scalar: f64,
    position: Point,
    speed: f64,
    target_position: Option<Point>,
    turn_accumulator: f64,
    turn_input: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MovementResult {
    direction: Point,
    heading_degrees: f64,
    position: Point,
    turn_accumulator: f64,
}
#[derive(Deserialize)]
struct HomingSample {
    input: HomingInput,
    result: MovementResult,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackingResult {
    #[serde(flatten)]
    movement: MovementResult,
    target_id: Option<String>,
    reacquires_target: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackingSample {
    target_id: Option<String>,
    reacquires_target: bool,
    result: TrackingResult,
}
#[derive(Deserialize)]
struct Goldens {
    targets: Vec<TargetRecord>,
    points: Vec<PointSample>,
    cones: Vec<RangeSample>,
    air: Vec<AirSample>,
    roots: Vec<RangeSample>,
    polygon: PolygonSample,
    ether: Option<String>,
    homing: Vec<HomingSample>,
    tracking: Vec<TrackingSample>,
    fan: Vec<(f64, usize, usize, f64, f64)>,
}

fn target_id(target: Option<&PrimarySpellTarget>) -> Option<&str> {
    target.map(|target| target.id.as_str())
}
fn target_ids(targets: Vec<&PrimarySpellTarget>) -> Vec<&str> {
    targets.into_iter().map(|target| target.id.as_str()).collect()
}

fn point_target(
    id: &str,
    position: Vec2,
    cell_binding_order: usize,
    actor_flags: u32,
) -> PrimarySpellTarget {
    PrimarySpellTarget {
        active: true,
        actor_flags,
        attachment: Vec2::ZERO,
        body_radius: 0.0,
        cell_binding_order,
        heading_degrees: None,
        grid_position: None,
        id: id.into(),
        kind: if actor_flags == 0x2 {
            PrimarySpellTargetKind::Enemy
        } else {
            PrimarySpellTargetKind::Scenery
        },
        priority: 0,
        pending_remove: false,
        position,
        query_lane: PrimarySpellQueryLane::Grid,
        registration_order: 0,
    }
}

#[test]
fn point_contacts_keep_cell_priority_ties_and_live_eligibility_across_target_lists() {
    let position = Vec2::new(10.0, 10.0);
    let mut targets = [
        point_target("scenery", position, 7, 0x4),
        point_target("equal-priority-enemy", position, 7, 0x2),
        point_target("inactive", position, 0, 0x2),
        point_target("pending", position, 1, 0x2),
        point_target("other-mask", position, 2, 0x8),
        point_target("other-cell", Vec2::new(110.0, 10.0), 3, 0x2),
        point_target("exact-radius", Vec2::new(16.0, 10.0), 4, 0x2),
    ];
    targets[2].active = false;
    targets[3].pending_remove = true;
    for split in 0..=targets.len() {
        assert_eq!(
            target_id(first_primary_point_contact(PrimaryPointContactQuery {
                actor_mask: 0x6,
                position,
                query_radius: 6.0,
                targets: targets[..split].iter().chain(&targets[split..]),
            })),
            Some("scenery"),
            "target list split {split}",
        );
    }
    targets[1].cell_binding_order = 6;
    assert_eq!(
        target_id(first_primary_point_contact(PrimaryPointContactQuery {
            actor_mask: 0x6,
            position,
            query_radius: 6.0,
            targets: &targets,
        })),
        Some("equal-priority-enemy"),
    );
    targets[1].active = false;
    assert_eq!(
        target_id(first_primary_point_contact(PrimaryPointContactQuery {
            actor_mask: 0x6,
            position,
            query_radius: 6.0,
            targets: targets.iter(),
        })),
        Some("scenery"),
    );
}

#[test]
fn fireball_corridors_consult_later_target_lists_before_accepting_scenery() {
    let position = Vec2::new(10.0, 10.0);
    let mut scenery = [point_target("scenery", Vec2::new(20.0, 10.0), 1, 0x4)];
    let mut enemies = [point_target("hostile-ahead", Vec2::new(210.0, 10.0), 2, 0x2)];
    let query = |scenery: &[PrimarySpellTarget], enemies: &[PrimarySpellTarget]| {
        first_fireball_point_contact(FireballPointContactQuery {
            contact: PrimaryPointContactQuery {
                actor_mask: 0x6,
                position,
                query_radius: 20.0,
                targets: scenery.iter().chain(enemies),
            },
            direction: Vec2::new(1.0, 0.0),
            hostile_corridor_length: 400.0,
        })
        .map(|target| target.id.clone())
    };
    assert_eq!(query(&scenery, &enemies), None);
    enemies[0].pending_remove = true;
    assert_eq!(query(&scenery, &enemies).as_deref(), Some("scenery"));
    enemies[0].pending_remove = false;
    enemies[0].position.y = 40.0;
    assert_eq!(query(&scenery, &enemies).as_deref(), Some("scenery"));
    enemies[0].position.y = 10.0;
    scenery[0].position = Vec2::new(11.0, 10.0);
    assert_eq!(query(&scenery, &enemies).as_deref(), Some("scenery"));
}

fn movement(actual: EtherPrimaryHomingResult, expected: MovementResult) {
    assert_eq!(actual.position, Vec2::from(expected.position));
    assert_eq!(actual.heading_degrees, expected.heading_degrees);
    assert_eq!(actual.turn_accumulator, expected.turn_accumulator);
    // Only the final sin/cos direction uses the platform trigonometry tolerance.
    const TRIG_TOLERANCE: f64 = 1e-9;
    assert!((actual.direction.x - expected.direction.x).abs() < TRIG_TOLERANCE);
    assert!((actual.direction.y - expected.direction.y).abs() < TRIG_TOLERANCE);
}

#[test]
fn spell_contacts_target_retention_and_ether_tracking_match_the_website() {
    let fixture: Goldens =
        serde_json::from_str(include_str!("fixtures/website/spell-targeting-goldens.json"))
            .unwrap();
    let targets: Vec<PrimarySpellTarget> = fixture.targets.into_iter().map(Into::into).collect();
    for sample in fixture.points {
        let contact = PrimaryPointContactQuery {
            actor_mask: 6,
            position: Vec2::new(sample.position[0], sample.position[1]),
            query_radius: 6.0,
            targets: &targets,
        };
        assert_eq!(target_id(first_primary_point_contact(contact)), sample.point.as_deref());
        let fire = FireballPointContactQuery {
            contact,
            direction: Vec2::new(0.0, 1.0),
            hostile_corridor_length: 400.0,
        };
        assert_eq!(target_id(first_fireball_point_contact(fire)), sample.fire.as_deref());
    }
    let clear = |target: &PrimarySpellTarget| target.id != "priority";
    for sample in fixture.cones {
        let query = PrimaryConeQuery {
            actor_mask: 6,
            aim_direction: Vec2::new(0.0, 1.0),
            half_angle_degrees: 15.0,
            has_line_of_sight: &clear,
            origin: Vec2::ZERO,
            reach: sample.reach,
            targets: &targets,
        };
        assert_eq!(target_ids(primary_cone_targets(query)), sample.ids);
    }
    for sample in fixture.air {
        let query = AirPrimaryTargetQuery {
            aim_direction: Vec2::new(0.0, 1.0),
            has_line_of_sight: &clear,
            maximum_range: sample.reach,
            origin: Vec2::ZERO,
            previous_target_id: Some("priority"),
            targets: &targets,
        };
        assert_eq!(target_id(select_air_primary_target(query)), sample.id.as_deref());
    }
    for sample in fixture.roots {
        assert_eq!(
            target_ids(primary_root_targets(Vec2::ZERO, sample.reach, 6, &targets)),
            sample.ids
        );
    }
    let polygon: Vec<_> =
        fixture.polygon.vertices.into_iter().map(|point| Vec2::new(point[0], point[1])).collect();
    assert_eq!(target_ids(primary_polygon_targets(6, &polygon, &targets)), fixture.polygon.ids);
    assert_eq!(
        target_id(select_ether_primary_target(Vec2::ZERO, Vec2::new(0.0, 1.0), &targets)),
        fixture.ether.as_deref()
    );
    for sample in fixture.homing {
        let input = sample.input;
        movement(
            advance_ether_primary_homing(EtherPrimaryHomingInput {
                heading_degrees: input.heading_degrees,
                movement_scalar: input.movement_scalar,
                position: input.position.into(),
                speed: input.speed,
                target_position: input.target_position.map(Into::into),
                turn_accumulator: input.turn_accumulator,
                turn_input: input.turn_input,
            }),
            sample.result,
        );
    }
    for sample in fixture.tracking {
        let result = advance_ether_primary_tracking(EtherPrimaryTrackingInput {
            heading_degrees: 180.0,
            movement_scalar: 1.0,
            position: Vec2::ZERO,
            speed: 3.0,
            target_id: sample.target_id.as_deref(),
            reacquires_target: sample.reacquires_target,
            targets: &targets,
            turn_accumulator: 0.01,
            turn_input: 2.0,
        });
        movement(result.movement, sample.result.movement);
        assert_eq!(result.target_id, sample.result.target_id);
        assert_eq!(result.reacquires_target, sample.result.reacquires_target);
    }
    for (heading, quantity, index, expected_heading, expected_turn_scale) in fixture.fan {
        assert_eq!(missile_fan_heading(heading, quantity, index), expected_heading);
        assert_eq!(missile_fan_turn_scale(index), expected_turn_scale);
    }
}
