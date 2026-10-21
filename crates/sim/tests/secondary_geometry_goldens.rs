//! Native Teleport, expanding placement rings and Phase probe boundaries.

use serde_json::{Value, json};
use solomon_sim::boneyard::collision::BoneyardCollisionWorld;
use solomon_sim::boneyard::{BoneyardBounds, BoneyardScene, BoneyardSpawn};
use solomon_sim::secondary::geometry::*;
use solomon_sim::{NativeRng, Vec2};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn compare(position: Vec2, rng: &NativeRng, expected: &Value, label: &str) {
    assert_eq!(position, point(&expected["position"]), "{label} position");
    assert_eq!(
        rng.index_a,
        expected["rng"]["indexA"].as_u64().unwrap() as usize,
        "{label} RNG cursor A"
    );
    assert_eq!(
        rng.index_b,
        expected["rng"]["indexB"].as_u64().unwrap() as usize,
        "{label} RNG cursor B"
    );
    assert_eq!(json!(&rng.words[..]), expected["rng"]["words"], "{label} RNG words");
}

#[test]
fn native_collision_rings_and_teleport_replay_the_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/secondary-geometry-goldens.json"))
            .unwrap();
    for case in fixture["rings"].as_array().unwrap() {
        let origin = point(&case["requested"]);
        let minimum_x = case["minimumX"].as_f64().unwrap();
        let blocked = case["blockedRadius"].as_f64().unwrap();
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let result = resolve_native_collision_adjusted_position(
            origin,
            case["radius"].as_f64().unwrap(),
            &mut rng,
            |position| {
                let dx = position.x - origin.x;
                let dy = position.y - origin.y;
                position.x >= minimum_x && dx * dx + dy * dy >= blocked * blocked
            },
        );
        compare(result, &rng, &case["result"], "placement ring");
    }
    for case in fixture["teleports"].as_array().unwrap() {
        let value = &case["bounds"];
        let bounds = BoneyardBounds::new(
            value["x"].as_f64().unwrap(),
            value["y"].as_f64().unwrap(),
            value["w"].as_f64().unwrap(),
            value["h"].as_f64().unwrap(),
        );
        let scene = BoneyardScene {
            name: "Placement oracle".into(),
            environment_mode: 0,
            bounds,
            spawn: BoneyardSpawn { position: Vec2::ZERO, facing_degrees: 0.0 },
            objects: Vec::new(),
            sprites: Vec::new(),
            roads: Vec::new(),
            fences: Vec::new(),
            terrain: Vec::new(),
            solomon_dig: None,
        };
        let collision = BoneyardCollisionWorld::from_scene(&scene);
        let bodies = case["bodies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| BoneyardNativeTeleportBody {
                position: point(&value["position"]),
                radius: value["radius"].as_f64().unwrap(),
            })
            .collect::<Vec<_>>();
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let position = resolve_boneyard_native_teleport(
            BoneyardNativeTeleportWorld { bodies: &bodies, bounds, collision: &collision },
            &mut rng,
        );
        compare(position, &rng, &case["result"], "Teleport");
    }
}

#[test]
fn phase_uses_the_first_legal_unrounded_probe_in_its_authored_range() {
    let origin = Vec2::new(0.125, -0.25);
    let direction = Vec2::new(0.333333333333, 0.666666666667);
    assert_eq!(
        resolve_native_phasing_destination(origin, direction, |_| true),
        Some(Vec2::new(origin.x + direction.x * 80.0, origin.y + direction.y * 80.0))
    );
    assert_eq!(
        resolve_native_phasing_destination(origin, direction, |position| position.x
            >= origin.x + direction.x * 270.0),
        Some(Vec2::new(origin.x + direction.x * 270.0, origin.y + direction.y * 270.0))
    );
    assert_eq!(resolve_native_phasing_destination(origin, direction, |_| false), None);
}
