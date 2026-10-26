#![cfg(feature = "persistence")]

use solomon_sim::Vec2;
use solomon_sim::enemies::config::EnemyArenaScalars;
use solomon_sim::enemies::model::{EnemyDamageRequest, EnemyTarget};
use solomon_sim::enemies::path::EnemyNavigation;
use solomon_sim::enemies::world::*;
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::painter::WorldPainterOrder;

struct ClearArena;

impl EnemyNavigation for ClearArena {
    fn direct_path_clear(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> bool {
        true
    }

    fn find_route(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> Option<Vec<Vec2>> {
        None
    }
}

impl EnemyGeometry for ClearArena {
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

fn step(world: &mut EnemyWorld, players: &[EnemyTarget], tick: u64, order: &mut WorldPainterOrder) {
    world.step(
        EnemyStepContext {
            enhanced_effects: true,
            tick,
            paused: false,
            players,
            geometry: &ClearArena,
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
        Some(order),
        None,
    );
}

#[test]
fn mixed_controllers_keep_identity_and_order_after_retirement_and_restore() {
    let mut live = EnemyWorld::new("enemy-ecs", 50);
    let mut order = WorldPainterOrder::default();
    let players = [EnemyTarget {
        id: "wizard".into(),
        summoned: false,
        alive: true,
        collision_radius: 25.0,
        connected: true,
        eligible: true,
        heading_deg: None,
        position: Vec2::new(450.0, 280.0),
        velocity_per_tick: Vec2::ZERO,
    }];
    let birth = |token, x| EnemySpawnIntent::new(token, Vec2::new(x, 250.0), 1, 0, 31);
    let spawn_context = || EnemySpawnContext {
        players: &players,
        tick: 0,
        arena_scalars: EnemyArenaScalars::default(),
        roll_loot_seed: None,
    };
    live.spawn(
        &[
            birth(EnemyToken::Skeleton, 150.0),
            birth(EnemyToken::SkeletonMage, 180.0),
            birth(EnemyToken::Imp, 210.0),
        ],
        spawn_context(),
        &ClearArena,
        Some(&mut order),
    );
    let admitted: Vec<_> = live.actors.iter().map(|actor| actor.id).collect();
    live.actors.remove(1);
    live.spawn(&[birth(EnemyToken::Wraith, 180.0)], spawn_context(), &ClearArena, Some(&mut order));
    let ids: Vec<_> = live.actors.iter().map(|actor| actor.id).collect();
    assert_eq!(&ids[..2], &[admitted[0], admitted[2]]);
    assert!(ids[2] > admitted[2]);
    live.set_hurricane_contact_cooldown(admitted[2], 15);
    let hit = EnemyDamageRequest::new(admitted[0], 0.125, 0, Some("wizard".into()));
    assert!(live.damage(hit, Some(&mut order)).accepted);

    let saved = serde_json::to_vec(&live).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    let rows = document["actors"].as_array().unwrap();
    assert!(rows.iter().all(|row| row.get("body").is_none()));
    assert!(rows.iter().all(|row| row.get("brain").is_some() && row.get("config").is_some()));
    let mut restored: EnemyWorld = serde_json::from_slice(&saved).unwrap();
    assert_eq!(serde_json::to_vec(&restored).unwrap(), saved);
    let mut restored_order = order;

    for tick in 1..=40 {
        step(&mut live, &players, tick, &mut order);
        step(&mut restored, &players, tick, &mut restored_order);
        assert_eq!(live.actors.iter().map(|actor| actor.id).collect::<Vec<_>>(), ids);
        assert_eq!(serde_json::to_vec(&restored).unwrap(), serde_json::to_vec(&live).unwrap());
        assert_eq!(restored_order, order);
    }
}
