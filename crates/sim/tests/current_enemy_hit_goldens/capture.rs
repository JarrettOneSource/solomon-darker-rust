//! Ether Drain owns terminal capture, while ordinary rewards and retirement still occur.

use serde::Deserialize;
use solomon_sim::Vec2;
use solomon_sim::enemies::config::EnemyArenaScalars;
use solomon_sim::enemies::model::{EnemyBrain, EnemyDamageRequest, SpiderPhase};
use solomon_sim::enemies::world::*;
use solomon_sim::enemies::{EnemyFlag, EnemySpawnIntent, EnemyToken};
use solomon_sim::painter::WorldPainterOrder;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CaptureCase {
    seed: String,
    token: String,
    flags: Vec<String>,
    amount: f64,
    damage: [f64; 4],
    captured: bool,
    spider_phase_captured: bool,
    actor_ids: Vec<usize>,
    next_actor_id: usize,
    effect_count: usize,
    projectile_count: usize,
    events: Vec<(String, Option<usize>, Option<String>)>,
    retired_ids: Vec<usize>,
    rewards: Vec<(usize, usize, f64, Option<String>)>,
    rng: u32,
    steering: Vec<u32>,
    painter: [usize; 2],
}

#[test]
fn current_captures_suppress_family_death_children_and_effects_but_retain_rewards() {
    let fixture = include_str!("../fixtures/website/current-enemy-hit-goldens.json");
    let goldens: super::Goldens = serde_json::from_str(fixture).unwrap();
    for case in goldens.capture_cases {
        let mut world = EnemyWorld::new(&case.seed, 50);
        let mut order = WorldPainterOrder::default();
        let mut intent = EnemySpawnIntent::new(
            EnemyToken::from_id(&case.token).unwrap(),
            Vec2::new(150.0, 250.0),
            1,
            0,
            31,
        );
        for flag in &case.flags {
            intent.flags.push(EnemyFlag::from_id(flag).unwrap());
        }
        world.spawn(
            &[intent],
            EnemySpawnContext {
                players: &[],
                tick: 0,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: None,
            },
            &super::ClearWorld,
            Some(&mut order),
        );
        world.last_step_tick = Some(0);
        let mut request = EnemyDamageRequest::new(1, case.amount, 0, Some("hero".into()));
        request.ether_drain_capture = true;
        request.suppress_hurt_sound = true;
        let result = world.damage(request, Some(&mut order));
        assert_eq!(
            [
                f64::from(u8::from(result.accepted)),
                result.health_damage,
                f64::from(u8::from(result.killed)),
                f64::from(u8::from(result.special_death_accepted)),
            ],
            case.damage,
            "{} damage admission",
            case.seed
        );
        assert_eq!(world.actors[0].ether_drain_captured, case.captured, "{} captured", case.seed);
        let spider_captured = matches!(
            &world.actors[0].brain,
            EnemyBrain::Spider(brain) if brain.phase == SpiderPhase::Captured
        );
        assert_eq!(spider_captured, case.spider_phase_captured, "{} Spider phase", case.seed);
        world.step(
            EnemyStepContext {
                enhanced_effects: true,
                tick: 1,
                paused: false,
                players: &[],
                geometry: &super::ClearWorld,
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
        let actor_ids: Vec<_> = world.actors.iter().map(|actor| actor.id).collect();
        assert_eq!(actor_ids, case.actor_ids, "{} retained actors", case.seed);
        assert_eq!(world.next_actor_id, case.next_actor_id, "{} child IDs", case.seed);
        assert_eq!(world.death_effects.len(), case.effect_count, "{} death effects", case.seed);
        assert_eq!(world.projectiles.len(), case.projectile_count, "{} projectiles", case.seed);
        let events: Vec<_> = world
            .events
            .iter()
            .map(|event| {
                (
                    event.kind.id().to_owned(),
                    event.count,
                    event.output.map(|output| output.id().to_owned()),
                )
            })
            .collect();
        assert_eq!(events, case.events, "{} terminal event ordering", case.seed);
        let retired: Vec<_> = world.retired.iter().map(|row| row.actor_id).collect();
        assert_eq!(retired, case.retired_ids, "{} retirements", case.seed);
        let rewards: Vec<_> = world
            .rewards
            .iter()
            .map(|row| (row.actor_id, row.event_id, row.experience, row.player_id.clone()))
            .collect();
        assert_eq!(rewards, case.rewards, "{} ordinary reward ownership", case.seed);
        assert_eq!(world.rng_state, case.rng, "{} actor RNG", case.seed);
        let mut steering =
            vec![world.steering_rng.index_a as u32, world.steering_rng.index_b as u32];
        steering.extend(world.steering_rng.words);
        assert_eq!(steering, case.steering, "{} steering RNG", case.seed);
        assert_eq!(
            [order.next_actor_ordinal, order.next_transient_ordinal],
            case.painter,
            "{} painter births",
            case.seed
        );
    }
}
