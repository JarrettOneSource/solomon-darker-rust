//! Spell-driven bodies must stay in the same playable domain as ordinary movement.

use std::collections::BTreeMap;

use solomon_sim::Vec2;
use solomon_sim::belt::NativeBeltEntry;
use solomon_sim::boneyard::arena::{ArenaPhase, ArenaTransition};
use solomon_sim::boneyard::collision::can_place_boneyard_body;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardScene, BoneyardSource, BoneyardSpawn, LoadedBoneyard,
};
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::secondary::{NativeSecondaryAbilityId, SecondaryActorRead, SecondaryActorWrite};
use solomon_sim::session::{GameSession, GameSessionOptions, GameSessionTickContext};

fn arena_session(north: bool, phase: ArenaPhase, skill: NativeSecondaryAbilityId) -> GameSession {
    let mut game = GameSession::new(
        "wizard".into(),
        NewGameSelection {
            name: "Wizard".into(),
            element: WizardElement::Fire,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    let bounds = BoneyardBounds::new(0.0, 0.0, 1200.0, 1800.0);
    let spawn = Vec2::new(600.0, if north { 100.0 } else { 1700.0 });
    game.install_boneyard(LoadedBoneyard {
        choice_id: "arena-spells".into(),
        choice_source: BoneyardSource::Default,
        mod_id: None,
        mod_name: None,
        name: "Arena spells".into(),
        run_id: "arena-spells".into(),
        seed: "00000001000000000000000000000000".into(),
        source_sha256: "a".repeat(64),
        geometry_sha256: "b".repeat(64),
        scene: BoneyardScene {
            name: "Arena spells".into(),
            environment_mode: 0,
            bounds,
            spawn: BoneyardSpawn { position: spawn, facing_degrees: 180.0 },
            objects: vec![],
            fences: vec![],
            sprites: vec![],
            roads: vec![],
            terrain: vec![],
            solomon_dig: None,
        }
        .into(),
    })
    .unwrap();
    let mut arena = ArenaTransition::new(bounds, spawn);
    arena.phase = phase;
    game.boneyard.as_mut().unwrap().arena_transition = Some(arena);
    let participant = game.participants.get_mut("wizard").unwrap();
    participant.wizard.position = Vec2::new(600.0, if north { 1500.0 } else { 500.0 });
    let skill = skill as usize;
    participant.player.book.permanent_ranks[skill] = 1;
    participant.player.book.effective_ranks[skill] = 1;
    participant.player.belt.entries[0] = Some(NativeBeltEntry::Skill { skill_id: skill });
    participant.player.combat.current_mana = 1000.0;
    participant.player.combat.maximum_mana = 1000.0;
    game
}

#[test]
fn teleport_keeps_wizards_inside_the_active_arena_and_enemy_births_reachable() {
    for north in [true, false] {
        for phase in [ArenaPhase::Open, ArenaPhase::Locking, ArenaPhase::Sealed] {
            for seed in 1..=8 {
                let mut game = arena_session(north, phase, NativeSecondaryAbilityId::Teleport);
                game.secondary.rng = solomon_sim::NativeRng::new(seed);
                let before = game.participants.get("wizard").unwrap().wizard.position;
                assert!(game.activate_belt_slot("wizard", 0, None).transaction.accepted);
                let position = game.participants.get("wizard").unwrap().wizard.position;
                assert_ne!(position, before, "the ordinary cast must teleport the wizard");
                let world = game.boneyard.as_ref().unwrap();
                assert!(
                    can_place_boneyard_body(
                        position,
                        world.active_bounds(),
                        &world.dynamic_collision,
                        40.0
                    ),
                    "Teleport left the playable arena: north={north}, phase={phase:?}, seed={seed}, position={position:?}",
                );
                let navigation = NavigationSet::for_scene(&world.loaded);
                let birth =
                    EnemySpawnIntent::new(EnemyToken::Zombie, Vec2::new(600.0, 900.0), 1, 0, 0);
                let step = game.step(
                    &BTreeMap::new(),
                    GameSessionTickContext {
                        navigation: Some(&navigation),
                        enemy_spawn_intents: &[birth],
                        ..Default::default()
                    },
                );
                assert!(step.advanced);
                assert_eq!(game.boneyard.as_ref().unwrap().enemies.actors.len(), 1);
            }
        }
    }
}

#[test]
fn phase_cannot_cross_a_closed_entrance_but_can_enter_an_open_one() {
    for north in [true, false] {
        for phase in [ArenaPhase::Open, ArenaPhase::Locking, ArenaPhase::Sealed] {
            let mut game = arena_session(north, phase, NativeSecondaryAbilityId::Phasing);
            let position = Vec2::new(600.0, if north { 400.0 } else { 1375.0 });
            let participant = game.participants.get_mut("wizard").unwrap();
            participant.wizard.position = position;
            participant.wizard.heading_index = if north { 0 } else { 12 };
            assert!(game.activate_belt_slot("wizard", 0, None).transaction.accepted);
            let after = game.participants.get("wizard").unwrap().wizard.position;
            if phase == ArenaPhase::Open {
                assert_ne!(after, position);
            } else {
                assert_eq!(after, position, "Phase must reject the excluded entrance");
            }
        }
    }
}

#[test]
fn summoned_golems_resolve_their_body_inside_the_playable_arena() {
    for north in [true, false] {
        for phase in [ArenaPhase::Open, ArenaPhase::Locking, ArenaPhase::Sealed] {
            let mut game = arena_session(north, phase, NativeSecondaryAbilityId::RaiseGolem);
            let participant = game.participants.get_mut("wizard").unwrap();
            participant.wizard.position = Vec2::new(600.0, if north { 400.0 } else { 1375.0 });
            participant.wizard.heading_degrees = if north { 0.0 } else { 180.0 };
            assert!(game.activate_belt_slot("wizard", 0, None).transaction.accepted);
            let golem = game
                .secondary
                .actors
                .iter()
                .find(|actor| actor.kind == solomon_sim::secondary::NativeSecondaryActorKind::Golem)
                .expect("Raise Golem must create its actor");
            let world = game.boneyard.as_ref().unwrap();
            assert!(
                can_place_boneyard_body(
                    golem.position,
                    world.active_bounds(),
                    &world.dynamic_collision,
                    25.0,
                ),
                "Golem was placed beyond the active arena: north={north}, phase={phase:?}"
            );
            let articulation = golem.golem().unwrap();
            for foot in [articulation.left_foot, articulation.right_foot] {
                assert!(can_place_boneyard_body(
                    foot,
                    world.active_bounds(),
                    &world.dynamic_collision,
                    0.0,
                ));
            }
        }
    }
}

#[test]
fn walking_golems_keep_their_body_and_feet_inside_the_active_arena() {
    for north in [true, false] {
        for phase in [ArenaPhase::Open, ArenaPhase::Locking, ArenaPhase::Sealed] {
            let mut game = arena_session(north, phase, NativeSecondaryAbilityId::RaiseGolem);
            game.participants.get_mut("wizard").unwrap().wizard.position =
                Vec2::new(600.0, if north { 400.0 } else { 1375.0 });
            assert!(game.activate_belt_slot("wizard", 0, None).transaction.accepted);
            {
                let mut actor =
                    game.secondary.actors.iter_mut().find(|actor| actor.golem().is_some()).unwrap();
                let origin = Vec2::new(600.0, if north { 405.0 } else { 1370.0 });
                let rotation = if north { 0.0 } else { std::f64::consts::PI };
                actor.position = origin;
                actor.age_ticks = 400;
                actor.rotation_radians = rotation;
                let golem = actor.golem_mut().unwrap();
                golem.phase = solomon_sim::secondary::golem::NativeGolemPhase::Active;
                golem.orbit_heading_radians = Some(rotation);
                golem.gait_tick = 99;
                golem.left_foot = origin;
                golem.right_foot = origin;
                golem.left_foot_next = origin;
                golem.right_foot_next = origin;
                golem.left_foot_previous = origin;
                golem.right_foot_previous = origin;
            }
            let navigation = NavigationSet::for_scene(&game.boneyard.as_ref().unwrap().loaded);
            assert!(
                game.step(
                    &BTreeMap::new(),
                    GameSessionTickContext { navigation: Some(&navigation), ..Default::default() }
                )
                .advanced
            );
            let actor = game.secondary.actors.iter().find(|actor| actor.golem().is_some()).unwrap();
            assert_eq!(actor.age_ticks, 401);
            let world = game.boneyard.as_ref().unwrap();
            assert!(
                can_place_boneyard_body(
                    actor.position,
                    world.active_bounds(),
                    &world.dynamic_collision,
                    30.0,
                ),
                "Golem walked beyond the arena: north={north}, phase={phase:?}"
            );
            let golem = actor.golem().unwrap();
            for foot in
                [golem.left_foot, golem.right_foot, golem.left_foot_next, golem.right_foot_next]
            {
                assert!(can_place_boneyard_body(
                    foot,
                    world.active_bounds(),
                    &world.dynamic_collision,
                    0.0,
                ));
            }
        }
    }
}
