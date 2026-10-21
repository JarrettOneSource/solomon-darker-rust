//! The Source area callback uses two real physics passes per target, then commits roots.
#![cfg(feature = "persistence")]
#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::world::{BoneyardPlayerBody, BoneyardPlayerStatus};
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardScene, BoneyardSource, BoneyardSpawn, BoneyardWorld, LoadedBoneyard,
};
use solomon_sim::enemies::config::EnemyArenaScalars;
use solomon_sim::enemies::path::EnemyNavigation;
use solomon_sim::enemies::targeting::boneyard_enemy_collision_radius;
use solomon_sim::enemies::world::{
    EnemyGeometry, EnemyMovementRequest, EnemyProjectileWorldQuery, EnemySpawnContext,
};
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::painter::WorldPainterOrder;

struct OpenGeometry;
impl EnemyNavigation for OpenGeometry {
    fn direct_path_clear(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> bool {
        true
    }
    fn find_route(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> Option<Vec<Vec2>> {
        None
    }
}
impl EnemyGeometry for OpenGeometry {
    fn resolve_movement(&self, request: EnemyMovementRequest) -> Vec2 {
        request.requested_position
    }
    fn line_obstruction(&self, _: Vec2, _: Vec2, _: u32) -> Option<Vec2> {
        None
    }
    fn projectile_world_blocked(&self, _: EnemyProjectileWorldQuery) -> bool {
        false
    }
}
fn world() -> BoneyardWorld {
    BoneyardWorld::new(LoadedBoneyard {
        choice_id: "empty".into(),
        choice_source: BoneyardSource::Default,
        mod_id: None,
        mod_name: None,
        name: "Empty".into(),
        geometry_sha256: "b".repeat(64),
        source_sha256: "c".repeat(64),
        run_id: "knockback-world".into(),
        seed: "knockback-world".into(),
        scene: BoneyardScene {
            name: "Knockback".into(),
            environment_mode: 2,
            bounds: BoneyardBounds::new(0.0, 0.0, 500.0, 500.0),
            spawn: BoneyardSpawn { position: Vec2::new(250.0, 250.0), facing_degrees: 180.0 },
            fences: Vec::new(),
            objects: Vec::new(),
            roads: Vec::new(),
            sprites: Vec::new(),
            terrain: Vec::new(),
            solomon_dig: None,
        },
    })
}
fn point(v: &Value) -> Vec2 {
    Vec2::new(v["x"].as_f64().unwrap(), v["y"].as_f64().unwrap())
}
fn snapshot(world: &BoneyardWorld) -> Value {
    json!({"actors":world.enemies.actors.iter().map(|actor|json!({"id":actor.id,"position":actor.position,"radius":boneyard_enemy_collision_radius(actor)})).collect::<Vec<_>>(),
        "lantern":world.lantern_position})
}
#[test]
fn area_movement_two_pass_contacts_commit_real_enemy_and_lantern_roots_per_target() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-knockback-world-goldens.json"))
            .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let mut world = world();
        let intents: Vec<_> = fixture["intents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                EnemySpawnIntent::new(
                    EnemyToken::Skeleton,
                    point(&row["position"]),
                    row["id"].as_u64().unwrap() as usize,
                    0,
                    31,
                )
            })
            .collect();
        world.enemies.spawn(
            &intents,
            EnemySpawnContext {
                players: &[],
                tick: 0,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: None,
            },
            &OpenGeometry,
            Some(&mut WorldPainterOrder::default()),
        );
        world.lantern_position =
            if case["lantern"] == true { Some(Vec2::new(144.0, 116.0)) } else { None };
        let players = [BoneyardPlayerBody {
            status: BoneyardPlayerStatus {
                id: "owner",
                alive: true,
                eligible: true,
                collision_enabled: true,
                movement_scale: 1.0,
            },
            position: point(&case["playerPosition"]),
        }];
        for (index, frame) in case["frames"].as_array().unwrap().iter().enumerate() {
            if !frame["move"].is_null() {
                let contacts = world.move_knockback_area_target(
                    frame["move"]["id"].as_u64().unwrap() as usize,
                    point(&frame["move"]["delta"]),
                    &players,
                );
                assert_eq!(json!(contacts), frame["contacts"], "actual area root recipients");
            }
            native_bits::compare(
                &native_bits::source_value(&snapshot(&world)),
                &native_bits::source_value(&frame["expected"]),
                &format!("player{} lantern{} move{index}", case["nearbyPlayer"], case["lantern"]),
            );
        }
    }
}
