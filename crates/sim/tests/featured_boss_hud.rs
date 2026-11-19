use solomon_sim::Vec2;
use solomon_sim::enemies::config::EnemyArenaScalars;
use solomon_sim::enemies::world::{
    EnemyGeometry, EnemyMovementRequest, EnemyProjectileWorldQuery, EnemySpawnContext, EnemyWorld,
};
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::hud::HudBoss;

struct OpenArena;
impl solomon_sim::enemies::path::EnemyNavigation for OpenArena {
    fn direct_path_clear(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> bool {
        true
    }
    fn find_route(&self, _: Vec2, end: Vec2, _: f64, _: f64) -> Option<Vec<Vec2>> {
        Some(vec![end])
    }
}
impl EnemyGeometry for OpenArena {
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
fn hud_reads_only_the_selected_identity_and_current_health() {
    let mut world = EnemyWorld::new("boss-hud", 0);
    world.spawn(
        &[
            EnemySpawnIntent::new(EnemyToken::Skeleton, Vec2::ZERO, 1, 0, 1),
            EnemySpawnIntent::new(EnemyToken::Skeleton, Vec2::new(100.0, 0.0), 2, 0, 1),
        ],
        EnemySpawnContext {
            players: &[],
            tick: 0,
            arena_scalars: EnemyArenaScalars::default(),
            roll_loot_seed: None,
        },
        &OpenArena,
        None,
    );
    assert_eq!(world.actors.len(), 2);
    assert_eq!(HudBoss::from_world(&world), None);
    let boss = &mut world.actors[1];
    boss.config.recipe_name = Some("Grand Necromancer".into());
    boss.config.maximum_health = 1000.0;
    boss.current_health = 275.0;
    world.featured_boss_id = Some(boss.id);
    assert_eq!(
        HudBoss::from_world(&world),
        Some(HudBoss {
            name: "Grand Necromancer".into(),
            current_health: 275.0,
            maximum_health: 1000.0,
        })
    );
    world.actors[1].current_health = 125.0;
    assert_eq!(HudBoss::from_world(&world).unwrap().current_health, 125.0);
    world.featured_boss_id = None;
    assert_eq!(
        HudBoss::from_world(&world),
        None,
        "another living enemy does not replace the cleared selection"
    );
    world.featured_boss_id = Some(usize::MAX);
    assert_eq!(HudBoss::from_world(&world), None, "a removed identity is not replaced");
}
