//! World contact, retention views and ordered shake values from the actual source kernels.

use serde_json::Value;
use solomon_assets::boneyard::decode_scene;
use solomon_sim::Vec2;
use solomon_sim::boneyard::collision::BoneyardCollisionWorld;
use solomon_sim::boneyard::feedback::{
    NativeEnemyWorldFeedback, native_enemy_world_feedback_impulses,
};
use solomon_sim::boneyard::projectile_world::{
    BoneyardProjectileViewport, BoneyardProjectileWorld, boneyard_projectile_point_gain,
};
use solomon_sim::enemies::model::EnemyTerminalOutput;
use solomon_sim::enemies::world::EnemyProjectileWorldQuery;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn viewport(value: &Value) -> BoneyardProjectileViewport {
    BoneyardProjectileViewport {
        position: point(&value["position"]),
        viewport: Vec2::new(
            value["viewportWidth"].as_f64().unwrap(),
            value["viewportHeight"].as_f64().unwrap(),
        ),
        alternate_player: value["alternatePlayer"].as_bool().unwrap(),
    }
}

fn feedback(value: &Value) -> NativeEnemyWorldFeedback {
    NativeEnemyWorldFeedback {
        accumulator: value["accumulator"].as_f64().unwrap(),
        magnitude: value["magnitude"].as_f64().unwrap(),
    }
}

fn terminal(value: &str) -> EnemyTerminalOutput {
    match value {
        "spider-collapse" => EnemyTerminalOutput::SpiderCollapse,
        "discorporeal-banish" => EnemyTerminalOutput::DiscorporealBanish,
        "faculty-break" => EnemyTerminalOutput::FacultyBreak,
        "archer-shatter" => EnemyTerminalOutput::ArcherShatter,
        "coffin-break" => EnemyTerminalOutput::CoffinBreak,
        "demon-split" => EnemyTerminalOutput::DemonSplit,
        "heartmonger-shatter" => EnemyTerminalOutput::HeartmongerShatter,
        "imp-split" => EnemyTerminalOutput::ImpSplit,
        "mage-shatter" => EnemyTerminalOutput::MageShatter,
        "portal-break" => EnemyTerminalOutput::PortalBreak,
        "skeleton-shatter" => EnemyTerminalOutput::SkeletonShatter,
        "wraith-fragments" => EnemyTerminalOutput::WraithFragments,
        "zombie-collapse" => EnemyTerminalOutput::ZombieCollapse,
        value => panic!("unknown retained terminal output {value}"),
    }
}

#[test]
fn projectile_contact_kinds_stock_retention_cameras_and_feedback_match_the_website() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/projectile-world-goldens.json"
    ))
    .unwrap();
    let scene = decode_scene(&fixture["scene"].to_string()).unwrap();
    let collision = BoneyardCollisionWorld::from_scene(&scene);
    for case in fixture["cases"].as_array().unwrap() {
        let viewports: Vec<_> =
            case["viewports"].as_array().unwrap().iter().map(viewport).collect();
        let world = BoneyardProjectileWorld::new(scene.bounds, &collision, &viewports);
        assert_eq!(world.explosion_point_gain(Vec2::ZERO).is_none(), viewports.is_empty());
        for record in case["queries"].as_array().unwrap() {
            let query = &record["query"];
            let query = match query["kind"].as_str().unwrap() {
                "line" => EnemyProjectileWorldQuery::Line {
                    projectile_id: 1,
                    start: point(&query["start"]),
                    end: point(&query["end"]),
                    native_exclusion_mask: query["nativeExclusionMask"].as_u64().unwrap() as u32,
                },
                "point" => EnemyProjectileWorldQuery::Point {
                    projectile_id: 1,
                    position: point(&query["position"]),
                    radius: query["radius"].as_f64().unwrap(),
                },
                "bounds" => EnemyProjectileWorldQuery::Bounds {
                    projectile_id: 1,
                    position: point(&query["position"]),
                    margin: query["margin"].as_f64().unwrap(),
                },
                "view" => EnemyProjectileWorldQuery::View {
                    projectile_id: 1,
                    position: point(&query["position"]),
                    margin: query["margin"].as_f64().unwrap(),
                },
                value => panic!("unknown retained projectile query {value}"),
            };
            assert_eq!(world.blocked(query), record["expected"].as_bool().unwrap(), "{query:?}");
        }
        for record in case["gains"].as_array().unwrap() {
            assert_eq!(
                boneyard_projectile_point_gain(
                    scene.bounds,
                    viewport(&record["viewport"]),
                    point(&record["position"])
                ),
                record["expected"].as_f64().unwrap()
            );
        }
    }
    for record in fixture["impulses"].as_array().unwrap() {
        let output = terminal(record["output"].as_str().unwrap());
        let count = record["count"].as_u64().map(|count| count as usize);
        let expected: Vec<f64> = serde_json::from_value(record["expected"].clone()).unwrap();
        assert_eq!(native_enemy_world_feedback_impulses(output, count), expected);
    }
    for record in fixture["transitions"].as_array().unwrap() {
        let mut state = feedback(&record["state"]);
        if let Some(elapsed) = record["elapsed"].as_u64() {
            state.step(elapsed);
        } else {
            state.apply(record["intensity"].as_f64().unwrap());
        }
        assert_eq!(state, feedback(&record["expected"]));
    }
}
