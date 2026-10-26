//! Current Coffin activation allocates twenty retained BoulderBits from native steering RNG.
#![cfg(feature = "persistence")]
#[path = "support/native_bits.rs"]
#[allow(dead_code)]
mod native_bits;
#[path = "support/secondary.rs"]
#[allow(dead_code)]
mod secondary;

use serde_json::{Value, json};
use solomon_sim::enemies::coffin_emergence::create_native_coffin_emergence;
use solomon_sim::enemies::config::EnemyArenaScalars;
use solomon_sim::enemies::model::{CoffinPhase, EnemyBrain, EnemyDeathEffect};
use solomon_sim::enemies::path::EnemyNavigation;
use solomon_sim::enemies::world::{
    EnemyGeometry, EnemyMovementRequest, EnemyProjectileWorldQuery, EnemySpawnContext,
    EnemyStepContext, EnemyWorld,
};
use solomon_sim::enemies::{EnemySpawnIntent, EnemyToken};
use solomon_sim::painter::WorldPainterOrder;
use solomon_sim::spells::earth_debris::step_native_boulder_debris_motion;
use solomon_sim::{NativeRng, Vec2};

#[test]
fn native_coffin_factories_and_all_retained_bounces_share_the_source_motion_program() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/current-coffin-emergence-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["kernels"].as_array().unwrap().len(), 18);
    for (case_index, case) in fixture["kernels"].as_array().unwrap().iter().enumerate() {
        let mut rng = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut particles = create_native_coffin_emergence(
            &mut rng,
            native_bits::input(&case["position"]),
            case["scale"].as_f64().unwrap(),
            case["radius"].as_f64().unwrap(),
            case["enhanced"].as_bool().unwrap(),
        );
        let frames = case["frames"].as_array().unwrap();
        let mut frame_index = 0;
        for tick in 0..=400 {
            if tick > 0 {
                let mut retained = Vec::new();
                for mut particle in particles {
                    if let Some(motion) =
                        step_native_boulder_debris_motion(particle.motion, tick, &mut rng)
                    {
                        particle.motion = motion;
                        retained.push(particle);
                    }
                }
                particles = retained;
            }
            if frames.get(frame_index).is_none_or(|frame| frame["tick"].as_u64() != Some(tick)) {
                continue;
            }
            let actual = native_bits::source_value(&json!({
                "particles": particles,
                "rng": secondary::rng_snapshot(&rng),
            }));
            native_bits::compare(
                &actual,
                &native_bits::source_value(&frames[frame_index]["expected"]),
                &format!("factory{case_index} tick{tick}"),
            );
            frame_index += 1;
        }
        assert_eq!(frame_index, frames.len());
    }
}

struct OpenWorld;
impl EnemyNavigation for OpenWorld {
    fn direct_path_clear(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> bool {
        true
    }
    fn find_route(&self, _: Vec2, _: Vec2, _: f64, _: f64) -> Option<Vec<Vec2>> {
        None
    }
}
impl EnemyGeometry for OpenWorld {
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
fn effect_snapshot(effect: &EnemyDeathEffect) -> Value {
    let mut value = serde_json::to_value(effect).unwrap();
    value["kind"] = json!(effect.kind.id());
    value["blend_mode"] = json!(effect.blend_mode.id());
    value["presentation_owner"] = json!(effect.presentation_owner.id());
    value["painter_registration"] =
        effect.painter_registration.map_or(Value::Null, |registration| {
            native_bits::source_value(&secondary::registration_snapshot(registration))
        });
    if effect.painter_sort_bias.is_none() {
        value.as_object_mut().unwrap().remove("painter_sort_bias");
    }
    if effect.scrap_oscillation.is_none() {
        value.as_object_mut().unwrap().remove("scrap_oscillation");
    }
    value
}
#[test]
fn actual_coffin_hidden_to_rising_birth_keeps_particle_rng_and_shared_registration_order() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/current-coffin-emergence-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in fixture["worlds"].as_array().unwrap() {
        let mut world = EnemyWorld::new(case["seed"].as_str().unwrap(), 50);
        let mut order = WorldPainterOrder::default();
        world.spawn(
            &[EnemySpawnIntent::new(EnemyToken::Coffin, Vec2::new(150.0, 250.0), 1, 0, 31)],
            EnemySpawnContext {
                players: &[],
                tick: 0,
                arena_scalars: EnemyArenaScalars::default(),
                roll_loot_seed: None,
            },
            &OpenWorld,
            Some(&mut order),
        );
        let frames = case["frames"].as_array().unwrap();
        let mut index = 0;
        for tick in 1..=case["birthTick"].as_u64().unwrap() + 5 {
            world.step(
                EnemyStepContext {
                    enhanced_effects: case["enhanced"].as_bool().unwrap(),
                    tick,
                    paused: false,
                    players: &[],
                    geometry: &OpenWorld,
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
            if frames.get(index).is_none_or(|frame| frame["tick"].as_u64() != Some(tick)) {
                continue;
            }
            let EnemyBrain::Coffin(brain) = world.actors.get(0).unwrap().brain else {
                panic!("actual Coffin constructor")
            };
            let phase = match brain.phase {
                CoffinPhase::Hidden => "hidden",
                CoffinPhase::Rising => "rising",
                CoffinPhase::Holding => "holding",
                CoffinPhase::Opening => "opening",
                CoffinPhase::Open => "open",
                CoffinPhase::Death => "death",
            };
            let actual = native_bits::source_value(&json!({
                "phase": phase, "phaseTick": brain.phase_tick,
                "phaseTicksRemaining": brain.phase_ticks_remaining,
                "effects": world.death_effects.iter().map(effect_snapshot).collect::<Vec<_>>(),
                "nextDeathEffectId": world.next_death_effect_id, "rng": world.rng_state,
                "steering": secondary::rng_snapshot(&world.steering_rng),
                "order": {"nextRegistrationOrdinal": {
                    "actor": order.next_actor_ordinal, "transient": order.next_transient_ordinal,
                }},
            }));
            native_bits::compare(
                &actual,
                &native_bits::source_value(&frames[index]["expected"]),
                &format!("quality{} tick{tick}", case["enhanced"]),
            );
            index += 1;
        }
        assert_eq!(index, frames.len());
    }
}
