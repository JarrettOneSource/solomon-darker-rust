//! Real movement, enemy, loot, Tutorial and late-wave composition against the pinned host.

#[path = "../../sim/tests/support/mod.rs"]
mod support;
#[path = "../../sim/tests/support/tutorial_snapshot.rs"]
mod tutorial_snapshot;

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};
use solomon_assets::boneyard::BoneyardCatalog;
use solomon_assets::navigation::decode_navigation;
use solomon_sim::boneyard::loot::BoneyardLootDeathProgram;
use solomon_sim::boneyard::navigation::NavigationSet;
use solomon_sim::boneyard::tutorial::NativeTutorialStage;
use solomon_sim::boneyard::world::{
    BoneyardPlayerCombatStatus, BoneyardTutorialPlayer, BoneyardWorldParticipant,
    BoneyardWorldTickContext, BoneyardWorldTickResult,
};
use solomon_sim::boneyard::world_light::BoneyardLightEnvironment;
use solomon_sim::boneyard::{BoneyardPlayerStatus, BoneyardWorld, LoadedBoneyard};
use solomon_sim::combat::PlayerCombatState;
use solomon_sim::economy::HubEconomyState;
use solomon_sim::enemies::model::{EnemyDamageRequest, EnemyLifeState, EnemyReward};
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::loot::{NativeBonusKind, NativeLootDropKind, NativeLootModifiers, NativeOrbKind};
use solomon_sim::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use solomon_sim::profile::WizardElement;
use solomon_sim::secondary::{NativeSecondaryAbilityId, NativeSecondaryPlayerState};
use solomon_sim::spells::boneyard::boneyard_primary_scenery_targets;
use solomon_sim::{HubWizard, NativeRng, Vec2};

fn point(position: Vec2) -> Value {
    json!([position.x, position.y])
}

fn input_point(value: &Value) -> Vec2 {
    Vec2::new(value[0].as_f64().unwrap(), value[1].as_f64().unwrap())
}

fn solomon_snapshot(world: &BoneyardWorld) -> Value {
    json!(world.encounter.as_ref().map(|encounter| json!({
        "phase": encounter.phase.id(),
        "position": point(encounter.position),
        "headingDeg": encounter.heading_degrees,
        "escapeTarget": encounter.escape_target.map(point),
        "escapeCollisionSourceIds": encounter.escape_collision_source_ids,
    })))
}

fn rng(state: &NativeRng) -> Value {
    json!({ "indexA": state.index_a, "indexB": state.index_b, "words": state.words.as_slice() })
}

fn registration(value: PainterRegistration) -> Value {
    let lane = match value.manager_lane {
        WorldManagerLane::Actor => "actor",
        WorldManagerLane::Transient => "transient",
    };
    json!([lane, value.registration_ordinal])
}

fn loot_kind(kind: NativeLootDropKind) -> &'static str {
    match kind {
        NativeLootDropKind::Bonus => "bonus",
        NativeLootDropKind::Gold => "gold",
        NativeLootDropKind::Orb => "orb",
        NativeLootDropKind::Sack => "sack",
    }
}

fn orb_kind(kind: NativeOrbKind) -> &'static str {
    match kind {
        NativeOrbKind::Health => "health",
        NativeOrbKind::Mana => "mana",
    }
}

fn bonus_kind(kind: NativeBonusKind) -> u8 {
    match kind {
        NativeBonusKind::SkillChoice => 0,
        NativeBonusKind::SkillRank => 1,
        NativeBonusKind::DamageX4 => 2,
    }
}

fn reward_snapshot(reward: &EnemyReward) -> Value {
    let source = &reward.loot_source;
    let mut loot = json!({ "actorSeed": source.actor_seed, "enemyToken": source.enemy_token.id(),
        "onDeathProgram": source.on_death_program.map(|program| match program {
            BoneyardLootDeathProgram::MinibossDie => "miniboss-die",
        }), "participantSlot": source.participant_slot,
        "position": { "x": source.position.x, "y": source.position.y } });
    if let Some(policies) = source.policies {
        loot["policies"] = json!({ "gold": policies.gold, "item": policies.item,
            "orb": policies.orb, "potion": policies.potion, "powerup": policies.powerup,
            "specificItem": policies.specific_item });
    }
    if let Some(uid) = source.recipe_uid {
        loot["recipeUid"] = json!(uid);
    }
    json!({ "actorId": reward.actor_id, "eventId": reward.event_id,
        "experience": reward.experience, "lootSource": loot, "playerId": reward.player_id })
}

fn snapshot(
    world: &BoneyardWorld,
    wizard: &HubWizard,
    order: WorldPainterOrder,
    result: &BoneyardWorldTickResult,
) -> Value {
    let store = &world.enemies;
    let actors: Vec<_> = store
        .actors
        .iter()
        .map(|actor| {
            let life = match actor.life_state {
                EnemyLifeState::Alive => "alive",
                EnemyLifeState::Dying => "dying",
            };
            json!([
                actor.id,
                actor.config.enemy_token.id(),
                point(actor.position),
                actor.heading_deg,
                actor.current_health,
                life,
                actor.spawn_tick,
                actor.config.scale,
                actor.registration_ordinal
            ])
        })
        .collect();
    let projectiles: Vec<_> = store
        .projectiles
        .iter()
        .map(|projectile| {
            let state = projectile.state();
            json!([
                state.id,
                projectile.kind().id(),
                point(state.position),
                state.age_ticks,
                state.lifetime_ticks,
                registration(state.painter_registration),
                state.light_registration.map(registration)
            ])
        })
        .collect();
    let pickups: Vec<_> = result
        .loot
        .pickups
        .iter()
        .map(|pickup| {
            json!([
                pickup.actor_id,
                pickup.amount,
                pickup.bonus_kind.map(bonus_kind),
                pickup.item.as_ref().map(|item| item.native_type_id),
                pickup.item.as_ref().map(|item| &item.name),
                loot_kind(pickup.kind),
                pickup.orb_kind.map(orb_kind),
                pickup.orb_value,
                pickup.player_id,
                point(pickup.source_position),
                pickup.tick
            ])
        })
        .collect();
    let gates: Vec<_> = world
        .gate_leaves
        .iter()
        .map(|leaf| {
            json!([point(leaf.hinge), point(leaf.tip), point(leaf.velocity), leaf.damping,])
        })
        .collect();
    json!({
        "player": [point(wizard.position), point(wizard.velocity), wizard.heading_index,
            wizard.gait_degrees, wizard.walk_cycle_primary],
        "lantern": world.lantern_position.map(point),
        "feedback": { "accumulator": world.enemy_world_feedback.accumulator,
            "magnitude": world.enemy_world_feedback.magnitude },
        "encounter": world.encounter.as_ref().map(|encounter| json!([encounter.phase.id(),
            encounter.run_event_id, encounter.voice_ticks_remaining, rng(&encounter.rng)])),
        "solomon": solomon_snapshot(world),
        "gates": gates,
        "waves": world.waves.as_ref().map(|waves| { let state = &waves.state;
            json!([state.phase.id(), state.wave_ordinal, state.low_population_ticks,
                state.active_group_index, state.spawn_countdown, rng(&state.rng)]) }),
        "tutorial": world.tutorial.as_ref().map(tutorial_snapshot::state_row),
        "actors": actors, "projectiles": projectiles,
        "counters": [store.next_actor_id, store.next_event_id, store.next_projectile_id,
            store.next_death_effect_id, store.next_native_cell_binding_order, store.next_native_registration_order],
        "enemyRng": [json!(store.rng_state), rng(&store.steering_rng), rng(&store.locomotion_rng), rng(&store.head_facing_rng)],
        "loot": { "sharedRng": rng(&world.loot.shared_rng), "nextActorId": world.loot.next_actor_id,
            "nextItemId": world.loot.next_item_id, "lastSuccessfulItemLevel": world.loot.last_successful_item_level,
            "nextKeyDropLevel": world.loot.next_key_drop_level,
            "actors": world.loot.actors.iter().map(|actor| json!([actor.id, loot_kind(actor.kind),
                point(actor.position), actor.age_ticks, actor.amount, actor.orb_value,
                actor.item.as_ref().map(|item|item.native_type_id), actor.item.as_ref().map(|item|&item.name),
                actor.spawn_tick, registration(actor.painter_registration)])).collect::<Vec<_>>(),
            "effects": world.loot.effects.iter().map(|effect| json!([effect.id, effect.age_ticks,
                effect.entry, effect.alpha])).collect::<Vec<_>>() },
        "damage": result.player_damage.iter().map(|damage| json!([damage.actor_id, damage.event_id,
            damage.player_id, damage.physical_damage, damage.magic_damage, damage.poison_damage,
            damage.cold_slow_ticks])).collect::<Vec<_>>(),
        "rewards": result.rewards.iter().map(reward_snapshot).collect::<Vec<_>>(),
        "pickups": pickups,
        "contacts": result.movement["local"].contacts.iter().map(|contact| json!({ "bodyId": contact.body_id,
            "staffHostile": contact.staff_hostile })).collect::<Vec<_>>(),
        "retainedTargets": [boneyard_primary_scenery_targets(world).len(), world.loot.goodies.len()],
        "order": { "nextRegistrationOrdinal": {
            "actor": order.next_actor_ordinal, "transient": order.next_transient_ordinal } },
    })
}

fn navigation(loaded: &LoadedBoneyard) -> NavigationSet {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../web/assets/navigation")
        .join(format!("{}.json", loaded.source_sha256));
    decode_navigation(&std::fs::read_to_string(path).unwrap(), loaded).unwrap()
}

fn compare_source(actual: &Value, expected: &Value, label: &str) -> (f64, f64) {
    // Enemy headings/positions feed sin/cos/atan2. Keep the project's 1e-9 trig exception
    // local to those fields; every RNG word, phase, timer, ID and reward stays exact.
    let mut trig_paths = Vec::new();
    let mut maximum_position_error = 0.0_f64;
    let mut maximum_heading_error = 0.0_f64;
    for index in 0..expected["actors"].as_array().unwrap().len() {
        trig_paths.push(format!("actors[{index}][2][0]"));
        trig_paths.push(format!("actors[{index}][2][1]"));
        trig_paths.push(format!("actors[{index}][3]"));
        for axis in 0..2 {
            let error = (actual["actors"][index][2][axis].as_f64().unwrap()
                - expected["actors"][index][2][axis].as_f64().unwrap())
            .abs();
            maximum_position_error = maximum_position_error.max(error);
        }
        let error = (actual["actors"][index][3].as_f64().unwrap()
            - expected["actors"][index][3].as_f64().unwrap())
        .abs();
        maximum_heading_error = maximum_heading_error.max(error);
    }
    for field in ["position[0]", "position[1]", "headingDeg", "escapeTarget[0]", "escapeTarget[1]"]
    {
        trig_paths.push(format!("solomon.{field}"));
    }
    if expected["solomon"].is_object() {
        for axis in 0..2 {
            maximum_position_error = maximum_position_error.max(
                (actual["solomon"]["position"][axis].as_f64().unwrap()
                    - expected["solomon"]["position"][axis].as_f64().unwrap())
                .abs(),
            );
        }
        maximum_heading_error = maximum_heading_error.max(
            (actual["solomon"]["headingDeg"].as_f64().unwrap()
                - expected["solomon"]["headingDeg"].as_f64().unwrap())
            .abs(),
        );
    }
    let trig_paths: Vec<_> = trig_paths.iter().map(String::as_str).collect();
    support::assert_website_json(actual, expected, label, &trig_paths);
    (maximum_position_error, maximum_heading_error)
}

fn step(
    world: &mut BoneyardWorld,
    wizard: &mut HubWizard,
    navigation: &NavigationSet,
    order: &mut WorldPainterOrder,
    tick: u64,
    scenario: &str,
    gate_birth_position: Option<Vec2>,
) -> BoneyardWorldTickResult {
    let mut external = Vec::new();
    if world.tutorial.is_some() {
        let combat = PlayerCombatState::new();
        let economy = HubEconomyState::new(0, WizardElement::Ether);
        let secondary = NativeSecondaryPlayerState {
            cast_sequence: 1,
            last_skill_id: Some(NativeSecondaryAbilityId::AcidRain),
            ..NativeSecondaryPlayerState::default()
        };
        external.extend(
            world
                .step_tutorial(
                    wizard,
                    BoneyardTutorialPlayer {
                        combat: &combat,
                        economy: &economy,
                        level: 1,
                        level_up_pending: false,
                        secondary: Some(&secondary),
                    },
                    tick,
                )
                .unwrap()
                .spawn_intents,
        );
    }
    if scenario == "enemy-rewards" && tick == 1 {
        for (index, token) in [
            EnemyToken::Skeleton,
            EnemyToken::SkeletonArcher,
            EnemyToken::SkeletonMage,
            EnemyToken::Imp,
            EnemyToken::Zombie,
            EnemyToken::Spider,
        ]
        .into_iter()
        .enumerate()
        {
            external.push(EnemySpawnIntent::new(
                token,
                Vec2::new(1050.0 + index as f64 * 30.0, 1100.0),
                index + 1,
                tick,
                0,
            ));
        }
    }
    if let Some(position) = gate_birth_position.filter(|_| tick == 1 || tick == 120) {
        external.push(EnemySpawnIntent::new(
            EnemyToken::Zombie,
            position,
            if tick == 1 { 1 } else { 2 },
            tick,
            0,
        ));
    }
    let movement = if gate_birth_position.is_some() && tick > 10 && tick <= 110 {
        Vec2::new(0.0, 1.0)
    } else {
        Vec2::ZERO
    };
    let result = world.step(
        BoneyardWorldTickContext {
            enhanced_effects: true,
            tick,
            participants: &mut [BoneyardWorldParticipant {
                wizard,
                combat: BoneyardPlayerCombatStatus {
                    status: BoneyardPlayerStatus {
                        id: "local",
                        alive: true,
                        eligible: true,
                        collision_enabled: true,
                        movement_scale: 1.0,
                    },
                    advanced_unlocks: &[false; 8],
                    inventory_has_health_potion: false,
                    level: 1,
                    loot_modifiers: NativeLootModifiers::default(),
                    owned_recipe_indexes: &[],
                },
                movement,
                force_movement_facing: false,
                viewport: Vec2::new(1600.0, 900.0),
                light_registration: Some(PainterRegistration {
                    manager_lane: WorldManagerLane::Actor,
                    registration_ordinal: 0,
                }),
                overlay_effect_phase: 0.0,
            }],
            light_environment: BoneyardLightEnvironment::default(),
            external_spawn_intents: external,
            navigation,
            custom_loot: None,
        },
        order,
    );
    world.enemies.commit_deferred_projectile_painters(order);
    result
}

#[test]
fn real_world_ticks_preserve_wave_births_reward_loot_entropy_and_tutorial_progression() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/world-tick-goldens.json"
    ))
    .unwrap();
    let catalog = BoneyardCatalog::load_embedded().unwrap();
    // JS catalog materialization retains each template.scene object across runs.
    // Its WeakMap navigation identity therefore survives these scenario boundaries.
    let mut navigation_by_scene = BTreeMap::new();
    let mut maximum_position_error = 0.0_f64;
    let mut maximum_heading_error = 0.0_f64;
    let mut source_tick_count = 0;
    for case in fixture["cases"].as_array().unwrap() {
        let scenario = case["scenario"].as_str().unwrap();
        let mut seed = [0; 16];
        seed[..4].copy_from_slice(&(case["templateIndex"].as_u64().unwrap() as u32).to_be_bytes());
        let mut loaded = if scenario == "tutorial-item-wave" {
            catalog.materialize_tutorial(seed)
        } else {
            catalog.materialize(seed)
        };
        loaded.seed = case["seed"].as_str().unwrap().to_owned();
        loaded.run_id = "oracle".into();
        let navigation = navigation_by_scene
            .entry(loaded.source_sha256.clone())
            .or_insert_with(|| navigation(&loaded))
            .clone();
        let mut world = BoneyardWorld::new(loaded);
        let mut wizard = world.spawn_wizard();
        world.lantern_light_registration = Some(PainterRegistration {
            manager_lane: WorldManagerLane::Actor,
            registration_ordinal: 1,
        });
        world.solomon_painter_registration = Some(PainterRegistration {
            manager_lane: WorldManagerLane::Actor,
            registration_ordinal: 2,
        });
        wizard.position = input_point(&case["initialPlayerPosition"]);
        if !scenario.starts_with("encounter-") && scenario != "pre-encounter-gate-birth" {
            world.encounter = None;
            world.waves = None;
            world.arena_transition = None;
        }
        if let Some(tutorial) = &mut world.tutorial {
            tutorial.intro_active = false;
            tutorial.intro_blend = 1.0;
            tutorial.intro_delay_ticks_remaining = 0;
            tutorial.intro_fade = 0.0;
            tutorial.intro_movement_ticks_remaining = 0;
            tutorial.stage = NativeTutorialStage::CastAcidRain;
        }
        let mut order = WorldPainterOrder { next_actor_ordinal: 3, next_transient_ordinal: 0 };
        let frames = case["frames"].as_array().unwrap();
        let solomon_frames = case["solomonFrames"].as_array().unwrap();
        let cache_frames = case["fullBoundsPrepared"].as_array().unwrap();
        let gate_birth_position = (!case["gateBirthPosition"].is_null()).then(|| {
            Vec2::new(
                case["gateBirthPosition"]["x"].as_f64().unwrap(),
                case["gateBirthPosition"]["y"].as_f64().unwrap(),
            )
        });
        let mut frame_index = 0;
        let mut solomon_frame_index = 0;
        if let Some(initial) = cache_frames.first() {
            assert_eq!(initial, &json!([0, false]));
            assert!(navigation.prepared_for(world.loaded.scene.bounds, 25.0).is_none());
        }
        let mut restored: Option<BoneyardWorld> = None;
        let mut restored_wizard = wizard.clone();
        let mut restored_order = order;
        for tick in 1..=case["finalTick"].as_u64().unwrap() {
            for injection in case["injected"].as_array().unwrap() {
                if injection["tick"].as_u64().unwrap() != tick {
                    continue;
                }
                let row = &injection["request"];
                let request = EnemyDamageRequest {
                    enhanced_effects: None,
                    actor_id: row["actorId"].as_u64().unwrap() as usize,
                    amount: row["amount"].as_f64().unwrap(),
                    tick: tick - 1,
                    source_player_id: Some("local".into()),
                    hit_strength: 1.0,
                    ether_drain_capture: false,
                    magic: false,
                    has_magic_damage: false,
                    suppress_hit_reaction: false,
                    suppress_hurt_sound: false,
                };
                world.enemies.damage(request.clone(), Some(&mut order));
                if let Some(restored) = &mut restored {
                    restored.enemies.damage(request, Some(&mut restored_order));
                }
            }
            source_tick_count += 1;
            let result = step(
                &mut world,
                &mut wizard,
                &navigation,
                &mut order,
                tick,
                scenario,
                gate_birth_position,
            );
            if let Some(restored) = &mut restored {
                let restored_result = step(
                    restored,
                    &mut restored_wizard,
                    &navigation,
                    &mut restored_order,
                    tick,
                    scenario,
                    gate_birth_position,
                );
                support::assert_website_json(
                    &snapshot(restored, &restored_wizard, restored_order, &restored_result),
                    &snapshot(&world, &wizard, order, &result),
                    &format!("{scenario} restore tick {tick}"),
                    &[],
                );
            }
            if solomon_frames
                .get(solomon_frame_index)
                .is_some_and(|frame| frame["tick"].as_u64().unwrap() == tick)
            {
                let actual = solomon_snapshot(&world);
                let expected = &solomon_frames[solomon_frame_index]["state"];
                for axis in 0..2 {
                    maximum_position_error = maximum_position_error.max(
                        (actual["position"][axis].as_f64().unwrap()
                            - expected["position"][axis].as_f64().unwrap())
                        .abs(),
                    );
                }
                maximum_heading_error = maximum_heading_error.max(
                    (actual["headingDeg"].as_f64().unwrap()
                        - expected["headingDeg"].as_f64().unwrap())
                    .abs(),
                );
                support::assert_website_json(
                    &actual,
                    expected,
                    &format!("{scenario} independent Solomon tick {tick}"),
                    &[
                        "position[0]",
                        "position[1]",
                        "headingDeg",
                        "escapeTarget[0]",
                        "escapeTarget[1]",
                    ],
                );
                solomon_frame_index += 1;
            }
            for cache_frame in cache_frames {
                if cache_frame[0].as_u64().unwrap() == tick {
                    assert_eq!(
                        navigation.prepared_for(world.loaded.scene.bounds, 25.0).is_some(),
                        cache_frame[1].as_bool().unwrap(),
                        "{scenario} actual birth mesh at tick {tick}",
                    );
                }
            }
            if frames.get(frame_index).is_some_and(|frame| frame["tick"].as_u64().unwrap() == tick)
            {
                let (position_error, heading_error) = compare_source(
                    &snapshot(&world, &wizard, order, &result),
                    &frames[frame_index]["state"],
                    &format!("{scenario} tick {tick}"),
                );
                maximum_position_error = maximum_position_error.max(position_error);
                maximum_heading_error = maximum_heading_error.max(heading_error);
                frame_index += 1;
            }
            if tick == 300 {
                restored =
                    Some(BoneyardWorld::restore(world.loaded.clone(), world.checkpoint()).unwrap());
                restored_wizard = wizard.clone();
                restored_order = order;
            }
        }
        assert_eq!(frame_index, frames.len());
        assert_eq!(solomon_frame_index, solomon_frames.len());
    }
    eprintln!(
        "{source_tick_count} source world ticks: maximum trig position error {maximum_position_error:e}, heading error {maximum_heading_error:e}; all other values exact"
    );
}
