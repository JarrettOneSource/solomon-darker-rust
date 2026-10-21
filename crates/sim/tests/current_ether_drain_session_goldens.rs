//! Actual shared Session ticks retain Source field forces, captures and private RNG.
#![cfg(feature = "persistence")]
#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;
#[path = "support/secondary.rs"]
#[allow(dead_code)]
mod secondary;

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::belt::NativeBeltCommand;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardScene, BoneyardSource, BoneyardSpawn, LoadedBoneyard,
};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::enemies::model::EnemyLifeState;
use solomon_sim::enemies::{EnemyFlag, EnemySpawnIntent, EnemyToken};
use solomon_sim::loot::{NativeLootDropKind, NativeLootDropSource, NativeLootDropSpec};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::secondary::NativeSecondaryActorKind;
use solomon_sim::session::{
    GameSession, GameSessionInput, GameSessionOptions, GameSessionTickContext,
};

fn loaded() -> LoadedBoneyard {
    LoadedBoneyard {
        choice_id: "empty".into(),
        choice_source: BoneyardSource::Default,
        mod_id: None,
        mod_name: None,
        name: "Empty".into(),
        source_sha256: "c".repeat(64),
        geometry_sha256: "b".repeat(64),
        run_id: "drain-session".into(),
        seed: "charms".into(),
        scene: BoneyardScene {
            name: "Drain Session".into(),
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
fn snapshot(state: &GameSession) -> Value {
    let world = state.boneyard.as_ref().unwrap();
    let players: Vec<_> = state
        .participant_order
        .iter()
        .map(|id| {
            let participant = &state.participants[id];
            let combat = &participant.player.combat;
            let life = match combat.life_state {
                PlayerLifeState::Alive => "alive",
                PlayerLifeState::LethalPending => "lethal-pending",
                PlayerLifeState::Dying => "dying",
                PlayerLifeState::Spectating => "spectating",
            };
            json!({
                "id": id, "position": participant.wizard.position,
                "health": combat.current_health, "mana": combat.current_mana,
                "life": life, "deathTick": combat.death_tick,
                "corpseConsumed": combat.corpse_consumed,
            })
        })
        .collect();
    let actors: Vec<_> = state
        .secondary
        .actors
        .iter()
        .map(|actor| {
            let mut value = secondary::actor_snapshot(actor);
            if actor.kind == NativeSecondaryActorKind::EtherDrain || actor.ether_drain.is_some() {
                value["etherDrain"] = serde_json::to_value(&actor.ether_drain).unwrap();
            }
            value
        })
        .collect();
    let enemies: Vec<_> = world
        .enemies
        .actors
        .iter()
        .map(|actor| {
            json!({
                "id": actor.id, "token": actor.config.enemy_token.id(), "position": actor.position,
                "health": actor.current_health,
                "life": if actor.life_state == EnemyLifeState::Alive { "alive" } else { "dying" },
                "captured": actor.ether_drain_captured,
            })
        })
        .collect();
    let events: Vec<_> = world
        .enemy_events
        .iter()
        .map(|event| {
            json!({"actorId": event.actor_id, "eventId": event.event_id,
            "tick": event.tick, "type": event.kind.id()})
        })
        .collect();
    let loot: Vec<_> = world
        .loot
        .actors
        .iter()
        .map(|actor| {
            let kind = match actor.kind {
                NativeLootDropKind::Gold => "gold",
                NativeLootDropKind::Orb => "orb",
                NativeLootDropKind::Sack => "sack",
                NativeLootDropKind::Bonus => "bonus",
            };
            json!({"id": actor.id, "kind": kind, "position": actor.position,
            "activationDelayTicks": actor.activation_delay_ticks,
            "registration": actor.painter_registration.registration_ordinal})
        })
        .collect();
    native_bits::source_value(&json!({
        "tick": state.ticks, "players": players, "actors": actors,
        "events": state.secondary.events.iter().map(secondary::event_snapshot).collect::<Vec<_>>(),
        "nextActorId": state.secondary.next_actor_id, "nextEventId": state.secondary.next_event_id,
        "rng": secondary::rng_snapshot(&state.secondary.rng), "flashes": state.screen_flashes,
        "order": {"nextRegistrationOrdinal": {
            "actor": state.painter_order.next_actor_ordinal,
            "transient": state.painter_order.next_transient_ordinal,
        }},
        "enemies": enemies, "enemyEvents": events, "loot": loot,
        "lootRng": secondary::rng_snapshot(&world.loot.shared_rng),
    }))
}

#[test]
fn current_session_field_uses_actual_bodies_loot_capture_and_shared_random_streams() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/current-ether-drain-session-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let selection = |name: &str, element| NewGameSelection {
            name: name.into(),
            element,
            discipline: WizardDiscipline::Arcane,
        };
        let mut state = GameSession::new_participants(
            vec![
                ("owner".into(), selection("Owner", WizardElement::Ether)),
                ("peer".into(), selection("Peer", WizardElement::Fire)),
            ],
            GameSessionOptions { gameplay_rng_seed: 17, combat_rng_seed: 91, ..Default::default() },
        )
        .unwrap();
        state.participants.get_mut("owner").unwrap().player.grant_skill_ranks(
            74,
            1,
            &mut state.gameplay_rng,
        );
        assert!(
            state.bind_belt("owner", NativeBeltCommand::BindSkill { skill_id: Some(74), slot: 0 })
        );
        state.install_boneyard(loaded()).unwrap();
        state.enhanced_effects = case["enhancedEffects"].as_bool().unwrap();
        state.participants.get_mut("owner").unwrap().wizard.position = Vec2::new(90.0, 250.0);
        state.participants.get_mut("peer").unwrap().wizard.position = Vec2::new(140.0, 250.0);
        let navigation = NavigationSet::for_scene(&state.boneyard.as_ref().unwrap().loaded);
        let specs = fixture["lootSpecs"]
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
        state.boneyard.as_mut().unwrap().loot.spawn_specs(specs, 0, &mut state.painter_order);
        let intents: Vec<_> = fixture["spawnIntents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let mut intent = EnemySpawnIntent::new(
                    EnemyToken::from_id(row["enemyToken"].as_str().unwrap()).unwrap(),
                    native_bits::input(&row["position"]),
                    row["id"].as_u64().unwrap() as usize,
                    row["spawnTick"].as_u64().unwrap(),
                    row["waveOrdinal"].as_u64().unwrap(),
                );
                intent.flags = row["flags"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|flag| EnemyFlag::from_id(flag.as_str().unwrap()).unwrap())
                    .collect();
                intent
            })
            .collect();
        let mut frame_index = 0;
        let frames = case["frames"].as_array().unwrap();
        for tick in 0..=220 {
            if tick > 0 {
                let input = GameSessionInput {
                    aim: Some(Vec2::new(250.0, 250.0)),
                    quickbar: if tick == 1 { Some(0) } else { None },
                    ..Default::default()
                };
                state.step(
                    &std::collections::BTreeMap::from([("owner".into(), input)]),
                    GameSessionTickContext {
                        navigation: Some(&navigation),
                        enemy_spawn_intents: if tick == 1 { &intents } else { &[] },
                        ..Default::default()
                    },
                );
            }
            if frames.get(frame_index).is_some_and(|frame| frame["tick"].as_u64() == Some(tick)) {
                native_bits::compare(
                    &snapshot(&state),
                    &native_bits::source_value(&frames[frame_index]["expected"]),
                    &format!("quality{} tick{tick}", case["enhancedEffects"]),
                );
                frame_index += 1;
            }
        }
        assert_eq!(frame_index, frames.len());
    }
}
