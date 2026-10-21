use std::collections::BTreeMap;

use serde_json::Value;
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::encounter::SolomonPhase;
use solomon_sim::boneyard::presentation::TreeOcclusion;
use solomon_sim::boneyard::scenery::{
    NativeTreeOcclusionFrame, NativeTreeOcclusionPresentation, native_tree_actor_positions,
    native_tree_secondary_position,
};
use solomon_sim::enemies::EnemyToken;
use solomon_sim::enemies::actor_presentation::{NativeEnemyAnimationState, NativeEnemyCoffinState};

use super::*;

fn fixture() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../sim/tests/fixtures/website/tree-history-goldens.json"
    )))
    .unwrap()
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}
fn point(value: &Value) -> Vec2 {
    Vec2::new(number(&value["x"]), number(&value["y"]))
}
fn bounds(value: &Value) -> BoneyardBounds {
    BoneyardBounds::new(
        number(&value["x"]),
        number(&value["y"]),
        number(&value["w"]),
        number(&value["h"]),
    )
}
fn poses(value: &Value) -> NativeTreeActorPoses {
    NativeTreeActorPoses {
        players: value["players"]
            .as_array()
            .unwrap()
            .iter()
            .map(|player| NativeTreePlayerPose {
                id: player["id"].as_str().unwrap().to_owned(),
                position: point(&player["position"]),
            })
            .collect(),
        enemies: value["enemies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|enemy| NativeTreeEnemyPose {
                id: enemy["id"].as_u64().unwrap() as usize,
                position: point(&enemy["position"]),
                enemy_token: EnemyToken::ALL
                    .into_iter()
                    .find(|token| Some(token.id()) == enemy["enemyToken"].as_str())
                    .unwrap(),
                native_type_id: enemy["nativeTypeId"].as_u64().unwrap() as u16,
                animation_state: match enemy["animationState"].as_str().unwrap() {
                    "idle" => NativeEnemyAnimationState::Idle,
                    "death" => NativeEnemyAnimationState::Death,
                    other => panic!("fixture animation {other}"),
                },
                coffin_state: match enemy["coffinState"].as_str() {
                    None | Some("closed") => NativeEnemyCoffinState::Closed,
                    Some("hidden") => NativeEnemyCoffinState::Hidden,
                    Some("opening") => NativeEnemyCoffinState::Opening,
                    other => panic!("fixture Coffin {other:?}"),
                },
            })
            .collect(),
        maggots: value["maggots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|maggot| NativeTreeMaggotPose {
                id: maggot["id"].as_u64().unwrap() as usize,
                position: point(&maggot["position"]),
                native_tree_query_member: maggot["nativeTreeQueryMember"].as_bool().unwrap(),
            })
            .collect(),
        encounter: if value["encounter"].is_null() {
            None
        } else {
            Some((
                point(&value["encounter"]["position"]),
                match value["encounter"]["phase"].as_str().unwrap() {
                    "digging" => SolomonPhase::Digging,
                    "gone" => SolomonPhase::Gone,
                    other => panic!("fixture Solomon {other}"),
                },
            ))
        },
    }
}

fn endpoint_poses(value: &Value) -> NativeTreeActorPoses {
    let mut projected = value["world"].clone();
    projected["players"] = Value::Array(
        value["players"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, player)| serde_json::json!({"id": id, "position": player["position"]}))
            .collect(),
    );
    for enemy in projected["enemies"].as_array_mut().unwrap() {
        enemy["animationState"] = enemy["animation"]["state"].clone();
        enemy["coffinState"] = enemy["animation"]["coffinState"].clone();
    }
    // serde_json's object map is sorted; the Source endpoint's two explicit
    // player keys have a different insertion order, which this fixture retains.
    let mut result = poses(&projected);
    result.players.sort_by_key(|player| if player.id == "local" { 0 } else { 1 });
    result
}

#[test]
fn delivered_tree_poses_keep_source_membership_and_discrete_category_changes() {
    let source = fixture();
    let cases = source["poses"].as_array().unwrap();
    let first = endpoint_poses(&source["older"]);
    let next = endpoint_poses(&source["newer"]);
    for case in cases.iter().filter(|case| !case["expected"].is_null()) {
        let blend = ((number(&case["tick"]) - 10.0) / 10.0).clamp(0.0, 1.0);
        let actual = interpolate_poses(&first, &next, blend);
        assert_eq!(actual, poses(&case["expected"]), "poses at {}", case["tick"]);
        let expected: Vec<_> = case["positions"].as_array().unwrap().iter().map(point).collect();
        assert_eq!(
            native_tree_actor_positions(&actual, "local"),
            expected,
            "membership at {}",
            case["tick"]
        );
    }
}

fn frame(value: &Value) -> NativeTreeOcclusionFrame {
    NativeTreeOcclusionFrame {
        actor_positions: value["actorPositions"].as_array().unwrap().iter().map(point).collect(),
        camera_bounds: bounds(&value["cameraBounds"]),
    }
}

#[test]
fn tree_scans_preserve_targets_across_camera_gaps_and_delivered_history() {
    let source = fixture();
    let id = source["tree"]["eid"].as_str().unwrap();
    let area = bounds(&source["bounds"]);
    let tree = TreeOcclusion::new(
        id,
        point(&source["tree"]["position"]),
        [area.x, area.y, area.width, area.height],
        source["polygon"].as_array().unwrap().iter().map(point).collect(),
    );
    let mut trees =
        NativeTreeOcclusionPresentation::new(BTreeMap::from([(id.to_owned(), tree)]), 10);
    for case in source["history"].as_array().unwrap() {
        trees.update(
            number(&case["tick"]),
            &frame(&case["current"]),
            case["fromTick"].as_u64(),
            |tick| {
                case["samples"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["tick"].as_u64() == Some(tick))
                    .filter(|row| !row["frame"].is_null())
                    .map(|row| frame(&row["frame"]))
            },
        );
        let tree = trees.get(id).unwrap();
        assert_eq!(
            tree.current_alpha,
            number(&case["expected"]["currentAlpha"]),
            "alpha at {}",
            case["tick"]
        );
        assert_eq!(
            tree.target_alpha,
            number(&case["expected"]["targetAlpha"]),
            "target at {}",
            case["tick"]
        );
        assert_eq!(
            u64::from(tree.countdown),
            case["expected"]["countdown"].as_u64().unwrap(),
            "scan at {}",
            case["tick"]
        );
    }
    for case in source["secondary"].as_array().unwrap() {
        assert_eq!(
            native_tree_secondary_position(point(&case["root"]), bounds(&case["camera"])),
            point(&case["expected"])
        );
    }
}
