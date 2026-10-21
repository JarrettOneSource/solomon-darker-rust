//! Native player contact feedback births, deadlines and exact RNG continuation.

use serde_json::{Value, json};
use solomon_sim::crow_blindness::*;
use solomon_sim::enemies::model::{EnemyDeathEffect, EnemyEvent};
use solomon_sim::enemies::world::EnemyWorld;
use solomon_sim::session::player_feedback::*;
use solomon_sim::{NativeRng, Vec2};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn point_snapshot(value: Vec2) -> Value {
    json!({ "x": value.x, "y": value.y })
}
fn rng_snapshot(value: &NativeRng) -> Value {
    json!({ "indexA": value.index_a, "indexB": value.index_b, "words": &value.words[..] })
}
fn read_rng(value: &Value) -> NativeRng {
    let mut rng = NativeRng::new(0);
    rng.index_a = value["indexA"].as_u64().unwrap() as usize;
    rng.index_b = value["indexB"].as_u64().unwrap() as usize;
    for (word, value) in rng.words.iter_mut().zip(value["words"].as_array().unwrap()) {
        *word = value.as_u64().unwrap() as u32;
    }
    rng
}
fn event_snapshot(value: &EnemyEvent) -> Value {
    json!({ "actorId": value.actor_id, "eventId": value.event_id,
        "gainScale": value.gain_scale, "pitch": value.pitch,
        "sound": value.sound.unwrap().id(), "sourcePosition": point_snapshot(value.source_position.unwrap()),
        "targetPlayerId": value.target_player_id, "tick": value.tick, "type": value.kind.id() })
}
fn effect_snapshot(value: &EnemyDeathEffect) -> Value {
    assert!(value.painter_sort_bias.is_none());
    assert!(value.painter_registration.is_none());
    assert!(value.scrap_oscillation.is_none());
    json!({ "ageTicks": value.age_ticks, "alpha": value.alpha, "alphaLossPerTick": value.alpha_loss_per_tick,
        "alphaMultiplier": value.alpha_multiplier, "angularVelocityDeg": value.angular_velocity_deg,
        "atlas": value.atlas.id(), "blendMode": value.blend_mode.id(),
        "bounceRetention": value.bounce_retention, "bounceVelocity": value.bounce_velocity,
        "entry": value.entry, "firstEntry": value.first_entry, "frameCount": value.frame_count,
        "framePhase": value.frame_phase, "frameTicks": value.frame_ticks,
        "frameVelocity": value.frame_velocity, "frameVelocityDamping": value.frame_velocity_damping,
        "height": value.height, "id": value.id, "kind": value.kind.id(),
        "lastStepTick": value.last_step_tick, "lifetimeTicks": value.lifetime_ticks,
        "opacityTimer": value.opacity_timer, "ownerActorId": value.owner_actor_id,
        "painterRegistration": Value::Null,
        "position": point_snapshot(value.position), "presentationOwner": value.presentation_owner.id(),
        "role": value.role, "rotationDeg": value.rotation_deg, "scale": value.scale,
        "scaleY": value.scale_y, "scaleMultiplier": value.scale_multiplier, "shadow": value.shadow,
        "spawnTick": value.spawn_tick, "tint": value.tint, "velocity": point_snapshot(value.velocity),
        "velocityDamping": value.velocity_damping, "verticalVelocity": value.vertical_velocity })
}
fn assert_snapshot(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(actual.as_f64(), expected.as_f64(), "{path}")
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_snapshot(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} field count");
            for (key, expected) in expected {
                assert_snapshot(&actual[key], expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn crow_blindness_status_bursts_and_ouch_deadlines_replay_the_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/player-feedback-goldens.json"))
            .unwrap();
    for (index, case) in fixture["blindness"].as_array().unwrap().iter().enumerate() {
        let mut rng = read_rng(&case["before"]);
        let duration = native_crow_blindness(
            case["currentHealth"].as_f64().unwrap(),
            case["remainingShield"].as_f64().unwrap(),
            case["chancePercent"].as_f64().unwrap(),
            &mut rng,
        );
        assert_snapshot(
            &json!({ "durationTicks": duration, "rng": rng_snapshot(&rng) }),
            &case["result"],
            &format!("blindness:{index}"),
        );
    }
    for case in fixture["opacity"].as_array().unwrap() {
        let result = native_crow_blindness_opacity(case["ticksRemaining"].as_f64().unwrap());
        assert_snapshot(
            &json!({ "cover": result.cover, "stamp": result.stamp }),
            &case["result"],
            "opacity",
        );
    }
    for (index, case) in fixture["bursts"].as_array().unwrap().iter().enumerate() {
        let mut world = EnemyWorld::new("feedback", 0);
        world.next_event_id = 83;
        world.next_death_effect_id = 71;
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let request = &case["request"];
        let event = emit_player_status_burst(
            &mut world,
            PlayerStatusBurstRequest {
                actor_id: request["actorId"].as_u64().unwrap() as usize,
                player_id: request["playerId"].as_str().unwrap(),
                position: point(&request["position"]),
                status: if request["status"] == "cold" {
                    PlayerStatusKind::Cold
                } else {
                    PlayerStatusKind::Poison
                },
                tick: request["tick"].as_u64().unwrap(),
            },
            &mut rng,
        );
        assert_snapshot(
            &json!({ "event": event_snapshot(&event), "rng": rng_snapshot(&rng),
            "store": { "deathEffects": world.death_effects.iter().map(effect_snapshot).collect::<Vec<_>>(),
            "nextDeathEffectId": world.next_death_effect_id, "nextEventId": world.next_event_id } }),
            &case["result"],
            &format!("burst:{index}"),
        );
        assert!(world.events.is_empty(), "status outbox belongs to the player contact phase");
    }
    for (index, case) in fixture["ouches"].as_array().unwrap().iter().enumerate() {
        let mut world = EnemyWorld::new("feedback", 0);
        world.next_event_id = 83;
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let request = &case["request"];
        let result = emit_boneyard_player_damage_sound(
            &mut world,
            PlayerDamageSoundRequest {
                actor_id: request["actorId"].as_u64().unwrap() as usize,
                player_id: request["playerId"].as_str().unwrap(),
                position: point(&request["position"]),
                current_health: request["currentHealth"].as_f64().unwrap(),
                tick: request["tick"].as_u64().unwrap(),
            },
            &mut rng,
        );
        assert_snapshot(
            &json!({ "deadlineTick": result.deadline_tick, "event": event_snapshot(&result.event),
            "rng": rng_snapshot(&rng), "store": { "nextEventId": world.next_event_id } }),
            &case["result"],
            &format!("ouch:{index}"),
        );
        let readiness = [-1, 0, 1].map(|delta| {
            native_wizard_ouch_cooldown_ready(
                result.deadline_tick.saturating_add_signed(delta),
                result.deadline_tick,
            )
        });
        assert_eq!(json!(readiness), case["readiness"], "ouch:{index} readiness");
        assert!(world.events.is_empty());
    }
}
