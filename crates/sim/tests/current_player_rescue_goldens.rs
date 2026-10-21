//! Real float-store protection expiry, moving birth roots and shared RNG/painter order.

use serde::Deserialize;
use solomon_sim::painter::{WorldManagerLane, WorldPainterOrder};
use solomon_sim::player_rescue::{
    NativePlayerRescueProtection, native_rescue_damage, native_sparkle_scale,
};
use solomon_sim::{NativeRng, Vec2};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Goldens {
    source_revision: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    seed: i64,
    fraction: f64,
    world_key: String,
    frames: Vec<Frame>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Frame {
    tick: u64,
    fraction: f64,
    next_particle_id: u64,
    next_ordinal: usize,
    rng: Vec<u32>,
    damage: [f64; 4],
    particles: Vec<Particle>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Particle {
    values: [f64; 9],
    lane: String,
    world_key: String,
}

#[test]
fn current_rescue_protection_preserves_all_float_stores_through_the_last_zero_alpha_birth() {
    let fixture = include_str!("fixtures/website/current-player-rescue-goldens.json");
    let goldens: Goldens = serde_json::from_str(fixture).unwrap();
    assert_eq!(goldens.source_revision, "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for case in goldens.cases {
        let mut protection = NativePlayerRescueProtection {
            fraction: case.fraction,
            ..NativePlayerRescueProtection::default()
        };
        let mut rng = NativeRng::new(case.seed);
        let mut order = WorldPainterOrder { next_actor_ordinal: 0, next_transient_ordinal: 17 };
        let mut tick = 0;
        for frame in case.frames {
            while tick < frame.tick {
                tick += 1;
                protection.step(
                    Vec2::new(tick as f64, 80.0 + tick as f64 * 0.125),
                    &case.world_key,
                    &mut rng,
                    &mut order,
                );
            }
            let context = format!("seed{} fraction{} tick{tick}", case.seed, case.fraction);
            assert_eq!(protection.fraction, frame.fraction, "{context} protection");
            assert_eq!(
                protection.next_particle_id, frame.next_particle_id,
                "{context} particle IDs"
            );
            assert_eq!(
                order.next_transient_ordinal, frame.next_ordinal,
                "{context} shared painter"
            );
            let mut random = vec![rng.index_a as u32, rng.index_b as u32];
            random.extend(rng.words);
            assert_eq!(random, frame.rng, "{context} full RNG");
            assert_eq!(
                [0.0, 0.125, 50.0, 100.0].map(|amount| native_rescue_damage(amount, &protection)),
                frame.damage,
                "{context} protected damage"
            );
            assert_eq!(protection.particles.len(), frame.particles.len(), "{context} particles");
            for (actual, expected) in protection.particles.iter().zip(&frame.particles) {
                let registration = actual.painter_registration.unwrap();
                assert_eq!(registration.manager_lane, WorldManagerLane::Transient);
                assert_eq!(expected.lane, "transient");
                assert_eq!(actual.world_key, expected.world_key, "{context} particle world");
                assert_eq!(
                    [
                        actual.alpha,
                        actual.decay,
                        actual.id as f64,
                        actual.position.x,
                        actual.position.y,
                        registration.registration_ordinal as f64,
                        actual.rotation_degrees,
                        actual.timer,
                        native_sparkle_scale(actual.timer),
                    ],
                    expected.values,
                    "{context} particle{}",
                    actual.id
                );
            }
        }
    }
}
