//! Current damage can suppress movement reaction without hiding the Puppet hit flash.

use serde::Deserialize;
use solomon_sim::Vec2;
use solomon_sim::enemies::config::EnemyArenaScalars;
use solomon_sim::enemies::model::{EnemyDamageRequest, EnemyTarget};
use solomon_sim::enemies::path::EnemyNavigation;
use solomon_sim::enemies::world::*;
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::painter::WorldPainterOrder;

#[path = "current_enemy_hit_goldens/capture.rs"]
mod capture;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Goldens {
    source_revision: String,
    cases: Vec<HitCase>,
    capture_cases: Vec<capture::CaptureCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HitCase {
    seed: String,
    token: String,
    hit_strength: f64,
    suppress_hit_reaction: bool,
    result: HitResult,
    frames: Vec<HitFrame>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HitResult {
    accepted: bool,
    health_damage: f64,
    killed: bool,
    special_death_accepted: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HitFrame {
    tick: u64,
    health: f64,
    hit: HitFeedback,
    reaction: f64,
    position: Point,
    next_movement_tick: u64,
    last_movement_tick: Option<u64>,
    rng: u32,
}

#[derive(Deserialize)]
struct HitFeedback {
    strength: f64,
    tick: u64,
    timer: f64,
}

#[derive(Deserialize)]
struct Point {
    x: f64,
    y: f64,
}

struct ClearWorld;

impl EnemyNavigation for ClearWorld {
    fn direct_path_clear(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> bool {
        true
    }

    fn find_route(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> Option<Vec<Vec2>> {
        None
    }
}

impl EnemyGeometry for ClearWorld {
    fn resolve_movement(&self, request: EnemyMovementRequest) -> Vec2 {
        request.requested_position
    }

    fn line_obstruction(&self, _: Vec2, _: Vec2, _: u32) -> Option<Vec2> {
        None
    }

    fn projectile_world_blocked(&self, _: EnemyProjectileWorldQuery) -> bool {
        false
    }
}

#[test]
fn current_hit_reaction_and_visible_feedback_have_independent_lifetimes() {
    let fixture = include_str!("fixtures/website/current-enemy-hit-goldens.json");
    let goldens: Goldens = serde_json::from_str(fixture).unwrap();
    assert_eq!(goldens.source_revision, "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in goldens.cases {
        let mut world = EnemyWorld::new(&case.seed, 50);
        let mut order = WorldPainterOrder::default();
        let players = [EnemyTarget {
            id: "hero".into(),
            summoned: false,
            alive: true,
            collision_radius: 25.0,
            connected: true,
            eligible: true,
            heading_deg: None,
            position: Vec2::new(450.0, 280.0),
            velocity_per_tick: Vec2::ZERO,
        }];
        world.spawn(
            &[EnemySpawnIntent::new(
                EnemyToken::from_id(&case.token).unwrap(),
                Vec2::new(150.0, 250.0),
                1,
                0,
                31,
            )],
            EnemySpawnContext {
                players: &players,
                tick: 0,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: None,
            },
            &ClearWorld,
            Some(&mut order),
        );
        // Source finishes the independently constructed initial store at tick zero.
        world.last_step_tick = Some(0);
        let mut request =
            EnemyDamageRequest::new(world.actors.get(0).unwrap().id, 0.125, 0, Some("hero".into()));
        request.hit_strength = case.hit_strength;
        request.suppress_hit_reaction = case.suppress_hit_reaction;
        let result = world.damage(request, Some(&mut order));
        assert_eq!(result.accepted, case.result.accepted, "{} accepted", case.seed);
        assert_eq!(result.health_damage, case.result.health_damage, "{} health damage", case.seed);
        assert_eq!(result.killed, case.result.killed, "{} killed", case.seed);
        assert!(!case.result.special_death_accepted, "nonlethal source fixture");
        for frame in case.frames {
            let tick = frame.tick;
            if tick > 0 {
                world.step(
                    EnemyStepContext {
                        enhanced_effects: true,
                        tick,
                        paused: false,
                        players: &players,
                        geometry: &ClearWorld,
                        ability_effects: &[],
                        arena_scalars: EnemyArenaScalars::default(),
                        dialogue_busy: false,
                        roll_loot_seed: None,
                        native_visibility: None,
                        projected_point_visible: None,
                        native_view_bounds: None,
                        puppet_targets: &[],
                        light_at: None,
                        native_movement_view: None,
                    },
                    Some(&mut order),
                    None,
                );
            }
            let actor = world.actors.get(0).unwrap();
            let context = format!("{} tick{tick}", case.seed);
            assert_eq!(actor.current_health, frame.health, "{context} health");
            assert_eq!(actor.hit_feedback.strength, frame.hit.strength, "{context} hit strength");
            assert_eq!(actor.hit_feedback.tick, frame.hit.tick, "{context} hit tick");
            assert_eq!(actor.hit_feedback.timer, frame.hit.timer, "{context} visible timer");
            assert_eq!(actor.hit_reaction_timer, frame.reaction, "{context} reaction timer");
            assert_eq!(
                actor.position,
                Vec2::new(frame.position.x, frame.position.y),
                "{context} root"
            );
            assert_eq!(actor.next_movement_tick, frame.next_movement_tick, "{context} next move");
            assert_eq!(actor.last_movement_tick, frame.last_movement_tick, "{context} last move");
            assert_eq!(world.rng_state, frame.rng, "{context} actor RNG");
        }
    }
}
