//! Retail spell contacts, participant feedback, and low-mana branches.

use std::collections::BTreeMap;

use solomon_sim::Vec2;
use solomon_sim::belt::NativeBeltCommand;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardScene, BoneyardSource, BoneyardSpawn, LoadedBoneyard,
};
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::enemies::model::EnemyEventKind;
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
fn steam_requires_funded_normal_particles_to_damage_a_mob() {
    for funded in [false, true] {
        let (mut session, navigation) = drain_session();
        let player = &mut session.participants.get_mut("owner").unwrap().player;
        player.grant_weld_build(1005, &mut session.gameplay_rng);
        assert!(player.select_primary(52));
        session.participants.get_mut("peer").unwrap().wizard.position = Vec2::new(90.0, 250.0);
        let intent = EnemySpawnIntent::new(EnemyToken::Skeleton, Vec2::new(400.0, 250.0), 1, 1, 0);
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
            let player = &mut session.participants.get_mut("owner").unwrap().player;
            player.combat.set_mana(if funded { player.combat.maximum_mana } else { 0.0 });
            let result = session.step(
                &BTreeMap::from([(
                    "owner".into(),
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
                assert_eq!(emission.underpowered, !funded);
            }
            contact_particles += session
                .spells
                .transients
                .iter()
                .filter(|actor| {
                    matches!(actor, PrimaryTransientRef::Weld(WeldWorldRef::Steam(particle))
                    if particle.contact_enabled)
                })
                .count();
        }
        assert!(emissions > 100, "the held Steam cast must reach its channel");
        let enemy = session.boneyard.as_ref().unwrap().enemies.actors.get(0).unwrap();
        if funded {
            assert!(contact_particles > 0);
            assert!(
                enemy.current_health < enemy.config.maximum_health,
                "funded Steam must hit the mob"
            );
        } else {
            // Retail 0x00542D20 / 0x0045B940: weak particles have no Mod_Steamed lane.
            assert_eq!(contact_particles, 0);
            assert_eq!(enemy.current_health, enemy.config.maximum_health);
        }
    }
}
