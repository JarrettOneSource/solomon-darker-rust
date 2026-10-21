//! Canonical actor physics against the source's ordered all-pairs solver.

use serde_json::Value;
use solomon_assets::boneyard::decode_scene;
use solomon_sim::Vec2;
use solomon_sim::actor_physics::{
    ActorPhysicsBody, ActorPhysicsWorld, resolve_actor_motion, resolve_unpushed_mover_motion,
};
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::collision::{
    BoneyardCollisionWorld, can_place_boneyard_body, resolve_boneyard_movement,
};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn body(value: &Value) -> ActorPhysicsBody {
    ActorPhysicsBody {
        collision_recipient: value["collisionRecipient"].as_bool().unwrap_or(true),
        id: value["id"].as_str().unwrap().into(),
        position: point(&value["position"]),
        delta: point(&value["delta"]),
        radius: value["radius"].as_f64().unwrap(),
        push_resistance: value["pushResistance"].as_f64().unwrap(),
        push_strength: value["pushStrength"].as_f64().unwrap(),
        driven: value["driven"].as_bool().unwrap_or(true),
        push_enabled: value["pushEnabled"].as_bool().unwrap_or(true),
    }
}

struct Geometry<'a> {
    bounds: BoneyardBounds,
    collision: &'a BoneyardCollisionWorld,
}

impl ActorPhysicsWorld for Geometry<'_> {
    fn can_place(&self, _body_id: &str, position: Vec2, radius: f64) -> bool {
        can_place_boneyard_body(position, self.bounds, self.collision, radius)
    }

    fn move_body(&mut self, _body_id: &str, position: Vec2, delta: Vec2, radius: f64) -> Vec2 {
        let requested = Vec2::new(position.x + delta.x, position.y + delta.y);
        resolve_boneyard_movement(position, requested, self.bounds, self.collision, radius)
    }
}

#[test]
fn recursive_push_epochs_root_contacts_and_nonpushing_enemy_moves_match_the_website() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/actor-physics-goldens.json"
    ))
    .unwrap();
    let scenes: Vec<_> = fixture["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| decode_scene(&value.to_string()).unwrap())
        .collect();
    let worlds: Vec<_> = scenes.iter().map(BoneyardCollisionWorld::from_scene).collect();
    for (case_index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let scene_index = case["sceneIndex"].as_u64().unwrap() as usize;
        let mut geometry =
            Geometry { bounds: scenes[scene_index].bounds, collision: &worlds[scene_index] };
        let mut bodies: Vec<_> = case["bodies"].as_array().unwrap().iter().map(body).collect();
        for (tick, expected) in case["ticks"].as_array().unwrap().iter().enumerate() {
            let mut contacts = Vec::new();
            resolve_actor_motion(
                &mut bodies,
                &mut geometry,
                |a, b| a.id != "ghost" && b.id != "ghost",
                Some(&mut contacts),
            );
            let positions: Vec<_> = bodies.iter().map(|body| body.position).collect();
            let expected_positions: Vec<_> =
                expected["positions"].as_array().unwrap().iter().map(point).collect();
            assert_eq!(
                positions, expected_positions,
                "actor positions case {case_index},tick {tick}"
            );
            let contacts: Vec<_> =
                contacts.into_iter().map(|contact| [contact.mover_id, contact.other_id]).collect();
            let expected_contacts: Vec<[String; 2]> =
                serde_json::from_value(expected["contacts"].clone()).unwrap();
            assert_eq!(contacts, expected_contacts, "root contacts case {case_index},tick {tick}");
        }
    }
    for (case_index, case) in fixture["unpushed"].as_array().unwrap().iter().enumerate() {
        let scene_index = case["sceneIndex"].as_u64().unwrap() as usize;
        let mut geometry =
            Geometry { bounds: scenes[scene_index].bounds, collision: &worlds[scene_index] };
        let bodies: Vec<_> = case["bodies"].as_array().unwrap().iter().map(body).collect();
        let mover = case["moverIndex"].as_u64().unwrap() as usize;
        let actual =
            resolve_unpushed_mover_motion(&bodies, mover, point(&case["delta"]), &mut geometry);
        assert_eq!(actual, point(&case["expected"]), "unpushed move case {case_index}");
    }
}
