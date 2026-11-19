//! Retail StormCloud burst ownership, independent of the historical one-shot recording.
#![cfg(feature = "persistence")]
#[path = "support/secondary.rs"]
#[allow(dead_code)]
mod support;

use serde_json::Value;
use solomon_sim::belt::PlayerBeltComponent;
use solomon_sim::painter::{WorldManagerLane, WorldPainterOrder};
use solomon_sim::secondary::storm::{contact_light, presentation};
use solomon_sim::secondary::*;
use solomon_sim::{HubWizard, Vec2};
use std::collections::BTreeMap;

#[test]
fn five_births_finish_after_target_death_and_cloud_expiry_and_survive_checkpoints() {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!(
        "fixtures/website/secondary-tick-goldens.json.zlib"
    ))
    .unwrap();
    let fixture: Value = serde_json::from_slice(&bytes).unwrap();
    let case =
        fixture["cases"].as_array().unwrap().iter().find(|case| case["skillId"] == 27).unwrap();
    let book = support::read_book(&case["authority"]["skillBook"]);
    let belt = PlayerBeltComponent::new(&book);
    let wizard = HubWizard::at(Vec2::ZERO);
    for tornado in [false, true] {
        for enhanced in [false, true] {
            let mut authority =
                support::authority_from_fixture(&case["authority"], &book, &belt, &wizard);
            authority.input.quickbar = None;
            authority.enhanced_effects = enhanced;
            let players = BTreeMap::from([("player".to_owned(), authority)]);
            let mut world = support::World {
                case,
                targets: vec![SecondaryTarget {
                    family: "ZOMBIE".into(),
                    id: 17,
                    light_registration: WorldPainterOrder::default()
                        .register(WorldManagerLane::Actor),
                    actor_flags: None,
                    registration_order: None,
                    position: Vec2::new(400.0, 0.0),
                    radius: 10.0,
                    scale: 1.0,
                    shield_health: 0.0,
                }],
            };
            let mut state = SecondarySimulationState::new(123);
            let mut cloud = SecondaryActorState::new(
                0,
                SecondaryActorKind::StormCloud,
                "player".into(),
                "boneyard:oracle".into(),
                Some(27),
            );
            cloud.age_ticks = 49;
            cloud.freeze_ticks = 50;
            cloud.lifetime_ticks = 151;
            cloud.quantity = 1.0;
            cloud.slow_factor = 1.0;
            cloud.damage = 4.0;
            cloud.phase = 6.0;
            cloud.variant = i32::from(tornado);
            state.spawn_actor(cloud);
            let mut order = WorldPainterOrder::default();
            let mut births = Vec::new();
            let mut damage = 0;
            let mut sounds = 0;
            for tick in 100..=110 {
                let context =
                    SecondaryTickContext { enhanced_effects: None, players: &players, tick };
                let mut resumed: SecondarySimulationState =
                    serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
                let mut resumed_order = order;
                let restored_result = resumed.step(&context, &mut world, Some(&mut resumed_order));
                let result = state.step(&context, &mut world, Some(&mut order));
                assert_eq!(state, resumed, "checkpoint at {tick}");
                assert_eq!(result.damage, restored_result.damage);
                damage += result.damage.len();
                sounds += state
                    .events
                    .iter()
                    .filter(|e| e.tick == tick && e.kind == SecondaryEventKind::Impact)
                    .count();
                let strikes: Vec<_> = state
                    .actors
                    .iter()
                    .filter(|a| a.kind == SecondaryActorKind::StormStrike)
                    .collect();
                for strike in &strikes {
                    if strike.age_ticks == 0 {
                        births.push((*strike).clone());
                    }
                    assert_eq!(
                        strike.painter_registrations[0].manager_lane,
                        WorldManagerLane::Transient
                    );
                    assert_eq!(
                        strike.light_registration.unwrap().manager_lane,
                        WorldManagerLane::Transient
                    );
                }
                assert_eq!(strikes.is_empty(), tick == 110, "publication at {tick}");
                if tick == 100 {
                    assert_eq!(
                        state
                            .actors
                            .iter()
                            .find(|a| a.kind == SecondaryActorKind::StormCloud)
                            .unwrap()
                            .storm_burst
                            .as_ref()
                            .unwrap()
                            .remaining_ticks,
                        4
                    );
                    let mut cleared = state.clone();
                    cleared.reset_world();
                    assert!(cleared.actors.is_empty());
                    let mut departed = state.clone();
                    departed.remove_owner("player");
                    assert!(departed.actors.is_empty());
                    world.targets.clear();
                }
            }
            assert_eq!((births.len(), damage, sounds), (5, 1, 2));
            for (index, birth) in births.iter().enumerate() {
                assert_eq!(birth.phase, (100 + index) as f64);
                assert_eq!(birth.lifetime_ticks, 6);
                assert_eq!(birth.enhanced, enhanced);
                assert_eq!(birth.position, births[0].position);
                assert_eq!(birth.midpoint, births[0].midpoint);
                assert_eq!(birth.endpoint, Vec2::new(400.0, -15.0));
            }
        }
    }
}

#[test]
fn body_and_coronas_have_independent_native_clocks_and_light_owner() {
    let mut actor = SecondaryActorState::new(
        51,
        SecondaryActorKind::StormStrike,
        "wizard".into(),
        "world".into(),
        Some(27),
    );
    actor.position = Vec2::new(100.0, 180.0);
    actor.midpoint = Vec2::new(100.0, 100.0);
    actor.endpoint = Vec2::new(100.0, 10.0);
    actor.phase = 123.0;
    let before = actor.clone();
    let alpha = [
        1.0,
        0.800000011920929,
        0.6000000238418579,
        0.40000003576278687,
        0.20000003278255463,
        2.9802322387695312e-8,
        0.0,
    ];
    for (age, expected) in alpha.into_iter().enumerate() {
        let plan = presentation(&actor, age as u64);
        assert_eq!(plan.body.is_some(), age < 2);
        if let Some(layers) = &plan.body {
            assert_eq!(layers[0].width, 0.5);
            assert_eq!(layers[1].width, 0.375);
            assert_eq!(layers[0].phase_degrees, -369.0);
        }
        assert_eq!(plan.contact_corona.alpha, expected);
        assert_eq!(plan.source_corona.as_ref().map(|p| p.alpha), (age < 6).then_some(expected));
        let light = contact_light(&actor, age as u64);
        assert_eq!(light.is_some(), age < 6);
        if let Some(light) = light {
            assert!(!light.casts_directional_shadow);
            assert!((0.75..1.5).contains(&light.radius));
            assert_eq!(light.position.x, actor.position.x + plan.contact_corona.center.x);
            assert_eq!(light.position.y, actor.position.y + plan.contact_corona.center.y);
        }
    }
    assert_eq!(actor, before, "drawing preserves canonical state and RNG");
    let coarse = presentation(&actor, 0).body.unwrap();
    actor.enhanced = true;
    let fine = presentation(&actor, 0).body.unwrap();
    assert!(fine[0].parameter_samples.len() > coarse[0].parameter_samples.len());
}
