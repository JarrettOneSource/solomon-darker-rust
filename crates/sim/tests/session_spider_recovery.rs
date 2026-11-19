//! Retail Mod_Webbed (0x00623BA0) wears partial Silk off through the retained movement
//! lane. PlayerWizard (0x0054B001) applies its slowdown only to collision displacement.

use std::collections::BTreeMap;

use solomon_sim::Vec2;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardScene, BoneyardSource, BoneyardSpawn, LoadedBoneyard,
};
use solomon_sim::enemies::model::EnemyTarget;
use solomon_sim::enemies::webbed::{apply_webbed, webbed_movement_scale};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::session::{
    GameSession, GameSessionInput, GameSessionOptions, GameSessionTickContext,
};

fn webbed_session(stacks: usize) -> GameSession {
    let mut game = GameSession::new(
        "wizard".into(),
        NewGameSelection {
            name: "Wizard".into(),
            element: WizardElement::Fire,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    let position = Vec2::new(500.0, 500.0);
    game.install_boneyard(LoadedBoneyard {
        choice_id: "silk-recovery".into(),
        choice_source: BoneyardSource::Default,
        mod_id: None,
        mod_name: None,
        name: "Silk recovery".into(),
        run_id: "silk-recovery".into(),
        seed: "00000001000000000000000000000000".into(),
        source_sha256: "a".repeat(64),
        geometry_sha256: "b".repeat(64),
        scene: BoneyardScene {
            name: "Silk recovery".into(),
            environment_mode: 0,
            bounds: BoneyardBounds::new(0.0, 0.0, 4000.0, 4000.0),
            spawn: BoneyardSpawn { position, facing_degrees: 90.0 },
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
    let world = game.boneyard.as_mut().unwrap();
    world.encounter = None;
    world.arena_transition = None;
    world.waves = None;
    let mut web = None;
    for _ in 0..stacks {
        web = Some(apply_webbed(web, 10.0));
    }
    if let Some(web) = web {
        world.enemies.webbed_players.insert("wizard".into(), web);
    }
    if stacks == 3 {
        let owner = EnemyTarget {
            id: "wizard".into(),
            summoned: false,
            alive: true,
            connected: true,
            eligible: true,
            collision_radius: 25.0,
            heading_deg: Some(90.0),
            position,
            velocity_per_tick: Vec2::ZERO,
        };
        drop(world.enemies.add_cocoon(
            "wizard",
            position,
            &[owner],
            game.ticks,
            Some(&mut game.painter_order),
        ));
    }
    game
}

fn step(game: &mut GameSession, ticks: usize, movement: Vec2) {
    let navigation = NavigationSet::for_scene(&game.boneyard.as_ref().unwrap().loaded);
    let inputs =
        BTreeMap::from([("wizard".into(), GameSessionInput { movement, ..Default::default() })]);
    for _ in 0..ticks {
        assert!(
            game.step(
                &inputs,
                GameSessionTickContext { navigation: Some(&navigation), ..Default::default() },
            )
            .advanced
        );
    }
}

#[test]
fn partial_webs_scale_collision_travel_without_attenuating_the_movement_lane() {
    let mut clear = webbed_session(0);
    step(&mut clear, 1, Vec2::new(1.0, 1.0));
    let raw = &clear.participants["wizard"].wizard;
    assert_eq!(webbed_movement_scale(None), 1.0);
    // Mod_Webbed's +0x218 float32 store, recovered independently of the old recording.
    for (stacks, bits) in [(1, 0x3f27_b961), (2, 0x3e9e_e585)] {
        let mut game = webbed_session(stacks);
        let web = game.boneyard.as_ref().unwrap().enemies.webbed_players["wizard"];
        let scale = f32::from_bits(bits) as f64;
        assert_eq!(webbed_movement_scale(Some(&web)), scale);
        step(&mut game, 1, Vec2::new(1.0, 1.0));
        let slowed = &game.participants["wizard"].wizard;
        assert_eq!(slowed.velocity, raw.velocity);
        assert_eq!(slowed.gait_degrees, raw.gait_degrees);
        assert_eq!(slowed.heading_degrees, raw.heading_degrees);
        for (actual, unscaled) in
            [(slowed.position.x, raw.position.x), (slowed.position.y, raw.position.y)]
        {
            assert!(((actual - 500.0) - (unscaled - 500.0) * scale).abs() < 0.0001);
            assert!(actual > 500.0 && actual < unscaled);
        }
    }
}

#[test]
fn one_silk_stack_wears_off_while_walking_without_another_hit() {
    let mut game = webbed_session(1);
    step(&mut game, 1500, Vec2::new(1.0, 0.0));
    assert!(!game.boneyard.as_ref().unwrap().enemies.webbed_players.contains_key("wizard"));
    assert!(game.participants["wizard"].wizard.position.x > 500.0);
}

#[test]
fn two_silk_stacks_wear_off_while_walking_without_a_third_hit() {
    let mut game = webbed_session(2);
    step(&mut game, 1500, Vec2::new(1.0, 0.0));
    let web = game.boneyard.as_ref().unwrap().enemies.webbed_players["wizard"];
    assert!(web.severity > 0.0 && web.severity < 0.75, "partial recovery: {web:?}");
    step(&mut game, 750, Vec2::new(1.0, 0.0));
    assert!(!game.boneyard.as_ref().unwrap().enemies.webbed_players.contains_key("wizard"));
}

#[test]
fn stationary_partial_webs_do_not_expire_or_depend_on_a_living_spider() {
    for stacks in [1, 2] {
        let mut game = webbed_session(stacks);
        step(&mut game, 2250, Vec2::ZERO);
        assert_eq!(
            game.boneyard.as_ref().unwrap().enemies.webbed_players["wizard"].severity,
            stacks as f64,
        );
        assert_eq!(game.participants["wizard"].wizard.position, Vec2::new(500.0, 500.0));
    }
}

#[test]
fn full_cocoons_hold_the_root_but_movement_still_admits_staff_escape() {
    let mut game = webbed_session(3);
    step(&mut game, 600, Vec2::ZERO);
    assert_eq!(game.participants["wizard"].wizard.position, Vec2::new(500.0, 500.0));
    let web = game.boneyard.as_ref().unwrap().enemies.webbed_players["wizard"];
    assert_eq!(web.severity, 3.0);
    assert_eq!(web.cocoon_health, 10.0);
    step(&mut game, 5, Vec2::new(1.0, 0.0));
    assert_eq!(game.participants["wizard"].wizard.position, Vec2::new(500.0, 500.0));
    assert!(game.participants["wizard"].wizard.gait_degrees > 0.0);
    step(&mut game, 995, Vec2::new(1.0, 0.0));
    assert!(!game.boneyard.as_ref().unwrap().enemies.webbed_players.contains_key("wizard"));
    assert!(game.participants["wizard"].wizard.position.x > 500.0);
}

#[cfg(feature = "persistence")]
#[test]
fn retained_partial_and_full_webs_recover_after_checkpoint_restore() {
    for stacks in 1..=3 {
        let game = webbed_session(stacks);
        let loaded = game.boneyard.as_ref().unwrap().loaded.clone();
        let expected = game.boneyard.as_ref().unwrap().enemies.webbed_players["wizard"];
        let bytes = serde_json::to_vec(&game.checkpoint()).unwrap();
        let checkpoint = serde_json::from_slice(&bytes).unwrap();
        let mut restored = GameSession::restore(checkpoint, Some(loaded)).unwrap();
        assert_eq!(restored.boneyard.as_ref().unwrap().enemies.webbed_players["wizard"], expected);
        step(&mut restored, 2250, Vec2::new(1.0, 0.0));
        assert!(
            !restored.boneyard.as_ref().unwrap().enemies.webbed_players.contains_key("wizard"),
            "restored {stacks}-stack modifier did not recover",
        );
    }
}

#[test]
fn walking_at_a_wall_clears_only_the_moving_players_webs() {
    let mut game = webbed_session(2);
    assert!(game.add_participant(
        "idle".into(),
        NewGameSelection {
            name: "Idle".into(),
            element: WizardElement::Water,
            discipline: WizardDiscipline::Body,
        },
    ));
    let idle_position = Vec2::new(1500.0, 1500.0);
    game.participants.get_mut("idle").unwrap().wizard.position = idle_position;
    let enemies = &mut game.boneyard.as_mut().unwrap().enemies;
    let web = enemies.webbed_players["wizard"];
    enemies.webbed_players.insert("idle".into(), web);
    step(&mut game, 2250, Vec2::new(-1.0, 0.0));
    let enemies = &game.boneyard.as_ref().unwrap().enemies;
    assert!(!enemies.webbed_players.contains_key("wizard"));
    assert_eq!(enemies.webbed_players["idle"], web);
    assert!(game.participants["wizard"].wizard.position.x < 50.0);
    assert_eq!(game.participants["idle"].wizard.position, idle_position);
    assert!(game.remove_participant("idle"));
    assert!(!game.boneyard.as_ref().unwrap().enemies.webbed_players.contains_key("idle"));
}
