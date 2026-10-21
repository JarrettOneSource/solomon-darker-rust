//! Actual current Source Wraith epochs, including degraded cadence and shared RNG.

use serde::Deserialize;
use solomon_sim::Vec2;
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::enemies::config::EnemyArenaScalars;
use solomon_sim::enemies::model::{EnemyBrain, EnemyTarget};
use solomon_sim::enemies::path::{EnemyNavigation, NativeEnemyPathState};
use solomon_sim::enemies::world::*;
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::painter::WorldPainterOrder;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WraithWorld {
    seed: String,
    view: String,
    has_target: bool,
    frames: Vec<WraithWorldFrame>,
}

#[derive(Deserialize)]
struct WraithWorldFrame {
    tick: u64,
    movement: [f64; 5],
    flight: [f64; 7],
    recipient: bool,
    path: [f64; 20],
    rng: u32,
    steering: Vec<f64>,
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

fn path_values(path: NativeEnemyPathState) -> [f64; 20] {
    let previous = path.route_previous_vector.unwrap_or(Vec2::ZERO);
    let waypoints = path.route_waypoints.unwrap_or([Vec2::ZERO; 2]);
    [
        path.base_turn_rate,
        path.flank_angle_deg,
        path.flank_radius,
        path.flank_ticks_remaining as f64,
        path.reorientation_ticks_remaining as f64,
        if path.route_previous_vector.is_some() { 1.0 } else { 0.0 },
        previous.x,
        previous.y,
        path.route_refresh_ticks_remaining as f64,
        path.route_ticks_remaining as f64,
        path.route_waypoint_index as f64,
        if path.route_waypoints.is_some() { 1.0 } else { 0.0 },
        waypoints[0].x,
        waypoints[0].y,
        waypoints[1].x,
        waypoints[1].y,
        path.speed_factor,
        path.stalled_movement_ticks as f64,
        path.turn_factor,
        path.wander_heading_deg,
    ]
}

#[test]
fn current_wraith_world_motion_and_shared_random_streams_match_the_source() {
    let fixture = include_str!("../fixtures/website/current-enemy-goldens.json");
    let goldens: super::CurrentEnemyGoldens = serde_json::from_str(fixture).unwrap();
    for case in goldens.wraith_worlds {
        let mut players = [EnemyTarget {
            id: "hero".into(),
            summoned: false,
            alive: true,
            collision_radius: 25.0,
            connected: true,
            eligible: case.has_target,
            heading_deg: None,
            position: Vec2::new(210.0, 280.0),
            velocity_per_tick: Vec2::new(0.75, -0.25),
        }];
        let mut world = EnemyWorld::new(&case.seed, 50);
        let mut order = WorldPainterOrder::default();
        world.spawn(
            &[EnemySpawnIntent::new(EnemyToken::Wraith, Vec2::new(150.0, 250.0), 1, 0, 31)],
            EnemySpawnContext {
                players: &players,
                tick: 0,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: None,
            },
            &ClearWorld,
            Some(&mut order),
        );
        let admitted = case.view != "unlit";
        let intensity = if admitted { 1.0 } else { 0.0 };
        let visible = |_| NativeEnemyVisibility { admitted, intensity };
        let light_at = |_| intensity;
        let cameras = [if case.view == "offscreen" {
            BoneyardBounds::new(2000.0, 2000.0, 500.0, 500.0)
        } else {
            BoneyardBounds::new(0.0, 0.0, 1000.0, 1000.0)
        }];
        for frame in case.frames {
            let tick = frame.tick;
            players[0].position = Vec2::new(210.0 + tick as f64 * 0.75, 280.0 - tick as f64 * 0.25);
            let time_scale = if (110..160).contains(&tick) {
                0.0
            } else if (480..500).contains(&tick) {
                0.35
            } else {
                1.0
            };
            let effects = [(
                world.actors[0].id,
                EnemyAbilityEffect {
                    time_scale,
                    disrupted_ticks: u64::from((240..260).contains(&tick)),
                    ..EnemyAbilityEffect::default()
                },
            )];
            world.step(
                EnemyStepContext {
                    enhanced_effects: true,
                    tick,
                    paused: false,
                    players: &players,
                    geometry: &ClearWorld,
                    ability_effects: &effects,
                    arena_scalars: EnemyArenaScalars::default(),
                    dialogue_busy: false,
                    roll_loot_seed: None,
                    native_visibility: Some(&visible),
                    projected_point_visible: None,
                    native_view_bounds: None,
                    puppet_targets: &[],
                    light_at: Some(&light_at),
                    native_movement_view: Some(NativeEnemyMovementView {
                        arena_bounds: BoneyardBounds::new(0.0, 0.0, 1000.0, 1000.0),
                        cameras: &cameras,
                        enhanced_effects: case.view != "basic",
                    }),
                },
                Some(&mut order),
                None,
            );
            let actor = &world.actors[0];
            let EnemyBrain::Wraith(brain) = &actor.brain else { panic!("live Wraith brain") };
            let context = format!("{} tick{tick}", case.seed);
            assert_eq!(
                [
                    actor.position.x,
                    actor.position.y,
                    actor.heading_deg,
                    actor.last_movement_tick.map_or(-1.0, |tick| tick as f64),
                    actor.next_movement_tick as f64,
                ],
                frame.movement,
                "{context} motion"
            );
            assert_eq!(super::flight_values(brain), frame.flight, "{context} flight");
            assert_eq!(brain.collision_recipient, frame.recipient, "{context} collision recipient");
            assert_eq!(path_values(actor.path), frame.path, "{context} path");
            assert_eq!(world.rng_state, frame.rng, "{context} actor RNG");
            let mut steering =
                vec![world.steering_rng.index_a as f64, world.steering_rng.index_b as f64];
            steering.extend(world.steering_rng.words.map(f64::from));
            assert_eq!(steering, frame.steering, "{context} steering RNG");
        }
    }
}
