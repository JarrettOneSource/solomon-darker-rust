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

#[test]
fn projectile_retirement_birth_and_restore_preserve_motion_and_event_order() {
    use solomon_sim::enemies::model::EnemyEventKind;
    use solomon_sim::enemies::projectile_emission::{
        SpawnEnemyProjectileOptions, spawn_projectile,
    };
    use solomon_sim::enemies::projectiles::{EnemyProjectileKind, EnemyProjectileRead};

    let mut live = EnemyWorld::new("projectile-ecs", 50);
    let mut order = WorldPainterOrder::default();
    live.spawn(
        &[EnemySpawnIntent::new(EnemyToken::SkeletonArcher, Vec2::new(100.0, 100.0), 1, 0, 31)],
        EnemySpawnContext {
            players: &[],
            tick: 0,
            arena_scalars: EnemyArenaScalars::default(),
            roll_loot_seed: None,
        },
        &ClearArena,
        Some(&mut order),
    );
    let owner = live.actors.remove(0);
    let mut records = Vec::new();
    for kind in [
        EnemyProjectileKind::Arrow,
        EnemyProjectileKind::Firebolt,
        EnemyProjectileKind::GuidedMissile,
        EnemyProjectileKind::DemonBomb,
        EnemyProjectileKind::PoisonPool,
    ] {
        records.push(spawn_projectile(
            &mut live,
            &owner,
            0,
            kind,
            1.0,
            SpawnEnemyProjectileOptions {
                heading_deg: Some(70.0),
                speed: Some(2.0),
                minimum_speed: Some(1.0),
                lifetime_ticks: Some(if kind == EnemyProjectileKind::Arrow { 100 } else { 1 }),
                ..Default::default()
            },
            true,
            &mut order,
        ));
    }
    // A save keeps the established enum variants and fields, including Arrow velocity.
    assert_eq!(
        serde_json::to_value(&live.projectiles).unwrap(),
        serde_json::to_value(&records).unwrap()
    );
    {
        let bomb = live.projectiles.get_mut(3).unwrap();
        bomb.state.speed = 0.0;
        bomb.state.settled_ticks_remaining = 1;
    }
    let admitted: Vec<_> = live.projectiles.iter().map(|p| p.state().id).collect();
    step(&mut live, &[], 1, &mut order);
    assert_eq!(
        live.projectiles.iter().map(|p| p.state().id).collect::<Vec<_>>(),
        [admitted[0], admitted[4]]
    );
    assert_eq!(
        live.events
            .iter()
            .filter(|event| event.kind == EnemyEventKind::ProjectileRetired)
            .map(|event| event.projectile_id.unwrap())
            .collect::<Vec<_>>(),
        admitted[1..4]
    );
    let newborn = spawn_projectile(
        &mut live,
        &owner,
        1,
        EnemyProjectileKind::Firebolt,
        1.0,
        SpawnEnemyProjectileOptions { lifetime_ticks: Some(2), ..Default::default() },
        true,
        &mut order,
    );
    assert_eq!(live.projectiles.get(2).unwrap().state().age_ticks, 0);
    assert_eq!(live.projectiles.get(2).unwrap().state().id, newborn.state().id);
    let saved = serde_json::to_vec(&live).unwrap();
    let mut restored: EnemyWorld = serde_json::from_slice(&saved).unwrap();
    let mut restored_order = order;
    assert_eq!(serde_json::to_vec(&restored).unwrap(), saved);
    for tick in 2..=20 {
        step(&mut live, &[], tick, &mut order);
        step(&mut restored, &[], tick, &mut restored_order);
        assert_eq!(
            serde_json::to_vec(&restored).unwrap(),
            serde_json::to_vec(&live).unwrap(),
            "tick {tick}"
        );
        assert_eq!(restored_order, order);
    }
}

#[test]
fn boss_spell_children_precede_the_parent_and_wait_for_the_next_tick() {
    use solomon_sim::enemies::boss_spell::{BossSpellKind, BossSpellOwner, NativeBossSpell};

    let mut live = EnemyWorld::new("boss-spell-ecs", 50);
    let mut order = WorldPainterOrder::default();
    let kinds = [
        BossSpellKind::HeartmongerFlicker { phase_deg: 0.0 },
        BossSpellKind::RainOfBones {
            alpha: 1.0,
            phase: 0.0,
            remaining_ticks: 100,
            rotation_sign: 1.0,
            scale: 1.0,
        },
        BossSpellKind::DeathMagic {
            scale: 1.0,
            alpha: 0.5,
            alpha_loss_per_tick: 1.0,
            painter_sort_bias: 0.0,
            light: None,
        },
        BossSpellKind::HeartmongerSoul { life_phase_deg: 0.0, bob_phase_deg: 0.0 },
    ];
    let mut records = Vec::new();
    for (index, kind) in kinds.into_iter().enumerate() {
        let record = NativeBossSpell {
            owner: BossSpellOwner {
                age_ticks: 0,
                damage: 1.0,
                id: index + 1,
                painter_registration: order.register(kind.manager_lane()),
                owner_actor_id: 100,
                position: Vec2::new(200.0, 200.0),
                spawn_tick: 0,
            },
            kind,
        };
        live.boss_spells.push(record.clone());
        records.push(record);
    }
    live.next_projectile_id = 5;
    assert_eq!(
        serde_json::to_value(&live.boss_spells).unwrap(),
        serde_json::to_value(&records).unwrap()
    );

    let mut first_birth = None;
    for tick in 1..=20 {
        step(&mut live, &[], tick, &mut order);
        let spells: Vec<_> = live.boss_spells.iter().collect();
        assert_eq!(spells.first().unwrap().owner.id, 1);
        assert_eq!(spells.last().unwrap().owner.id, 4);
        assert!(!spells.iter().any(|spell| spell.owner.id == 3));
        if let Some(index) = spells.iter().position(|spell| spell.owner.id >= 5) {
            let child = spells[index];
            assert_eq!(spells[index + 1].owner.id, 2);
            assert_eq!(child.owner.age_ticks, 0);
            assert_eq!(child.owner.spawn_tick, tick);
            let BossSpellKind::FallingBone { height, .. } = *child.kind else {
                panic!("Rain of Bones births a falling bone");
            };
            assert_eq!(height, -175.0);
            first_birth = Some((tick, child.owner.id));
            break;
        }
    }
    let (birth_tick, child_id) =
        first_birth.expect("fixed-seed rain produces a child within 20 ticks");
    let saved = serde_json::to_vec(&live).unwrap();
    let mut restored: EnemyWorld = serde_json::from_slice(&saved).unwrap();
    let mut restored_order = order;
    for tick in birth_tick + 1..=birth_tick + 20 {
        step(&mut live, &[], tick, &mut order);
        step(&mut restored, &[], tick, &mut restored_order);
        let child = live.boss_spells.iter().find(|spell| spell.owner.id == child_id).unwrap();
        assert_eq!(child.owner.age_ticks, tick - birth_tick);
        assert_eq!(
            serde_json::to_vec(&restored).unwrap(),
            serde_json::to_vec(&live).unwrap(),
            "tick {tick}"
        );
        assert_eq!(restored_order, order);
    }
}
