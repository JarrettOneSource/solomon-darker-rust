//! Current Staff/Golem native area recurrence and ownership of the shared RNG.
#![cfg(feature = "persistence")]
#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;

use serde_json::{Value, json};
use solomon_sim::spells::model::PrimarySpellTransientBase;
use solomon_sim::spells::staff::{NativeStaffKnockbackActor, step_native_staff_knockback};
use solomon_sim::{NativeRng, Vec2};
use std::collections::BTreeMap;

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn rng(rng: &NativeRng) -> Value {
    json!({"indexA":rng.index_a,"indexB":rng.index_b,"words":rng.words.as_slice()})
}
fn actor(actor: &NativeStaffKnockbackActor) -> Value {
    json!({"ageTicks":actor.common.age_ticks,"arcDegrees":actor.arc_degrees,"id":actor.common.id,"kind":"player-staff-knockback",
        "origin":actor.origin,"ownerId":actor.common.owner_id,"remainingDistance":actor.remaining_distance,"targetIds":actor.target_ids,"worldKey":actor.common.world_key})
}
#[test]
fn current_staff_area_uses_native_proposals_and_removes_missing_rows_before_expiry() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-knockback-area-goldens.json"))
            .unwrap();
    for case in fixture["staffCases"].as_array().unwrap() {
        let initial = &case["initial"];
        let mut current = NativeStaffKnockbackActor {
            common: PrimarySpellTransientBase {
                age_ticks: 0,
                id: 1,
                light_registration: None,
                owner_id: "owner".into(),
                painter_registrations: Vec::new(),
                world_key: "boneyard:area".into(),
            },
            arc_degrees: 120.0,
            origin: Vec2::ZERO,
            remaining_distance: case["budget"].as_f64().unwrap(),
            target_ids: initial["targetIds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap().into())
                .collect(),
        };
        let mut positions: BTreeMap<_, _> = case["positions"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, value)| (id.clone(), point(value)))
            .collect();
        let mut random = NativeRng::new(31);
        for (index, frame) in case["frames"].as_array().unwrap().iter().enumerate() {
            let stepped = step_native_staff_knockback(&current, &mut positions, &mut random);
            let actual = json!({"cameraDisplacement":stepped.camera_displacement,"actor":stepped.actor.as_ref().map(actor),"dazzledTargetIds":stepped.dazzled_target_ids,
                "displacements":stepped.displacements.iter().map(|row|json!({"targetId":row.target_id,"delta":row.delta})).collect::<Vec<_>>(),
                "headingPerturbations":stepped.heading_perturbations.iter().map(|row|json!({"targetId":row.target_id,"headingDegrees":row.heading_degrees})).collect::<Vec<_>>(),"rng":rng(&random)});
            let expected = frame["expected"].clone();
            native_bits::compare(
                &native_bits::source_value(&actual),
                &native_bits::source_value(&expected),
                &format!("budget{} tick{}", case["budget"], index + 1),
            );
            if let Some(next) = stepped.actor {
                current = next;
            } else {
                assert_eq!(index + 1, case["frames"].as_array().unwrap().len());
                break;
            }
        }
    }
}

struct AreaWorld {
    positions: BTreeMap<String, Vec2>,
    collision_chain: bool,
    moves: Vec<Value>,
}
impl solomon_sim::knockback::NativeKnockbackAreaWorld<String> for AreaWorld {
    fn position(&self, id: &String) -> Option<Vec2> {
        self.positions.get(id).copied()
    }
    fn move_actor(&mut self, id: &String, delta: Vec2) -> Vec<String> {
        if !self.collision_chain {
            return Vec::new();
        }
        self.moves.push(json!({"id":id,"delta":delta}));
        let position = self.positions.get_mut(id).unwrap();
        position.x = solomon_sim::float::fround(position.x + delta.x);
        position.y = solomon_sim::float::fround(position.y + delta.y);
        match id.as_str() {
            "first" => vec!["second".into(), "second".into()],
            "second" => vec!["third".into(), "first".into()],
            _ => Vec::new(),
        }
    }
}
#[test]
fn shared_area_queries_observe_each_move_and_visit_new_collision_recipients_once() {
    use solomon_sim::knockback::{NativeKnockbackArea, step_native_knockback_area};
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/current-knockback-area-goldens.json"))
            .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let initial = &case["initial"];
        let mut world = AreaWorld {
            positions: case["initialPositions"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(id, p)| (id.clone(), point(p)))
                .collect(),
            collision_chain: case["mode"] == "collision-chain",
            moves: Vec::new(),
        };
        let mut target_ids: Vec<String> = initial["targetIds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap().into())
            .collect();
        let mut remaining_distance = initial["remainingDistance"].as_f64().unwrap();
        let mut random = NativeRng::new(31);
        for (index, frame) in case["frames"].as_array().unwrap().iter().enumerate() {
            world.moves.clear();
            let step = step_native_knockback_area(
                NativeKnockbackArea {
                    origin: point(&initial["origin"]),
                    remaining_distance,
                    target_ids: &target_ids,
                },
                &mut random,
                &mut world,
            );
            let actual = json!({"cameraDisplacement":step.camera_displacement,"displacements":step.displacements.iter().map(|row|json!({"targetId":row.target_id,"delta":row.delta})).collect::<Vec<_>>(),
                "headingPerturbations":step.heading_perturbations.iter().map(|row|json!({"targetId":row.target_id,"headingDegrees":row.heading_degrees})).collect::<Vec<_>>(),
                "remainingDistance":step.remaining_distance,"rng":rng(&random),"targetIds":step.target_ids,"terminal":step.terminal,
                "moves":world.moves,"positions":world.positions});
            native_bits::compare(
                &native_bits::source_value(&actual),
                &native_bits::source_value(&frame["expected"]),
                &format!("{} budget{} tick{}", case["mode"], case["budget"], index + 1),
            );
            remaining_distance = step.remaining_distance;
            target_ids = step.target_ids;
        }
    }
}
