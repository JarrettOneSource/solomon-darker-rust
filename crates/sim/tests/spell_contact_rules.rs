//! Retail spell contacts, participant feedback, and low-mana branches.

use std::collections::BTreeMap;

use solomon_sim::Vec2;
use solomon_sim::belt::NativeBeltCommand;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardScene, BoneyardSource, BoneyardSpawn, LoadedBoneyard,
};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::enemies::model::{EnemyEvent, EnemyEventKind};
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::secondary::NativeSecondaryActorKind;
use solomon_sim::secondary::presentation::native_secondary_presentation_plan;
use solomon_sim::session::{
    GameSession, GameSessionInput, GameSessionOptions, GameSessionTickContext,
};
use solomon_sim::spells::model::PrimaryTransientRef;
use solomon_sim::spells::transient_storage::WeldWorldRef;

fn drain_session() -> (GameSession, NavigationSet) {
    let mut session = GameSession::new_participants(
        ["owner", "peer"]
            .into_iter()
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
            .collect(),
        GameSessionOptions { gameplay_rng_seed: 17, combat_rng_seed: 91, ..Default::default() },
    )
    .unwrap();
    session.participants.get_mut("owner").unwrap().player.grant_skill_ranks(
        74,
        1,
        &mut session.gameplay_rng,
    );
    assert!(
        session.bind_belt("owner", NativeBeltCommand::BindSkill { skill_id: Some(74), slot: 0 },)
    );
    session
        .install_boneyard(LoadedBoneyard {
            choice_id: "empty".into(),
            choice_source: BoneyardSource::Default,
            mod_id: None,
            mod_name: None,
            name: "Empty".into(),
            source_sha256: "c".repeat(64),
            geometry_sha256: "b".repeat(64),
            run_id: "drain-feedback".into(),
            seed: "charms".into(),
            scene: BoneyardScene {
                name: "Drain feedback".into(),
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
        })
        .unwrap();
    let navigation = NavigationSet::for_scene(&session.boneyard.as_ref().unwrap().loaded);
    (session, navigation)
}

fn step_drain(session: &mut GameSession, navigation: &NavigationSet) {
    session.step(
        &BTreeMap::from([(
            "owner".into(),
            GameSessionInput {
                aim: Some(Vec2::new(250.0, 250.0)),
                quickbar: (session.ticks == 0).then_some(0),
                ..Default::default()
            },
        )]),
        GameSessionTickContext { navigation: Some(navigation), ..Default::default() },
    );
}

#[test]
fn ether_drain_retains_cheat_death_feedback_for_caster_and_peer() {
    for victim in ["owner", "peer"] {
        let (mut session, navigation) = drain_session();
        for id in ["owner", "peer"] {
            let participant = session.participants.get_mut(id).unwrap();
            participant.wizard.position =
                if id == victim { Vec2::new(250.0, 250.0) } else { Vec2::new(90.0, 250.0) };
        }
        let participant = session.participants.get_mut(victim).unwrap();
        participant.player.economy.owned_perk_selectors = vec![7];
        participant.player.progression.hagatha_runtime.purchase(&[7]);
        participant.player.combat.current_health = 0.01;

        for _ in 0..120 {
            step_drain(&mut session, &navigation);
            if session
                .participants
                .get(victim)
                .unwrap()
                .player
                .progression
                .hagatha_runtime
                .cheat_death_charges
                == 0
            {
                break;
            }
        }

        let participant = session.participants.get(victim).unwrap();
        assert_eq!(participant.player.progression.hagatha_runtime.cheat_death_charges, 0);
        assert_eq!(participant.player.combat.life_state, PlayerLifeState::Alive);
        assert!(participant.player.combat.current_health > 0.0);
        assert!(session.secondary.actors.iter().any(|actor| {
            actor.kind == NativeSecondaryActorKind::RescueShockwave && actor.owner_id == victim
        }));
        let rescues: Vec<_> = session
            .boneyard
            .as_ref()
            .unwrap()
            .enemy_events
            .iter()
            .filter(|event| event.kind == EnemyEventKind::PlayerCheatDeath)
            .collect();
        assert_eq!(rescues.len(), 1, "{victim}: accepted rescue must reach banner/audio");
        assert_eq!(rescues[0].target_player_id.as_ref().unwrap().as_deref(), Some(victim));
        assert_eq!(rescues[0].tick, session.ticks);
    }
}

#[test]
fn ether_drain_retains_accepted_charm_loss_without_ordinary_hurt_feedback() {
    for selectors in [&[24][..], &[25][..], &[24, 25][..], &[7, 24, 25][..]] {
        for victim in ["owner", "peer"] {
            let (mut session, navigation) = drain_session();
            for id in ["owner", "peer"] {
                session.participants.get_mut(id).unwrap().wizard.position =
                    if id == victim { Vec2::new(250.0, 250.0) } else { Vec2::new(90.0, 250.0) };
            }
            let player = &mut session.participants.get_mut(victim).unwrap().player;
            player.economy.owned_perk_selectors = selectors.to_vec();
            player.progression.hagatha_runtime.purchase(selectors);
            if selectors.contains(&7) {
                player.combat.current_health = 0.01;
            }

            let mut feedback: Vec<EnemyEvent> = Vec::new();
            for _ in 0..120 {
                step_drain(&mut session, &navigation);
                let events = &session.boneyard.as_ref().unwrap().enemy_events;
                assert!(
                    !events.iter().any(|event| event.kind == EnemyEventKind::PlayerDamageSound)
                );
                for event in events.iter().filter(|event| {
                    matches!(
                        event.kind,
                        EnemyEventKind::PlayerCharmLost | EnemyEventKind::PlayerCheatDeath
                    )
                }) {
                    if !feedback.iter().any(|prior| prior.event_id == event.event_id) {
                        feedback.push(event.clone());
                    }
                }
                let runtime =
                    session.participants.get(victim).unwrap().player.progression.hagatha_runtime;
                if !runtime.reverie_active
                    && !runtime.serendipity_active
                    && (!selectors.contains(&7) || runtime.cheat_death_charges == 0)
                {
                    break;
                }
            }
            let player = &session.participants.get(victim).unwrap().player;
            assert!(!player.progression.hagatha_runtime.reverie_active);
            assert!(!player.progression.hagatha_runtime.serendipity_active);
            assert_eq!(player.combat.life_state, PlayerLifeState::Alive);
            let expected = if selectors.contains(&7) {
                vec![EnemyEventKind::PlayerCharmLost, EnemyEventKind::PlayerCheatDeath]
            } else {
                vec![EnemyEventKind::PlayerCharmLost]
            };
            assert_eq!(
                feedback.iter().map(|event| event.kind).collect::<Vec<_>>(),
                expected,
                "{victim} {selectors:?}: accepted state loss must reach notification/audio"
            );
            assert!(
                feedback.iter().all(
                    |event| event.target_player_id.as_ref().unwrap().as_deref() == Some(victim)
                )
            );
            assert_eq!(feedback.last().unwrap().tick, session.ticks);
            assert!(feedback.windows(2).all(|events| events[0].event_id < events[1].event_id
                && events[0].tick <= events[1].tick));
            let loss_id = feedback[0].event_id;

            for _ in 0..20 {
                step_drain(&mut session, &navigation);
            }
            let events = &session.boneyard.as_ref().unwrap().enemy_events;
            assert!(
                events
                    .iter()
                    .filter(|event| event.kind == EnemyEventKind::PlayerCharmLost)
                    .all(|event| event.event_id == loss_id),
                "spent charms must not generate repeated losses"
            );
            assert!(
                !events.iter().any(|event| event.kind == EnemyEventKind::PlayerDamageSound),
                "retaining semantic loss must not restore suppressed Ouch audio"
            );
        }
    }
}

#[test]
fn a_living_player_at_the_center_does_not_expand_the_capture_pulse() {
    let (mut session, navigation) = drain_session();
    session.participants.get_mut("peer").unwrap().wizard.position = Vec2::new(90.0, 250.0);
    let player = &mut session.participants.get_mut("owner").unwrap().player;
    player.combat.maximum_health = 100_000.0;
    player.combat.current_health = 100_000.0;
    let mut field_ticks = 0;

    for _ in 0..120 {
        step_drain(&mut session, &navigation);
        for field in session
            .secondary
            .actors
            .iter()
            .filter(|actor| actor.kind == NativeSecondaryActorKind::EtherDrain)
        {
            field_ticks += 1;
            assert_eq!(field.slow_factor, 0.0, "living contact is not capture");
            let plan = native_secondary_presentation_plan(&field, session.ticks as f64, 1.0);
            assert!(!plan.draws.iter().any(|draw| draw.role == "ether-drain-capture-pulse"));
        }
    }

    assert!(field_ticks > 80, "the cast must reach sustained center contact");
    let player = &session.participants.get("owner").unwrap().player;
    assert_eq!(player.combat.life_state, PlayerLifeState::Alive);
    assert!(player.combat.current_health < 100_000.0, "the center must still hurt the player");
    assert!(
        session
            .boneyard
            .as_ref()
            .unwrap()
            .enemy_events
            .iter()
            .all(|event| event.kind != EnemyEventKind::PlayerDamageSound),
        "field damage must keep ordinary hit audio suppressed"
    );
}

#[test]
fn steam_normal_particles_damage_a_mob_with_empty_insufficient_or_funded_mana() {
    for caster in ["owner", "peer"] {
        for reserve in [Some(0.0), Some(0.000_001), None] {
            let (mut session, navigation) = drain_session();
            let player = &mut session.participants.get_mut(caster).unwrap().player;
            player.grant_weld_build(1005, &mut session.gameplay_rng);
            assert!(player.select_primary(52));
            let other = if caster == "owner" { "peer" } else { "owner" };
            session.participants.get_mut(other).unwrap().wizard.position = Vec2::new(90.0, 250.0);
            let intent =
                EnemySpawnIntent::new(EnemyToken::Skeleton, Vec2::new(400.0, 250.0), 1, 1, 0);
            session.step(
                &BTreeMap::new(),
                GameSessionTickContext {
                    navigation: Some(&navigation),
                    enemy_spawn_intents: &[intent],
                    ..Default::default()
                },
            );
            let mut enemy = session.boneyard.as_mut().unwrap().enemies.actors.get_mut(0).unwrap();
            enemy.config.maximum_health = 100_000.0;
            enemy.current_health = 100_000.0;
            let mut emissions = 0;
            let mut contact_particles = 0;
            for _ in 0..180 {
                let player = &mut session.participants.get_mut(caster).unwrap().player;
                player.combat.set_mana(reserve.unwrap_or(player.combat.maximum_mana));
                let result = session.step(
                    &BTreeMap::from([(
                        caster.into(),
                        GameSessionInput {
                            aim: Some(Vec2::new(450.0, 250.0)),
                            primary: true,
                            ..Default::default()
                        },
                    )]),
                    GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
                );
                for emission in &result.primary.unwrap().channel_emissions {
                    emissions += 1;
                    assert_eq!(emission.underpowered, reserve.is_some());
                }
                for actor in session.spells.transients.iter() {
                    let PrimaryTransientRef::Weld(WeldWorldRef::Steam(particle)) = actor else {
                        continue;
                    };
                    contact_particles += usize::from(particle.contact_enabled);
                    if reserve.is_some() {
                        assert_eq!(particle.alpha_multiplier, 0.25);
                    }
                }
            }
            assert!(emissions > 100, "the held Steam cast must reach its channel");
            let enemy = session.boneyard.as_ref().unwrap().enemies.actors.get(0).unwrap();
            assert!(
                enemy.current_health < enemy.config.maximum_health,
                "{caster} {reserve:?}: retail weak normal Steam still damages a mob"
            );
            assert!(contact_particles > 0, "{caster} {reserve:?}: normal contact remains enabled");
            // Stop casting. Already admitted particles/modifiers retire on their own clocks.
            for _ in 0..1100 {
                session.step(
                    &BTreeMap::new(),
                    GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
                );
            }
            assert!(
                !session.spells.transients.iter().any(|actor| matches!(
                    actor,
                    PrimaryTransientRef::Weld(WeldWorldRef::Steam(_))
                ))
            );
        }
    }
}

#[test]
fn boulder_real_cast_release_acquires_and_guidance_runs_after_the_world_move() {
    use solomon_sim::spells::weld::WeldPersistentRef;
    for funded in [true, false] {
        let (mut session, navigation) = drain_session();
        let player = &mut session.participants.get_mut("owner").unwrap().player;
        player.grant_weld_build(1006, &mut session.gameplay_rng);
        player.grant_skill_ranks(10, 2, &mut session.gameplay_rng);
        player.grant_skill_ranks(9, 3, &mut session.gameplay_rng);
        assert!(player.select_primary(52));
        session.step(
            &BTreeMap::new(),
            GameSessionTickContext {
                navigation: Some(&navigation),
                enemy_spawn_intents: &[EnemySpawnIntent::new(
                    EnemyToken::Skeleton,
                    Vec2::new(400.0, 100.0),
                    1,
                    1,
                    0,
                )],
                ..Default::default()
            },
        );
        {
            let mut enemy = session.boneyard.as_mut().unwrap().enemies.actors.get_mut(0).unwrap();
            enemy.config.maximum_health = 100_000.0;
            enemy.current_health = 100_000.0;
        }
        let mut birth = None;
        let mut guided = false;
        for tick in 0..150 {
            let player = &mut session.participants.get_mut("owner").unwrap().player;
            player.combat.set_mana(if funded { player.combat.maximum_mana } else { 0.0 });
            session.step(
                &BTreeMap::from([(
                    "owner".into(),
                    GameSessionInput {
                        aim: Some(Vec2::new(450.0, 250.0)),
                        primary: tick < 80,
                        ..Default::default()
                    },
                )]),
                GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
            );
            for actor in session.spells.transients.iter() {
                let PrimaryTransientRef::Weld(WeldWorldRef::Persistent(
                    WeldPersistentRef::EtherealBoulder(rock),
                )) = actor
                else {
                    continue;
                };
                if rock.flight_ticks == 0
                    && rock.phase == solomon_sim::spells::model::PrimarySpellProjectilePhase::Flight
                {
                    assert!(
                        rock.target_id.is_some(),
                        "real normal and weak release callers pass target rows"
                    );
                    assert_eq!(rock.reacquires_target, funded);
                    assert_eq!(rock.flight_speed, 2.25);
                    birth = Some((rock.common.owner.id, rock.common.origin, rock.heading_degrees));
                    break;
                }
                if let Some((id, origin, heading)) = birth
                    && rock.common.owner.id == id
                    && rock.flight_ticks == 1
                {
                    assert_ne!(rock.common.origin, origin, "the admitted actor moved");
                    assert_ne!(
                        rock.heading_degrees, heading,
                        "combat tail steers even without a contact"
                    );
                    guided = true;
                }
            }
            if guided {
                break;
            }
        }
        assert!(guided, "funded={funded} reaches a released actor and its next guidance tick");
    }
}
