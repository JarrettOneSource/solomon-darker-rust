//! Current Source births under both Enhanced Effects settings.

use super::*;
use solomon_sim::enemies::boss_spell::BossSpellKind;
use solomon_sim::enemies::effects::{BouncerOptions, DeathEffectOwner};
use solomon_sim::enemies::model::EnemyActorRead;
use solomon_sim::enemies::model::{EnemyBrain, EnemyLifeState, EnemyTarget, MagePhase};
use solomon_sim::enemies::projectile_emission::{
    SpawnEnemyProjectileOptions, spawn_projectile, step_mage_lightning_pulse,
};
use solomon_sim::enemies::projectiles::EnemyProjectileKind;
use solomon_sim::enemies::world::{EnemySpawnContext, EnemyStepContext, EnemyWorld};
use solomon_sim::painter::WorldPainterOrder;

#[test]
fn current_enemy_quality_preserves_actual_births_rng_and_painter_order() {
    let cases: Value =
        serde_json::from_str(&fixture("current-enemy-quality-goldens.json")).unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let program = case["program"].as_str().unwrap();
        let enhanced_effects = case["enhancedEffects"].as_bool().unwrap();
        let position = Vec2::new(150.25, 250.75);
        let owner = DeathEffectOwner { id: 1, position };
        let players = [EnemyTarget {
            id: "hero".to_owned(),
            summoned: false,
            alive: true,
            collision_radius: 25.0,
            connected: true,
            eligible: true,
            heading_deg: None,
            position: if program == "bomb" { Vec2::new(700.0, 750.0) } else { position },
            velocity_per_tick: Vec2::ZERO,
        }];
        let mut world = EnemyWorld::new(case["seed"].as_str().unwrap(), 50);
        let mut order = WorldPainterOrder { next_actor_ordinal: 100, next_transient_ordinal: 70 };
        world.spawn(
            &[spawn_intent(&case["intent"])],
            EnemySpawnContext {
                players: &players,
                tick: 10,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: None,
            },
            &UnobstructedEnemyGeometry,
            Some(&mut order),
        );
        let mut actor = world.actors.get(0).unwrap().snapshot();
        let mut context = EnemyStepContext {
            enhanced_effects,
            tick: 10,
            paused: false,
            players: &players,
            geometry: &UnobstructedEnemyGeometry,
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
        };
        let mut retained = None;
        match program {
            "bouncer" | "bouncer-explicit-clock" => world.spawn_bouncer(
                owner,
                10,
                |_| 113,
                program,
                |_| BouncerOptions {
                    opacity_timer: (program == "bouncer-explicit-clock").then_some(1.5),
                    ..Default::default()
                },
                enhanced_effects,
                &mut order,
            ),
            "terminal" => {
                actor.current_health = 0.0;
                actor.life_state = EnemyLifeState::Dying;
                actor.death_started_tick = Some(10);
                actor.last_damaged_by_player_id = Some("hero".to_owned());
                if actor.config.enemy_token == EnemyToken::DireFaculty {
                    actor.death_tick = 249;
                    context.tick = 260;
                }
                retained = Some(world.step_dying_actor(&mut actor, &mut context, &mut order));
            }
            "arrow" | "bomb" => {
                let kind = if program == "arrow" {
                    EnemyProjectileKind::Arrow
                } else {
                    EnemyProjectileKind::DemonBomb
                };
                let options = if program == "arrow" {
                    SpawnEnemyProjectileOptions {
                        speed: Some(6.0),
                        minimum_speed: Some(6.0),
                        lifetime_ticks: Some(40),
                        heading_deg: Some(0.0),
                        ..Default::default()
                    }
                } else {
                    SpawnEnemyProjectileOptions::default()
                };
                spawn_projectile(
                    &mut world,
                    &actor,
                    10,
                    kind,
                    8.0,
                    options,
                    enhanced_effects,
                    &mut order,
                );
                for tick in 11..=10 + case["ticks"].as_u64().unwrap() {
                    context.tick = tick;
                    world.step_projectiles(&context, &mut order);
                }
            }
            "mage" => {
                let EnemyBrain::Mage(brain) = &mut actor.brain else {
                    panic!("the Source case constructs a Mage")
                };
                brain.phase = MagePhase::Cast;
                brain.lightning_ticks_remaining = 4;
                brain.lightning_target_player_id = Some("hero".to_owned());
                brain.lightning_target_position = Some(position);
                step_mage_lightning_pulse(&mut world, &mut actor, &context, &mut order);
            }
            "eye-trail" => {
                solomon_sim::enemies::demon_skull_effects::spawn_unholy_eye_trail(
                    &mut world,
                    owner,
                    10,
                    37.25,
                    enhanced_effects,
                    &mut order,
                );
            }
            "falling-bone" | "tragic-circle" | "ultra-banish" => {
                let kind = match program {
                    "falling-bone" => BossSpellKind::FallingBone {
                        entry: 113,
                        height: -1.0,
                        color_ramp: 0.0,
                        rotation_deg: 25.0,
                    },
                    "tragic-circle" => BossSpellKind::TragicCircle { remaining_ticks: 500 },
                    _ => BossSpellKind::UltraBanish {
                        remaining_ticks: 130,
                        alpha: 1.0,
                        flash_alpha: 0.0,
                        light_radius: 1.0,
                        mega_death: true,
                    },
                };
                // This program starts from the actual Source factory output. Owner
                // allocation is fixture input; the public retained-step API is tested here.
                let input = &case["spellInput"];
                let registration = &input["painterRegistration"];
                let lane = match registration["managerLane"].as_str().unwrap() {
                    "actor" => solomon_sim::painter::WorldManagerLane::Actor,
                    "transient" => solomon_sim::painter::WorldManagerLane::Transient,
                    _ => panic!("a Source world-manager lane"),
                };
                let ordinal = registration["registrationOrdinal"].as_u64().unwrap() as usize;
                let painter_registration = solomon_sim::painter::PainterRegistration {
                    manager_lane: lane,
                    registration_ordinal: ordinal,
                };
                match lane {
                    solomon_sim::painter::WorldManagerLane::Actor => {
                        order.next_actor_ordinal = ordinal + 1;
                    }
                    solomon_sim::painter::WorldManagerLane::Transient => {
                        order.next_transient_ordinal = ordinal + 1;
                    }
                }
                let id = input["id"].as_u64().unwrap() as usize;
                world.next_projectile_id = id + 1;
                world.boss_spells.push(solomon_sim::enemies::boss_spell::NativeBossSpell {
                    owner: solomon_sim::enemies::boss_spell::BossSpellOwner {
                        age_ticks: input["ageTicks"].as_u64().unwrap(),
                        damage: number(input, "damage"),
                        id,
                        painter_registration,
                        owner_actor_id: input["ownerActorId"].as_u64().unwrap() as usize,
                        position: point_from_json(&input["position"]),
                        spawn_tick: input["spawnTick"].as_u64().unwrap(),
                    },
                    kind,
                });
                for tick in 11..=10 + case["ticks"].as_u64().unwrap() {
                    context.tick = tick;
                    world.step_boss_spells(&mut context, &mut order);
                }
            }
            _ => panic!("unknown Source quality program {program}"),
        }
        let actual = json!({
            "retained": retained,
            "deathEffects": world.death_effects.iter().map(death_effect_json).collect::<Vec<_>>(),
            "projectileEffects": world.projectile_effects.iter().map(projectile_cases::projectile_effect_json).collect::<Vec<_>>(),
            "projectiles": world.projectiles.iter().map(enemy_projectile_json).collect::<Vec<_>>(),
            "pulses": world.mage_lightning_pulses.iter().map(mage_lightning_json).collect::<Vec<_>>(),
            "bossSpells": world.boss_spells.iter().map(boss_cases::boss_spell_json).collect::<Vec<_>>(),
            "rng": world.rng_state,
            "steering": rng_record(&world.steering_rng),
            "nextDeathEffectId": world.next_death_effect_id,
            "nextProjectileEffectId": world.next_projectile_effect_id,
            "painterOrder": {"actor":order.next_actor_ordinal,"transient":order.next_transient_ordinal},
        });
        assert_current_enemy_json(
            &actual,
            &case["expected"],
            &format!("{program} enhanced={enhanced_effects} {}", case["intent"]["enemyToken"]),
        );
    }
}
