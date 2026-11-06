//! Direct authored damage and accepted rescue effects use the actual shared Session.

#[path = "support/player_combat.rs"]
#[allow(dead_code)] // Reuses the exact numeric recursive comparator from the player fixtures.
mod support;

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardScene, BoneyardSource, BoneyardSpawn, LoadedBoneyard,
};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions, GameSessionTickContext};

fn point(value: Vec2) -> Value {
    json!({"x":value.x,"y":value.y})
}
fn registration(value: PainterRegistration) -> Value {
    json!({"managerLane":if value.manager_lane==WorldManagerLane::Actor {"actor"}else{"transient"},"registrationOrdinal":value.registration_ordinal})
}
fn snapshot(state: &GameSession) -> Value {
    let players:Vec<_>=state.participant_order.iter().map(|id|{
        let row=&state.participants[id]; let p=&row.player.combat;let h=row.player.progression.hagatha_runtime;
        let particles:Vec<_>=p.rescue_protection.particles.iter().map(|v|json!({"id":v.id,"alpha":v.alpha,
            "decay":v.decay,"position":point(v.position),"painterRegistration":v.painter_registration.map(registration),
            "rotationDegrees":v.rotation_degrees,"timer":v.timer,"worldKey":v.world_key})).collect();
        json!({"playerId":id,"health":p.current_health,"life":match p.life_state{
            PlayerLifeState::Alive=>"alive",PlayerLifeState::LethalPending=>"lethal-pending",PlayerLifeState::Dying=>"dying",PlayerLifeState::Spectating=>"spectating"},
            "protection":{"fraction":p.rescue_protection.fraction,"nextParticleId":p.rescue_protection.next_particle_id,"particles":particles},
            "hagatha":{"cheatDeathCharges":h.cheat_death_charges,"reverieActive":h.reverie_active,"serendipityActive":h.serendipity_active},
            "blindness":row.lighting.blindness_ticks_remaining})
    }).collect();
    let events:Vec<_>=state.boneyard.as_ref().map_or(&[][..],|world|world.enemy_events.as_slice()).iter().map(|event|json!({
        "actorId":event.actor_id,"eventId":event.event_id,"sourcePosition":event.source_position.map(point),
        "targetPlayerId":event.target_player_id,"tick":event.tick,"type":event.kind.id()})).collect();
    let writes:Vec<_>=state.screen_flashes.writes.iter().map(|row|json!({"order":row.order,"tick":row.tick,"worldKey":row.world_key,
        "position":point(row.position),"onlyIfClear":row.only_if_clear,"flash":{"alpha":row.flash.alpha,"red":row.flash.red,
            "green":row.flash.green,"blue":row.flash.blue,"decayPerTick":row.flash.decay_per_tick,"pointAttenuated":row.flash.point_attenuated}})).collect();
    let actors:Vec<_>=state.secondary.actors.iter().map(|a|json!({"id":a.id,"kind":a.kind.as_str(),"ageTicks":a.age_ticks,
        "ownerId":a.owner_id,"worldKey":a.world_key,"position":point(a.position),"alpha":a.alpha,"radius":a.radius,
        "quantity":a.quantity,"phase":a.phase,"slowFactor":a.slow_factor,"damage":a.damage,
        "lightRegistration":a.light_registration.map(registration),
        "painterRegistrations":a.painter_registrations.iter().copied().map(registration).collect::<Vec<_>>()})).collect();
    json!({"tick":state.ticks,"players":players,"events":events,
        "flashes":{"epoch":state.screen_flashes.epoch,"nextOrder":state.screen_flashes.next_order,"writes":writes},
        "painterOrder":{"nextRegistrationOrdinal":{"actor":state.painter_order.next_actor_ordinal,"transient":state.painter_order.next_transient_ordinal}},
        "rng":support::rng_snapshot(&state.secondary.rng),"actors":actors})
}
fn loaded() -> LoadedBoneyard {
    LoadedBoneyard {
        choice_id: "empty".into(),
        choice_source: BoneyardSource::Default,
        mod_id: None,
        mod_name: None,
        name: "Empty".into(),
        source_sha256: "c".repeat(64),
        geometry_sha256: "b".repeat(64),
        run_id: "rescue-session".into(),
        seed: "charms".into(),
        scene: BoneyardScene {
            name: "Rescue".into(),
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

#[test]
fn current_public_damage_emits_loss_then_rescue_once_and_keeps_hub_births_out_of_the_scene() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/current-player-rescue-session-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["cases"].as_array().unwrap() {
        let spec = &case["spec"];
        let name = spec["name"].as_str().unwrap();
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
        if spec["hub"].as_bool() == Some(false) {
            state.install_boneyard(loaded()).unwrap();
        }
        let navigation =
            state.boneyard.as_ref().map(|world| NavigationSet::for_scene(&world.loaded));
        let perks: Vec<_> = spec["perks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect();
        let participant = state.participants.get_mut("owner").unwrap();
        participant.player.economy.owned_perk_selectors = perks.clone();
        participant.player.economy.first_mixed_selectors = perks.clone();
        participant.player.economy.revision += 1;
        participant.player.progression.hagatha_runtime.purchase(&perks);
        participant.lighting.blindness_ticks_remaining = spec["blindness"].as_f64().unwrap();
        for (index, frame) in case["frames"].as_array().unwrap().iter().enumerate() {
            match frame["request"]["kind"].as_str() {
                Some("damage") => {
                    state.damage_player(
                        "owner",
                        frame["request"]["amount"].as_f64().unwrap(),
                        state.ticks,
                    );
                }
                Some("step") => {
                    state.step(
                        &Default::default(),
                        GameSessionTickContext {
                            navigation: navigation.as_ref(),
                            ..GameSessionTickContext::default()
                        },
                    );
                }
                None => {}
                other => panic!("unknown Source request {other:?}"),
            }
            support::assert_snapshot(
                &snapshot(&state),
                &frame["expected"],
                &format!("{name} frame{index}"),
            );
        }
    }
}
