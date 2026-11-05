//! Actual shared Session ticks retain Source field forces, captures and private RNG.
#![cfg(feature = "persistence")]
#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;
#[path = "support/secondary.rs"]
#[allow(dead_code)]
mod secondary;

use std::collections::HashSet;

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
use solomon_sim::painter::WorldManagerLane;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::secondary::NativeSecondaryActorKind;
use solomon_sim::secondary::SecondaryActorRead;
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
        }
        .into(),
    }
}
fn snapshot(state: &GameSession) -> Value {
    let world = state.boneyard.as_ref().unwrap();
    let players: Vec<_> = state
        .participant_order
        .iter()
        .map(|id| {
            let participant = &state.participants.get(id).expect("participant exists");
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
            let mut value = secondary::actor_snapshot(&actor);
            if actor.kind == NativeSecondaryActorKind::EtherDrain || actor.ether_drain().is_some() {
                value["etherDrain"] = serde_json::to_value(actor.ether_drain()).unwrap();
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
                let mut expected = frames[frame_index]["expected"].clone();
                if tick == 2 {
                    // The retained Source recording leaves loot flare2 unregistered
                    // until tick3. Completed snapshots must already be drawable:
                    // move that exact Transient1 enrollment to its birth tick.
                    // All later recorded frames remain unchanged.
                    let flare = &mut expected["actors"][1];
                    assert_eq!(flare["id"], "f64:4000000000000000");
                    assert_eq!(flare["kind"], "ether-drain-capture-flare");
                    assert_eq!(flare["painterRegistrations"], json!([]));
                    flare["painterRegistrations"] = json!([{
                        "managerLane": "transient", "registrationOrdinal": 1
                    }]);
                    assert_eq!(
                        expected["order"]["nextRegistrationOrdinal"]["transient"],
                        "f64:3ff0000000000000"
                    );
                    expected["order"]["nextRegistrationOrdinal"]["transient"] = json!(2);
                }
                native_bits::compare(
                    &snapshot(&state),
                    &native_bits::source_value(&expected),
                    &format!("quality{} tick{tick}", case["enhancedEffects"]),
                );
                frame_index += 1;
            }
        }
        assert_eq!(frame_index, frames.len());
    }
}

fn painter_session() -> (GameSession, NavigationSet) {
    let mut state = GameSession::new_participants(
        ["owner", "peer"]
            .map(|id| {
                (
                    id.into(),
                    NewGameSelection {
                        name: id.into(),
                        element: WizardElement::Ether,
                        discipline: WizardDiscipline::Arcane,
                    },
                )
            })
            .into(),
        GameSessionOptions { gameplay_rng_seed: 17, combat_rng_seed: 91, ..Default::default() },
    )
    .unwrap();
    state.participants.get_mut("owner").unwrap().player.grant_skill_ranks(
        74,
        1,
        &mut state.gameplay_rng,
    );
    assert!(state.bind_belt("owner", NativeBeltCommand::BindSkill { skill_id: Some(74), slot: 0 }));
    state.install_boneyard(loaded()).unwrap();
    state.participants.get_mut("owner").unwrap().wizard.position = Vec2::new(90.0, 250.0);
    state.participants.get_mut("peer").unwrap().wizard.position = Vec2::new(90.0, 350.0);
    let navigation = NavigationSet::for_scene(&state.boneyard.as_ref().unwrap().loaded);
    let input = GameSessionInput {
        aim: Some(Vec2::new(250.0, 250.0)),
        quickbar: Some(0),
        ..Default::default()
    };
    state.step(
        &std::collections::BTreeMap::from([("owner".into(), input)]),
        GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
    );
    let mut field = state
        .secondary
        .actors
        .iter_mut()
        .find(|a| a.kind == NativeSecondaryActorKind::EtherDrain)
        .unwrap();
    // Start the ordinary public tick at the field's fully grown contact phase.
    field.phase = 1.0;
    field.alpha = 1.0;
    field.scale = 1.0;
    field.quantity = 0.0;
    (state, navigation)
}

fn assert_born_flare(
    state: &GameSession,
    scale: f64,
) -> (u64, solomon_sim::painter::PainterRegistration) {
    use solomon_sim::painter::WorldManagerLane;
    let flares: Vec<_> = state
        .secondary
        .actors
        .iter()
        .filter(|a| a.kind == NativeSecondaryActorKind::EtherDrainCaptureFlare)
        .collect();
    assert_eq!(flares.len(), 1);
    let flare = flares[0];
    assert_eq!(flare.age_ticks, 0, "late births must not advance on their birth tick");
    assert_eq!(flare.scale, scale);
    assert_eq!(flare.painter_registrations.len(), 1, "the completed snapshot must be drawable");
    let registration = flare.painter_registrations[0];
    assert_eq!(registration.manager_lane, WorldManagerLane::Transient);
    assert!(registration.registration_ordinal < state.painter_order.next_transient_ordinal);
    assert!(
        state
            .secondary
            .actors
            .iter()
            .filter(|a| a.id != flare.id)
            .all(|a| !a.painter_registrations.contains(&registration)),
        "the flare owns a distinct painter"
    );
    (flare.id, registration)
}

fn assert_flash_birth(state: &GameSession) {
    let children: Vec<_> = state
        .secondary
        .actors
        .iter()
        .filter(|actor| {
            matches!(
                actor.kind,
                NativeSecondaryActorKind::FlashResponseGrow
                    | NativeSecondaryActorKind::FlashResponseFade
            )
        })
        .collect();
    let expected = [
        vec![NativeSecondaryActorKind::FlashResponseGrow; 8],
        vec![NativeSecondaryActorKind::FlashResponseFade; 4],
    ]
    .concat();
    assert_eq!(children.iter().map(|actor| actor.kind).collect::<Vec<_>>(), expected);
    let roots: Vec<_> = children
        .iter()
        .map(|actor| {
            assert_eq!(
                actor.painter_registrations.len(),
                1,
                "each Flash child needs its own root on its birth tick"
            );
            let root = actor.painter_registrations[0];
            assert_eq!(root.manager_lane, WorldManagerLane::Transient);
            root.registration_ordinal
        })
        .collect();
    assert!(
        roots.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "grow and fade children register in birth order"
    );
    let live_roots: Vec<_> = state
        .secondary
        .actors
        .iter()
        .flat_map(|actor| actor.painter_registrations.clone())
        .filter(|root| root.manager_lane == WorldManagerLane::Transient)
        .map(|root| root.registration_ordinal)
        .collect();
    assert_eq!(
        HashSet::<_>::from_iter(live_roots.iter()).len(),
        live_roots.len(),
        "live secondary actors cannot share a transient root"
    );
    assert!(roots[11] < state.painter_order.next_transient_ordinal);
    let published = serde_json::to_value(state.snapshot()).unwrap();
    let published_actors = published["secondary"]["actors"].as_array().unwrap();
    for child in &children {
        let row = published_actors.iter().find(|row| row["id"] == json!(child.id)).unwrap();
        assert_eq!(
            row["painter_registrations"],
            serde_json::to_value(&child.painter_registrations).unwrap()
        );
    }
    let restored = GameSession::restore(state.checkpoint(), Some(loaded())).unwrap();
    let restored_children: Vec<_> = restored
        .secondary
        .actors
        .iter()
        .filter(|actor| {
            matches!(
                actor.kind,
                NativeSecondaryActorKind::FlashResponseGrow
                    | NativeSecondaryActorKind::FlashResponseFade
            )
        })
        .collect();
    assert_eq!(
        restored_children.iter().map(|actor| &actor.painter_registrations).collect::<Vec<_>>(),
        children.iter().map(|actor| &actor.painter_registrations).collect::<Vec<_>>()
    );
}

#[test]
fn flash_post_secondary_ether_drain_contact_registers_children_before_snapshot() {
    let (mut state, navigation) = painter_session();
    state.participants.get_mut("peer").unwrap().player.grant_skill_ranks(
        53,
        1,
        &mut state.gameplay_rng,
    );
    let peer = state.participants.get_mut("peer").unwrap();
    peer.wizard.position = Vec2::new(250.0, 250.0);
    peer.player.combat.maximum_health = 100_000.0;
    peer.player.combat.current_health = 100_000.0;
    for _ in 0..300 {
        state.step(
            &Default::default(),
            GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
        );
        if state.secondary.events.iter().any(|event| {
            event.skill_id == Some(53) && event.owner_id == "peer" && event.tick == state.ticks
        }) {
            break;
        }
    }
    assert!(
        state.secondary.events.iter().any(|event| event.skill_id == Some(53)
            && event.owner_id == "peer"
            && event.tick == state.ticks),
        "the ordinary field contact must admit a peer Flash response"
    );
    assert!(state.participants.get("peer").unwrap().player.combat.current_health < 100_000.0);
    assert_flash_birth(&state);
}

#[test]
fn post_secondary_ether_drain_shield_break_registers_children_before_snapshot() {
    let (mut state, navigation) = painter_session();
    state.participants.get_mut("peer").unwrap().wizard.position = Vec2::new(250.0, 250.0);
    let shield = state.secondary.players.get_mut("peer").unwrap();
    shield.magic_shield_absorb = 1.0;
    shield.magic_shield_maximum = 1.0;
    shield.magic_shield_explosion_damage = 12.0;
    for _ in 0..300 {
        state.step(
            &Default::default(),
            GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
        );
        if state.secondary.events.iter().any(|event| {
            event.kind == solomon_sim::secondary::NativeSecondaryEventKind::ShieldBreak
                && event.owner_id == "peer"
                && event.tick == state.ticks
        }) {
            break;
        }
    }
    assert!(
        state.secondary.events.iter().any(|event| event.kind
            == solomon_sim::secondary::NativeSecondaryEventKind::ShieldBreak
            && event.owner_id == "peer"
            && event.tick == state.ticks),
        "the field must break the active shield during late magic contact"
    );
    let children: Vec<_> = state
        .secondary
        .actors
        .iter()
        .filter(|actor| {
            actor.owner_id == "peer"
                && matches!(
                    actor.kind,
                    NativeSecondaryActorKind::ShieldBreak
                        | NativeSecondaryActorKind::ShieldExplosion
                        | NativeSecondaryActorKind::Shockwave
                )
                && actor.skill_id == Some(54)
        })
        .collect();
    assert_eq!(children.len(), 22);
    assert!(children[..20].iter().all(|actor| actor.kind == NativeSecondaryActorKind::ShieldBreak));
    assert_eq!(children[20].kind, NativeSecondaryActorKind::ShieldExplosion);
    assert_eq!(children[21].kind, NativeSecondaryActorKind::Shockwave);
    let roots: Vec<_> = children
        .iter()
        .map(|actor| {
            assert_eq!(
                actor.painter_registrations.len(),
                1,
                "each shield child needs its birth painter"
            );
            let root = actor.painter_registrations[0];
            assert_eq!(root.manager_lane, WorldManagerLane::Transient);
            root.registration_ordinal
        })
        .collect();
    assert!(roots.windows(2).all(|pair| pair[1] == pair[0] + 1));
    assert!(roots[21] < state.painter_order.next_transient_ordinal);
    let wave_light = children[21].light_registration.expect("late shockwave owns a light");
    assert_eq!(wave_light.manager_lane, WorldManagerLane::Actor);
    assert!(wave_light.registration_ordinal < state.painter_order.next_actor_ordinal);
    let published = serde_json::to_value(state.snapshot()).unwrap();
    let published_actors = published["secondary"]["actors"].as_array().unwrap();
    for child in &children {
        let row = published_actors.iter().find(|row| row["id"] == json!(child.id)).unwrap();
        assert_eq!(
            row["painter_registrations"],
            serde_json::to_value(&child.painter_registrations).unwrap()
        );
    }
    let restored = GameSession::restore(state.checkpoint(), Some(loaded())).unwrap();
    let restored_children: Vec<_> = restored
        .secondary
        .actors
        .iter()
        .filter(|actor| children.iter().any(|child| child.id == actor.id))
        .collect();
    assert_eq!(
        restored_children.iter().map(|actor| &actor.painter_registrations).collect::<Vec<_>>(),
        children.iter().map(|actor| &actor.painter_registrations).collect::<Vec<_>>()
    );
}

#[test]
fn post_secondary_ordinary_magic_shield_break_registers_twenty_children() {
    let (mut state, navigation) = painter_session();
    state.participants.get_mut("peer").unwrap().wizard.position = Vec2::new(250.0, 250.0);
    let shield = state.secondary.players.get_mut("peer").unwrap();
    shield.magic_shield_absorb = 1.0;
    shield.magic_shield_maximum = 1.0;
    shield.magic_shield_explosion_damage = 0.0;
    for _ in 0..300 {
        state.step(
            &Default::default(),
            GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
        );
        if state.secondary.events.iter().any(|event| {
            event.kind == solomon_sim::secondary::NativeSecondaryEventKind::ShieldBreak
                && event.owner_id == "peer"
                && event.tick == state.ticks
        }) {
            break;
        }
    }
    assert!(
        state.secondary.events.iter().any(|event| event.kind
            == solomon_sim::secondary::NativeSecondaryEventKind::ShieldBreak
            && event.owner_id == "peer"
            && event.tick == state.ticks)
    );
    let children: Vec<_> = state
        .secondary
        .actors
        .iter()
        .filter(|actor| {
            actor.owner_id == "peer" && actor.kind == NativeSecondaryActorKind::ShieldBreak
        })
        .collect();
    assert_eq!(children.len(), 20);
    assert!(children.iter().all(|actor| actor.painter_registrations.len() == 1
        && actor.painter_registrations[0].manager_lane == WorldManagerLane::Transient));
    assert!(!state.secondary.actors.iter().any(|actor| actor.owner_id == "peer"
        && matches!(
            actor.kind,
            NativeSecondaryActorKind::ShieldExplosion | NativeSecondaryActorKind::Shockwave
        )
        && actor.skill_id == Some(54)));
}

#[test]
fn flash_post_secondary_poison_response_registers_children_before_snapshot() {
    let selection = NewGameSelection {
        name: "Owner".into(),
        element: WizardElement::Ether,
        discipline: WizardDiscipline::Arcane,
    };
    let mut state = GameSession::new_participants(
        vec![("owner".into(), selection)],
        GameSessionOptions { gameplay_rng_seed: 17, combat_rng_seed: 91, ..Default::default() },
    )
    .unwrap();
    state.participants.get_mut("owner").unwrap().player.grant_skill_ranks(
        53,
        1,
        &mut state.gameplay_rng,
    );
    state.install_boneyard(loaded()).unwrap();
    let navigation = NavigationSet::for_scene(&state.boneyard.as_ref().unwrap().loaded);
    let owner = state.participants.get_mut("owner").unwrap();
    // Poison cannot kill; at zero health, positive raw poison produces no health damage.
    owner.player.combat.current_health = 0.0;
    owner.player.combat.poison_damage_per_tick = 1.0;
    owner.player.combat.poison_ticks_remaining = 2;
    state.secondary.rng = solomon_sim::NativeRng::new(18);
    state.step(
        &Default::default(),
        GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
    );
    assert!(
        state.secondary.events.iter().any(|event| event.skill_id == Some(53)
            && event.owner_id == "owner"
            && event.tick == state.ticks),
        "the zero-health poison floor must admit a real Flash response"
    );
    assert_eq!(
        state.participants.get("owner").unwrap().player.combat.life_state,
        PlayerLifeState::Alive
    );
    assert_flash_birth(&state);
}

#[test]
fn replacement_capture_enrolls_its_flare_before_snapshot_and_keeps_it_through_restore() {
    use solomon_sim::secondary::ether_drain::{
        NativeEtherDrainAtlas, NativeEtherDrainCapturedImage,
    };
    let (mut state, navigation) = painter_session();
    // A previous victim is still in the field when the next enemy is captured.
    state.secondary.capture_ether_drain_image(
        1,
        Some(NativeEtherDrainCapturedImage {
            atlas: NativeEtherDrainAtlas::BadGuys,
            entry: 1477,
            tint: 0xffffff,
            body_y_offset: 23,
        }),
        state.ticks,
    );
    let mut spawn = EnemySpawnIntent::new(EnemyToken::Skeleton, Vec2::new(250.0, 250.0), 1, 1, 1);
    spawn.flags = vec![EnemyFlag::Weak, EnemyFlag::HpDown];
    state.secondary.actors.iter_mut().find(|a| a.id == 1).unwrap().damage = 1000.0;
    let before_parent =
        state.secondary.actors.iter().find(|a| a.id == 1).unwrap().painter_registrations.clone();
    state.step(
        &Default::default(),
        GameSessionTickContext {
            navigation: Some(&navigation),
            enemy_spawn_intents: &[spawn],
            ..Default::default()
        },
    );
    assert!(state.boneyard.as_ref().unwrap().enemies.actors.iter().any(|a| a.ether_drain_captured));
    let (flare_id, registration) = assert_born_flare(&state, 1.5);
    assert_eq!(
        state.secondary.actors.iter().find(|a| a.id == 1).unwrap().painter_registrations,
        before_parent
    );
    let mut restored = GameSession::restore(state.checkpoint(), Some(loaded())).unwrap();
    for session in [&mut state, &mut restored] {
        session.step(
            &Default::default(),
            GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
        );
        let flare = session.secondary.actors.iter().find(|a| a.id == flare_id).unwrap();
        assert_eq!(flare.age_ticks, 1);
        assert_eq!(flare.painter_registrations, [registration]);
    }
    assert_eq!(
        serde_json::to_value(state.checkpoint()).unwrap(),
        serde_json::to_value(restored.checkpoint()).unwrap()
    );
}

#[test]
fn consumed_loot_enrolls_its_flare_on_the_contact_tick() {
    let (mut state, navigation) = painter_session();
    let mut spec = NativeLootDropSpec::new(
        NativeLootDropKind::Gold,
        1,
        Vec2::new(250.0, 250.0),
        NativeLootDropSource::Script,
    );
    spec.amount = Some(1);
    state.boneyard.as_mut().unwrap().loot.spawn_specs(
        vec![spec],
        state.ticks,
        &mut state.painter_order,
    );
    for loot in state.boneyard.as_mut().unwrap().loot.actors.iter_mut() {
        loot.position = Vec2::new(250.0, 250.0);
        loot.activation_delay_ticks = 0;
    }
    state.step(
        &Default::default(),
        GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
    );
    assert!(state.boneyard.as_ref().unwrap().loot.actors.is_empty());
    assert_born_flare(&state, 1.0);
}

#[test]
fn consumed_corpse_enrolls_its_flare_during_play_and_game_over() {
    for game_over in [false, true] {
        let (mut state, navigation) = painter_session();
        let peer = state.participants.get_mut("peer").unwrap();
        peer.wizard.position = Vec2::new(250.0, 250.0);
        peer.player.combat.current_health = 0.0;
        peer.player.combat.life_state = PlayerLifeState::Dying;
        peer.player.combat.death_age_ticks = 216;
        peer.player.combat.death_tick = 129;
        peer.player.combat.death_epoch = 1;
        if game_over {
            state.run.phase = solomon_sim::game_run::GameRunPhase::GameOver;
        }
        state.step(
            &Default::default(),
            GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
        );
        assert!(state.participants.get("peer").unwrap().player.combat.corpse_consumed);
        assert_born_flare(&state, 2.0);
    }
}
