//! A dropped weapon is an independent world animation, including GameOver and
//! the original owner's removal. Its motion consumes the actual secondary RNG.
#![cfg(feature = "persistence")]
#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardScene, BoneyardSource, BoneyardSpawn, LoadedBoneyard,
};
use solomon_sim::combat::PlayerCombatState;
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::death_animations::NativeDeathWeaponActor;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::player_presentation::{PlayerDeathBurstCrossingTracker, PlayerDeathObservation};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions, GameSessionTickContext};
use solomon_sim::spells::casting::NativePlayerWeaponKind;

fn loaded() -> LoadedBoneyard {
    LoadedBoneyard {
        choice_id: "empty".into(),
        name: "Empty".into(),
        choice_source: BoneyardSource::Default,
        mod_id: None,
        mod_name: None,
        geometry_sha256: "b".repeat(64),
        source_sha256: "c".repeat(64),
        run_id: "death-weapons".into(),
        seed: "weapons".into(),
        scene: BoneyardScene {
            name: "Death weapons".into(),
            environment_mode: 2,
            bounds: BoneyardBounds::new(0.0, 0.0, 500.0, 500.0),
            spawn: BoneyardSpawn { position: Vec2::new(250.0, 250.0), facing_degrees: 180.0 },
            fences: Vec::new(),
            objects: Vec::new(),
            roads: Vec::new(),
            sprites: Vec::new(),
            terrain: Vec::new(),
            solomon_dig: None,
        }
        .into(),
    }
}

fn registration(value: PainterRegistration) -> Value {
    json!({"managerLane":match value.manager_lane {
        WorldManagerLane::Actor=>"actor",WorldManagerLane::Transient=>"transient",
    },"registrationOrdinal":value.registration_ordinal})
}

fn weapon(actor: &NativeDeathWeaponActor) -> Value {
    let mut value = serde_json::to_value(actor).unwrap();
    value["weapon"]["kind"] = json!(match actor.weapon.kind {
        NativePlayerWeaponKind::Staff => "staff",
        NativePlayerWeaponKind::Wand => "wand",
    });
    value["painter_registration"] =
        native_bits::source_value(&registration(actor.painter_registration));
    value
}

fn snapshot(state: &GameSession) -> Value {
    assert_eq!(state.secondary.rng.divisor, 100000, "native float divisor is fixed in Source");
    let players:Vec<_>=state.participant_order.iter().map(|id|{
        let row=&state.participants.get(id).expect("participant exists"); let p=&row.player.combat;
        json!({"id":id,"health":p.current_health,"life":match p.life_state {
            PlayerLifeState::Alive=>"alive",PlayerLifeState::LethalPending=>"lethal-pending",
            PlayerLifeState::Dying=>"dying",PlayerLifeState::Spectating=>"spectating",
        },"deathAgeTicks":p.death_age_ticks,"deathTick":p.death_tick,"deathEpoch":p.death_epoch,
        "corpseConsumed":p.corpse_consumed,"registration":row.lighting.death_weapon_painter_registration.map(registration)})
    }).collect();
    let mut run = serde_json::to_value(&state.run).unwrap();
    run["phase"] = json!(match state.run.phase {
        solomon_sim::game_run::GameRunPhase::Hub => "hub",
        solomon_sim::game_run::GameRunPhase::Active => "active",
        solomon_sim::game_run::GameRunPhase::GameOver => "game-over",
        solomon_sim::game_run::GameRunPhase::Loadout => "loadout",
    });
    native_bits::source_value(&json!({"tick":state.ticks,"run":run,"players":players,
        "weapons":state.boneyard.as_ref().map_or(Vec::new(),|world|world.death_weapons.iter().map(weapon).collect()),
        "nextWeaponId":state.boneyard.as_ref().map(|world|world.next_death_weapon_id),
        "order":{"nextRegistrationOrdinal":{"actor":state.painter_order.next_actor_ordinal,
            "transient":state.painter_order.next_transient_ordinal}},
        "secondaryRng":{"indexA":state.secondary.rng.index_a,
            "indexB":state.secondary.rng.index_b,"words":&state.secondary.rng.words[..]}}))
}

#[test]
fn current_death_weapons_keep_their_actual_shared_rng_lifetime_after_owner_removal() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/current-death-weapon-session-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let spec = &case["spec"];
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
            GameSessionOptions {
                gameplay_rng_seed: 17,
                combat_rng_seed: 91,
                ..GameSessionOptions::default()
            },
        )
        .unwrap();
        if !spec["hub"].as_bool().unwrap() {
            state.install_boneyard(loaded()).unwrap();
        }
        let navigation =
            state.boneyard.as_ref().map(|world| NavigationSet::for_scene(&world.loaded));
        for id in spec["dead"].as_array().unwrap() {
            state.damage_player(id.as_str().unwrap(), 10000.0, 0);
        }
        let frames = case["frames"].as_array().unwrap();
        let mut frame_index = 0;
        for tick in 1..=600 {
            if spec["removeOwner"].as_bool().unwrap() && tick == 20 {
                assert!(state.remove_participant("owner"));
            }
            state.step(
                &Default::default(),
                GameSessionTickContext {
                    navigation: navigation.as_ref(),
                    ..GameSessionTickContext::default()
                },
            );
            if frames.get(frame_index).is_none_or(|frame| frame["tick"].as_u64() != Some(tick)) {
                continue;
            }
            native_bits::compare(
                &snapshot(&state),
                &native_bits::source_value(&frames[frame_index]["expected"]),
                &format!("{} tick{tick}", spec["name"]),
            );
            frame_index += 1;
        }
        assert_eq!(frame_index, frames.len());
    }
    for case in fixture["burstCases"].as_array().unwrap() {
        let mut combat = PlayerCombatState::new();
        combat.life_state = PlayerLifeState::Dying;
        combat.death_epoch = 1;
        combat.death_tick = 263;
        combat.corpse_consumed = case["corpseConsumed"].as_bool().unwrap();
        let mut tracker = PlayerDeathBurstCrossingTracker::new(
            "death-weapons",
            &[PlayerDeathObservation {
                player_id: "owner",
                combat: &combat,
                position: Vec2::new(250.0, 250.0),
            }],
        );
        combat.death_tick = 264;
        let triggers = tracker.update(
            "death-weapons",
            &[PlayerDeathObservation {
                player_id: "owner",
                combat: &combat,
                position: Vec2::new(250.0, 250.0),
            }],
        );
        assert_eq!(triggers.len() as u64, case["expectedCount"].as_u64().unwrap());
    }
}
