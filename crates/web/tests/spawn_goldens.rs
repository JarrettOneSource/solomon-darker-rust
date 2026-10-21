//! Native enemy birth placement, before the world binds real navigation and light queries.

use serde_json::Value;
use solomon_assets::boneyard::{decode_bounds, decode_scene};
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::collision::BoneyardCollisionWorld;
use solomon_sim::boneyard::spawn::{
    SpawnPlacement, SpawnPolicyContext, boneyard_spawn_position_is_offscreen,
    native_spawn_ring_sample_count,
};
use solomon_sim::enemies::SpawnPositionPolicy;
use solomon_sim::{NativeRng, Vec2};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

struct PolicyContext<'a> {
    kind: &'a str,
    bounds: BoneyardBounds,
    policy_bounds: BoneyardBounds,
    retry_bounds: BoneyardBounds,
    focuses: &'a [Vec2],
}

impl SpawnPolicyContext for PolicyContext<'_> {
    fn light_at(&self, position: Vec2) -> f64 {
        if self.kind != "unlit"
            && position.x >= 1150.0
            && position.x <= 1250.0
            && position.y >= 750.0
            && position.y <= 1050.0
        {
            1.0
        } else {
            0.0
        }
    }

    fn accepts_domain(&self, position: Vec2, _radius: f64) -> bool {
        match self.kind {
            "domain" => position.x >= 1230.0,
            "never" => false,
            _ => true,
        }
    }

    fn is_offscreen(&self, position: Vec2) -> bool {
        boneyard_spawn_position_is_offscreen(position, self.bounds, self.focuses)
    }

    fn is_outside_policy_bounds(&self, position: Vec2) -> bool {
        let bounds = self.policy_bounds;
        position.x < bounds.x
            || position.x > bounds.x + bounds.width
            || position.y < bounds.y
            || position.y > bounds.y + bounds.height
    }

    fn retry_bounds(&self) -> Option<BoneyardBounds> {
        if self.kind == "retry" { Some(self.retry_bounds) } else { None }
    }
}

#[test]
fn collision_safe_spawns_preserve_source_policies_and_every_native_random_word() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../sim/tests/fixtures/website/spawn-goldens.json"))
            .unwrap();
    let scenes: Vec<_> = fixture["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| decode_scene(&value.to_string()).unwrap())
        .collect();
    let worlds: Vec<_> = scenes.iter().map(BoneyardCollisionWorld::from_scene).collect();
    let focuses: Vec<_> = fixture["focuses"].as_array().unwrap().iter().map(point).collect();
    let policy_bounds = decode_bounds(&fixture["policyBounds"].to_string()).unwrap();
    let retry_bounds = decode_bounds(&fixture["retryBounds"].to_string()).unwrap();

    for (case_index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let scene_index = case["sceneIndex"].as_u64().unwrap() as usize;
        let request = SpawnPlacement {
            position: point(&case["position"]),
            bounds: scenes[scene_index].bounds,
            collision: &worlds[scene_index],
            radius: case["radius"].as_f64().unwrap(),
        };
        let context = PolicyContext {
            kind: case["context"].as_str().unwrap(),
            bounds: request.bounds,
            policy_bounds,
            retry_bounds,
            focuses: &focuses,
        };
        let policy = match case["policy"].as_str().unwrap() {
            "direct" => SpawnPositionPolicy::Direct,
            "dark" => SpawnPositionPolicy::Dark,
            "light" => SpawnPositionPolicy::Light,
            "offscreen" => SpawnPositionPolicy::Offscreen,
            "edge" => SpawnPositionPolicy::Edge,
            value => panic!("unknown recorded policy {value}"),
        };
        let original_rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut rng = original_rng.clone();
        let actual = request.resolve_native(policy, &mut rng, &context);
        let expected = &case["expected"];
        if expected.is_null() {
            assert!(actual.is_err(), "placement {case_index} should be exhausted");
            assert_eq!(rng, original_rng, "failed source calls publish no speculative draws");
        } else {
            assert_eq!(actual.unwrap(), point(&expected["position"]), "placement {case_index}");
            let expected_rng = &expected["rngState"];
            assert_eq!(rng.index_a, expected_rng["indexA"].as_u64().unwrap() as usize);
            assert_eq!(rng.index_b, expected_rng["indexB"].as_u64().unwrap() as usize);
            let words: Vec<u32> = serde_json::from_value(expected_rng["words"].clone()).unwrap();
            assert_eq!(rng.words.as_slice(), words, "random stream for placement {case_index}");
        }
        // Legacy placement is policy-independent; each position/radius pair records it once.
        if case_index % 25 == 0 {
            for legacy in case["legacy"].as_array().unwrap() {
                let actual = request.resolve(legacy["angle"].as_f64());
                if legacy["position"].is_null() {
                    assert!(actual.is_err());
                } else {
                    assert_eq!(actual.unwrap(), point(&legacy["position"]));
                }
            }
        }
    }
    for case in fixture["offscreen"].as_array().unwrap() {
        let bounds = decode_bounds(&case["bounds"].to_string()).unwrap();
        let focuses: Vec<_> = case["focuses"].as_array().unwrap().iter().map(point).collect();
        assert_eq!(
            boneyard_spawn_position_is_offscreen(point(&case["position"]), bounds, &focuses),
            case["expected"].as_bool().unwrap()
        );
    }
    for case in fixture["samples"].as_array().unwrap() {
        assert_eq!(
            native_spawn_ring_sample_count(
                case["radius"].as_f64().unwrap(),
                case["step"].as_f64().unwrap()
            ),
            case["expected"].as_u64().unwrap() as usize
        );
    }
}
