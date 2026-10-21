//! Receipt-time sampling against the actual pinned JS client timelines.

use serde::Deserialize;
use serde_json::Value;

type PlayerPose = (String, f64, f64, f64, f64, f64, usize, f64, f64, f64, f64);

#[derive(Debug, Deserialize, PartialEq)]
struct RenderRow {
    tick: f64,
    players: Vec<PlayerPose>,
    spells: Vec<Vec<f64>>,
}
use solomon_assets::boneyard::BoneyardCatalog;
use solomon_sim::Vec2;
use solomon_sim::float::fround;
use solomon_sim::game_run::GameRunPhase;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::profile::primary::NativeSpentEmber;
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionInput, GameSessionOptions};
use solomon_sim::spells::model::{
    PrimarySpellFireEmber, PrimarySpellFireParticle, PrimarySpellTransient,
    PrimarySpellTransientBase, PrimarySpellWaterHail,
};
use solomon_web::presentation::NativePresentationTimeline;

fn primary_endpoint(session: &mut GameSession, tick: u64) {
    let registration =
        PainterRegistration { manager_lane: WorldManagerLane::Actor, registration_ordinal: 77 };
    let common = |id, age_ticks, light_registration| PrimarySpellTransientBase {
        age_ticks,
        id,
        light_registration,
        owner_id: "local".into(),
        painter_registrations: vec![registration],
        world_key: "presentation".into(),
    };
    let age = tick as f64;
    if (5..10).contains(&tick) {
        session.spells.transients.push(PrimarySpellTransient::FireParticle(
            PrimarySpellFireParticle {
                enhanced_effects: true,
                common: common(101, tick - 5, None),
                direction: Vec2::new(0.6, -0.8),
                origin: Vec2::new(age * 2.0, age * 3.0),
                variant: 1,
            },
        ));
    }
    session.spells.transients.push(PrimarySpellTransient::FireEmber(PrimarySpellFireEmber {
        common: common(102, tick, Some(registration)),
        burn_damage: 0.0,
        contact_cadence: 1,
        contact_due: false,
        damage: 0.0,
        height: -age * 1.5,
        horizontal_velocity: Vec2::new(age * 0.2, -age * 0.3),
        life: 2.0 - age * 0.01,
        phase: (3.8 + age * 0.4) % 4.0,
        position: Vec2::new(age * 3.0, age * 4.0),
        spent_ember: NativeSpentEmber::None,
        vertical_velocity: age * 0.1,
    }));
    session.spells.transients.push(PrimarySpellTransient::WaterHail(PrimarySpellWaterHail {
        common: common(103, tick, None),
        birth_tick: 0,
        bounce_progress: 0.0,
        bounce_sound_index: None,
        bounce_sound_pitch: None,
        bounce_sound_sequence: 0,
        height: -age * 0.75,
        horizontal_velocity: Vec2::new(fround(age * 0.3), fround(-age * 0.4)),
        life: 0.5,
        position: Vec2::new(age * 5.0, age * 6.0),
        rotation_degrees: 350.0 + age * 4.0,
        rotation_step_degrees: 1.0,
        saved_bounce_velocity: -2.0,
        scale: 0.5,
        vertical_velocity: fround(age * 0.2),
    }));
}

fn initial(world: &str) -> GameSession {
    let selections = vec![
        (
            "local".into(),
            NewGameSelection {
                name: "Local".into(),
                element: WizardElement::Ether,
                discipline: WizardDiscipline::Arcane,
            },
        ),
        (
            "remote".into(),
            NewGameSelection {
                name: "Remote".into(),
                element: WizardElement::Fire,
                discipline: WizardDiscipline::Body,
            },
        ),
    ];
    let mut session = GameSession::new_participants(
        selections,
        GameSessionOptions {
            gameplay_rng_seed: 17,
            combat_rng_seed: 91,
            ..GameSessionOptions::default()
        },
    )
    .unwrap();
    if world == "boneyard" {
        let catalog = BoneyardCatalog::load_embedded().unwrap();
        let loaded = catalog.materialize([0; 16]);
        session.install_boneyard(loaded).unwrap();
    }
    session
}

fn endpoint(base: &GameSession, tick: u64, shift: f64) -> GameSession {
    let mut session = base.clone();
    session.ticks = tick;
    for (index, id) in session.participant_order.clone().iter().enumerate() {
        let participant = session.participants.get_mut(id).unwrap();
        let wizard = &mut participant.wizard;
        let tick = tick as f64;
        wizard.position = Vec2::new(
            100.0 + index as f64 * 200.0 + tick * 2.0 + shift,
            300.0 + index as f64 * 100.0 + tick * 3.0,
        );
        wizard.velocity = Vec2::new(7.0 + tick, -3.0 - tick);
        wizard.gait_degrees = (350.0 + tick * 5.0) % 360.0;
        wizard.set_facing(((23 + (tick / 5.0).floor() as usize) % 24) as f64 * 15.0);
        wizard.walk_cycle_primary = (4.5 + tick * 0.4) % 5.0;
        wizard.primary_cast.weapon_pulse = 1.0 - tick * 0.02;
        wizard.primary_cast.ether_blast_charge = tick * 0.125;
        participant.lighting.overlay_effect_phase = tick * 0.25;
    }
    primary_endpoint(&mut session, tick);
    session
}

#[test]
fn client_clock_and_player_poses_follow_receipt_time_without_extrapolating() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/presentation-timeline-goldens.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let base = initial(case["world"].as_str().unwrap());
        let mut timeline = NativePresentationTimeline::new(100, 20);
        timeline.push(&endpoint(&base, 0, 0.0), 0.0, "local", None).unwrap();
        for (index, operation) in fixture["operations"].as_array().unwrap().iter().enumerate() {
            if let Some(tick) = operation["push"].as_u64() {
                timeline
                    .push(
                        &endpoint(&base, tick, operation["shift"].as_f64().unwrap_or(0.0)),
                        operation["received"].as_f64().unwrap(),
                        "local",
                        None,
                    )
                    .unwrap();
            } else {
                // Hub's timeline keeps its local endpoint; prediction has its own source fixture.
                let frame = timeline
                    .sample(
                        operation["sample"].as_f64().unwrap(),
                        "local",
                        GameSessionInput::default(),
                        true,
                    )
                    .unwrap()
                    .unwrap();
                let players: Vec<_> = frame
                    .session
                    .participant_order
                    .iter()
                    .map(|id| {
                        let participant = &frame.session.participants[id];
                        let wizard = &participant.wizard;
                        (
                            id.clone(),
                            wizard.position.x,
                            wizard.position.y,
                            wizard.velocity.x,
                            wizard.velocity.y,
                            wizard.gait_degrees,
                            wizard.heading_index,
                            wizard.walk_cycle_primary,
                            wizard.primary_cast.weapon_pulse,
                            wizard.primary_cast.ether_blast_charge,
                            participant.lighting.overlay_effect_phase,
                        )
                    })
                    .collect();
                let spells = frame
                    .session
                    .spells
                    .transients
                    .iter()
                    .map(|effect| {
                        let common = effect.common();
                        let age = frame.clocks.primary[&common.id];
                        match effect {
                            PrimarySpellTransient::FireParticle(effect) => {
                                vec![0.0, common.id as f64, age, effect.origin.x, effect.origin.y]
                            }
                            PrimarySpellTransient::FireEmber(effect) => vec![
                                1.0,
                                common.id as f64,
                                age,
                                effect.position.x,
                                effect.position.y,
                                effect.height,
                                effect.horizontal_velocity.x,
                                effect.horizontal_velocity.y,
                                effect.life,
                                effect.phase,
                                effect.vertical_velocity,
                            ],
                            PrimarySpellTransient::WaterHail(effect) => vec![
                                2.0,
                                common.id as f64,
                                age,
                                effect.position.x,
                                effect.position.y,
                                effect.height,
                                effect.horizontal_velocity.x,
                                effect.horizontal_velocity.y,
                                effect.life,
                                effect.rotation_degrees,
                                effect.vertical_velocity,
                            ],
                            _ => panic!("unexpected fixture actor"),
                        }
                    })
                    .collect();
                let actual = RenderRow { tick: frame.tick, players, spells };
                let expected = case["samples"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|sample| sample["operation"] == index)
                    .unwrap();
                let expected: RenderRow =
                    serde_json::from_value(expected["expected"].clone()).unwrap();
                assert_eq!(actual, expected, "{} sample {index}", case["world"]);
            }
        }
    }
}

#[test]
fn college_prediction_stops_after_one_snapshot_interval() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/presentation-timeline-goldens.json"
    ))
    .unwrap();
    for case in fixture["prediction"].as_array().unwrap() {
        let session = endpoint(&initial("hub"), 0, 0.0);
        let mut timeline = NativePresentationTimeline::new(100, 20);
        timeline.push(&session, 0.0, "local", None).unwrap();
        let input = GameSessionInput {
            movement: Vec2::new(
                case["movement"]["x"].as_f64().unwrap(),
                case["movement"]["y"].as_f64().unwrap(),
            ),
            ..GameSessionInput::default()
        };
        for (index, milliseconds) in
            [0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 200.0].into_iter().enumerate()
        {
            let frame = timeline.sample(milliseconds, "local", input, false).unwrap().unwrap();
            let wizard = &frame.session.participants["local"].wizard;
            let actual = (
                wizard.position.x,
                wizard.position.y,
                wizard.velocity.x,
                wizard.velocity.y,
                wizard.gait_degrees,
                wizard.heading_index,
                wizard.walk_cycle_primary,
                frame.session.collision_rng,
            );
            let expected: (f64, f64, f64, f64, f64, usize, f64, u32) =
                serde_json::from_value(case["samples"][index.min(5)].clone()).unwrap();
            assert_eq!(actual, expected, "prediction at {milliseconds}ms");
        }
    }
}

#[test]
fn college_timeline_follows_the_local_native_manager_owner() {
    // `sameHubPresentationOwner` in the current Source client. Remote admissions
    // retain interpolation; post-run Create and local manager replacement do not.
    let base = initial("hub");
    let mut timeline = NativePresentationTimeline::new(100, 20);
    timeline.push(&endpoint(&base, 0, 0.0), 0.0, "local", None).unwrap();
    let mut next = endpoint(&base, 5, 0.0);
    next.participants
        .get_mut("remote")
        .unwrap()
        .lighting
        .light_registration
        .registration_ordinal += 1;
    timeline.push(&next, 50.0, "local", None).unwrap();
    let frame = timeline.sample(50.0, "local", GameSessionInput::default(), true).unwrap().unwrap();
    assert_eq!(frame.tick, 0.0);

    next.ticks = 6;
    next.run.phase = GameRunPhase::Loadout;
    timeline.push(&next, 60.0, "local", None).unwrap();
    let frame = timeline.sample(60.0, "local", GameSessionInput::default(), true).unwrap().unwrap();
    assert_eq!(frame.tick, 6.0);

    next.ticks = 7;
    next.participants.get_mut("local").unwrap().lighting.light_registration.registration_ordinal +=
        1;
    timeline.push(&next, 70.0, "local", None).unwrap();
    let frame = timeline.sample(70.0, "local", GameSessionInput::default(), true).unwrap().unwrap();
    assert_eq!(frame.tick, 7.0);
}
