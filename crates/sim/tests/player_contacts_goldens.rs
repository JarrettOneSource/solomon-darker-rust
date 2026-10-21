//! Enemy contact host orchestration, delayed outboxes and random continuation.
// These shared source projections are used in different subsets by each host fixture.
#[allow(dead_code)]
#[path = "support/player_combat.rs"]
mod player;
// These source projections serve different subsets of the ownership and host fixtures.
#[allow(dead_code)]
#[path = "support/secondary_state.rs"]
mod secondary;

use serde_json::{Value, json};
use solomon_sim::boneyard::tutorial::NativeTutorialState;
use solomon_sim::boneyard::{
    BoneyardBounds, BoneyardScene, BoneyardSpawn, BoneyardWorld, LoadedBoneyard,
};
use solomon_sim::enemies::model::{EnemyEvent, EnemyPlayerDamage, EnemyPlayerDamageSource};
use solomon_sim::enemies::path::EnemyNavigation;
use solomon_sim::enemies::voices::BossStreamCue;
use solomon_sim::enemies::world::{
    EnemyGeometry, EnemyMovementRequest, EnemyProjectileWorldQuery, EnemySpawnContext,
};
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::harden_effects::NativeHardenShardBody;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use solomon_sim::session::PlayerLightingState;
use solomon_sim::session::player_contacts::*;
use solomon_sim::{HubWizard, Vec2};

fn read_damage(value: &Value) -> EnemyPlayerDamage {
    let mut damage = EnemyPlayerDamage::new(
        value["actorId"].as_u64().unwrap() as usize,
        value["eventId"].as_u64().unwrap() as usize,
        value["playerId"].as_str().unwrap().into(),
    );
    damage.physical_damage = value["physicalDamage"].as_f64().unwrap();
    damage.magic_damage = value["magicDamage"].as_f64().unwrap();
    damage.cold_slow_ticks = value["coldSlowTicks"].as_u64().unwrap();
    damage.dazzle_ticks = value["dazzleTicks"].as_u64().unwrap();
    damage.poison_damage = value["poisonDamage"].as_f64().unwrap();
    damage.poison_duration = value["poisonDuration"].as_f64().unwrap();
    damage.poison_contact_damage = value["poisonContactDamage"].as_f64();
    damage.hit_strength = value["hitStrength"].as_f64();
    damage.webbed_strength = value["webbedStrength"].as_f64();
    damage.crow_blind_chance_percent = value["crowBlindChancePercent"].as_f64();
    damage.mana_damage_maximum_fraction = value["manaDamageMaximumFraction"].as_f64();
    damage.suppress_hit_response = value["suppressHitResponse"].as_bool().unwrap_or(false);
    damage.suppress_flash = value["suppressFlash"].as_bool().unwrap_or(false);
    damage.tragic_circle = value["tragicCircle"].as_bool().unwrap_or(false);
    if let Some(source) = value.get("source") {
        damage.source = Some(if source.is_null() {
            None
        } else {
            Some(EnemyPlayerDamageSource {
                position: secondary::point(&source["position"]),
                collision_radius: source["collisionRadius"].as_f64().unwrap(),
                reflectable_actor_id: source["reflectableActorId"].as_u64().map(|id| id as usize),
            })
        });
    }
    damage
}
fn damage_snapshot(damage: &EnemyPlayerDamage, original: &Value) -> Value {
    let mut value = original.clone();
    value.as_object_mut().unwrap().remove("source");
    if let Some(source) = damage.source {
        value["source"] = source.map_or(Value::Null, |source| {
            json!({ "position": secondary::point_snapshot(source.position),
                "collisionRadius": source.collision_radius,
                "reflectableActorId": source.reflectable_actor_id })
        });
    }
    value["physicalDamage"] = json!(damage.physical_damage);
    value["poisonDamage"] = json!(damage.poison_damage);
    value["poisonDuration"] = json!(damage.poison_duration);
    value
}

#[cfg(feature = "persistence")]
#[test]
fn contact_sources_retain_omitted_null_and_explicit_values_across_snapshots() {
    let mut damage = EnemyPlayerDamage::new(7, 19, "local".into());
    for source in [
        None,
        Some(None),
        Some(Some(EnemyPlayerDamageSource {
            position: Vec2::new(361.25, 245.5),
            collision_radius: 15.0,
            reflectable_actor_id: Some(7),
        })),
    ] {
        damage.source = source;
        let value = serde_json::to_value(&damage).unwrap();
        assert_eq!(value.get("source").is_some(), source.is_some());
        let restored: EnemyPlayerDamage = serde_json::from_value(value).unwrap();
        assert_eq!(restored, damage);
    }
}
fn shard_snapshot(shard: NativeHardenShardBody) -> Value {
    json!({ "bounceVelocity": shard.bounce_velocity, "enhancedShadow": shard.enhanced_shadow, "height": shard.height, "life": shard.life,
        "position": secondary::point_snapshot(shard.position), "record": shard.record,
        "rotationDegrees": shard.rotation_degrees, "rotationStepDegrees": shard.rotation_step_degrees,
        "velocity": secondary::point_snapshot(shard.velocity), "verticalVelocity": shard.vertical_velocity })
}
fn sound_snapshot(event: &EnemyEvent) -> Value {
    json!({ "actorId": event.actor_id, "eventId": event.event_id, "gainScale": event.gain_scale,
        "pitch": event.pitch, "sound": event.sound.unwrap().id(),
        "sourcePosition": event.source_position.map(secondary::point_snapshot),
        "targetPlayerId": event.target_player_id, "tick": event.tick, "type": event.kind.id() })
}
fn world() -> BoneyardWorld {
    BoneyardWorld::new(LoadedBoneyard {
        choice_id: "contact-oracle".into(),
        choice_source: solomon_sim::boneyard::BoneyardSource::Mod,
        mod_id: None,
        mod_name: None,
        name: "Contact Oracle".into(),
        run_id: "oracle".into(),
        seed: "00112233445566778899aabbccddeeff".into(),
        source_sha256: String::new(),
        geometry_sha256: String::new(),
        scene: BoneyardScene {
            name: "Contact Oracle".into(),
            environment_mode: 0,
            bounds: BoneyardBounds::new(0.0, 0.0, 1200.0, 1200.0),
            spawn: BoneyardSpawn { position: Vec2::new(321.25, 245.5), facing_degrees: 45.0 },
            objects: Vec::new(),
            sprites: Vec::new(),
            roads: Vec::new(),
            fences: Vec::new(),
            terrain: Vec::new(),
            solomon_dig: None,
        },
    })
}

fn world_snapshot(world: &BoneyardWorld) -> Value {
    let mut webbed = json!({});
    for (id, web) in &world.enemies.webbed_players {
        webbed[id] = json!({ "severity": web.severity, "strength": web.strength, "cocoonHealth": web.cocoon_health, "hitPulse": web.hit_pulse });
    }
    let enemies = world.enemies.actors.iter().map(|actor| json!({ "id": actor.id,
            "position": secondary::point_snapshot(actor.position), "registration": { "managerLane": "actor", "registrationOrdinal": actor.registration_ordinal } })).collect::<Vec<_>>();
    let particles = world.enemies.death_effects.iter().map(|effect| json!({ "id": effect.id,
            "entry": effect.entry, "position": secondary::point_snapshot(effect.position), "velocity": secondary::point_snapshot(effect.velocity),
            "height": effect.height, "verticalVelocity": effect.vertical_velocity, "tint": effect.tint })).collect::<Vec<_>>();
    json!({ "ouchDeadline": world.player_ouch_deadline_tick,
        "counters": [world.enemies.next_actor_id, world.enemies.next_event_id, world.enemies.next_death_effect_id],
        "enemyRng": world.enemies.rng_state, "steeringRng": player::rng_snapshot(&world.enemies.steering_rng),
        "webbed": webbed, "enemies": enemies, "particles": particles,
    })
}
fn outbox_snapshot(result: &PlayerContactResult, damage: &Value) -> Value {
    let chips = result.harden_chips.iter().map(|pending| json!({
            "chip": { "pitch": pending.chip.pitch, "shard": shard_snapshot(pending.chip.shard) },
            "ownerId": pending.owner_id, "position": secondary::point_snapshot(pending.position), "worldKey": pending.world_key,
        })).collect::<Vec<_>>();
    let adjusted = result
        .applied_player_damage
        .iter()
        .map(|row| {
            let original = damage
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry["eventId"].as_u64() == Some(row.event_id as u64))
                .unwrap();
            damage_snapshot(row, original)
        })
        .collect::<Vec<_>>();
    let reflections = result.reflected_enemy_damage.iter().map(|row| json!({ "actorId": row.actor_id, "amount": row.amount, "playerId": row.player_id })).collect::<Vec<_>>();
    let deflections = result
        .deflect_pitches_by_event_id
        .iter()
        .map(|(id, pitch)| json!([id, pitch]))
        .collect::<Vec<_>>();
    let sounds = result.player_damage_sound_events.iter().map(sound_snapshot).collect::<Vec<_>>();
    json!({ "chips": chips, "damage": adjusted, "reflections": reflections,
        "deflections": deflections, "sounds": sounds })
}
fn order_snapshot(order: &WorldPainterOrder) -> Value {
    json!({ "nextRegistrationOrdinal": {
        "actor": order.next_actor_ordinal, "transient": order.next_transient_ordinal,
    } })
}

fn current_secondary_snapshot(
    state: &solomon_sim::secondary::NativeSecondarySimulationState,
) -> Value {
    let mut snapshot = secondary::state_snapshot(state);
    snapshot["stoneskinWarp"] = json!(state.stoneskin_warp);
    snapshot["cameraDisplacements"] = json!(
        state
            .camera_displacements
            .iter()
            .map(|row| {
                json!({"displacement":secondary::point_snapshot(row.displacement),
            "tick":row.tick,"worldKey":row.world_key})
            })
            .collect::<Vec<_>>()
    );
    snapshot
}

struct UnobstructedContactGeometry;

impl EnemyNavigation for UnobstructedContactGeometry {
    fn direct_path_clear(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> bool {
        true
    }

    fn find_route(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> Option<Vec<Vec2>> {
        None
    }
}

impl EnemyGeometry for UnobstructedContactGeometry {
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
fn player_contacts_replay_delayed_statuses_reflections_web_births_and_feedback_exactly() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/player-contacts-goldens.json"))
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let label = case["name"].as_str().unwrap();
        let (mut player, _, _) = player::configured_player(&case["spec"]);
        let options = &case["options"];
        let mut world = world();
        if options["sourceActor"].as_bool().unwrap_or(false) {
            world.enemies.next_actor_id = 7;
            world.enemies.spawn(
                &[EnemySpawnIntent::new(EnemyToken::Spider, Vec2::new(361.25, 245.5), 7, 1, 31)],
                EnemySpawnContext {
                    players: &[],
                    tick: 1,
                    arena_scalars: Default::default(),
                    roll_loot_seed: None,
                },
                &UnobstructedContactGeometry,
                None,
            );
        }
        let mut wizard = HubWizard::at(Vec2::new(321.25, 245.5));
        wizard.set_facing(45.0);
        world.enemies.next_event_id = 83;
        world.enemies.next_death_effect_id = 71;
        world.player_ouch_deadline_tick = options["deadline"].as_u64().unwrap_or(0);
        if options["protected"].as_bool().unwrap_or(false) {
            let mut tutorial = NativeTutorialState::new(wizard.position, 0, &world.loaded.seed);
            tutorial.damage_protection = true;
            world.tutorial = Some(tutorial);
        }
        if options["dialogue"].as_bool().unwrap_or(false) {
            world.enemies.boss_narration.pending.push(BossStreamCue::Trap);
        }
        let mut secondary = secondary::read_state(&case["initialSecondary"]);
        let mut screen_flashes = solomon_sim::screen_flash::NativeScreenFlashState::default();
        let mut order = WorldPainterOrder { next_actor_ordinal: 300, next_transient_ordinal: 500 };
        let mut lighting = PlayerLightingState::new(PainterRegistration {
            manager_lane: WorldManagerLane::Actor,
            registration_ordinal: 1,
        });
        let damage = case["damage"].as_array().unwrap().iter().map(read_damage).collect::<Vec<_>>();
        let initial_enemy_events = world.enemies.events.clone();
        let mut participants = [PlayerContactPlayer {
            player_id: "local",
            player: &mut player,
            wizard: &mut wizard,
            lighting: &mut lighting,
        }];
        let result = apply_player_contacts(
            PlayerContactContext {
                enhanced_effects: true,
                players: &mut participants,
                world: &mut world,
                secondary: &mut secondary,
                tick: options["tick"].as_u64().unwrap(),
                painter_order: &mut order,
                screen_flashes: &mut screen_flashes,
            },
            &damage,
        );
        let mut actual = world_snapshot(&world);
        actual["player"] = player::player_snapshot(&player);
        actual["heading"] = json!(wizard.heading_index);
        actual["blindness"] = json!(lighting.blindness_ticks_remaining);
        actual["secondary"] = secondary::state_snapshot(&secondary);
        actual["painterOrder"] = order_snapshot(&order);
        actual["outbox"] = outbox_snapshot(&result, &case["damage"]);
        player::assert_snapshot(&actual, &case["expected"], label);
        assert_eq!(
            world.enemies.events, initial_enemy_events,
            "{label}: contact feedback preserves the incoming enemy event queue"
        );
    }
}

#[test]
fn interleaved_participant_and_golem_contacts_keep_one_global_source_epoch() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/player-contact-multiplayer-goldens.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let label = case["name"].as_str().unwrap();
        let mut owned = case["players"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, input)| {
                let (player, mut wizard, _) = player::configured_player(&input["spec"]);
                wizard.position = secondary::point(&input["position"]);
                wizard.set_facing(input["heading"].as_f64().unwrap() * 15.0);
                let lighting = PlayerLightingState::new(PainterRegistration {
                    manager_lane: WorldManagerLane::Actor,
                    registration_ordinal: index + 1,
                });
                (input["id"].as_str().unwrap(), player, wizard, lighting)
            })
            .collect::<Vec<_>>();
        let mut world = world();
        world.enemies.next_event_id = 83;
        world.enemies.next_death_effect_id = 71;
        let mut secondary = secondary::read_state(&case["initialSecondary"]);
        let mut screen_flashes = solomon_sim::screen_flash::NativeScreenFlashState::default();
        let mut order = WorldPainterOrder { next_actor_ordinal: 300, next_transient_ordinal: 500 };
        let damage = case["damage"].as_array().unwrap().iter().map(read_damage).collect::<Vec<_>>();
        let mut participants = owned
            .iter_mut()
            .map(|(id, player, wizard, lighting)| PlayerContactPlayer {
                player_id: id,
                player,
                wizard,
                lighting,
            })
            .collect::<Vec<_>>();
        let result = apply_player_contacts(
            PlayerContactContext {
                enhanced_effects: true,
                players: &mut participants,
                world: &mut world,
                secondary: &mut secondary,
                tick: 201,
                painter_order: &mut order,
                screen_flashes: &mut screen_flashes,
            },
            &damage,
        );
        let mut players = json!({});
        for (id, player, wizard, lighting) in &owned {
            players[id] = json!({ "player": player::player_snapshot(player), "heading": wizard.heading_index,
                "blindness": lighting.blindness_ticks_remaining });
        }
        let actual = json!({
            "players": players, "world": world_snapshot(&world),
            "secondary": current_secondary_snapshot(&secondary), "painterOrder": order_snapshot(&order),
            "outbox": outbox_snapshot(&result, &case["damage"]),
        });
        player::assert_snapshot(&actual, &case["expected"], label);
        assert!(world.enemies.events.is_empty(), "{label}: feedback stays in the contact outbox");
    }
}
