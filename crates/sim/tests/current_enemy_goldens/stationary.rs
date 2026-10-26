//! Actual external impulses before Portal materialization and held stationary ticks.

use std::cell::Cell;

use serde::Deserialize;
use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::enemies::config::*;
use solomon_sim::enemies::model::EnemyBrain;
use solomon_sim::enemies::path::EnemyNavigation;
use solomon_sim::enemies::world::*;
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::painter::WorldPainterOrder;

#[derive(Deserialize)]
pub(super) struct StationaryWorld {
    token: String,
    seed: String,
    recipe: Option<Value>,
    frames: Vec<StationaryFrame>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StationaryFrame {
    tick: u64,
    position: [f64; 2],
    anchor: Option<[f64; 2]>,
    movement_calls: u32,
    movement_radius: Option<f64>,
    rng: u32,
    steering: Vec<f64>,
}

#[derive(Default)]
struct SettlingWorld {
    calls: Cell<u32>,
    radius: Cell<Option<f64>>,
}

impl EnemyNavigation for SettlingWorld {
    fn direct_path_clear(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> bool {
        true
    }

    fn find_route(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> Option<Vec<Vec2>> {
        None
    }
}

impl EnemyGeometry for SettlingWorld {
    fn resolve_movement(&self, request: EnemyMovementRequest) -> Vec2 {
        if request.purpose == EnemyMovementPurpose::SpawnPlacement {
            return request.requested_position;
        }
        self.calls.set(self.calls.get() + 1);
        self.radius.set(Some(request.radius));
        Vec2::new(request.requested_position.x + 3.25, request.requested_position.y - 1.5)
    }

    fn line_obstruction(&self, _: Vec2, _: Vec2, _: u32) -> Option<Vec2> {
        None
    }

    fn projectile_world_blocked(&self, _: EnemyProjectileWorldQuery) -> bool {
        false
    }
}

fn portal_recipe(recipe: &Value) -> AuthoredEnemyRecipe {
    let number = |key: &str| recipe[key].as_f64().unwrap();
    let loot = &recipe["lootPolicies"];
    let policy = |key: &str| loot[key].as_u64().unwrap() as u8;
    assert_eq!(recipe["classification"], "multiple-boss");
    assert_eq!(recipe["family"]["kind"], "portal");
    AuthoredEnemyRecipe {
        uid: recipe["uid"].as_u64().unwrap() as usize,
        name: recipe["name"].as_str().unwrap().into(),
        classification: EnemyClassification::MultipleBoss,
        maximum_health: number("maximumHealth"),
        primary_damage: number("primaryDamage"),
        secondary_damage: number("secondaryDamage"),
        tertiary_damage: number("tertiaryDamage"),
        extra_damage: number("extraDamage"),
        attack_speed: number("attackSpeed"),
        chase_speed: number("chaseSpeed"),
        movement_scale: number("movementScale"),
        experience_bonus: number("experienceBonus"),
        archer_accuracy_mode: recipe["archerAccuracyMode"].as_u64().unwrap() as u8,
        family: AuthoredEnemyFamily::Portal {
            frequency: recipe["family"]["frequency"].as_u64().unwrap() as u8,
        },
        loot_policies: EnemyLootPolicies {
            gold: policy("gold"),
            item: policy("item"),
            orb: policy("orb"),
            potion: policy("potion"),
            powerup: policy("powerup"),
            specific_item: loot["specificItem"].as_u64().unwrap() as u32,
        },
        miniboss_die_program: recipe["onDeathProgram"].as_str() == Some("miniboss-die"),
    }
}

#[test]
fn current_stationary_roots_settle_then_resist_external_impulses_even_while_held() {
    let fixture = include_str!("../fixtures/website/current-enemy-goldens.json");
    let goldens: super::CurrentEnemyGoldens = serde_json::from_str(fixture).unwrap();
    for case in goldens.stationary_worlds {
        let mut world = EnemyWorld::new(&case.seed, 50);
        let geometry = SettlingWorld::default();
        let mut order = WorldPainterOrder::default();
        let mut intent = EnemySpawnIntent::new(
            EnemyToken::from_id(&case.token).unwrap(),
            Vec2::new(150.0, 250.0),
            1,
            0,
            31,
        );
        intent.authored_recipe = case.recipe.as_ref().map(portal_recipe);
        world.spawn(
            &[intent],
            EnemySpawnContext {
                players: &[],
                tick: 0,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: None,
            },
            &geometry,
            Some(&mut order),
        );
        for frame in case.frames {
            let tick = frame.tick;
            world.actors.get_mut(0).unwrap().position.x += 100.0;
            world.actors.get_mut(0).unwrap().position.y += 80.0;
            let effects = [(
                world.actors.get(0).unwrap().id,
                EnemyAbilityEffect {
                    time_scale: if (5..9).contains(&tick) || (25..30).contains(&tick) {
                        0.0
                    } else {
                        1.0
                    },
                    ..EnemyAbilityEffect::default()
                },
            )];
            geometry.calls.set(0);
            geometry.radius.set(None);
            world.step(
                EnemyStepContext {
                    enhanced_effects: true,
                    tick,
                    paused: false,
                    players: &[],
                    geometry: &geometry,
                    ability_effects: &effects,
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
            let actor = world.actors.get(0).unwrap();
            let anchor = match &actor.brain {
                EnemyBrain::Coffin(brain) => Some(brain.anchor_position),
                EnemyBrain::Portal(brain) => brain.anchor_position,
                _ => panic!("recorded stationary family"),
            };
            let context = format!("{} tick{tick}", case.seed);
            assert_eq!([actor.position.x, actor.position.y], frame.position, "{context} root");
            assert_eq!(anchor.map(|point| [point.x, point.y]), frame.anchor, "{context} anchor");
            assert_eq!(geometry.calls.get(), frame.movement_calls, "{context} movement calls");
            assert_eq!(geometry.radius.get(), frame.movement_radius, "{context} collision radius");
            assert_eq!(world.rng_state, frame.rng, "{context} actor RNG");
            let mut steering =
                vec![world.steering_rng.index_a as f64, world.steering_rng.index_b as f64];
            steering.extend(world.steering_rng.words.map(f64::from));
            assert_eq!(steering, frame.steering, "{context} steering RNG");
        }
    }
}
