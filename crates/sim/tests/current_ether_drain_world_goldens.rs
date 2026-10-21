//! Source's ordered force receiver over actual Session and ground-loot constructors.
#![cfg(feature = "persistence")]
#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;
#[path = "support/player_combat.rs"]
#[allow(dead_code)]
mod support;

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::world::{BoneyardPlayerBody, BoneyardPlayerStatus};
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardScene, BoneyardSource, BoneyardSpawn, LoadedBoneyard,
};
use solomon_sim::loot::{NativeLootDropKind, NativeLootDropSource, NativeLootDropSpec};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::secondary::ether_drain::NativeEtherDrainContact;
use solomon_sim::session::{GameSession, GameSessionOptions};

fn loaded() -> LoadedBoneyard {
    LoadedBoneyard {
        choice_id: "empty".into(),
        choice_source: BoneyardSource::Default,
        mod_id: None,
        mod_name: None,
        name: "Empty".into(),
        source_sha256: "c".repeat(64),
        geometry_sha256: "b".repeat(64),
        run_id: "drain-forces".into(),
        seed: "charms".into(),
        scene: BoneyardScene {
            name: "Forces".into(),
            environment_mode: 2,
            bounds: BoneyardBounds::new(0.0, 0.0, 500.0, 500.0),
            spawn: BoneyardSpawn { position: Vec2::new(250.0, 250.0), facing_degrees: 180.0 },
            objects: vec![],
            sprites: vec![],
            roads: vec![],
            fences: vec![],
            terrain: vec![],
            solomon_dig: None,
        },
    }
}
#[test]
fn player_pairs_loose_sweeps_and_registration_reuse_follow_the_source_transaction() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/current-ether-drain-world-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let selection = |name: &str, element| NewGameSelection {
            name: name.into(),
            element,
            discipline: WizardDiscipline::Arcane,
        };
        let mut session = GameSession::new_participants(
            vec![
                ("owner".into(), selection("Owner", WizardElement::Ether)),
                ("peer".into(), selection("Peer", WizardElement::Fire)),
            ],
            GameSessionOptions { gameplay_rng_seed: 17, combat_rng_seed: 91, ..Default::default() },
        )
        .unwrap();
        session.install_boneyard(loaded()).unwrap();
        session.participants.get_mut("owner").unwrap().wizard.position = Vec2::new(90.0, 250.0);
        session.participants.get_mut("peer").unwrap().wizard.position = Vec2::new(140.0, 250.0);
        let specs = case["specs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let mut spec = NativeLootDropSpec::new(
                    NativeLootDropKind::Gold,
                    row["id"].as_u64().unwrap(),
                    native_bits::input(&row["position"]),
                    NativeLootDropSource::Script,
                );
                spec.amount = row["amount"].as_u64();
                spec
            })
            .collect();
        session.boneyard.as_mut().unwrap().loot.spawn_specs(specs, 0, &mut session.painter_order);
        let bodies: Vec<_> = session
            .participant_order
            .iter()
            .map(|id| BoneyardPlayerBody {
                status: BoneyardPlayerStatus {
                    id,
                    alive: true,
                    eligible: true,
                    collision_enabled: id != "owner" || !case["inactiveOwner"].as_bool().unwrap(),
                    movement_scale: 1.0,
                },
                position: session.participants[id].wizard.position,
            })
            .collect();
        let contacts: Vec<NativeEtherDrainContact> = native_bits::input(&case["contacts"]);
        let result = session.boneyard.as_mut().unwrap().apply_ether_drain_forces(
            &contacts,
            &bodies,
            &mut session.secondary,
        );
        for (id, position) in result.player_positions {
            session.participants.get_mut(&id).unwrap().wizard.position = position;
        }
        let world = session.boneyard.as_ref().unwrap();
        let players: Vec<_> = session
            .participant_order
            .iter()
            .map(|id| json!({"id":id,"position":session.participants[id].wizard.position}))
            .collect();
        let actors: Vec<_> = world
            .loot
            .actors
            .iter()
            .map(|actor| {
                json!({
                    "id": actor.id,
                    "position": actor.position,
                    "registration": actor.painter_registration.registration_ordinal,
                })
            })
            .collect();
        let actual = json!({
            "consumed": result.consumed_field_actor_ids,
            "players": players,
            "loot": actors,
            "rng": support::rng_snapshot(&world.loot.shared_rng),
            "order": {
                "nextRegistrationOrdinal": {
                    "actor": session.painter_order.next_actor_ordinal,
                    "transient": session.painter_order.next_transient_ordinal,
                },
            },
        });
        support::assert_snapshot(&actual, &case["expected"], case["name"].as_str().unwrap());
    }
}
