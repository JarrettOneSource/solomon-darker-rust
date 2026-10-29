//! Pike placement through the real Boneyard movement and enemy-spawn owners.

use std::collections::BTreeMap;

use solomon_assets::boneyard::BoneyardCatalog;
use solomon_sim::Vec2;
use solomon_sim::boneyard::arena::ArenaPhase;
use solomon_sim::boneyard::collision::can_place_boneyard_body;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::enemies::model::{EnemyBrain, SkeletonPhase, SkeletonPike};
use solomon_sim::enemies::{EnemyFlag, EnemySpawnIntent, EnemyToken};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameSession, GameSessionOptions, GameSessionTickContext};

const PINNED: Vec2 = Vec2::new(843.515869140625, 1999.8958740234375);
const PIKEMAN: Vec2 = Vec2::new(936.0854756088522, 1954.7309953258293);

fn arena(position: Vec2) -> (GameSession, NavigationSet) {
    let mut seed = [0; 16];
    seed[..4].copy_from_slice(&20_261_028_u32.to_be_bytes());
    let loaded = BoneyardCatalog::load_embedded().unwrap().materialize(seed);
    assert_eq!(
        loaded.source_sha256,
        "2118053783606f5ef9dc848671d6eecd8e87aa0a3610c8c2119f08452e15a22f"
    );
    let navigation = NavigationSet::for_scene(&loaded);
    let mut game = GameSession::new(
        "wizard".into(),
        NewGameSelection {
            name: "Pinned wizard".into(),
            element: WizardElement::Fire,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    game.install_boneyard(loaded).unwrap();
    let world = game.boneyard.as_mut().unwrap();
    world.encounter = None;
    world.waves = None;
    world.lantern_position = None;
    world.arena_transition.as_mut().unwrap().phase = ArenaPhase::Sealed;
    game.participants.get_mut("wizard").unwrap().wizard.position = position;
    (game, navigation)
}

fn legal(game: &GameSession, position: Vec2) -> bool {
    let world = game.boneyard.as_ref().unwrap();
    can_place_boneyard_body(position, world.active_bounds(), &world.dynamic_collision, 25.0)
}

fn next_birth(game: &mut GameSession, navigation: &NavigationSet) {
    // The failed wave-54 request. The wizard's footprint is the destination
    // constraint, independent of this small enemy's placement radius.
    let mut birth = EnemySpawnIntent::new(
        EnemyToken::Imp,
        Vec2::new(2005.711669921875, 758.7805786132813),
        2,
        game.ticks + 1,
        54,
    );
    birth.placement_radius = Some(9.806253563147038);
    birth.reachability_radius = Some(9.806253563147038);
    let before = game.boneyard.as_ref().unwrap().enemies.actors.len();
    let result = game.step(
        &BTreeMap::new(),
        GameSessionTickContext {
            navigation: Some(navigation),
            enemy_spawn_intents: &[birth],
            ..Default::default()
        },
    );
    assert!(result.advanced);
    assert_eq!(game.boneyard.as_ref().unwrap().enemies.actors.len(), before + 1);
}

fn latch(game: &mut GameSession, navigation: &NavigationSet) {
    let current = game.participants.get("wizard").unwrap().wizard.position;
    let mut birth = EnemySpawnIntent::new(EnemyToken::Skeleton, PIKEMAN, 1, 0, 54);
    birth.flags.push(EnemyFlag::Pike);
    game.step(
        &BTreeMap::new(),
        GameSessionTickContext {
            navigation: Some(navigation),
            enemy_spawn_intents: &[birth],
            ..Default::default()
        },
    );
    {
        let mut actor = game.boneyard.as_mut().unwrap().enemies.actors.get_mut(0).unwrap();
        actor.position = PIKEMAN;
        actor.next_movement_tick = u64::MAX;
        actor.target_player_id = Some("wizard".into());
        let EnemyBrain::Skeleton(brain) = &mut actor.brain else { panic!("Pike skeleton") };
        brain.phase = SkeletonPhase::Attack;
        brain.marker_emitted = true;
        brain.pike =
            Some(SkeletonPike { player_id: "wizard".into(), position: current, distance: 103.0 });
    }
}

#[test]
fn pike_pull_keeps_the_wizard_and_retained_pin_outside_scenery_before_the_next_birth() {
    // A legal point on the observed Pike's ray requests the same blocked root
    // as the wave-54 run. This isolates placement without replaying its combat.
    let current = Vec2::new((PIKEMAN.x + PINNED.x) / 2.0, (PIKEMAN.y + PINNED.y) / 2.0);
    let (mut game, navigation) = arena(current);
    assert!(legal(&game, current));
    assert!(!legal(&game, PINNED));
    latch(&mut game, &navigation);
    game.step(
        &BTreeMap::new(),
        GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
    );
    let position = game.participants.get("wizard").unwrap().wizard.position;
    assert!(legal(&game, position), "Pike committed a blocked wizard root: {position:?}");
    assert_ne!(position, current, "the constraint still moves the wizard");
    let actor = game.boneyard.as_ref().unwrap().enemies.actors.get(0).unwrap();
    let EnemyBrain::Skeleton(brain) = &actor.brain else { panic!("Pike skeleton") };
    assert_eq!(brain.pike.as_ref().unwrap().position, position);
    next_birth(&mut game, &navigation);
}

#[test]
fn a_clear_pike_constraint_keeps_its_native_distance() {
    let current = Vec2::new(PIKEMAN.x + 100.0, PIKEMAN.y);
    let (mut game, navigation) = arena(current);
    assert!(legal(&game, current));
    latch(&mut game, &navigation);
    game.step(
        &BTreeMap::new(),
        GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
    );
    let position = game.participants.get("wizard").unwrap().wizard.position;
    assert!(legal(&game, position));
    assert_eq!(position, Vec2::new(1039.08544921875, 1954.73095703125));
    let actor = game.boneyard.as_ref().unwrap().enemies.actors.get(0).unwrap();
    let EnemyBrain::Skeleton(brain) = &actor.brain else { panic!("Pike skeleton") };
    assert_eq!(brain.pike.as_ref().unwrap().position, position);
}

#[test]
fn an_existing_blocked_wizard_root_recovers_before_enemy_spawn_reachability() {
    let (mut game, navigation) = arena(PINNED);
    assert!(!legal(&game, PINNED));
    next_birth(&mut game, &navigation);
    assert!(legal(&game, game.participants.get("wizard").unwrap().wizard.position));
}
