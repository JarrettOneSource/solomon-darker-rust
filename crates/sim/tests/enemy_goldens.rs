//! Enemy-family differential fixtures recorded from the pinned Website kernels.

#![recursion_limit = "256"]

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::enemies::config::*;
use solomon_sim::enemies::model::EnemyActorRead;
use solomon_sim::enemies::{EnemyFlag, EnemySpawnIntent, EnemyToken};

#[path = "support/mod.rs"]
mod source;

#[path = "enemy_goldens/bosses.rs"]
mod boss_cases;
#[path = "enemy_goldens/presentation.rs"]
mod presentation_cases;
#[path = "enemy_goldens/projectiles.rs"]
mod projectile_cases;
#[path = "enemy_goldens/quality.rs"]
mod quality_cases;
#[path = "enemy_goldens/spider.rs"]
mod spider_cases;

#[derive(Deserialize)]
struct ConfigGoldens {
    tokens: Vec<(String, u16)>,
    flags: Vec<String>,
    cases: Vec<ConfigCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigCase {
    token: String,
    flags: Vec<String>,
    random: ConfigRandom,
    wave_ordinal: u64,
    authored_recipe: Option<Value>,
    arena_scalars: Value,
    expected: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigRandom {
    base_speed_unit: f64,
    collision_radius_unit: f64,
    random_armor: bool,
    split_many_gate_unit: f64,
    split_many_unit: f64,
    split_unit: u8,
}

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/website/{name}", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path}: {error}"));
    let bytes = if name.ends_with(".zlib") {
        miniz_oxide::inflate::decompress_to_vec_zlib(&bytes)
            .unwrap_or_else(|error| panic!("inflate {path}: {error:?}"))
    } else {
        bytes
    };
    String::from_utf8(bytes).unwrap_or_else(|error| panic!("decode {path}: {error}"))
}

#[test]
fn enemy_constructor_stats_authored_recipes_and_ordered_flags_match_the_website() {
    let goldens: ConfigGoldens =
        serde_json::from_str(&fixture("enemy-config-goldens.json")).unwrap();
    assert_eq!(
        EnemyToken::ALL.map(|token| (token.id().to_owned(), token.native_type_id())),
        goldens.tokens.as_slice()
    );
    assert_eq!(EnemyFlag::ALL.map(|flag| flag.id().to_owned()), goldens.flags.as_slice());
    for (index, case) in goldens.cases.iter().enumerate() {
        let token = EnemyToken::from_id(&case.token).unwrap();
        let mut intent = EnemySpawnIntent::new(token, Vec2::ZERO, 1, 0, case.wave_ordinal);
        intent.flags = case.flags.iter().map(|flag| EnemyFlag::from_id(flag).unwrap()).collect();
        intent.authored_recipe = case.authored_recipe.as_ref().map(recipe);
        let random = EnemyConfigRandom {
            base_speed_unit: case.random.base_speed_unit,
            collision_radius_unit: case.random.collision_radius_unit,
            random_armor: case.random.random_armor,
            split_many_gate_unit: case.random.split_many_gate_unit,
            split_many_unit: case.random.split_many_unit,
            split_unit: case.random.split_unit,
        };
        let scalars = EnemyArenaScalars {
            health: scalar(&case.arena_scalars, "health"),
            primary_damage: scalar(&case.arena_scalars, "primaryDamage"),
            secondary_damage: scalar(&case.arena_scalars, "secondaryDamage"),
            tertiary_damage: scalar(&case.arena_scalars, "tertiaryDamage"),
            extra_damage: scalar(&case.arena_scalars, "extraDamage"),
            chase_speed: scalar(&case.arena_scalars, "chaseSpeed"),
            attack_speed: scalar(&case.arena_scalars, "attackSpeed"),
            experience: scalar(&case.arena_scalars, "experience"),
        };
        let actual = std::panic::catch_unwind(|| evaluate_enemy_config(&intent, scalars, random));
        let Some(expected) = &case.expected else {
            assert!(
                actual.is_err(),
                "case {index} {} should reject its dormant payload",
                case.token
            );
            continue;
        };
        let actual =
            actual.unwrap_or_else(|_| panic!("case {index} {} unexpectedly rejected", case.token));
        assert_json_equal(
            &config_json(&actual),
            expected,
            &format!("case {index} {} flags {:?}", case.token, case.flags),
        );
    }
}

// JSON has one numeric type at the source; serde preserves the writer's integer spelling.
// Compare the doubles rather than treating the encodings `1` and `1.0` as different values.
fn assert_json_equal(actual: &Value, expected: &Value, context: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            let actual = actual.as_f64().unwrap();
            let expected = expected.as_f64().unwrap();
            if context.ends_with(".headingDeg")
                || context.ends_with(".limbHeadingDeg")
                || context.ends_with(".brain.bodyHeadingDeg")
                || (context.contains(".brain.articulation.")
                    && (context.ends_with(".x")
                        || context.ends_with(".y")
                        || context.ends_with(".liftY")))
            {
                assert_trig(actual, expected, context);
            } else {
                assert_eq!(actual, expected, "{context}");
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{context} length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_json_equal(actual, expected, &format!("{context}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{context} keys");
            for (key, actual) in actual {
                assert_json_equal(actual, &expected[key], &format!("{context}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{context}"),
    }
}

fn scalar(value: &Value, field: &str) -> f64 {
    value[field].as_f64().unwrap_or(1.0)
}
fn number(value: &Value, field: &str) -> f64 {
    value[field].as_f64().unwrap()
}
fn integer(value: &Value, field: &str) -> u8 {
    value[field].as_u64().unwrap() as u8
}
fn boolean(value: &Value, field: &str) -> bool {
    value[field].as_bool().unwrap()
}

fn recipe(value: &Value) -> AuthoredEnemyRecipe {
    let source = &value["family"];
    let family = match source["kind"].as_str().unwrap() {
        "default" => AuthoredEnemyFamily::Default,
        "demon-skull" => {
            AuthoredEnemyFamily::DemonSkull { capabilities: integer(source, "capabilities") }
        }
        "heartmonger" => AuthoredEnemyFamily::Heartmonger {
            crow_count: source["crowCount"].as_u64().unwrap() as usize,
            blind_mode: integer(source, "blindMode"),
            summon_mode: integer(source, "summonMode"),
        },
        "skeleton" => AuthoredEnemyFamily::Skeleton {
            armor: boolean(source, "armor"),
            headgear: integer(source, "headgear"),
            weapon: weapon(source["weapon"].as_str().unwrap()),
        },
        "archer" => AuthoredEnemyFamily::Archer {
            arrow_type: arrow_type(source["arrowType"].as_str().unwrap()),
            extra_arrows: source["extraArrows"].as_u64().unwrap() as usize,
            headgear: integer(source, "headgear"),
            multi_arrow_mode: integer(source, "multiArrowMode"),
            range_mode: integer(source, "rangeMode"),
            strafing: boolean(source, "strafing"),
        },
        "portal" => AuthoredEnemyFamily::Portal { frequency: integer(source, "frequency") },
        "zombie" => AuthoredEnemyFamily::Zombie {
            body_type: integer(source, "bodyType"),
            flyblown: boolean(source, "flyblown"),
            poison_duration: number(source, "poisonDuration"),
            poison_pool_damage: number(source, "poisonPoolDamage"),
            poison_punch_damage: number(source, "poisonPunchDamage"),
        },
        "faculty" => AuthoredEnemyFamily::Faculty(FacultyAppearance {
            body_color: serde_json::from_value(source["bodyColor"].clone()).unwrap(),
            head_color: serde_json::from_value(source["headColor"].clone()).unwrap(),
            female: boolean(source, "female"),
            headgear: integer(source, "headgear"),
            primary: integer(source, "primary"),
            secondary: integer(source, "secondary"),
        }),
        other => panic!("unknown authored family {other}"),
    };
    let loot = &value["lootPolicies"];
    AuthoredEnemyRecipe {
        uid: value["uid"].as_u64().unwrap() as usize,
        name: value["name"].as_str().unwrap().to_owned(),
        classification: match value["classification"].as_str().unwrap() {
            "normal" => EnemyClassification::Normal,
            "boss" => EnemyClassification::Boss,
            "miniboss" => EnemyClassification::Miniboss,
            "multiple-boss" => EnemyClassification::MultipleBoss,
            other => panic!("unknown enemy classification {other}"),
        },
        maximum_health: number(value, "maximumHealth"),
        primary_damage: number(value, "primaryDamage"),
        secondary_damage: number(value, "secondaryDamage"),
        tertiary_damage: number(value, "tertiaryDamage"),
        extra_damage: number(value, "extraDamage"),
        attack_speed: number(value, "attackSpeed"),
        chase_speed: number(value, "chaseSpeed"),
        movement_scale: number(value, "movementScale"),
        experience_bonus: number(value, "experienceBonus"),
        archer_accuracy_mode: integer(value, "archerAccuracyMode"),
        family,
        loot_policies: EnemyLootPolicies {
            gold: integer(loot, "gold"),
            item: integer(loot, "item"),
            orb: integer(loot, "orb"),
            potion: integer(loot, "potion"),
            powerup: integer(loot, "powerup"),
            specific_item: loot["specificItem"].as_u64().unwrap() as u32,
        },
        miniboss_die_program: value["onDeathProgram"].as_str() == Some("miniboss-die"),
    }
}

fn weapon(name: &str) -> SkeletonWeapon {
    match name {
        "claw" => SkeletonWeapon::Claw,
        "sword" => SkeletonWeapon::Sword,
        "mace" => SkeletonWeapon::Mace,
        "flail" => SkeletonWeapon::Flail,
        "axe" => SkeletonWeapon::Axe,
        "pike" => SkeletonWeapon::Pike,
        other => panic!("unknown skeleton weapon {other}"),
    }
}
fn arrow_type(name: &str) -> ArrowType {
    match name {
        "normal" => ArrowType::Normal,
        "fire" => ArrowType::Fire,
        "poison" => ArrowType::Poison,
        other => panic!("unknown arrow type {other}"),
    }
}

fn config_json(config: &EnemyConfig) -> Value {
    let family = match &config.family {
        EnemyFamilyConfig::Skeleton { armor, headgear, weapon } => {
            json!({"armor":armor,"headgear":headgear,
            "weapon":match weapon {SkeletonWeapon::Claw=>"claw",SkeletonWeapon::Sword=>"sword",SkeletonWeapon::Mace=>"mace",
                SkeletonWeapon::Flail=>"flail",SkeletonWeapon::Axe=>"axe",SkeletonWeapon::Pike=>"pike"}})
        }
        EnemyFamilyConfig::Archer {
            accuracy_mode,
            arrow_type,
            extra_arrows,
            headgear,
            multi_arrow_mode,
            range_mode,
            strafing,
        } => json!({
            "accuracyMode":accuracy_mode,"arrowType":match arrow_type {ArrowType::Normal=>"normal",ArrowType::Fire=>"fire",ArrowType::Poison=>"poison"},
            "extraArrows":extra_arrows,"headgear":headgear,"multiArrowMode":multi_arrow_mode,"rangeMode":range_mode,"strafing":strafing}),
        EnemyFamilyConfig::Mage {
            cloak,
            element,
            headgear,
            other_shield,
            other_shield_health,
            range_mode,
            self_shield,
            self_shield_health,
            shield_interval,
        } => json!({
            "cloak":cloak,"element":match element {MageElement::Fire=>"fire",MageElement::Lightning=>"lightning",MageElement::Frost=>"frost",MageElement::Poison=>"poison"},
            "headgear":headgear,"otherShield":other_shield,"otherShieldHealth":other_shield_health,"rangeMode":range_mode,
            "selfShield":self_shield,"selfShieldHealth":self_shield_health,"shieldInterval":shield_interval}),
        EnemyFamilyConfig::Imp { split_depth } => json!({"splitDepth":split_depth}),
        EnemyFamilyConfig::Zombie {
            body_type,
            poison_duration,
            poison_pool_damage,
            poison_punch_damage,
            rotten,
        } => json!({
            "bodyType":body_type,"poisonDuration":poison_duration,"poisonPoolDamage":poison_pool_damage,"poisonPunchDamage":poison_punch_damage,"rotten":rotten}),
        EnemyFamilyConfig::Wraith => json!({"dazzle":true}),
        EnemyFamilyConfig::Demon { split_count } => json!({"splitCount":split_count}),
        EnemyFamilyConfig::Coffin {
            maggot_damage,
            maggot_health,
            maggot_poison_damage,
            maximum_maggots,
        } => json!({
            "maggotDamage":maggot_damage,"maggotHealth":maggot_health,"maggotPoisonDamage":maggot_poison_damage,"maximumMaggots":maximum_maggots}),
        EnemyFamilyConfig::Portal { frequency } => json!({"frequency":frequency}),
        EnemyFamilyConfig::Heartmonger { crow_count, blind_mode, summon_mode } => {
            json!({"crowCount":crow_count,"blindMode":blind_mode,"summonMode":summon_mode})
        }
        EnemyFamilyConfig::Faculty(appearance) => {
            json!({"bodyColor":appearance.body_color,"headColor":appearance.head_color,
            "female":appearance.female,"headgear":appearance.headgear,"primary":appearance.primary,"secondary":appearance.secondary})
        }
        EnemyFamilyConfig::DemonSkull { capabilities } => json!({"capabilities":capabilities}),
        EnemyFamilyConfig::Spider { cocoon_health, spit_webs, suck_damage_per_second } => {
            json!({"cocoonHealth":cocoon_health,"spitWebs":spit_webs,"suckDamagePerSecond":suck_damage_per_second})
        }
        EnemyFamilyConfig::Cocoon => json!({"kind":"cocoon"}),
    };
    json!({
        "numbers":[config.native_type_id,config.attack_speed,config.base_speed,config.chase_speed,
            config.collision_radius,config.experience,config.extra_damage,config.maximum_health,
            config.primary_damage,config.scale,config.secondary_damage,config.tertiary_damage],
        "enemyToken":config.enemy_token.id(),"flags":config.flags.iter().map(|flag|flag.id()).collect::<Vec<_>>(),
        "family":family,"ignoredSourceFlags":config.ignored_source_flags.iter().map(|flag|flag.id()).collect::<Vec<_>>(),
        "burning":config.burning,"flanking":config.flanking,"classification":match config.classification {
            EnemyClassification::Normal=>"normal",EnemyClassification::Boss=>"boss",EnemyClassification::Miniboss=>"miniboss",EnemyClassification::MultipleBoss=>"multiple-boss"},
        "recipeName":config.recipe_name,"recipeUid":config.recipe_uid,
        "lootPolicies":{"gold":config.loot_policies.gold,"item":config.loot_policies.item,"orb":config.loot_policies.orb,
            "potion":config.loot_policies.potion,"powerup":config.loot_policies.powerup,"specificItem":config.loot_policies.specific_item},
        "onDeathProgram":if config.miniboss_die_program {Some("miniboss-die")} else {None},"pathfindingMode":config.pathfinding_mode,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathGoldens {
    constructions: Vec<PathConstruction>,
    steering: Vec<SteeringCase>,
    recovery: Vec<RecoveryCase>,
    routes: Vec<RouteCase>,
    reorientation: Vec<ReorientationCase>,
    ranges: Vec<RangeCase>,
    target_refresh_ticks: Vec<u64>,
}

#[derive(Deserialize)]
struct PathConstruction {
    seed: i64,
    state: Vec<f64>,
    rng: Vec<f64>,
}
#[derive(Deserialize)]
struct SteeringCase {
    seed: i64,
    flank: bool,
    target: Option<[f64; 2]>,
    radial: i8,
    tangent: i8,
    goal: [f64; 2],
    delta: [f64; 2],
    heading: f64,
    state: Vec<f64>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryCase {
    seed: i64,
    flanking_enabled: bool,
    samples: Vec<(Vec<f64>, bool)>,
    rng: Vec<f64>,
}
#[derive(Deserialize)]
struct RouteCase {
    mode: String,
    samples: Vec<RouteSample>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RouteSample {
    position: [f64; 2],
    goal: [f64; 2],
    state: Vec<f64>,
    turn_around: bool,
}
#[derive(Deserialize)]
struct ReorientationCase {
    target: Option<[f64; 2]>,
    samples: Vec<ReorientationSample>,
}
#[derive(Deserialize)]
struct ReorientationSample {
    heading: f64,
    state: Vec<f64>,
}
#[derive(Deserialize)]
struct RangeCase {
    seed: i64,
    first: f64,
    second: f64,
    value: f64,
    rng: Vec<f64>,
}

fn vector(point: [f64; 2]) -> Vec2 {
    Vec2::new(point[0], point[1])
}

fn rng_record(rng: &solomon_sim::NativeRng) -> Vec<f64> {
    let mut record = vec![rng.index_a as f64, rng.index_b as f64];
    record.extend(rng.words.iter().copied().map(f64::from));
    record
}

fn path_record(state: &solomon_sim::enemies::path::NativeEnemyPathState) -> [f64; 20] {
    let previous = state.route_previous_vector.unwrap_or(Vec2::ZERO);
    let waypoints = state.route_waypoints.unwrap_or([Vec2::ZERO; 2]);
    [
        state.base_turn_rate,
        state.flank_angle_deg,
        state.flank_radius,
        state.flank_ticks_remaining as f64,
        state.reorientation_ticks_remaining as f64,
        if state.route_previous_vector.is_some() { 1.0 } else { 0.0 },
        previous.x,
        previous.y,
        state.route_refresh_ticks_remaining as f64,
        state.route_ticks_remaining as f64,
        state.route_waypoint_index as f64,
        if state.route_waypoints.is_some() { 1.0 } else { 0.0 },
        waypoints[0].x,
        waypoints[0].y,
        waypoints[1].x,
        waypoints[1].y,
        state.speed_factor,
        state.stalled_movement_ticks as f64,
        state.turn_factor,
        state.wander_heading_deg,
    ]
}

// Trigonometric results alone may differ in the last bit across V8 and the platform libm.
fn assert_trig(actual: f64, expected: f64, context: &str) {
    assert!((actual - expected).abs() <= 1e-9, "{context}: expected {expected}, got {actual}");
}

struct OracleNavigation {
    direct: bool,
    route: Option<Vec<Vec2>>,
}
impl solomon_sim::enemies::path::EnemyNavigation for OracleNavigation {
    fn direct_path_clear(
        &self,
        _start: Vec2,
        _end: Vec2,
        _clearance: f64,
        _body_radius: f64,
    ) -> bool {
        self.direct
    }
    fn find_route(
        &self,
        _start: Vec2,
        _end: Vec2,
        _clearance: f64,
        _body_radius: f64,
    ) -> Option<Vec<Vec2>> {
        self.route.clone()
    }
}

#[test]
fn enemy_steering_routes_flanking_and_recovery_match_the_website() {
    use solomon_sim::NativeRng;
    use solomon_sim::enemies::path::*;
    let goldens: PathGoldens = serde_json::from_str(&fixture("enemy-path-goldens.json")).unwrap();
    for case in &goldens.constructions {
        let mut rng = NativeRng::new(case.seed);
        let state = NativeEnemyPathState::new(&mut rng);
        assert_eq!(
            path_record(&state),
            case.state.as_slice(),
            "path constructor seed {}",
            case.seed
        );
        assert_eq!(rng_record(&rng), case.rng, "path constructor RNG seed {}", case.seed);
    }
    for (index, case) in goldens.steering.iter().enumerate() {
        let mut state = NativeEnemyPathState::new(&mut NativeRng::new(case.seed));
        state.flank_ticks_remaining = if case.flank { 3 } else { 0 };
        let request = EnemySteeringRequest {
            actor_heading_deg: 359.0,
            actor_position: Vec2::new(10.0, 20.0),
            cadence_ticks: 2,
            full_movement: true,
            goal_position: None,
            movement_per_tick: 0.375,
            status_factor: 0.75,
            target_heading_deg: 90.0,
            target_position: case.target.map(vector),
            radial_direction: match case.radial {
                -1 => RadialDirection::Retreat,
                0 => RadialDirection::Hold,
                1 => RadialDirection::Approach,
                _ => unreachable!(),
            },
            tangent_direction: match case.tangent {
                -1 => TangentDirection::Counterclockwise,
                0 => TangentDirection::Hold,
                1 => TangentDirection::Clockwise,
                _ => unreachable!(),
            },
        };
        let goal = native_enemy_steering_goal(&state, &request);
        let result = build_native_enemy_steering(state, request);
        let context = format!("steering case {index}");
        assert_trig(goal.x, case.goal[0], &context);
        assert_trig(goal.y, case.goal[1], &context);
        assert_trig(result.delta.x, case.delta[0], &context);
        assert_trig(result.delta.y, case.delta[1], &context);
        assert_trig(result.heading_deg, case.heading, &context);
        assert_eq!(path_record(&result.state), case.state.as_slice(), "{context} state");
    }
    for case in &goldens.recovery {
        let mut rng = NativeRng::new(case.seed);
        let mut state = NativeEnemyPathState::new(&mut rng);
        state.speed_factor = 1.5;
        state.turn_factor = 1.4_f32 as f64;
        for (index, (expected, triggered)) in case.samples.iter().enumerate() {
            let blocked = index < 70 || (100..140).contains(&index);
            let actual = state.step_recovery(
                &mut rng,
                PathRecoveryRequest {
                    flanking_enabled: case.flanking_enabled,
                    requested_distance: 1.0,
                    status_factor: 0.5,
                    tick: index as u64 * 2,
                    traveled_distance: if blocked { 0.0 } else { 1.0 },
                },
            );
            assert_eq!(actual, *triggered, "recovery seed {} epoch {index} branch", case.seed);
            assert_eq!(
                path_record(&state),
                expected.as_slice(),
                "recovery seed {} epoch {index}",
                case.seed
            );
        }
        assert_eq!(rng_record(&rng), case.rng, "recovery seed {} final RNG", case.seed);
    }
    for case in &goldens.routes {
        let mut state = NativeEnemyPathState::new(&mut NativeRng::new(71));
        let navigation = OracleNavigation {
            direct: case.mode == "direct",
            route: match case.mode.as_str() {
                "missing" => None,
                "short" => Some(vec![Vec2::ZERO, Vec2::new(200.0, 200.0)]),
                _ => Some(vec![
                    Vec2::ZERO,
                    Vec2::new(100.0, 0.0),
                    Vec2::new(100.0, 100.0),
                    Vec2::new(200.0, 200.0),
                ]),
            },
        };
        for (index, sample) in case.samples.iter().enumerate() {
            let result = resolve_native_enemy_path_goal(
                state,
                EnemyPathGoalRequest {
                    actor_position: vector(sample.position),
                    body_radius: 20.0,
                    cadence_ticks: 2,
                    navigation_clearance: 45.0,
                    raw_goal: Vec2::new(200.0, 50.0),
                    target_position: Some(Vec2::new(200.0, 200.0)),
                    target_refresh_ticks: 300,
                },
                &navigation,
            );
            state = result.state;
            assert_eq!(result.goal, vector(sample.goal), "route {} epoch {index}", case.mode);
            assert_eq!(result.turn_around, sample.turn_around);
            assert_eq!(path_record(&state), sample.state.as_slice());
        }
    }
    for case in &goldens.reorientation {
        let mut state = NativeEnemyPathState::new(&mut NativeRng::new(71));
        state.reorientation_ticks_remaining = 2;
        let mut heading = 17.0;
        for sample in &case.samples {
            heading = state.step_reorientation(heading, Vec2::ZERO, case.target.map(vector));
            assert_trig(heading, sample.heading, "reorientation heading");
            assert_eq!(path_record(&state), sample.state.as_slice());
        }
    }
    for case in &goldens.ranges {
        let mut rng = NativeRng::new(case.seed);
        assert_eq!(rng.draw_float_range(case.first, case.second), case.value);
        assert_eq!(rng_record(&rng), case.rng);
    }
    for (mode, expected) in goldens.target_refresh_ticks.iter().enumerate() {
        assert_eq!(native_enemy_target_refresh_ticks(mode as u8), *expected);
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RangedGoldens {
    ranges: Vec<RangedRangeCase>,
    volleys: Vec<VolleyCase>,
    fan_offsets: Vec<f64>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RangedRangeCase {
    family: String,
    mode: u8,
    seed: i64,
    range: f64,
    pending: bool,
    restored_range: f64,
    restored_pending: bool,
    rng: Vec<f64>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VolleyCase {
    seed: u32,
    accuracy_mode: u8,
    multi_arrow_mode: u8,
    arrow_type: String,
    aim_offset_radius: f64,
    effective_accuracy_mode: u8,
    shot_pitch: f64,
    rng: Vec<f64>,
    arrows: Vec<[f64; 8]>,
}

#[test]
fn enemy_attack_ranges_and_private_seeded_arrow_volleys_match_the_website() {
    use solomon_sim::NativeRng;
    use solomon_sim::enemies::ranged::*;
    let goldens: RangedGoldens =
        serde_json::from_str(&fixture("enemy-ranged-goldens.json")).unwrap();
    for case in &goldens.ranges {
        let mut rng = NativeRng::new(case.seed);
        let family = match case.family.as_str() {
            "archer" => RangedFamily::Archer,
            "mage" => RangedFamily::Mage,
            _ => unreachable!(),
        };
        let mut range = construct_native_ranged_attack_range(family, case.mode, &mut rng);
        assert_eq!(range.range, case.range);
        assert_eq!(range.range_easy_pending, case.pending);
        assert_eq!(rng_record(&rng), case.rng);
        restore_native_range_easy_after_volley(&mut range);
        assert_eq!(range.range, case.restored_range);
        assert_eq!(range.range_easy_pending, case.restored_pending);
    }
    for (index, case) in goldens.volleys.iter().enumerate() {
        let mut rng = NativeRng::new(128);
        let arrow_type = arrow_type(&case.arrow_type);
        let volley = build_native_archer_volley(
            ArcherVolleyRequest {
                accuracy_mode: case.accuracy_mode,
                arrow_type,
                extra_arrows: 4,
                multi_arrow_mode: case.multi_arrow_mode,
                origin: Vec2::new(10.25, 20.75),
                private_seed: case.seed,
                target_position: Vec2::new(450.5, 190.125),
                target_velocity_per_tick: Vec2::new(0.75, -0.25),
            },
            &mut rng,
        );
        assert_eq!(volley.aim_offset_radius, case.aim_offset_radius);
        assert_eq!(volley.effective_accuracy_mode, case.effective_accuracy_mode);
        assert_eq!(volley.shot_pitch, case.shot_pitch);
        assert_eq!(rng_record(&rng), case.rng);
        assert_eq!(volley.arrows.len(), case.arrows.len());
        for (arrow_index, (arrow, expected)) in volley.arrows.iter().zip(&case.arrows).enumerate() {
            assert_eq!(arrow.arrow_type, arrow_type);
            let velocity = native_enemy_projectile_velocity(arrow.heading_deg, arrow.speed);
            let actual = [
                arrow.heading_deg,
                arrow.lifetime_ticks as f64,
                arrow.position.x,
                arrow.position.y,
                arrow.speed,
                arrow.visual_heading_deg,
                velocity.x,
                velocity.y,
            ];
            assert_eq!(actual, *expected, "volley {index} arrow {arrow_index}");
        }
    }
    for (index, expected) in goldens.fan_offsets.iter().enumerate() {
        assert_eq!(native_archer_fan_offset(index), *expected);
    }
}

#[derive(Deserialize)]
struct ConstructionGoldens {
    cases: Vec<ConstructionCase>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConstructionCase {
    name: String,
    seed: String,
    intents: Vec<Value>,
    actors: Vec<Value>,
    rng: u32,
    steering: Vec<f64>,
    locomotion: Vec<f64>,
    head_facing: Vec<f64>,
    next_actor_id: usize,
    next_death_effect_id: usize,
    next_native_cell_binding_order: usize,
    next_native_registration_order: usize,
    next_event_id: usize,
    featured_boss_id: Option<usize>,
    faculty_voice_controller: Value,
    demon_skull_encounter: Value,
    spawned: Vec<usize>,
    events: Vec<Value>,
    painter_base: Option<EffectPainterOrder>,
    painter_order: Option<EffectPainterOrder>,
    #[serde(default)]
    no_targets: bool,
    shared_loot_seed: Option<i64>,
    shared_rng: Option<Vec<f64>>,
}

fn point_json(point: Vec2) -> Value {
    json!({"x":point.x,"y":point.y})
}
fn point_from_json(point: &Value) -> Vec2 {
    Vec2::new(number(point, "x"), number(point, "y"))
}

fn skeleton_pike_json(value: &solomon_sim::enemies::model::SkeletonPike) -> Value {
    json!({
        "playerId":value.player_id,
        "position":point_json(value.position),
        "distance":value.distance,
    })
}

fn archer_strafe_state_json(value: &solomon_sim::enemies::model::ArcherStrafeState) -> Value {
    json!({
        "direction":value.direction,
        "limbHeadingDeg":value.limb_heading_deg,
        "movementRamp":value.movement_ramp,
        "turnBlend":value.turn_blend,
    })
}

fn demon_extremity_json(value: &solomon_sim::enemies::model::DemonExtremity) -> Value {
    json!({
        "current":point_json(value.current),
        "liftY":value.lift_y,
        "phase":value.phase,
        "start":point_json(value.start),
        "target":point_json(value.target),
    })
}

fn demon_articulation_json(value: &solomon_sim::enemies::model::DemonArticulation) -> Value {
    json!({
        "completedSteps":value.completed_steps,
        "front":demon_extremity_json(&value.front),
        "frontBaseRotationDeg":value.front_base_rotation_deg,
        "rear":demon_extremity_json(&value.rear),
        "rearBaseRotationDeg":value.rear_base_rotation_deg,
        "stepIntervalTicks":value.step_interval_ticks,
    })
}

fn faculty_action_json(value: &solomon_sim::enemies::model::FacultyAction) -> Value {
    use solomon_sim::enemies::model::*;
    json!({
        "end":value.end,
        "frames":value.frames,
        "handMask":value.hand_mask,
        "kind":match value.kind { FacultyActionKind::Throw=>"throw",FacultyActionKind::TwoHand=>"two-hand",FacultyActionKind::Lightning=>"lightning" },
        "marker":value.marker,
        "progress":value.progress,
        "rate":value.rate,
    })
}

fn crow_state_json(value: &solomon_sim::enemies::model::CrowState) -> Value {
    use solomon_sim::enemies::model::*;
    json!({
        "age":value.age,
        "diveSpeed":value.dive_speed,
        "flapCycles":value.flap_cycles,
        "flapPhase":value.flap_phase,
        "glideTicks":value.glide_ticks,
        "headingDeg":value.heading_deg,
        "height":value.height,
        "heightPerDistance":value.height_per_distance,
        "id":value.id,
        "lightIntensity":value.light_intensity,
        "maximumSpeed":value.maximum_speed,
        "orbitPhaseDeg":value.orbit_phase_deg,
        "orbitRadius":value.orbit_radius,
        "phase":match value.phase { CrowPhase::Orbit=>"orbit",CrowPhase::Dive=>"dive" },
        "position":value.position.map(point_json),
        "speed":value.speed,
        "targetId":value.target_id,
    })
}

fn puppet_hit_json(value: &solomon_sim::enemies::model::PuppetHit) -> Value {
    json!({
        "strength":value.strength,
        "tick":value.tick,
        "timer":value.timer,
    })
}

fn enemy_lighting_json(value: &solomon_sim::enemies::model::EnemyLighting) -> Value {
    json!({
        "charge":value.charge,
        "glow":value.glow,
        "providerCopies":value.provider_copies,
    })
}

fn brain_json(brain: &solomon_sim::enemies::model::EnemyBrain) -> Value {
    use solomon_sim::enemies::model::*;
    match brain {
        EnemyBrain::Skeleton(value) => json!({"family":"skeleton",
            "actionProgress":value.action_progress,
            "markerEmitted":value.marker_emitted,
            "verticalOffset":value.vertical_offset,
            "verticalVelocity":value.vertical_velocity,
            "action":match value.action { SkeletonAction::Claw=>"claw",SkeletonAction::Pike=>"pike",SkeletonAction::Weapon=>"weapon" },
            "contactTargetPlayerId":value.contact_target_player_id,
            "pike":value.pike.as_ref().map(skeleton_pike_json),
            "phase":match value.phase { SkeletonPhase::Approach=>"approach",SkeletonPhase::Attack=>"attack",SkeletonPhase::Death=>"death" },
        }),
        EnemyBrain::Archer(value) => json!({"family":"archer",
            "actionProgress":value.action_progress,
            "markerEmitted":value.marker_emitted,
            "verticalOffset":value.vertical_offset,
            "verticalVelocity":value.vertical_velocity,
            "aimSeed":value.aim_seed,
            "attackRange":value.attack_range,
            "phase":match value.phase { ArcherPhase::RangeControl=>"range-control",ArcherPhase::Attack=>"attack",ArcherPhase::Death=>"death" },
            "rangeEasyPending":value.range_easy_pending,
            "strafe":archer_strafe_state_json(&value.strafe),
        }),
        EnemyBrain::Mage(value) => json!({"family":"mage",
            "actionProgress":value.action_progress,
            "markerEmitted":value.marker_emitted,
            "verticalOffset":value.vertical_offset,
            "verticalVelocity":value.vertical_velocity,
            "attackRange":value.attack_range,
            "castProgram":match value.cast_program { MageCastProgram::Long=>"long",MageCastProgram::Short=>"short" },
            "castRoll":value.cast_roll,
            "disabledPrimaryTicks":value.disabled_primary_ticks,
            "lightningTargetPlayerId":value.lightning_target_player_id,
            "lightningTargetPosition":value.lightning_target_position.map(point_json),
            "lightningTicksRemaining":value.lightning_ticks_remaining,
            "phase":match value.phase { MagePhase::RangeControl=>"range-control",MagePhase::Cast=>"cast",MagePhase::Death=>"death" },
            "rangeEasyPending":value.range_easy_pending,
            "shieldTicksRemaining":value.shield_ticks_remaining,
        }),
        EnemyBrain::Imp(value) => json!({"family":"imp",
            "baseHorizontalSpeed":value.base_horizontal_speed,
            "bodyRotationDeg":value.body_rotation_deg,
            "bodyVariant":value.body_variant,
            "effectAlpha":value.effect_alpha,
            "effectPhase":value.effect_phase,
            "horizontalSpeed":value.horizontal_speed,
            "verticalOffset":value.vertical_offset,
            "verticalVelocity":value.vertical_velocity,
            "escapeHeadingDeg":value.escape_heading_deg,
            "phase":match value.phase { FlightPhase::Flight=>"flight",FlightPhase::Death=>"death" },
            "visualRngState":value.visual_rng_state,
        }),
        EnemyBrain::Zombie(value) => json!({"family":"zombie",
            "actionProgress":value.action_progress,
            "actionRate":value.action_rate,
            "actionSwing":value.action_swing,
            "angularOffsetDeg":value.angular_offset_deg,
            "attackSide":value.attack_side,
            "bodyPhaseDeg":value.body_phase_deg,
            "bodyType":value.body_type,
            "contactTargetPlayerId":value.contact_target_player_id,
            "frontArmBaseRotationDeg":value.front_arm_base_rotation_deg,
            "headBaseRotationDeg":value.head_base_rotation_deg,
            "headPhaseDeg":value.head_phase_deg,
            "headType":value.head_type,
            "impactStateTicksRemaining":value.impact_state_ticks_remaining,
            "markerEmitted":value.marker_emitted,
            "phase":match value.phase { ZombiePhase::Approach=>"approach",ZombiePhase::Swipe=>"swipe",ZombiePhase::Knockback=>"knockback",ZombiePhase::Death=>"death" },
            "phaseTicksRemaining":value.phase_ticks_remaining,
            "rearArmBaseRotationDeg":value.rear_arm_base_rotation_deg,
            "verticalOffset":value.vertical_offset,
            "verticalVelocity":value.vertical_velocity,
            "visualRngState":value.visual_rng_state,
        }),
        EnemyBrain::Wraith(value) => json!({"family":"wraith",
            "baseFlybySpeed":value.base_flyby_speed,
            "contactCooldownTicks":value.contact_cooldown_ticks,
            "currentSpeed":value.current_speed,
            "currentTurnGain":value.current_turn_gain,
            "flybyTicksRemaining":value.flyby_ticks_remaining,
            "restingSpeed":value.resting_speed,
            "targetTurnGain":value.target_turn_gain,
            "collisionRecipient":value.collision_recipient,
            "phase":match value.phase { FlightPhase::Flight=>"flight",FlightPhase::Death=>"death" },
        }),
        EnemyBrain::Demon(value) => json!({"family":"demon",
            "actionProgress":value.action_progress,
            "articulation":demon_articulation_json(&value.articulation),
            "markerEmitted":value.marker_emitted,
            "phase":match value.phase { DemonPhase::Approach=>"approach",DemonPhase::Bomb=>"bomb",DemonPhase::Death=>"death" },
        }),
        EnemyBrain::Coffin(value) => json!({"family":"coffin",
            "anchorPosition":point_json(value.anchor_position),
            "launchRotationDeg":value.launch_rotation_deg,
            "launchScale":value.launch_scale,
            "maggotCharge":value.maggot_charge,
            "phase":match value.phase { CoffinPhase::Hidden=>"hidden",CoffinPhase::Rising=>"rising",CoffinPhase::Holding=>"holding",CoffinPhase::Opening=>"opening",CoffinPhase::Open=>"open",CoffinPhase::Death=>"death" },
            "phaseTick":value.phase_tick,
            "phaseTicksRemaining":value.phase_ticks_remaining,
        }),
        EnemyBrain::Portal(value) => json!({"family":"portal",
            "anchorPosition":value.anchor_position.map(point_json),
            "ageTicks":value.age_ticks,
            "alpha":value.alpha,
            "auraPhase":value.aura_phase,
            "bodyPhase":value.body_phase,
            "fixedScale":value.fixed_scale,
            "ticksUntilEjection":value.ticks_until_ejection,
            "hurtTicksRemaining":value.hurt_ticks_remaining,
            "phase":match value.phase { PortalPhase::Active=>"active",PortalPhase::Death=>"death" },
        }),
        EnemyBrain::DemonSkull(value) => json!({"family":"demon-skull",
            "bodyHeadingDeg":value.body_heading_deg,
            "bodyOffset":point_json(value.body_offset),
            "bodyPhaseDeg":value.body_phase_deg,
            "bodyPose":value.body_pose,
            "chargeGlow":value.charge_glow,
            "eyeCharge":value.eye_charge,
            "flairGlow":value.flair_glow,
            "flickerPhaseDeg":value.flicker_phase_deg,
            "jitter":point_json(value.jitter),
            "lightIntensity":value.light_intensity,
            "spin":value.spin,
            "attackTicksRemaining":value.attack_ticks_remaining,
            "capabilities":value.capabilities,
            "headingDelayTicks":value.heading_delay_ticks,
            "pendingAttack":value.pending_attack.map(|attack| match attack {DemonSkullRangedAttack::Eyes=>"eyes",DemonSkullRangedAttack::Mouth=>"mouth",DemonSkullRangedAttack::Spit=>"spit"}),
            "recoil":point_json(value.recoil),
            "screamActive":value.scream_active,
            "seen":value.seen,
            "speed":value.speed,
            "targetSpeed":value.target_speed,
            "deathCountdown":value.death_countdown,
            "actions":value.actions.iter().map(demon_skull_action_json).collect::<Vec<_>>(),
            "phase":match value.phase { BossPhase::RangeControl=>"range-control",BossPhase::Cast=>"cast",BossPhase::Death=>"death" },
        }),
        EnemyBrain::Faculty(value) => json!({"family":"faculty",
            "bodyHeadingDeg":value.body_heading_deg,
            "action":value.action.as_ref().map(faculty_action_json),
            "attackRange":value.attack_range,
            "bodyPose":value.body_pose,
            "completedDispatches":value.completed_dispatches,
            "disabledPrimaryTicks":value.disabled_primary_ticks,
            "gaitPhase":value.gait_phase,
            "handMask":value.hand_mask,
            "headingLocked":value.heading_locked,
            "lightningActive":value.lightning_active,
            "lightIntensity":value.light_intensity,
            "lightPhase":value.light_phase,
            "orbitSign":value.orbit_sign,
            "primaryPoll":value.primary_poll,
            "secondaryCooldown":value.secondary_cooldown,
            "phase":match value.phase { BossPhase::RangeControl=>"range-control",BossPhase::Cast=>"cast",BossPhase::Death=>"death" },
        }),
        EnemyBrain::Heartmonger(value) => json!({"family":"heartmonger",
            "activated":value.activated,
            "crows":value.crows.iter().map(crow_state_json).collect::<Vec<_>>(),
            "crowCooldownTicks":value.crow_cooldown_ticks,
            "headVariant":value.head_variant,
            "summonTicksRemaining":value.summon_ticks_remaining,
            "phase":match value.phase { HeartmongerPhase::Approach=>"approach",HeartmongerPhase::Death=>"death" },
            "legPhase":value.leg_phase,
            "torsoPhase":value.torso_phase,
        }),
        EnemyBrain::Spider(value) => json!({"family":"spider",
            "actionState":value.action_state,
            "preferredDistance":value.preferred_distance,
            "bodyHeadingDeg":value.body_heading_deg,
            "moveTicksRemaining":value.move_ticks_remaining,
            "strafeSign":value.strafe_sign,
            "strafeTicksRemaining":value.strafe_ticks_remaining,
            "actionTicksRemaining":value.action_ticks_remaining,
            "frame":value.frame,
            "spitTicksRemaining":value.spit_ticks_remaining,
            "attached":value.attached,
            "verticalOffset":value.vertical_offset,
            "attachmentAngleDeg":value.attachment_angle_deg,
            "attachmentDistance":value.attachment_distance,
            "suckTicks":value.suck_ticks,
            "phase":match value.phase { SpiderPhase::Active=>"active",SpiderPhase::Death=>"death",SpiderPhase::Captured=>"captured" },
        }),
        EnemyBrain::Cocoon(value) => json!({"family":"cocoon",
            "phase":match value.phase { CocoonPhase::Active=>"active",CocoonPhase::Death=>"death" },
            "ownerPlayerId":value.owner_player_id,
            "ownerPosition":point_json(value.owner_position),
        }),
    }
}

fn demon_skull_action_json(action: &solomon_sim::enemies::model::DemonSkullAction) -> Value {
    use solomon_sim::enemies::model::DemonSkullActionProgram;
    let mut value = match &action.program {
        DemonSkullActionProgram::Scream => json!({"kind":"scream"}),
        DemonSkullActionProgram::Bite { progress, rate, marker_emitted } => {
            json!({"kind":"bite","progress":progress,"rate":rate,"markerEmitted":marker_emitted})
        }
        DemonSkullActionProgram::Eyes {
            warmup_ticks,
            shots_remaining,
            recovery_ticks,
            attack_speed,
        } => {
            json!({"kind":"eyes","warmupTicks":warmup_ticks,"shotsRemaining":shots_remaining,"recoveryTicks":recovery_ticks,"attackSpeed":attack_speed})
        }
        DemonSkullActionProgram::Mouth {
            warmup_ticks,
            warmup_jitter,
            warmup_turn_speed,
            beam_power,
            tracking_delay_ticks,
            tracking_ramp,
            recovery_ticks,
        } => {
            json!({"kind":"mouth","warmupTicks":warmup_ticks,"warmupJitter":warmup_jitter,"warmupTurnSpeed":warmup_turn_speed,"beamPower":beam_power,"trackingDelayTicks":tracking_delay_ticks,"trackingRamp":tracking_ramp,"recoveryTicks":recovery_ticks})
        }
        DemonSkullActionProgram::Spit {
            shots_remaining,
            cooldown_ticks,
            mouth_ticks,
            target_heading_deg,
        } => {
            json!({"kind":"spit","shotsRemaining":shots_remaining,"cooldownTicks":cooldown_ticks,"mouthTicks":mouth_ticks,"targetHeadingDeg":target_heading_deg})
        }
        DemonSkullActionProgram::Flair {
            remaining_ticks,
            settling_ticks,
            glow,
            original_heading_deg,
        } => {
            json!({"kind":"flair","remainingTicks":remaining_ticks,"settlingTicks":settling_ticks,"glow":glow,"originalHeadingDeg":original_heading_deg})
        }
    };
    value["retired"] = json!(action.retired);
    value
}

fn path_json(state: &solomon_sim::enemies::path::NativeEnemyPathState) -> Value {
    json!({
        "baseTurnRate":state.base_turn_rate,"flankAngleDeg":state.flank_angle_deg,"flankRadius":state.flank_radius,
        "flankTicksRemaining":state.flank_ticks_remaining,"reorientationTicksRemaining":state.reorientation_ticks_remaining,
        "routePreviousVector":state.route_previous_vector.map(point_json),
        "routeRefreshTicksRemaining":state.route_refresh_ticks_remaining,"routeTicksRemaining":state.route_ticks_remaining,
        "routeWaypointIndex":state.route_waypoint_index,"routeWaypoints":state.route_waypoints.map(|points|points.map(point_json)),
        "speedFactor":state.speed_factor,"stalledMovementTicks":state.stalled_movement_ticks,"turnFactor":state.turn_factor,
        "wanderHeadingDeg":state.wander_heading_deg,
    })
}

fn actor_json(value: &impl EnemyActorRead) -> Value {
    use solomon_sim::enemies::model::*;
    let mut record = json!({
        "hitFeedback":puppet_hit_json(&value.hit_feedback),
        "hitReactionTimer":value.hit_reaction_timer,
        "blizzardPushAccumulator":value.blizzard_push_accumulator,
        "blizzardPushLastTick":value.blizzard_push_last_tick,
        "bodyGaitPhase":value.body_gait_phase,
        "bodyPose":value.body_pose,
        "brain":brain_json(value.brain()),
        "config":config_json(&value.config),
        "currentHealth":value.current_health,
        "deathEpoch":value.death_epoch,
        "deathPresentationStarted":value.death_presentation_started,
        "deathStartedTick":value.death_started_tick,
        "deathTick":value.death_tick,
        "gaitPose":value.gait_pose,
        "headFacingOffset":value.head_facing_offset,
        "headingDeg":value.heading_deg,
        "hurricaneContactCooldown":value.hurricane_contact_cooldown,
        "id":value.id,
        "lastDamagedByPlayerId":value.last_damaged_by_player_id,
        "lastDamageTick":value.last_damage_tick,
        "lastMovementTick":value.last_movement_tick,
        "lethalMagicDamage":value.lethal_magic_damage,
        "lifeState":match value.life_state { EnemyLifeState::Alive=>"alive",EnemyLifeState::Dying=>"dying" },
        "lightRegistration":{"managerLane":"actor","registrationOrdinal":value.registration_ordinal},
        "lighting":enemy_lighting_json(&value.lighting),
        "lootSeed":value.loot_seed,
        "nextMovementTick":value.next_movement_tick,
        "nextTargetRefreshTick":value.next_target_refresh_tick,
        "nativeCellBindingOrder":value.native_cell_binding_order,
        "nativeRegistrationOrder":value.native_registration_order,
        "path":path_json(&value.path),
        "position":point_json(value.position),
        "rewardGranted":value.reward_granted,
        "restBodyPose":value.rest_body_pose,
        "shieldHealth":value.shield_health,
        "shadowLateralOffset":value.shadow_lateral_offset,
        "shieldMaximumHealth":value.shield_maximum_health,
        "shieldPulse":value.shield_pulse,
        "shieldSoundCooldownTicks":value.shield_sound_cooldown_ticks,
        "sourceSpawnIntentId":value.source_spawn_intent_id,
        "spawnTick":value.spawn_tick,
        "staffActionFactor":value.staff_action_factor,
        "staffMovementFactor":value.staff_movement_factor,
        "stridePhaseDeg":value.stride_phase_deg,
        "targetPlayerId":value.target_player_id,
        "terminalEmitted":value.terminal_emitted,
        "waveOrdinal":value.wave_ordinal,
    });
    if value.ether_drain_captured {
        record["etherDrainCaptured"] = json!(true);
    }
    record
}

fn maggot_json(value: solomon_sim::enemies::model::MaggotRef<'_>) -> Value {
    use solomon_sim::enemies::model::*;
    let mut record = json!({
        "hitFeedback":puppet_hit_json(&value.hit_feedback),"blizzardPushAccumulator":value.blizzard_push_accumulator,"blizzardPushLastTick":value.blizzard_push_last_tick,
        "combatActive":value.combat_active,"nativeTreeQueryMember":value.native_tree_query_member,"collisionRadius":value.collision_radius,"currentHealth":value.current_health,
        "deathOffsets":value.death_offsets.iter().copied().map(point_json).collect::<Vec<_>>(),"damage":value.damage,"deathEpoch":value.death_epoch,
        "deathStartedTick":value.death_started_tick,"deathTick":value.death_tick,"gaitPose":value.gait_pose,"headingDeg":value.heading_deg,
        "hurricaneContactCooldown":value.hurricane_contact_cooldown,"id":value.id,"emergenceTick":value.controller.emergence_tick,"emergencePhase":value.controller.emergence_phase,
        "launchTrajectory":match value.controller.launch_trajectory {MaggotLaunchTrajectory::Edge=>"edge",MaggotLaunchTrajectory::Lid=>"lid"},"launchVelocity":point_json(value.controller.launch_velocity),
        "landingBounceVelocity":value.controller.landing_bounce_velocity,"lastAttackTick":value.controller.last_attack_tick,"lastDamagedByPlayerId":value.last_damaged_by_player_id,
        "lastDamageTick":value.last_damage_tick,"lastMovementTick":value.controller.last_movement_tick,"lifeState":match value.life_state {EnemyLifeState::Alive=>"alive",EnemyLifeState::Dying=>"dying"},
        "lightRegistration":{"managerLane":"actor","registrationOrdinal":value.registration_ordinal},"maximumHealth":value.maximum_health,
        "nextAttackTick":value.controller.next_attack_tick,"nextMovementTick":value.controller.next_movement_tick,"nextTargetRefreshTick":value.controller.next_target_refresh_tick,
        "nativeCellBindingOrder":value.native_cell_binding_order,"nativeRegistrationOrder":value.native_registration_order,"ownerCoffinActorId":value.owner_coffin_actor_id,
        "path":path_json(&value.controller.path),"poisonDamage":value.poison_damage,"poisonDuration":value.poison_duration,"position":point_json(value.position),
        "movementPhase":match value.controller.movement_phase {MaggotMovementPhase::Crawl=>"crawl",MaggotMovementPhase::Emerging=>"emerging"},"spawnTick":value.spawn_tick,
        "staffActionFactor":value.staff_action_factor,"staffMovementFactor":value.staff_movement_factor,"targetPlayerId":value.controller.target_player_id,
        "terminalEmitted":value.terminal_emitted,"verticalOffset":value.vertical_offset,"verticalVelocity":value.vertical_velocity,"visualScale":value.visual_scale,
    });
    if value.ether_drain_captured {
        record["etherDrainCaptured"] = json!(true);
    }
    record
}

fn source_config_record(source: &Value) -> Value {
    json!({
        "enemyToken":source["enemyToken"],"flags":source["flags"],
        "numbers":[source["nativeTypeId"],source["attackSpeed"],source["baseSpeed"],source["chaseSpeed"],
            source["collisionRadius"],source["experience"],source["extraDamage"],source["maximumHealth"],
            source["primaryDamage"],source["scale"],source["secondaryDamage"],source["tertiaryDamage"]],
        "family":source["family"],"ignoredSourceFlags":source["ignoredSourceFlags"],"burning":source["burning"],
        "flanking":source["flanking"],"classification":source["classification"],"recipeName":source["recipeName"],
        "recipeUid":source["recipeUid"],"lootPolicies":source["lootPolicies"],"onDeathProgram":source["onDeathProgram"],
        "pathfindingMode":source["pathfindingMode"],
    })
}

fn spawn_intent(value: &Value) -> EnemySpawnIntent {
    use solomon_sim::enemies::*;
    let token = EnemyToken::from_id(value["enemyToken"].as_str().unwrap()).unwrap();
    let mut intent = EnemySpawnIntent::new(
        token,
        point_from_json(&value["position"]),
        value["id"].as_u64().unwrap() as usize,
        value["spawnTick"].as_u64().unwrap(),
        value["waveOrdinal"].as_u64().unwrap(),
    );
    intent.flags = value["flags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|flag| EnemyFlag::from_id(flag.as_str().unwrap()).unwrap())
        .collect();
    intent.native_type_id = value["nativeTypeId"].as_u64().unwrap() as u16;
    intent.authored_recipe = value.get("authoredRecipe").map(recipe);
    intent.flanking = value["flanking"].as_bool().unwrap_or(true);
    intent.pathfinding_mode = value["pathfindingMode"].as_u64().unwrap_or(1) as u8;
    intent.mage_cloak = value["mageCloak"].as_bool().unwrap_or(false);
    intent.archer_strafing = value["archerStrafing"].as_bool().unwrap_or(false);
    intent.zombie_body_type = value["zombieBodyType"].as_u64().unwrap_or(0) as u8;
    intent.placement_group_id = value["placementGroupId"].as_u64().map(|id| id as usize);
    intent.placement_radius = value["placementRadius"].as_f64();
    intent.navigation_clearance = value["navigationClearance"].as_f64();
    intent.reachability_radius = value["reachabilityRadius"].as_f64();
    intent.green_imp_spit_damage = value["greenImpSpitDamage"].as_f64();
    intent.cocoon_target_player_id = value["cocoonTargetPlayerId"].as_str().map(str::to_owned);
    intent.enable_discorporeal_health_gates =
        value["enableDiscorporealHealthGates"].as_bool().unwrap_or(false);
    if let Some(source) = value.get("portalEjection") {
        intent.portal_ejection = Some(PortalEjection {
            child_heading_deg: number(source, "childHeadingDeg"),
            inherited_primary_damage: number(source, "inheritedPrimaryDamage"),
            parent_heading_deg: number(source, "parentHeadingDeg"),
            parent_position: point_from_json(&source["parentPosition"]),
            vertical_velocity: number(source, "verticalVelocity"),
        });
    }
    intent
}

struct UnobstructedEnemyGeometry;
impl solomon_sim::enemies::path::EnemyNavigation for UnobstructedEnemyGeometry {
    fn direct_path_clear(
        &self,
        _start: Vec2,
        _end: Vec2,
        _clearance: f64,
        _body_radius: f64,
    ) -> bool {
        true
    }
    fn find_route(
        &self,
        start: Vec2,
        end: Vec2,
        _clearance: f64,
        _body_radius: f64,
    ) -> Option<Vec<Vec2>> {
        Some(vec![start, end])
    }
}
impl solomon_sim::enemies::world::EnemyGeometry for UnobstructedEnemyGeometry {
    fn projectile_world_blocked(
        &self,
        _query: solomon_sim::enemies::world::EnemyProjectileWorldQuery,
    ) -> bool {
        false
    }
    fn resolve_movement(&self, request: solomon_sim::enemies::world::EnemyMovementRequest) -> Vec2 {
        request.requested_position
    }
    fn line_obstruction(
        &self,
        _start: Vec2,
        _end: Vec2,
        _native_exclusion_mask: u32,
    ) -> Option<Vec2> {
        None
    }
}

#[test]
fn all_native_enemy_family_constructors_caps_and_payloads_match_the_website() {
    use solomon_sim::enemies::model::EnemyTarget;
    use solomon_sim::enemies::world::{EnemySpawnContext, EnemyWorld};
    let goldens: ConstructionGoldens =
        serde_json::from_str(&fixture("enemy-construction-goldens.json")).unwrap();
    let mut presentations = presentation_cases::EnemyPresentationGoldens::new();
    for case in &goldens.cases {
        let mut world = EnemyWorld::new(&case.seed, 50);
        let players = [
            EnemyTarget {
                id: "hero".to_owned(),
                summoned: false,
                alive: true,
                collision_radius: 25.0,
                connected: true,
                eligible: !case.no_targets,
                heading_deg: None,
                position: Vec2::new(500.0, 500.0),
                velocity_per_tick: Vec2::ZERO,
            },
            EnemyTarget {
                id: "ghost".to_owned(),
                summoned: false,
                alive: false,
                collision_radius: 25.0,
                connected: true,
                eligible: true,
                heading_deg: None,
                position: Vec2::new(100.0, 100.0),
                velocity_per_tick: Vec2::ZERO,
            },
        ];
        let intents: Vec<_> = case.intents.iter().map(spawn_intent).collect();
        let mut order =
            case.painter_base.as_ref().map(|base| solomon_sim::painter::WorldPainterOrder {
                next_actor_ordinal: base.actor,
                next_transient_ordinal: base.transient,
            });
        let mut shared_rng = case.shared_loot_seed.map(solomon_sim::NativeRng::new);
        let mut roll_seed = |bound| shared_rng.as_mut().unwrap().draw_integer(bound as i32) as u32;
        let roll_loot_seed: Option<&mut dyn FnMut(u32) -> u32> =
            if case.shared_loot_seed.is_some() { Some(&mut roll_seed) } else { None };
        let spawned = world.spawn(
            &intents,
            EnemySpawnContext {
                players: &players,
                tick: 10,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed,
            },
            &UnobstructedEnemyGeometry,
            order.as_mut(),
        );
        if let Some(expected) = &case.shared_rng {
            assert_eq!(
                rng_record(shared_rng.as_ref().unwrap()),
                *expected,
                "{} shared loot RNG",
                case.name
            );
        }
        if let Some(expected) = &case.painter_order {
            let actual = order.expect("a shared source painter order was supplied");
            assert_eq!(actual.next_actor_ordinal, expected.actor);
            assert_eq!(actual.next_transient_ordinal, expected.transient);
        }
        assert_eq!(spawned, case.spawned, "{} returned actor IDs", case.name);
        assert_eq!(world.actors.len(), case.actors.len(), "{} accepted count", case.name);
        for actor in world.actors.iter() {
            presentations.check_actor("construction", &case.name, 10, &actor);
        }
        for (actual, expected) in world.actors.iter().zip(&case.actors) {
            let mut expected = expected.clone();
            expected["config"] = source_config_record(&expected["config"]);
            assert_json_equal(
                &actor_json(&actual),
                &expected,
                &format!("{} actor {}", case.name, actual.id),
            );
        }
        assert_eq!(world.rng_state, case.rng, "{} actor RNG", case.name);
        assert_eq!(rng_record(&world.steering_rng), case.steering, "{} steering RNG", case.name);
        assert_eq!(
            rng_record(&world.locomotion_rng),
            case.locomotion,
            "{} locomotion RNG",
            case.name
        );
        assert_eq!(
            rng_record(&world.head_facing_rng),
            case.head_facing,
            "{} head-facing RNG",
            case.name
        );
        assert_eq!(world.next_actor_id, case.next_actor_id);
        assert_eq!(world.next_death_effect_id, case.next_death_effect_id);
        assert_eq!(world.next_native_cell_binding_order, case.next_native_cell_binding_order);
        assert_eq!(world.next_native_registration_order, case.next_native_registration_order);
        assert_eq!(world.next_event_id, case.next_event_id);
        assert_eq!(world.featured_boss_id, case.featured_boss_id);
        assert_eq!(world.spawned_actor_ids, case.spawned);
        let faculty = world
            .faculty_voice_controller
            .map(|voice| json!({"line":voice.line,"ticksRemaining":voice.ticks_remaining}))
            .unwrap_or(Value::Null);
        assert_json_equal(&faculty, &case.faculty_voice_controller, "faculty voice constructor");
        let encounter = world.demon_skull_encounter;
        assert_json_equal(
            &json!({"deathStreamTicksRemaining":encounter.death_stream_ticks_remaining,
            "screamStreamTicksRemaining":encounter.scream_stream_ticks_remaining,"healthTriggersEnabled":encounter.health_triggers_enabled,
            "healthTriggersFiredMask":encounter.health_triggers_fired_mask,"pendingCapabilities":encounter.pending_capabilities}),
            &case.demon_skull_encounter,
            "DemonSkull encounter constructor",
        );
        let events: Vec<_> = world
            .events
            .iter()
            .map(|event| {
                json!({"actorId":event.actor_id,"eventId":event.event_id,
            "tick":event.tick,"targetPlayerId":event.target_player_id.as_ref().unwrap(),"type":"enemy-spawned"})
            })
            .collect();
        assert_json_equal(&json!(events), &json!(case.events), "spawn events");
    }
    presentations.assert_complete("construction");
}

#[derive(Deserialize)]
struct EffectGoldens {
    kinds: Vec<String>,
    births: Vec<Value>,
    snapshots: Vec<EffectSnapshot>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EffectSnapshot {
    tick: u64,
    effects: Vec<Value>,
    rng: u32,
    next_id: usize,
    painter_order: EffectPainterOrder,
}
#[derive(Deserialize)]
struct EffectPainterOrder {
    actor: usize,
    transient: usize,
}

fn painter_registration_json(registration: solomon_sim::painter::PainterRegistration) -> Value {
    use solomon_sim::painter::WorldManagerLane;
    json!({"managerLane":match registration.manager_lane {WorldManagerLane::Actor=>"actor",WorldManagerLane::Transient=>"transient"},
        "registrationOrdinal":registration.registration_ordinal})
}

fn death_effect_json(effect: &solomon_sim::enemies::model::EnemyDeathEffect) -> Value {
    let mut record = json!({
        "ageTicks":effect.age_ticks,
        "alpha":effect.alpha,
        "alphaMultiplier":effect.alpha_multiplier,
        "alphaLossPerTick":effect.alpha_loss_per_tick,
        "angularVelocityDeg":effect.angular_velocity_deg,
        "atlas":effect.atlas.id(),
        "blendMode":effect.blend_mode.id(),
        "bounceRetention":effect.bounce_retention,
        "bounceVelocity":effect.bounce_velocity,
        "entry":effect.entry,
        "firstEntry":effect.first_entry,
        "frameCount":effect.frame_count,
        "framePhase":effect.frame_phase,
        "frameVelocity":effect.frame_velocity,
        "frameVelocityDamping":effect.frame_velocity_damping,
        "frameTicks":effect.frame_ticks,
        "height":effect.height,
        "id":effect.id,
        "kind":effect.kind.id(),
        "lastStepTick":effect.last_step_tick.map_or(-1,|tick|tick as i64),
        "lifetimeTicks":effect.lifetime_ticks,
        "opacityTimer":effect.opacity_timer,
        "ownerActorId":effect.owner_actor_id,
        "painterRegistration":effect.painter_registration.map(painter_registration_json),
        "presentationOwner":effect.presentation_owner.id(),
        "position":point_json(effect.position),
        "role":effect.role,
        "rotationDeg":effect.rotation_deg,
        "scale":effect.scale,
        "scaleY":effect.scale_y,
        "scaleMultiplier":effect.scale_multiplier,
        "shadow":effect.shadow,
        "spawnTick":effect.spawn_tick,
        "tint":effect.tint,
        "verticalVelocity":effect.vertical_velocity,
        "velocity":point_json(effect.velocity),
        "velocityDamping":effect.velocity_damping,
    });
    if let Some(bias) = effect.painter_sort_bias {
        record["painterSortBias"] = json!(bias);
    }
    if let Some(oscillation) = effect.scrap_oscillation {
        record["scrapOscillation"] = json!({
        "phaseDeg":oscillation.phase_deg,"stepDeg":oscillation.step_deg,"amplitudeDeg":oscillation.amplitude_deg});
    }
    record
}

fn assert_death_effect_equal(
    effect: &solomon_sim::enemies::model::EnemyDeathEffect,
    expected: &Value,
    context: &str,
) {
    use solomon_sim::enemies::model::EnemyDeathEffectKind;
    let mut actual = death_effect_json(effect);
    let bouncer = matches!(
        effect.kind,
        EnemyDeathEffectKind::Bouncer
            | EnemyDeathEffectKind::SmokyBouncer
            | EnemyDeathEffectKind::BlackSmokyBouncer
    );
    let cocoon_fragment = effect.role == "cocoon-fragment";
    let faculty_smoke = effect.role.starts_with("faculty-") && effect.role.ends_with("-smoke");
    // Radial trajectories and smoke offsets start with sin/cos; all their other fields,
    // and the random stream itself, still compare exactly.
    if bouncer
        || cocoon_fragment
        || effect.role.ends_with(":smoke")
        || effect.role.starts_with("maggot-perspective-fade:")
        || faculty_smoke
        || effect.role == "mouth-beam-source"
        || effect.role == "discorporeal-bite-trail"
        || effect.role.starts_with("zombie-late-splat:")
        || effect.kind == EnemyDeathEffectKind::Scrap
        || effect.role == "discorporeal-death-branch"
        || effect.role.starts_with("demon-death-fire:")
    {
        for coordinate in ["x", "y"] {
            assert_trig(
                actual["position"][coordinate].as_f64().unwrap(),
                expected["position"][coordinate].as_f64().unwrap(),
                context,
            );
        }
        actual["position"] = expected["position"].clone();
    }
    if bouncer || cocoon_fragment || faculty_smoke || effect.kind == EnemyDeathEffectKind::Scrap {
        for coordinate in ["x", "y"] {
            assert_trig(
                actual["velocity"][coordinate].as_f64().unwrap(),
                expected["velocity"][coordinate].as_f64().unwrap(),
                context,
            );
        }
        actual["velocity"] = expected["velocity"].clone();
    }
    if effect.kind == EnemyDeathEffectKind::Scrap || effect.role == "discorporeal-mouth-warmup" {
        assert_trig(effect.rotation_deg, expected["rotationDeg"].as_f64().unwrap(), context);
        actual["rotationDeg"] = expected["rotationDeg"].clone();
    }
    if effect.kind == EnemyDeathEffectKind::MoveFadeSin {
        assert_trig(effect.alpha, expected["alpha"].as_f64().unwrap(), context);
        actual["alpha"] = expected["alpha"].clone();
    }
    assert_json_equal(&actual, expected, context);
}

fn death_effect_kind(id: &str) -> solomon_sim::enemies::model::EnemyDeathEffectKind {
    use solomon_sim::enemies::model::EnemyDeathEffectKind;
    match id {
        "banish" => EnemyDeathEffectKind::Banish,
        "bouncer" => EnemyDeathEffectKind::Bouncer,
        "smoky-bouncer" => EnemyDeathEffectKind::SmokyBouncer,
        "fade" => EnemyDeathEffectKind::Fade,
        "fade-additive" => EnemyDeathEffectKind::FadeAdditive,
        "fade-perspective" => EnemyDeathEffectKind::FadePerspective,
        "fade-perspective-clipped" => EnemyDeathEffectKind::FadePerspectiveClipped,
        "fade-scale-perspective" => EnemyDeathEffectKind::FadeScalePerspective,
        "fade-scale" => EnemyDeathEffectKind::FadeScale,
        "fire-array" => EnemyDeathEffectKind::FireArray,
        "late-splat" => EnemyDeathEffectKind::LateSplat,
        "move-fade" => EnemyDeathEffectKind::MoveFade,
        "move-fade-perspective" => EnemyDeathEffectKind::MoveFadePerspective,
        "sprite-array" => EnemyDeathEffectKind::SpriteArray,
        "unbind" => EnemyDeathEffectKind::Unbind,
        "banish-black" => EnemyDeathEffectKind::BanishBlack,
        "scrap" => EnemyDeathEffectKind::Scrap,
        "black-smoky-bouncer" => EnemyDeathEffectKind::BlackSmokyBouncer,
        "move-fade-sin" => EnemyDeathEffectKind::MoveFadeSin,
        other => panic!("unknown native death-effect program {other}"),
    }
}

#[test]
fn native_death_effect_programs_birth_order_and_skipped_ticks_match_the_website() {
    use solomon_sim::enemies::effects::*;
    use solomon_sim::enemies::model::*;
    use solomon_sim::enemies::world::EnemyWorld;
    use solomon_sim::painter::WorldPainterOrder;
    use solomon_sim::rng::next_xorshift_random;
    let goldens: EffectGoldens =
        serde_json::from_str(&fixture("enemy-effects-goldens.json")).unwrap();
    let mut world = EnemyWorld::new("effect-oracle", 0);
    let mut order = WorldPainterOrder::default();
    let owner = DeathEffectOwner { id: 11, position: Vec2::new(200.5, 300.25) };
    for (index, id) in goldens.kinds.iter().enumerate() {
        let kind = death_effect_kind(id);
        if matches!(
            kind,
            EnemyDeathEffectKind::Bouncer
                | EnemyDeathEffectKind::SmokyBouncer
                | EnemyDeathEffectKind::BlackSmokyBouncer
        ) {
            let bouncer_kind = match kind {
                EnemyDeathEffectKind::SmokyBouncer => BouncerKind::Smoky,
                EnemyDeathEffectKind::BlackSmokyBouncer => BouncerKind::BlackSmoky,
                _ => BouncerKind::Plain,
            };
            world.spawn_bouncer(
                owner,
                0,
                |world| 92 + (next_xorshift_random(&mut world.rng_state) * 4.0).floor() as usize,
                &format!("oracle-{id}"),
                |world| {
                    let heading =
                        next_xorshift_random(&mut world.rng_state) * 360.0 * std::f64::consts::PI
                            / 180.0;
                    BouncerOptions {
                        kind: bouncer_kind,
                        height: Some(-5.0),
                        scale: Some(1.25),
                        opacity_timer: Some(2.0),
                        velocity: Some(Vec2::new(heading.sin() * 1.5, -heading.cos() * 1.5)),
                        ..BouncerOptions::default()
                    }
                },
                true,
                &mut order,
            );
        } else {
            world.spawn_simple_death_effect(
                owner,
                0,
                SimpleDeathEffectSpec {
                    alpha: 1.0,
                    alpha_loss_per_tick: 0.02,
                    atlas: EnemyEffectAtlas::BadGuys,
                    blend_mode: EnemyEffectBlend::Add,
                    entry: 92,
                    kind,
                    lifetime_ticks: 42,
                    role: &format!("oracle-{id}"),
                    scale: 1.25,
                },
                DeathEffectOverrides {
                    opacity_timer: Some(2.0),
                    frame_count: Some(4),
                    frame_velocity: Some(if kind == EnemyDeathEffectKind::MoveFadeSin {
                        15.0
                    } else {
                        0.5
                    }),
                    frame_velocity_damping: Some(0.95),
                    frame_ticks: Some(2.0),
                    velocity: Some(Vec2::new(0.6, -0.25)),
                    velocity_damping: Some(0.9),
                    scale_y: Some(0.75),
                    scale_multiplier: Some(1.1),
                    rotation_deg: Some(10.0),
                    angular_velocity_deg: Some(2.0),
                    painter_sort_bias: if index == 0 { Some(-0.25) } else { None },
                    presentation_owner: if index == 3 {
                        Some(DeathPresentationOwner::PreWorldQueue)
                    } else {
                        None
                    },
                    spawn_delay_ticks: if index == 4 { Some(2) } else { None },
                    scrap_oscillation: if kind == EnemyDeathEffectKind::Scrap {
                        Some(ScrapOscillation {
                            phase_deg: 10.0,
                            step_deg: 11.0,
                            amplitude_deg: 30.0,
                        })
                    } else {
                        None
                    },
                    ..DeathEffectOverrides::default()
                },
                &mut order,
            );
        }
    }
    assert_eq!(world.death_effects.len(), goldens.births.len());
    for (effect, expected) in world.death_effects.iter().zip(&goldens.births) {
        assert_death_effect_equal(effect, expected, &format!("effect birth {}", effect.id));
    }
    world.step_pre_world_effect_births(0);
    for snapshot in &goldens.snapshots {
        world.step_death_effects(snapshot.tick, &mut order);
        assert_eq!(
            world.death_effects.len(),
            snapshot.effects.len(),
            "effect population tick {}",
            snapshot.tick
        );
        for (effect, expected) in world.death_effects.iter().zip(&snapshot.effects) {
            assert_death_effect_equal(
                effect,
                expected,
                &format!("effect {} at tick {}", effect.id, snapshot.tick),
            );
        }
        assert_eq!(world.rng_state, snapshot.rng, "effect RNG at tick {}", snapshot.tick);
        assert_eq!(world.next_death_effect_id, snapshot.next_id);
        assert_eq!(order.next_actor_ordinal, snapshot.painter_order.actor);
        assert_eq!(order.next_transient_ordinal, snapshot.painter_order.transient);
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VoiceGoldens {
    durations: Vec<(String, u64)>,
    cases: Vec<VoiceCase>,
    narration_samples: Vec<NarrationSample>,
}
#[derive(Deserialize)]
struct VoiceCase {
    seed: i64,
    line: usize,
    members: Vec<bool>,
    busy: bool,
    samples: Vec<VoiceSample>,
    rng: Vec<f64>,
}
#[derive(Deserialize)]
struct VoiceSample {
    state: Option<VoiceControllerGolden>,
    cues: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VoiceControllerGolden {
    line: usize,
    ticks_remaining: u64,
}
#[derive(Deserialize)]
struct NarrationSample {
    tick: u64,
    state: Value,
}

fn boss_narration_json(state: &solomon_sim::enemies::voices::BossNarration) -> Value {
    json!({"current":state.current.map(|line|json!({"cue":line.cue.id(),"eventId":line.event_id,"startedTick":line.started_tick})),
        "idleTicks":state.idle_ticks,"mix":state.mix,"nextEventId":state.next_event_id,
        "pending":state.pending.iter().map(|cue|cue.id()).collect::<Vec<_>>(),"ticksRemaining":state.ticks_remaining})
}

fn enemy_event_json(event: &solomon_sim::enemies::model::EnemyEvent) -> Value {
    let mut record = json!({"actorId":event.actor_id,"eventId":event.event_id,
        "tick":event.tick,"type":event.kind.id()});
    if let Some(sound) = event.sound {
        record["sound"] = json!(sound.id());
    }
    if let Some(pitch) = event.pitch {
        record["pitch"] = json!(pitch);
    }
    if let Some(gain) = event.gain_scale {
        record["gainScale"] = json!(gain);
    }
    if let Some(position) = event.source_position {
        record["sourcePosition"] = point_json(position);
    }
    if let Some(target) = &event.target_player_id {
        record["targetPlayerId"] = json!(target);
    }
    if let Some(registration) = event.painter_registration {
        record["painterRegistration"] = painter_registration_json(registration);
    }
    if let Some(id) = event.projectile_id {
        record["projectileId"] = json!(id);
    }
    if let Some(count) = event.count {
        record["count"] = json!(count);
    }
    if let Some(flash) = event.screen_flash {
        record["screenFlash"] = json!({"alpha":flash.alpha,"red":flash.red,
            "green":flash.green,"blue":flash.blue,"decayPerTick":flash.decay_per_tick,
            "pointAttenuated":flash.point_attenuated});
    }
    if let Some(shake) = event.camera_shake {
        record["cameraShake"] = json!({"attenuation":shake.attenuation.id(),"displacement":point_json(shake.displacement)});
    }
    if let Some(output) = event.output {
        record["output"] = json!(output.id());
    }
    if let Some(stream) = &event.stream {
        record["stream"] = json!(stream);
    }
    if let Some(clear) = event.screen_flash_only_if_clear {
        record["screenFlashOnlyIfClear"] = json!(clear);
    }
    record
}

fn primary_target_json(target: &solomon_sim::spells::targeting::PrimarySpellTarget) -> Value {
    json!({"active":target.active,"actorFlags":target.actor_flags,"attachment":point_json(target.attachment),
        "bodyRadius":target.body_radius,"cellBindingOrder":target.cell_binding_order,"id":target.id,
        "headingDeg":target.heading_degrees,"kind":"enemy","nativePriority":target.native_priority,
        "queryLane":"grid","pendingRemove":target.pending_remove,"position":point_json(target.position),
        "registrationOrder":target.registration_order})
}

fn player_damage_json(damage: &solomon_sim::enemies::model::EnemyPlayerDamage) -> Value {
    let mut record = json!({"actorId":damage.actor_id,"physicalDamage":damage.physical_damage,
        "magicDamage":damage.magic_damage,"coldSlowTicks":damage.cold_slow_ticks,"dazzleTicks":damage.dazzle_ticks,
        "eventId":damage.event_id,"playerId":damage.player_id,"poisonDamage":damage.poison_damage,"poisonDuration":damage.poison_duration});
    if let Some(chance) = damage.crow_blind_chance_percent {
        record["crowBlindChancePercent"] = json!(chance);
    }
    if damage.suppress_flash {
        record["suppressFlash"] = json!(true);
    }
    if damage.suppress_hit_response {
        record["suppressHitResponse"] = json!(true);
    }
    if let Some(amount) = damage.poison_contact_damage {
        record["poisonContactDamage"] = json!(amount);
    }
    if let Some(strength) = damage.webbed_strength {
        record["webbedStrength"] = json!(strength);
    }
    if let Some(strength) = damage.hit_strength {
        record["hitStrength"] = json!(strength);
    }
    if let Some(source) = damage.source {
        record["source"] = source.map_or(Value::Null, |source| {
            json!({"position":point_json(source.position),"collisionRadius":source.collision_radius,
                "reflectableActorId":source.reflectable_actor_id})
        });
    }
    if let Some(fraction) = damage.mana_damage_maximum_fraction {
        record["manaDamageMaximumFraction"] = json!(fraction);
    }
    if damage.tragic_circle {
        record["tragicCircle"] = json!(true);
    }
    record
}

struct GroundEnemyGeometry {
    blocked: bool,
    spell_blocked: bool,
}
impl solomon_sim::enemies::path::EnemyNavigation for GroundEnemyGeometry {
    fn direct_path_clear(
        &self,
        _start: Vec2,
        _end: Vec2,
        _clearance: f64,
        _body_radius: f64,
    ) -> bool {
        !self.blocked
    }
    fn find_route(
        &self,
        _start: Vec2,
        _end: Vec2,
        _clearance: f64,
        _body_radius: f64,
    ) -> Option<Vec<Vec2>> {
        None
    }
}
impl solomon_sim::enemies::world::EnemyGeometry for GroundEnemyGeometry {
    fn projectile_world_blocked(
        &self,
        _query: solomon_sim::enemies::world::EnemyProjectileWorldQuery,
    ) -> bool {
        false
    }
    fn resolve_movement(&self, request: solomon_sim::enemies::world::EnemyMovementRequest) -> Vec2 {
        if self.blocked { request.position } else { request.requested_position }
    }
    fn line_obstruction(&self, _start: Vec2, _end: Vec2, _mask: u32) -> Option<Vec2> {
        None
    }
    fn clip_spell_segment(&self, start: Vec2, end: Vec2) -> Vec2 {
        if self.spell_blocked {
            Vec2::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5)
        } else {
            end
        }
    }
}

fn enemy_projectile_json(
    projectile: solomon_sim::enemies::projectiles::EnemyProjectileRef<'_>,
) -> Value {
    use solomon_sim::enemies::projectiles::{EnemyProjectileMotion, EnemyProjectileRead};
    let state = projectile.state();
    let mut record = json!({
        "ageTicks":state.age_ticks,"bounceVelocity":state.bounce_velocity,"chillTumbleAccumulator":state.chill_tumble_accumulator,
        "coldSlowTicks":state.cold_slow_ticks,"contactRadius":state.contact_radius,"damage":state.damage,"secondaryDamage":state.secondary_damage,
        "headingDeg":state.heading_deg,"hitPlayerIds":state.hit_player_ids,"homing":state.homing,"id":state.id,"lastStepTick":state.last_step_tick,
        "lightRegistration":state.light_registration.map(painter_registration_json),"lifetimeTicks":state.lifetime_ticks,"minimumSpeed":state.minimum_speed,
        "nativeTypeId":state.native_type_id,"nativeCellBindingOrder":state.native_cell_binding_order,"nativeRegistrationOrder":state.native_registration_order,
        "ownerActorId":state.owner_actor_id,"painterRegistration":painter_registration_json(state.painter_registration),"payload":state.payload.id(),
        "poisonDamage":state.poison_damage,"poisonDuration":state.poison_duration,"position":point_json(state.position),"speed":state.speed,
        "settledTicksRemaining":state.settled_ticks_remaining,"spawnTick":state.spawn_tick,"targetPlayerId":state.target_player_id,"turnSpeed":state.turn_speed,
        "verticalOffset":state.vertical_offset,"verticalVelocity":state.vertical_velocity,"visualPhaseDeg":state.visual_phase_deg,"visualScale":state.visual_scale,
        "kind":projectile.kind().id(),
    });
    if let EnemyProjectileMotion::Arrow { velocity } = projectile.motion() {
        record["velocity"] = point_json(velocity);
    }
    record
}

fn mage_lightning_json(pulse: &solomon_sim::enemies::projectiles::MageLightningPulse) -> Value {
    use solomon_sim::enemies::projectiles::MageLightningContact;
    let contact = match &pulse.contact {
        MageLightningContact::World { position } => {
            json!({"kind":"world","position":point_json(*position)})
        }
        MageLightningContact::TargetAttached { local_offset, target_player_id } => {
            json!({"kind":"target-attached","localOffset":point_json(*local_offset),"targetPlayerId":target_player_id})
        }
    };
    json!({"enhancedEffects":pulse.enhanced_effects,"contact":contact,"endpoint":point_json(pulse.endpoint),"id":pulse.id,"lightRegistration":painter_registration_json(pulse.light_registration),
        "midpoint":point_json(pulse.midpoint),"ownerActorId":pulse.owner_actor_id,"painterRegistrations":pulse.painter_registrations.iter().copied().map(painter_registration_json).collect::<Vec<_>>(),
        "seed":pulse.seed,"source":point_json(pulse.source),"tick":pulse.tick})
}

// Current authority traces use the canonical V8 math kernels. Retain every
// independently evolving root, heading and effect value in the exact comparison.
fn assert_current_enemy_json(actual: &Value, expected: &Value, label: &str) {
    source::assert_website_json(actual, expected, label, &[]);
}

#[test]
fn enemy_approach_flight_contact_ejections_and_articulation_match_the_website() {
    use solomon_sim::boneyard::BoneyardBounds;
    use solomon_sim::enemies::demon_skull_actions::begin_demon_skull_action;
    use solomon_sim::enemies::demon_skull_controller::DemonSkullActionKind;
    use solomon_sim::enemies::faculty_controller::create_native_faculty_action;
    use solomon_sim::enemies::model::*;
    use solomon_sim::enemies::webbed::apply_native_webbed;
    use solomon_sim::enemies::world::NativeEnemyMovementView;
    use solomon_sim::enemies::world::*;
    use solomon_sim::painter::WorldPainterOrder;
    let goldens: Value = serde_json::from_str(&fixture("enemy-ai-goldens.json.zlib")).unwrap();
    assert_eq!(goldens["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    let mut presentations = presentation_cases::EnemyPresentationGoldens::new();
    let mut replay_case = |case: &Value| {
        let name = case["name"].as_str().unwrap();
        let geometry = GroundEnemyGeometry {
            blocked: case["blocked"].as_bool().unwrap_or(false),
            spell_blocked: case["spellBlocked"].as_bool().unwrap_or(false),
        };
        let mut players = [EnemyTarget {
            id: "hero".to_owned(),
            summoned: case["summoned"].as_bool().unwrap_or(false),
            alive: true,
            connected: true,
            eligible: !case["noTargets"].as_bool().unwrap_or(false),
            collision_radius: 25.0,
            heading_deg: None,
            position: Vec2::new(500.0, 500.0),
            velocity_per_tick: if case["stationary"].as_bool() == Some(true) {
                Vec2::ZERO
            } else {
                Vec2::new(0.75, -0.25)
            },
        }];
        let mut world = EnemyWorld::new(case["seed"].as_str().unwrap(), 50);
        let mut order = WorldPainterOrder { next_actor_ordinal: 100, next_transient_ordinal: 70 };
        let intents: Vec<_> =
            case["intents"].as_array().unwrap().iter().map(spawn_intent).collect();
        let mut shared_loot_rng = case["sharedLootSeed"].as_i64().map(solomon_sim::NativeRng::new);
        let mut roll_loot =
            |bound| shared_loot_rng.as_mut().unwrap().draw_integer(bound as i32) as u32;
        world.spawn(
            &intents,
            EnemySpawnContext {
                players: &players,
                tick: 10,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: if case["sharedLootSeed"].is_number() {
                    Some(&mut roll_loot)
                } else {
                    None
                },
            },
            &geometry,
            Some(&mut order),
        );
        if let Some(kind) = case["facultyAction"].as_str() {
            let kind = match kind {
                "throw" => FacultyActionKind::Throw,
                "lightning" => FacultyActionKind::Lightning,
                "two-hand" => FacultyActionKind::TwoHand,
                _ => panic!("recorded Faculty action"),
            };
            let action = create_native_faculty_action(kind, &mut world.steering_rng);
            let EnemyBrain::Faculty(brain) = world.actors.get_mut(0).unwrap().brain else {
                panic!("Faculty action setup")
            };
            brain.hand_mask = action.hand_mask;
            brain.action = Some(action);
            brain.primary_poll = 10000;
            brain.secondary_cooldown = 10000;
        }
        if case["megaDeath"].as_bool() == Some(true) {
            let EnemyBrain::DemonSkull(brain) = world.actors.get_mut(0).unwrap().brain else {
                panic!("mega-death setup")
            };
            brain.capabilities |= 16;
        }
        if let Some(kind) = case["skullAction"].as_str() {
            let kind = match kind {
                "bite" => DemonSkullActionKind::Bite,
                "scream" => DemonSkullActionKind::Scream,
                "flair" => DemonSkullActionKind::Flair,
                "eyes" => DemonSkullActionKind::Eyes,
                "mouth" => DemonSkullActionKind::Mouth,
                "spit" => DemonSkullActionKind::Spit,
                _ => panic!("recorded DemonSkull action"),
            };
            let mut actor = world.actors.get(0).unwrap().snapshot();
            let speed = actor.config.attack_speed;
            begin_demon_skull_action(
                &mut world,
                &mut actor,
                kind,
                &EnemyStepContext {
                    enhanced_effects: true,
                    paused: false,
                    tick: 10,
                    players: &players,
                    geometry: &geometry,
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
                speed,
            );
            world.actors.replace(0, actor);
        }
        if case["webbed"].as_bool() == Some(true) {
            world.webbed_players.insert(
                "hero".to_owned(),
                WebbedState { severity: 3.0, strength: 45.0, cocoon_health: 45.0, hit_pulse: 0.0 },
            );
        }
        world.last_step_tick = Some(10);
        if case["contactBirth"].as_bool() == Some(true) {
            let _events = world.add_native_cocoon(
                "hero",
                players[0].position,
                &players,
                10,
                Some(&mut order),
            );
        }
        if case["opening"].as_bool() == Some(true) {
            let EnemyBrain::Coffin(brain) = world.actors.get_mut(0).unwrap().brain else {
                panic!("Coffin opening setup");
            };
            brain.phase = CoffinPhase::Opening;
            brain.phase_ticks_remaining = 1;
            brain.phase_tick = 45;
        }
        let mut dampened_after_trail = false;
        let mut samples = case["samples"].as_array().unwrap().iter().peekable();
        for tick in 11..=10 + case["ticks"].as_u64().unwrap_or(400) {
            let motion = case["motion"].as_str().unwrap();
            players[0].position = if motion == "middle" {
                Vec2::new(375.0, 250.0)
            } else if motion == "follow" {
                players[0].heading_deg = Some((((tick - 11) / 10) * 15 % 360) as f64);
                Vec2::new(
                    100.0 + ((tick - 11) / 20) as f64 * 5.0,
                    200.0 + ((tick - 11) % 40) as f64,
                )
            } else if motion == "near" || motion == "leaves" {
                Vec2::new(185.0, 250.0)
            } else {
                Vec2::new(500.0 + ((tick - 11) / 100) as f64 * 50.0, 500.0)
            };
            players[0].eligible = !case["noTargets"].as_bool().unwrap_or(false)
                && (motion != "leaves" || !(120..230).contains(&tick));
            if case["ownerDiesAt"].as_u64() == Some(tick) {
                players[0].alive = false;
            }
            if case["removeCocoonAt"].as_u64() == Some(tick) {
                world.remove_cocoon_owner("hero");
            }
            let visible = |_position| NativeEnemyVisibility {
                admitted: !case["offscreen"].as_bool().unwrap_or(false),
                intensity: case["light"].as_f64().unwrap_or(1.0),
            };
            let light_at = |_position| case["light"].as_f64().unwrap_or(1.0);
            let projected = |position: Vec2| {
                (0.0..=1000.0).contains(&position.x) && (0.0..=1000.0).contains(&position.y)
            };
            let view = match goldens["movementViewInputs"][name].as_str() {
                Some("spiderMovementView") => None,
                Some("nativeMovementView") => Some(case["view"].as_str().unwrap()),
                None => {
                    assert!(
                        case["view"].is_null(),
                        "{name}: view input requires its Source contract"
                    );
                    None
                }
                Some(other) => panic!("{name}: unknown Source movement view input {other}"),
            };
            let mut puppet_targets = Vec::new();
            if case["puppets"].as_bool() == Some(true) {
                use solomon_sim::enemies::puppet_hits::*;
                use solomon_sim::spells::targeting::PrimarySpellQueryLane;
                for (index, kind) in [
                    EnemyPuppetHitKind::Scenery,
                    EnemyPuppetHitKind::Goodie,
                    EnemyPuppetHitKind::Meteor,
                    EnemyPuppetHitKind::Leviathan,
                    EnemyPuppetHitKind::Arrow,
                    EnemyPuppetHitKind::Firebolt,
                ]
                .into_iter()
                .enumerate()
                {
                    let mut target = boneyard_puppet_target(
                        format!("fixture:{}", kind.id()),
                        Vec2::new(275.0 + index as f64 * 20.0, 250.0),
                        if kind == EnemyPuppetHitKind::Meteor {
                            8
                        } else if kind == EnemyPuppetHitKind::Leviathan {
                            0x200
                        } else {
                            4
                        },
                        10 + index,
                        Some(kind),
                        if index >= 4 {
                            PrimarySpellQueryLane::Transient
                        } else {
                            PrimarySpellQueryLane::Grid
                        },
                    );
                    if kind == EnemyPuppetHitKind::Meteor {
                        target.target.grid_position = Some(Vec2::ZERO);
                    }
                    puppet_targets.push(target);
                }
            }
            let cameras = [if view == Some("offscreen") {
                BoneyardBounds::new(2000.0, 2000.0, 500.0, 500.0)
            } else {
                BoneyardBounds::new(0.0, 0.0, 1000.0, 1000.0)
            }];
            let statuses = case["statuses"].as_bool() == Some(true);
            let effects: Vec<_> = if statuses {
                world
                    .actors
                    .iter()
                    .map(|actor| {
                        (
                            actor.id,
                            EnemyAbilityEffect {
                                time_scale: if (100..140).contains(&tick) {
                                    0.0
                                } else if (40..100).contains(&tick) {
                                    0.35
                                } else {
                                    1.0
                                },
                                weaken_factor: if (60..180).contains(&tick) { 0.25 } else { 1.0 },
                                disrupted_ticks: u64::from((180..210).contains(&tick)),
                                flee_ticks: u64::from((260..310).contains(&tick)),
                                dazzle_ticks: u64::from((360..390).contains(&tick)),
                            },
                        )
                    })
                    .collect()
            } else {
                Vec::new()
            };
            if case["dampenAt"].as_u64() == Some(tick) {
                let ids: Vec<_> = world.actors.iter().map(|actor| actor.id).collect();
                world.dampen_casters(&ids, tick, Some(&mut order));
            }
            let mut roll_loot =
                |bound| shared_loot_rng.as_mut().unwrap().draw_integer(bound as i32) as u32;
            let context = EnemyStepContext {
                enhanced_effects: true,
                paused: case["pause"].as_bool() == Some(true) && (50..90).contains(&tick),
                tick,
                players: &players,
                geometry: &geometry,
                ability_effects: &effects,
                arena_scalars: EnemyArenaScalars::default(),
                dialogue_busy: statuses && (400..450).contains(&tick),
                roll_loot_seed: if case["sharedLootSeed"].is_number() {
                    Some(&mut roll_loot)
                } else {
                    None
                },
                native_visibility: Some(&visible),
                projected_point_visible: Some(&projected),
                native_view_bounds: (case["token"] == "DIREFACULTY"
                    || case["token"] == "DEMONSKULL")
                    .then_some(BoneyardBounds::new(0.0, 0.0, 1000.0, 1000.0)),
                puppet_targets: &puppet_targets,
                light_at: Some(&light_at),
                native_movement_view: view.map(|view| NativeEnemyMovementView {
                    arena_bounds: BoneyardBounds::new(0.0, 0.0, 1000.0, 1000.0),
                    cameras: &cameras,
                    enhanced_effects: view != "basic",
                }),
            };
            if case["killAt"].as_u64() == Some(tick) {
                let mut damage = EnemyDamageRequest::new(
                    world.actors.get(0).unwrap().id,
                    1e9,
                    tick,
                    Some("hero".to_owned()),
                );
                damage.ether_drain_capture = case["etherCapture"].as_bool().unwrap_or(false);
                world.damage(damage, Some(&mut order));
            }
            if case["removeOwnerAt"].as_u64() == Some(tick) {
                world.actors.clear();
            }
            if case["removeActorAt"].as_u64() == Some(tick) {
                world.actors.clear();
            }
            if case["maggotHitAt"].as_u64() == Some(tick) {
                let id = world
                    .maggots
                    .iter()
                    .find(|maggot| {
                        maggot.combat_active && maggot.life_state == EnemyLifeState::Alive
                    })
                    .expect("an admitted Maggot before its recorded hit")
                    .id;
                let result = world.damage(
                    EnemyDamageRequest::new(id, 500.0, tick, Some("hero".to_owned())),
                    Some(&mut order),
                );
                assert!(result.accepted && result.killed);
            }
            world.pending_spawn_intents.clear();
            if case["detachAt"].as_u64() == Some(tick) {
                let actor = world.actors.get(0).unwrap().snapshot();
                world.detach_heartmonger_crows(&actor, tick);
                world.actors.clear();
            }
            let mut counts = Vec::new();
            let mut resolve = |live: EnemyLiveCounts| {
                counts.push(
                    json!({"enemies":live.enemies,"zombies":live.zombies,"bosses":live.bosses}),
                );
                case["lateIntents"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|intent| intent["spawnTick"].as_u64() == Some(tick))
                    .map(spawn_intent)
                    .collect()
            };
            if case["deferred"].as_bool() == Some(true) {
                world.begin_deferred_projectile_painters();
                assert!(!world.painter_registrations_committed());
            }
            world.step(context, Some(&mut order), Some(&mut resolve));
            let dampen_after_trail = case["dampenAfterTrail"].as_bool() == Some(true)
                && !dampened_after_trail
                && world.projectile_effects.iter().any(|effect| {
                    matches!(
                        effect.kind,
                        solomon_sim::enemies::projectiles::EnemyProjectileEffectKind::FireboltTrail
                    ) && effect.state.spawn_tick == tick
                });
            if dampen_after_trail {
                let ids: Vec<_> = world.actors.iter().map(|actor| actor.id).collect();
                world.dampen_casters(&ids, tick, Some(&mut order));
                dampened_after_trail = true;
            }
            if case["deferred"].as_bool() == Some(true) {
                world.commit_deferred_projectile_painters(&mut order);
            }
            world.assert_painter_registrations_committed();
            if case["applyWebContacts"].as_bool() == Some(true) {
                for damage in world.player_damage.clone() {
                    let Some(strength) = damage.webbed_strength else {
                        continue;
                    };
                    let prior = world.webbed_players.get(&damage.player_id).copied();
                    let next = apply_native_webbed(prior, strength);
                    world.webbed_players.insert(damage.player_id.clone(), next);
                    if next.severity == 3.0 && prior.is_none_or(|prior| prior.severity < 3.0) {
                        let _events = world.add_native_cocoon(
                            &damage.player_id,
                            players[0].position,
                            &players,
                            tick,
                            Some(&mut order),
                        );
                    }
                }
            }
            let Some(expected) =
                samples.peek().filter(|sample| sample["tick"].as_u64() == Some(tick))
            else {
                continue;
            };
            presentations.check_world(name, tick, &world);
            let label = format!("{name} tick {tick}");
            assert_eq!(
                dampen_after_trail,
                expected["dampenAfterTrail"].as_bool().unwrap_or(false),
                "{label} primary-phase caster producer"
            );
            assert_current_enemy_json(
                &shared_loot_rng.as_ref().map_or(Value::Null, |rng| json!(rng_record(rng))),
                &expected["sharedLootRng"],
                &format!("{label} shared loot RNG"),
            );
            assert_current_enemy_json(
                &json!(counts),
                &expected["liveCounts"],
                &format!("{label} late-wave counts"),
            );
            let faculty = world
                .faculty_voice_controller
                .map(|voice| json!({"line":voice.line,"ticksRemaining":voice.ticks_remaining}))
                .unwrap_or(Value::Null);
            assert_current_enemy_json(
                &faculty,
                &expected["facultyVoiceController"],
                &format!("{label} Faculty voice"),
            );
            assert_current_enemy_json(
                &boss_narration_json(&world.boss_narration),
                &expected["bossNarration"],
                &format!("{label} narration"),
            );

            let rewards:Vec<_>=world.rewards.iter().map(|reward| {
                let source=&reward.loot_source;
                let mut loot=json!({"actorSeed":source.actor_seed,"enemyToken":source.enemy_token.id(),
                    "onDeathProgram":source.on_death_program.map(|_|"miniboss-die"),"participantSlot":source.participant_slot,"position":point_json(source.position)});
                if let Some(policies)=source.policies {loot["policies"]=json!({"gold":policies.gold,"item":policies.item,"orb":policies.orb,
                    "potion":policies.potion,"powerup":policies.powerup,"specificItem":policies.specific_item});}
                if let Some(uid)=source.recipe_uid {loot["recipeUid"]=json!(uid);}
                json!({"actorId":reward.actor_id,"eventId":reward.event_id,"experience":reward.experience,"lootSource":loot,"playerId":reward.player_id})
            }).collect();
            assert_current_enemy_json(&json!(rewards), &expected["rewards"], &label);
            let puppet_hits: Vec<_> = world
                .puppet_hits
                .iter()
                .map(|hit| {
                    json!({"feedback":puppet_hit_json(&hit.feedback),
                "hitTick":hit.hit_tick,"kind":hit.kind.id(),"targetId":hit.target_id})
                })
                .collect();
            assert_current_enemy_json(&json!(puppet_hits), &expected["puppetHits"], &label);
            assert_current_enemy_json(
                &json!({"deathStreamTicksRemaining":world.demon_skull_encounter.death_stream_ticks_remaining,
                "screamStreamTicksRemaining":world.demon_skull_encounter.scream_stream_ticks_remaining,
                "healthTriggersEnabled":world.demon_skull_encounter.health_triggers_enabled,
                "healthTriggersFiredMask":world.demon_skull_encounter.health_triggers_fired_mask,
                "pendingCapabilities":world.demon_skull_encounter.pending_capabilities}),
                &expected["demonSkullEncounter"],
                &label,
            );
            let boss_spells: Vec<_> =
                world.boss_spells.iter().map(boss_cases::boss_spell_json).collect();
            assert_current_enemy_json(
                &json!(boss_spells),
                &expected["bossSpells"],
                &format!("{label} boss spells"),
            );
            let crows: Vec<_> = world.detached_crows.iter().map(|crow| json!({
                "flight":crow_state_json(&crow.flight),"ownerActorId":crow.owner_actor_id,"spawnTick":crow.spawn_tick,
            })).collect();
            assert_current_enemy_json(
                &json!(crows),
                &expected["detachedCrows"],
                &format!("{label} detached flock"),
            );
            assert_eq!(
                world.actors.len(),
                expected["actors"].as_array().unwrap().len(),
                "{label} actor count"
            );
            for (actor, expected_actor) in
                world.actors.iter().zip(expected["actors"].as_array().unwrap())
            {
                let mut expected_actor = expected_actor.clone();
                expected_actor["config"] = source_config_record(&expected_actor["config"]);
                assert_current_enemy_json(&actor_json(&actor), &expected_actor, &label);
            }
            assert_eq!(
                world.maggots.len(),
                expected["maggots"].as_array().unwrap().len(),
                "{label} Maggot count"
            );
            for (maggot, expected) in
                world.maggots.iter().zip(expected["maggots"].as_array().unwrap())
            {
                assert_current_enemy_json(
                    &maggot_json(maggot),
                    expected,
                    &format!("{label} Maggot {}", maggot.id),
                );
            }
            let retired:Vec<_>=world.retired.iter().map(|retirement|json!({"actorId":retirement.actor_id,"eventId":retirement.event_id})).collect();
            assert_current_enemy_json(
                &json!(retired),
                &expected["retired"],
                &format!("{label} retired"),
            );
            assert_current_enemy_json(
                &json!(world.spawned_actor_ids),
                &expected["spawned"],
                &format!("{label} spawned"),
            );
            assert_current_enemy_json(
                &json!(world.next_death_epoch),
                &expected["nextDeathEpoch"],
                &label,
            );
            let silks: Vec<_> = world.silks.iter().map(spider_cases::silk_actor_json).collect();
            assert_current_enemy_json(
                &json!(silks),
                &expected["silks"],
                &format!("{label} Silk rows"),
            );
            let webbed:serde_json::Map<String,Value>=world.webbed_players.iter().map(|(id,web)| {
                (id.clone(),json!({"severity":web.severity,"strength":web.strength,"cocoonHealth":web.cocoon_health,"hitPulse":web.hit_pulse}))
            }).collect();
            assert_current_enemy_json(
                &json!(webbed),
                &expected["webbedPlayers"],
                &format!("{label} webs"),
            );
            assert_current_enemy_json(
                &json!(world.spider_spit_ticks_remaining),
                &expected["spiderSpitTicksRemaining"],
                &label,
            );
            assert!(
                world.silk_fragments.is_empty()
                    && expected["silkFragments"].as_array().unwrap().is_empty()
            );
            let remains: Vec<_> = world
                .spider_remains
                .iter()
                .map(|remains| {
                    json!({"id":remains.id,"spawnTick":remains.spawn_tick,
                    "state":spider_cases::dead_spider_json(&remains.state)})
                })
                .collect();
            assert_current_enemy_json(
                &json!(remains),
                &expected["spiderRemains"],
                &format!("{label} Spider remains"),
            );
            let events: Vec<_> = world.events.iter().map(enemy_event_json).collect();
            assert_current_enemy_json(&json!(events), &expected["events"], &label);
            assert_current_enemy_json(
                &json!(world.player_damage.iter().map(player_damage_json).collect::<Vec<_>>()),
                &expected["damage"],
                &label,
            );
            let knockbacks: Vec<_> = world
                .player_knockbacks
                .iter()
                .map(|value| {
                    json!({"actorId":value.actor_id,"delta":point_json(value.delta),
                    "eventId":value.event_id,"playerId":value.player_id})
                })
                .collect();
            assert_current_enemy_json(&json!(knockbacks), &expected["knockbacks"], &label);
            let retained_knockbacks:Vec<_>=world.projectile_knockbacks.iter().map(|knockback| {
                json!({"actorId":knockback.actor_id,"delta":point_json(knockback.delta),"eventId":knockback.event_id,"playerId":knockback.player_id,
                    "lastStepTick":knockback.last_step_tick,"remainingTicks":knockback.remaining_ticks})
            }).collect();
            assert_current_enemy_json(
                &json!(retained_knockbacks),
                &expected["retainedKnockbacks"],
                &format!("{label} retained knockbacks"),
            );
            let positions: serde_json::Map<String, Value> = world
                .player_positions
                .iter()
                .map(|(id, position)| (id.clone(), point_json(*position)))
                .collect();
            assert_current_enemy_json(&json!(positions), &expected["positions"], &label);
            assert_eq!(
                world.death_effects.len(),
                expected["effects"].as_array().unwrap().len(),
                "{label}"
            );
            for (effect, expected) in
                world.death_effects.iter().zip(expected["effects"].as_array().unwrap())
            {
                assert_current_enemy_json(&death_effect_json(effect), expected, &label);
            }
            assert_current_enemy_json(&json!(world.rng_state), &expected["rng"], &label);
            assert_current_enemy_json(
                &json!(rng_record(&world.steering_rng)),
                &expected["steering"],
                &label,
            );
            assert_current_enemy_json(
                &json!(rng_record(&world.head_facing_rng)),
                &expected["headFacing"],
                &label,
            );
            assert_current_enemy_json(
                &json!(world.next_event_id),
                &expected["nextEventId"],
                &label,
            );
            assert_current_enemy_json(
                &json!(world.next_death_effect_id),
                &expected["nextDeathEffectId"],
                &label,
            );
            for (actual, key) in [
                (world.next_actor_id, "nextActorId"),
                (world.next_synthetic_spawn_intent_id, "nextSyntheticSpawnIntentId"),
                (world.next_native_cell_binding_order, "nextNativeCellBindingOrder"),
                (world.next_native_registration_order, "nextNativeRegistrationOrder"),
                (world.next_projectile_id, "nextProjectileId"),
                (world.next_mage_lightning_pulse_id, "nextMageLightningPulseId"),
                (world.next_projectile_effect_id, "nextProjectileEffectId"),
            ] {
                assert_current_enemy_json(&json!(actual), &expected[key], &label);
            }
            let projectiles: Vec<_> = world.projectiles.iter().map(enemy_projectile_json).collect();
            assert_current_enemy_json(
                &json!(projectiles),
                &expected["projectiles"],
                &format!("{label} projectiles"),
            );
            let projectile_effects: Vec<_> = world
                .projectile_effects
                .iter()
                .map(projectile_cases::projectile_effect_json)
                .collect();
            assert_current_enemy_json(
                &json!(projectile_effects),
                &expected["projectileEffects"],
                &format!("{label} projectile effects"),
            );
            let lightning: Vec<_> =
                world.mage_lightning_pulses.iter().map(mage_lightning_json).collect();
            assert_current_enemy_json(
                &json!(lightning),
                &expected["lightning"],
                &format!("{label} lightning"),
            );
            assert_current_enemy_json(
                &json!({"actor":order.next_actor_ordinal,"transient":order.next_transient_ordinal}),
                &expected["painterOrder"],
                &label,
            );
            samples.next();
        }
        if case["dampenAfterTrail"].as_bool() == Some(true) {
            assert!(
                dampened_after_trail,
                "the deferred registration case must reach a real competing primary-phase producer"
            );
        }
        assert!(samples.next().is_none(), "all {name} source snapshots consumed");
    };
    let mut failed_cases = Vec::new();
    for case in goldens["cases"].as_array().unwrap() {
        // Every case constructs fresh authority and RNG. Keep its exact first
        // assertion failure, then continue other independent source programs.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| replay_case(case)));
        if result.is_err() {
            failed_cases.push(case["name"].as_str().unwrap());
        }
    }
    assert!(failed_cases.is_empty(), "enemy source programs failed: {}", failed_cases.join(", "));
    presentations.assert_complete("ai");
}

#[test]
fn damage_absorption_armor_breaks_restraints_and_terminal_brains_match_the_website() {
    use solomon_sim::enemies::model::*;
    use solomon_sim::enemies::voices::BossStreamCue;
    use solomon_sim::enemies::world::{EnemySpawnContext, EnemyWorld};
    use solomon_sim::painter::WorldPainterOrder;
    let goldens: Value = serde_json::from_str(&fixture("enemy-damage-goldens.json")).unwrap();
    let players = [EnemyTarget {
        id: "hero".to_owned(),
        summoned: false,
        alive: true,
        collision_radius: 25.0,
        connected: true,
        eligible: true,
        heading_deg: None,
        position: Vec2::new(500.0, 500.0),
        velocity_per_tick: Vec2::ZERO,
    }];
    for case in goldens["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut world = EnemyWorld::new(case["seed"].as_str().unwrap(), 50);
        let mut order = WorldPainterOrder { next_actor_ordinal: 100, next_transient_ordinal: 70 };
        let intents: Vec<_> =
            case["intents"].as_array().unwrap().iter().map(spawn_intent).collect();
        world.spawn(
            &intents,
            EnemySpawnContext {
                players: &players,
                tick: 10,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: None,
            },
            &UnobstructedEnemyGeometry,
            Some(&mut order),
        );
        let setup = &case["setup"];
        let mut actor = world.actors.get_mut(0).unwrap();
        if let Some(shield) = setup["shield"].as_f64() {
            actor.shield_health = shield;
            actor.shield_maximum_health = shield;
        }
        if setup["pike"].as_bool() == Some(true) {
            let EnemyBrain::Skeleton(brain) = &mut actor.brain else { panic!("skeleton setup") };
            brain.phase = SkeletonPhase::Attack;
            brain.action_progress = 5.0;
            actor.body.body_pose = 2.0;
            brain.pike = Some(SkeletonPike {
                player_id: "hero".to_owned(),
                position: Vec2::new(180.0, 260.0),
                distance: 103.0,
            });
        }
        if setup["spiderCommitted"].as_bool() == Some(true) {
            let EnemyBrain::Spider(brain) = &mut actor.brain else { panic!("spider setup") };
            brain.action_state = 3;
            brain.strafe_ticks_remaining = 50;
        }
        if setup["lighting"].as_bool() == Some(true) {
            actor.lighting = EnemyLighting { charge: 0.7, glow: 0.25, provider_copies: 2 };
        }
        if setup["mageLightning"].as_bool() == Some(true) {
            let EnemyBrain::Mage(brain) = &mut actor.brain else { panic!("mage setup") };
            brain.phase = MagePhase::Cast;
            brain.lightning_target_player_id = Some("hero".to_owned());
            brain.lightning_ticks_remaining = 4;
        }
        if setup["webbed"].as_bool() == Some(true) {
            world.webbed_players.insert(
                "hero".to_owned(),
                WebbedState { severity: 3.0, strength: 10.0, cocoon_health: 10.0, hit_pulse: 0.0 },
            );
        }
        if setup["narration"].as_bool() == Some(true) {
            world.boss_narration.enqueue(&[BossStreamCue::FacultyJoinUs1], 10, false);
        }
        for (sample_index, sample) in case["samples"].as_array().unwrap().iter().enumerate() {
            let input = &sample["request"];
            let mut request = EnemyDamageRequest::new(
                input["actorId"].as_u64().unwrap() as usize,
                number(input, "amount"),
                input["tick"].as_u64().unwrap(),
                Some("hero".to_owned()),
            );
            request.hit_strength = input["hitStrength"].as_f64().unwrap_or(1.0);
            request.magic = input["magic"].as_bool().unwrap_or(false);
            request.has_magic_damage = input["hasMagicDamage"].as_bool().unwrap_or(false);
            request.ether_drain_capture = input["etherDrainCapture"].as_bool().unwrap_or(false);
            request.suppress_hurt_sound = input["suppressHurtSound"].as_bool().unwrap_or(false);
            world.events.clear();
            let result = world.damage(request, Some(&mut order));
            let context = format!("{name} hit {sample_index}");
            assert_json_equal(
                &json!({"accepted":result.accepted,"healthDamage":result.health_damage,"killed":result.killed}),
                &sample["result"],
                &context,
            );
            let expected_actors = sample["actors"].as_array().unwrap();
            assert_eq!(world.actors.len(), expected_actors.len(), "{context}");
            for (actual, expected) in world.actors.iter().zip(expected_actors) {
                let mut expected = expected.clone();
                expected["config"] = source_config_record(&expected["config"]);
                assert_json_equal(
                    &actor_json(&actual),
                    &expected,
                    &format!("{context} actor {}", actual.id),
                );
            }
            assert_json_equal(
                &json!(world.events.iter().map(enemy_event_json).collect::<Vec<_>>()),
                &sample["events"],
                &format!("{context} events"),
            );
            assert_eq!(
                world.death_effects.len(),
                sample["effects"].as_array().unwrap().len(),
                "{context} effect count"
            );
            for (actual, expected) in
                world.death_effects.iter().zip(sample["effects"].as_array().unwrap())
            {
                assert_death_effect_equal(
                    actual,
                    expected,
                    &format!("{context} effect {}", actual.id),
                );
            }
            let webbed: serde_json::Map<String,Value> = world.webbed_players.iter().map(|(id,state)| {
                (id.clone(),json!({"severity":state.severity,"strength":state.strength,"cocoonHealth":state.cocoon_health,"hitPulse":state.hit_pulse}))
            }).collect();
            assert_json_equal(&json!(webbed), &sample["webbedPlayers"], &context);
            let targets: Vec<_> = world.spell_targets().iter().map(primary_target_json).collect();
            assert_json_equal(&json!(targets), &sample["targets"], &format!("{context} targets"));
            assert_json_equal(
                &json!(world.rng_state),
                &sample["rng"],
                &format!("{context} actor RNG"),
            );
            assert_json_equal(
                &json!(rng_record(&world.steering_rng)),
                &sample["steering"],
                &format!("{context} steering RNG"),
            );
            assert_json_equal(&json!(world.next_death_epoch), &sample["nextDeathEpoch"], &context);
            assert_json_equal(
                &json!(world.next_death_effect_id),
                &sample["nextDeathEffectId"],
                &context,
            );
            assert_json_equal(&json!(world.next_event_id), &sample["nextEventId"], &context);
            assert_json_equal(&json!(world.featured_boss_id), &sample["featuredBossId"], &context);
            assert_json_equal(
                &boss_narration_json(&world.boss_narration),
                &sample["bossNarration"],
                &context,
            );
            let encounter = world.demon_skull_encounter;
            assert_json_equal(
                &json!({"deathStreamTicksRemaining":encounter.death_stream_ticks_remaining,
                "screamStreamTicksRemaining":encounter.scream_stream_ticks_remaining,"healthTriggersEnabled":encounter.health_triggers_enabled,
                "healthTriggersFiredMask":encounter.health_triggers_fired_mask,"pendingCapabilities":encounter.pending_capabilities}),
                &sample["demonSkullEncounter"],
                &context,
            );
            assert_json_equal(
                &json!({"actor":order.next_actor_ordinal,"transient":order.next_transient_ordinal}),
                &sample["painterOrder"],
                &context,
            );
        }
    }
}

#[test]
fn faculty_banter_rng_and_boss_stream_queue_timing_match_the_website() {
    use solomon_sim::NativeRng;
    use solomon_sim::enemies::voices::*;
    let goldens: VoiceGoldens =
        serde_json::from_str(&fixture("enemy-voices-goldens.json")).unwrap();
    for (index, case) in goldens.cases.iter().enumerate() {
        let mut rng = NativeRng::new(case.seed);
        let mut state = Some(FacultyVoiceController { line: case.line, ticks_remaining: 1 });
        for sample in &case.samples {
            let stepped = state.expect("a living voice controller before this source sample").step(
                &mut rng,
                &case.members,
                case.busy,
            );
            state = stepped.state;
            assert_eq!(
                stepped.cues.iter().map(|cue| cue.id()).collect::<Vec<_>>(),
                sample.cues,
                "voice case {index} cues"
            );
            assert_eq!(
                state.map(|state| (state.line, state.ticks_remaining)),
                sample.state.as_ref().map(|state| (state.line, state.ticks_remaining))
            );
        }
        assert_eq!(rng_record(&rng), case.rng, "voice case {index} RNG");
    }
    let mut narration = BossNarration::default();
    let mut samples = goldens.narration_samples.iter().peekable();
    for tick in 0..=1000 {
        if tick == 0 {
            narration.enqueue(
                &[BossStreamCue::FacultyJoinUs1, BossStreamCue::FacultyDeadIsBetter1Female],
                tick,
                true,
            );
        }
        narration.step(tick, tick < 5);
        if tick == 100 {
            narration.enqueue(&[BossStreamCue::FacultyJoinUs2Female], tick, false);
        }
        if tick == 500 {
            narration.cancel();
        }
        if tick == 600 {
            narration.enqueue(&[BossStreamCue::FacultyJoinUs2], tick, false);
        }
        if samples.peek().is_some_and(|sample| sample.tick == tick) {
            let expected = samples.next().unwrap();
            assert_json_equal(
                &boss_narration_json(&narration),
                &expected.state,
                &format!("boss narration tick {tick}"),
            );
        }
    }
    assert!(samples.next().is_none());
    let cues = [
        BossStreamCue::Trap,
        BossStreamCue::UnholyDie,
        BossStreamCue::UnholyScream,
        BossStreamCue::FacultyDie,
        BossStreamCue::FacultyNo,
        BossStreamCue::FacultyNoFemale,
        BossStreamCue::HeartBreak,
        BossStreamCue::FacultyJoinUs1,
        BossStreamCue::FacultyJoinUs1Female,
        BossStreamCue::FacultyJoinUs2,
        BossStreamCue::FacultyJoinUs2Female,
        BossStreamCue::FacultyDeadIsBetter1,
        BossStreamCue::FacultyDeadIsBetter1Female,
        BossStreamCue::FacultyDeadIsBetter2,
        BossStreamCue::FacultyDeadIsBetter2Female,
    ];
    for (cue, (id, ticks)) in cues.iter().zip(&goldens.durations) {
        assert_eq!(cue.id(), id);
        assert_eq!(cue.ticks(), *ticks);
    }
}
