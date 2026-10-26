//! The world-facing source sound adapters and retained narration/ambient ownership.

use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::audio::combat::*;
use solomon_sim::audio::{AudioCue, AudioEvent, AudioOptions};
use solomon_sim::boneyard::loot::{BoneyardLootEvent, BoneyardLootEventType, BoneyardLootSound};
use solomon_sim::enemies::boss_spell::{BossSpellKind, BossSpellOwner, NativeBossSpell};
use solomon_sim::enemies::config::EnemyArenaScalars;
use solomon_sim::enemies::model::*;
use solomon_sim::enemies::path::EnemyNavigation;
use solomon_sim::enemies::world::*;
use solomon_sim::enemies::{EnemyFlag, EnemySpawnIntent, EnemyToken};
use solomon_sim::painter::{WorldManagerLane, WorldPainterOrder};
use solomon_sim::profile::WizardElement;
use solomon_sim::spells::casting::PlayerPrimaryCastState;
use solomon_sim::spells::fire::{CreateNativeFirePatchOptions, create_native_fire_patch};
use solomon_sim::spells::model::{FirePatchType, SpellState};

struct EmptyGeometry;
impl EnemyNavigation for EmptyGeometry {
    fn direct_path_clear(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> bool {
        true
    }
    fn find_route(&self, _: Vec2, end: Vec2, _: f64, _: f64) -> Option<Vec<Vec2>> {
        Some(vec![end])
    }
}
impl EnemyGeometry for EmptyGeometry {
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

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn sound_json(request: Option<NativeEnemyEventSoundRequest>) -> Value {
    request.map_or(Value::Null, |request| json!({ "cue": request.cue.as_str(), "playbackRate": request.playback_rate,
        "sourcePosition": request.source_position.map(|position| json!({ "x": position.x, "y": position.y })), "volume": request.volume }))
}

fn compare(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(actual.as_f64(), expected.as_f64())
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len());
            for (key, expected) in expected {
                compare(&actual[key], expected);
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len());
            for (actual, expected) in actual.iter().zip(expected) {
                compare(actual, expected);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}

fn ambient_world(value: &Value) -> EnemyWorld {
    let input = &value["world"]["enemies"][0];
    let token = EnemyToken::from_id(input["enemyToken"].as_str().unwrap()).unwrap();
    let mut intent = EnemySpawnIntent::new(token, point(&input["position"]), 1, 0, 0);
    if token == EnemyToken::Zombie
        && input["flags"].as_array().unwrap().iter().any(|flag| flag == "FLAG_ROTTEN")
    {
        intent.flags.push(EnemyFlag::Rotten);
    }
    let mut world = EnemyWorld::new("audio", 0);
    let mut order = WorldPainterOrder::default();
    world.spawn(
        &[intent],
        EnemySpawnContext {
            players: &[],
            tick: 0,
            arena_scalars: EnemyArenaScalars::default(),
            roll_loot_seed: None,
        },
        &EmptyGeometry,
        Some(&mut order),
    );
    assert_eq!(world.actors.len(), 1);
    let mut actor = world.actors.get_mut(0).unwrap();
    actor.id = input["id"].as_u64().unwrap() as usize;
    actor.life_state = if input["animation"]["state"] == "death" {
        EnemyLifeState::Dying
    } else {
        EnemyLifeState::Alive
    };
    if let EnemyBrain::Faculty(brain) = &mut actor.brain {
        brain.hand_mask = input["faculty"]["handMask"].as_u64().unwrap() as u8;
        brain.lightning_active = input["faculty"]["lightningActive"].as_bool().unwrap();
    }
    if token == EnemyToken::Coffin && actor.life_state == EnemyLifeState::Alive {
        let mut coffin = world.actors.remove(0);
        let EnemyBrain::Coffin(brain) = &mut coffin.brain else { unreachable!() };
        brain.phase = CoffinPhase::Open;
        brain.maggot_charge = 0.0;
        solomon_sim::enemies::coffin::step_coffin(
            &mut world,
            &mut coffin,
            &EnemyStepContext {
                enhanced_effects: true,
                tick: 1,
                paused: false,
                players: &[],
                geometry: &EmptyGeometry,
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
            &mut order,
        );
        let template = world.maggots.get(0).unwrap().snapshot();
        let count = value["world"]["maggots"].as_array().unwrap().len();
        while world.maggots.len() > count {
            world.maggots.remove(world.maggots.len() - 1);
        }
        while world.maggots.len() < count {
            world.maggots.push(template.clone());
        }
        world.actors.push(coffin);
    }
    world.boss_spells.clear();
    for input in value["world"]["bossSpells"].as_array().unwrap() {
        let kind = match input["kind"].as_str().unwrap() {
            "green-fire" => {
                let fire = create_native_fire_patch(
                    CreateNativeFirePatchOptions {
                        id: 1,
                        owner_id: "boss".to_owned(),
                        world_key: "boneyard:audio".to_owned(),
                        position: point(&input["position"]),
                        damage: 0.0,
                        burn_damage: 0.0,
                        native_type: FirePatchType::Fire,
                        atlas_phase_step: None,
                        life: None,
                        draw_alpha: None,
                        fade_alpha: None,
                        scale: None,
                        supplemental_contact: false,
                        velocity: None,
                        velocity_multiplier: None,
                        painter_registration: None,
                    },
                    0.0,
                    1.0,
                );
                BossSpellKind::GreenFire { fire: Box::new(fire), glow: true }
            }
            "mouth-beam-segment" => BossSpellKind::MouthBeamSegment {
                vertices: [Vec2::ZERO; 4],
                uv_offset: 0.0,
                start_alpha: 1.0,
                end_alpha: 0.0,
            },
            "ultra-banish" => BossSpellKind::UltraBanish {
                remaining_ticks: 1,
                alpha: input["alpha"].as_f64().unwrap(),
                flash_alpha: 1.0,
                light_radius: 100.0,
                mega_death: false,
            },
            "rain-of-bones" => BossSpellKind::RainOfBones {
                alpha: input["alpha"].as_f64().unwrap(),
                phase: 0.0,
                remaining_ticks: 1,
                rotation_sign: 1.0,
                scale: 1.0,
            },
            "tragic-circle" => BossSpellKind::TragicCircle { remaining_ticks: 1 },
            "skull-missile" => BossSpellKind::SkullMissile {
                heading_deg: 0.0,
                minimum_speed: 1.0,
                phase_deg: 0.0,
                remaining_ticks: 1.0,
                speed: 1.0,
                turn_rate: 1.0,
                visual_scale: 1.0,
                target_player_id: None,
            },
            "dark-fireball" => BossSpellKind::DarkFireball {
                arc_phase: 0.0,
                arc_phase_step: 0.0,
                heading_deg: 0.0,
                remaining_ticks: 1.0,
                velocity: Vec2::ZERO,
                ground_fire_damage: 0.0,
            },
            kind => panic!("ambient fixture spell {kind}"),
        };
        world.boss_spells.push(NativeBossSpell {
            owner: BossSpellOwner {
                age_ticks: 0,
                damage: 0.0,
                id: 1,
                owner_actor_id: input["ownerActorId"].as_u64().unwrap() as usize,
                position: point(&input["position"]),
                spawn_tick: 0,
                painter_registration: order.register(WorldManagerLane::Actor),
            },
            kind,
        });
    }
    world
}

#[test]
fn enemy_sound_routing_and_boss_ambient_mixes_match_the_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/combat-audio-goldens.json")).unwrap();
    for record in fixture["enemyEvents"].as_array().unwrap() {
        let input = &record["input"];
        let kind = match input["type"].as_str().unwrap() {
            "enemy-action-sound" => EnemyEventKind::EnemyActionSound,
            "enemy-damage-sound" => EnemyEventKind::EnemyDamageSound,
            "enemy-death-sound" => EnemyEventKind::EnemyDeathSound,
            "player-damage-sound" => EnemyEventKind::PlayerDamageSound,
            "player-status-sound" => EnemyEventKind::PlayerStatusSound,
            "attack-marker" => EnemyEventKind::AttackMarker,
            "player-deflected" => EnemyEventKind::PlayerDeflected,
            kind => panic!("fixture enemy event {kind}"),
        };
        let mut event = EnemyEvent::new(kind, 1, 0);
        event.sound = match input["sound"].as_str() {
            Some("fireball-hit") => Some(EnemySound::FireballHit),
            Some("poisoned") => Some(EnemySound::Poisoned),
            Some("magic-missile-hit") => Some(EnemySound::MagicMissileHit),
            None => None,
            sound => panic!("fixture sound {sound:?}"),
        };
        event.deflect_pitch = input["deflectPitch"].as_f64();
        event.pitch = input["pitch"].as_f64();
        event.gain_scale = input["gainScale"].as_f64();
        event.source_position = input.get("sourcePosition").map(point);
        compare(&sound_json(native_enemy_event_sound_request(&event)), &record["expected"]);
    }
    for record in fixture["ambient"].as_array().unwrap() {
        let world = ambient_world(&record["input"]);
        let requests = native_boneyard_enemy_ambient_requests(&world, 100, &|_| 0.8, &|_| 0.6);
        let actual: Vec<_> = requests
            .iter()
            .map(|request| json!({ "cue": request.cue.as_str(), "gain": request.gain }))
            .collect();
        compare(&json!(actual), &record["expected"]);
    }
    for record in fixture["lootEvents"].as_array().unwrap() {
        let input = &record["input"];
        let event = BoneyardLootEvent {
            actor_id: 1,
            event_id: 1,
            goodie_id: None,
            phase: None,
            playback_rate: input["playbackRate"].as_f64(),
            player_id: None,
            position: point(&input["position"]),
            sound: input["sound"].as_str().map(|_| BoneyardLootSound::PickupCoin),
            text: None,
            tick: 1,
            event_type: BoneyardLootEventType::LootPickup,
        };
        compare(&sound_json(native_loot_event_sound_request(&event)), &record["expected"]);
    }
    // Boss narration resumes from the authoritative remaining duration; the presentation
    // cursor does not own or advance the native voice clock.
    let mut world = EnemyWorld::new("audio-voice", 0);
    world.boss_narration.current = Some(solomon_sim::enemies::voices::BossNarrationLine {
        cue: solomon_sim::enemies::voices::BossStreamCue::FacultyJoinUs1,
        event_id: 3,
        started_tick: 100,
    });
    world.boss_narration.ticks_remaining = 262;
    world.boss_narration.mix = 0.5;
    let primary = SpellState::default();
    let secondary = solomon_sim::secondary::NativeSecondarySimulationState::new(1);
    let frame = CombatAudioFrame {
        tick: 200,
        run_id: Some("audio-voice"),
        listener_id: "local",
        players: &[],
        primary: &primary,
        secondary: &secondary,
        staff_contacts: &[],
        enemies: Some(&world),
        enemy_events: &[],
        loot_events: &[],
        encounter: None,
        point_gain: &|_| 1.0,
        hit_gain: &|_| 1.0,
    };
    let mut cursor = CombatAudioCursor::new(&frame);
    let output = cursor.update(&frame);
    assert_eq!(output.narration_mix, Some(0.5));
    assert_eq!(
        output.events.last(),
        Some(&AudioEvent::Play(
            AudioCue::FacultyJoinUs1,
            AudioOptions { offset_seconds: 1.0, ..AudioOptions::default() }
        ))
    );
    assert!(cursor.update(&frame).events.is_empty());
    let mut event = EnemyEvent::new(EnemyEventKind::EnemyDamageSound, 1, 200);
    event.event_id = 12;
    event.sound = Some(EnemySound::FireballHit);
    event.pitch = Some(0.875);
    event.gain_scale = Some(0.6);
    event.source_position = Some(Vec2::new(300.0, 200.0));
    let events = [event];
    let event_frame = CombatAudioFrame { enemy_events: &events, point_gain: &|_| 0.25, ..frame };
    assert_eq!(
        cursor.update(&event_frame).events,
        vec![AudioEvent::Play(
            AudioCue::FireballHit,
            AudioOptions { playback_rate: 0.875, volume: 0.15, offset_seconds: 0.0 }
        )]
    );
    assert!(cursor.update(&event_frame).events.is_empty());

    let cast = PlayerPrimaryCastState::default();
    for record in fixture["footsteps"].as_array().unwrap() {
        let id = record["playerId"].as_str().unwrap();
        let previous = CombatAudioPlayer {
            id,
            world_key: "boneyard:steps",
            position: Vec2::ZERO,
            element: WizardElement::Ether,
            selected_primary_skill_id: 8,
            weld_build_id: None,
            cast: &cast,
            footstep_tick: record["previous"].as_u64().unwrap_or(0),
        };
        let current =
            CombatAudioPlayer { footstep_tick: record["current"].as_u64().unwrap(), ..previous };
        let previous_players = [previous];
        let current_players = [current];
        let initial_frame = CombatAudioFrame {
            listener_id: id,
            run_id: Some("steps"),
            enemies: None,
            enemy_events: &[],
            players: if record["previous"].is_null() { &[] } else { &previous_players },
            tick: 0,
            primary: &primary,
            secondary: &secondary,
            staff_contacts: &[],
            loot_events: &[],
            encounter: None,
            point_gain: &|_| 1.0,
            hit_gain: &|_| 1.0,
        };
        let mut cursor = CombatAudioCursor::new(&initial_frame);
        let current_frame = CombatAudioFrame { players: &current_players, ..initial_frame };
        let events = cursor.update(&current_frame).events;
        if record["expected"].is_null() {
            assert!(events.is_empty());
        } else {
            assert_eq!(
                events,
                vec![AudioEvent::Play(
                    AudioCue::from_id(record["expected"]["cue"].as_str().unwrap()).unwrap(),
                    AudioOptions { volume: 0.5, ..AudioOptions::default() }
                )]
            );
        }
    }
}
