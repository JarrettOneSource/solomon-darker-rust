//! Replays the retail-recorded goldens in `tests/fixtures/native/`: language-neutral JSON
//! captured from the instrumented retail executable, each file carrying its native contract
//! (addresses, state layout, capture lane) in its own header.
//!
//! Set `SOLOMON_FIXTURE_DIR` to point at a different fixture directory.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use solomon_sim::constants::PLAYER_MOVEMENT;
use solomon_sim::float::float32_bits;
use solomon_sim::rng::{STOCK_DIVISOR, WORD_COUNT};
use solomon_sim::{
    CollisionRectangle, KnockbackState, NativeRng, PlayerActor, PlayerMovementState, Vec2,
};

fn fixture_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SOLOMON_FIXTURE_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/native")
}

fn read_fixture<T: DeserializeOwned>(name: &str) -> T {
    let path = fixture_dir().join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("native golden fixture {} is required: {error}", path.display())
    });
    serde_json::from_str(&text).unwrap_or_else(|error| {
        panic!("native golden fixture {} is malformed: {error}", path.display())
    })
}

#[derive(Deserialize)]
struct RngSnapshot {
    index_a: usize,
    index_b: usize,
    state_words: Vec<u32>,
    divisor: u32,
}

fn assert_rng_state(rng: &NativeRng, expected: &RngSnapshot, label: &str) {
    assert_eq!(rng.index_a, expected.index_a, "{label} index_a");
    assert_eq!(rng.index_b, expected.index_b, "{label} index_b");
    assert_eq!(rng.divisor, expected.divisor, "{label} divisor");
    assert_eq!(expected.state_words.len(), WORD_COUNT, "{label} fixture word count");
    assert_eq!(&rng.words[..], &expected.state_words[..], "{label} state words");
}

#[derive(Deserialize)]
struct RngGoldens {
    sequences: Vec<RngSequence>,
}

#[derive(Deserialize)]
struct RngSequence {
    seed: i64,
    range: i32,
    count: usize,
    outputs: Vec<i32>,
    final_index_a: usize,
    final_index_b: usize,
    final_state_words: Vec<u32>,
    stream: String,
}

#[test]
fn integer_rng_sequences_match_the_retail_captures() {
    let goldens: RngGoldens = read_fixture("rng-goldens.json");
    assert!(!goldens.sequences.is_empty(), "the RNG fixture has no sequences");
    for sequence in &goldens.sequences {
        let label =
            format!("seed {} range {} ({})", sequence.seed, sequence.range, sequence.stream);
        assert_eq!(sequence.outputs.len(), sequence.count, "{label} count");
        let mut rng = NativeRng::new(sequence.seed);
        for (index, expected) in sequence.outputs.iter().enumerate() {
            assert_eq!(rng.draw_integer(sequence.range), *expected, "{label} draw {index}");
        }
        assert_eq!(rng.index_a, sequence.final_index_a, "{label} final_index_a");
        assert_eq!(rng.index_b, sequence.final_index_b, "{label} final_index_b");
        assert_eq!(&rng.words[..], &sequence.final_state_words[..], "{label} final state words");
    }
}

#[derive(Deserialize)]
struct FloatGoldens {
    native_contract: FloatContract,
    captures: Vec<FloatCapture>,
}

#[derive(Deserialize)]
struct FloatContract {
    scaled_float32_rounding_points: u32,
    unit_float32_rounding_points: u32,
    stock_divisor: u32,
    state_word_count: usize,
}

#[derive(Deserialize)]
struct FloatCapture {
    request: FloatRequest,
    draws: Vec<FloatDraw>,
}

#[derive(Deserialize)]
struct FloatRequest {
    label: String,
    seed: i64,
    primitive: String,
    magnitude_float32_bits: String,
    signed: bool,
    count: usize,
}

#[derive(Deserialize)]
struct FloatDraw {
    draw_index: usize,
    pre_call: RngSnapshot,
    returned_float32_bits: String,
    post_call: RngSnapshot,
}

fn float_from_bits_text(text: &str) -> f64 {
    let hex =
        text.strip_prefix("0x").unwrap_or_else(|| panic!("float32 bits {text} lack the 0x prefix"));
    let bits =
        u32::from_str_radix(hex, 16).unwrap_or_else(|error| panic!("float32 bits {text}: {error}"));
    f64::from(f32::from_bits(bits))
}

#[test]
fn float_rng_corpus_matches_bit_for_bit_including_sign_draws() {
    let goldens: FloatGoldens = read_fixture("float-rng-goldens.json");
    assert_eq!(goldens.native_contract.scaled_float32_rounding_points, 3);
    assert_eq!(goldens.native_contract.unit_float32_rounding_points, 2);
    assert_eq!(goldens.native_contract.stock_divisor, STOCK_DIVISOR);
    assert_eq!(goldens.native_contract.state_word_count, WORD_COUNT);

    let labels: BTreeSet<&str> =
        goldens.captures.iter().map(|capture| capture.request.label.as_str()).collect();
    let expected_labels: BTreeSet<&str> = [
        "scaled-magnitude-1-unsigned",
        "scaled-magnitude-3-unsigned",
        "scaled-magnitude-4_5-signed",
        "scaled-endpoint-zero",
        "scaled-endpoint-positive",
        "unit-unsigned",
        "unit-signed",
        "unit-endpoint-zero",
        "unit-endpoint-positive",
    ]
    .into_iter()
    .collect();
    assert_eq!(labels, expected_labels);

    for capture in &goldens.captures {
        let request = &capture.request;
        let label = request.label.as_str();
        let magnitude = float_from_bits_text(&request.magnitude_float32_bits);
        assert_eq!(capture.draws.len(), request.count, "{label} draw count");
        let mut rng = NativeRng::new(request.seed);
        for draw in &capture.draws {
            let index = draw.draw_index;
            assert_rng_state(&rng, &draw.pre_call, &format!("{label} pre {index}"));
            let value = match request.primitive.as_str() {
                "scaled" => rng.draw_scaled_float(magnitude, request.signed),
                "unit" => rng.draw_unit_float(request.signed),
                other => panic!("{label} uses unknown primitive {other}"),
            };
            assert_eq!(
                float32_bits(value),
                draw.returned_float32_bits,
                "{label} draw {index} returned bits"
            );
            assert_rng_state(&rng, &draw.post_call, &format!("{label} post {index}"));
        }
    }
}

#[derive(Deserialize)]
struct MovementGoldens {
    header: MovementHeader,
    scenarios: Vec<MovementScenario>,
}

#[derive(Deserialize)]
struct MovementHeader {
    epsilon: MovementEpsilon,
}

#[derive(Deserialize)]
struct MovementEpsilon {
    position_absolute: f64,
    scalar_absolute: f64,
}

#[derive(Deserialize)]
struct MovementScenario {
    id: String,
    samples: Vec<MovementSample>,
}

#[derive(Deserialize)]
struct MovementSample {
    applied_input_x: f64,
    applied_input_y: f64,
    index: usize,
    knockback_present: bool,
    radius: f64,
    speed_scalar: f64,
    vx: f64,
    vy: f64,
    x: f64,
    y: f64,
}

fn player_from_sample(sample: &MovementSample) -> PlayerActor {
    PlayerActor {
        position: Vec2::new(sample.x, sample.y),
        radius: sample.radius,
        movement: PlayerMovementState {
            intent: Vec2::ZERO,
            velocity: Vec2::new(sample.vx, sample.vy),
            transient_multiplier: PLAYER_MOVEMENT.transient_multiplier,
            move_speed_scale: PLAYER_MOVEMENT.move_speed_scale,
            progression_multiplier: PLAYER_MOVEMENT.progression_multiplier,
            move_step_scale: sample.speed_scalar,
            controlled_damping: false,
        },
        knockback: None,
    }
}

#[test]
fn movement_wall_and_knockback_replays_land_every_retail_sample() {
    let goldens: MovementGoldens = read_fixture("movement-goldens.json");
    let position_epsilon = goldens.header.epsilon.position_absolute;
    let scalar_epsilon = goldens.header.epsilon.scalar_absolute;
    assert_eq!(goldens.scenarios.len(), 9);
    let north_wall = vec![CollisionRectangle::new(
        "movement-fixture-north-wall",
        -10_000.0,
        -10_000.0,
        10_000.0,
        -15.0,
    )];
    let mut worst_position_error = 0.0f64;
    let mut worst_velocity_error = 0.0f64;

    for scenario in &goldens.scenarios {
        let id = scenario.id.as_str();
        assert!(scenario.samples.len() > 100, "{id} must contain real recorded ticks");
        let mut actor = player_from_sample(&scenario.samples[0]);
        let rectangles: &[CollisionRectangle] =
            if id.starts_with("wall_") { &north_wall } else { &[] };

        for pair in scenario.samples.windows(2) {
            let (current, expected) = (&pair[0], &pair[1]);
            actor.movement.intent = Vec2::new(current.applied_input_x, current.applied_input_y);
            actor.tick_movement(rectangles);
            if current.knockback_present {
                actor.tick_knockback(rectangles);
            }
            if id == "knockback_contact" && expected.index == 2 {
                actor.knockback = Some(KnockbackState {
                    origin: Vec2::new(950.0, 1050.0),
                    remaining_distance: 20.0,
                });
            }

            let tick = expected.index;
            let dx = (actor.position.x - expected.x).abs();
            let dy = (actor.position.y - expected.y).abs();
            let dvx = (actor.movement.velocity.x - expected.vx).abs();
            let dvy = (actor.movement.velocity.y - expected.vy).abs();
            worst_position_error = worst_position_error.max(dx).max(dy);
            worst_velocity_error = worst_velocity_error.max(dvx).max(dvy);
            assert!(
                dx <= position_epsilon,
                "{id} tick {tick} x: {} vs {}",
                actor.position.x,
                expected.x
            );
            assert!(
                dy <= position_epsilon,
                "{id} tick {tick} y: {} vs {}",
                actor.position.y,
                expected.y
            );
            assert!(
                dvx <= scalar_epsilon,
                "{id} tick {tick} vx: {} vs {}",
                actor.movement.velocity.x,
                expected.vx
            );
            assert!(
                dvy <= scalar_epsilon,
                "{id} tick {tick} vy: {} vs {}",
                actor.movement.velocity.y,
                expected.vy
            );
            assert_eq!(actor.radius, expected.radius, "{id} tick {tick} radius");
            assert_eq!(
                actor.knockback.is_some(),
                expected.knockback_present,
                "{id} tick {tick} knockback presence"
            );
        }
    }
    eprintln!(
        "movement replay worst errors: position {worst_position_error:e} (epsilon {position_epsilon:e}), velocity {worst_velocity_error:e} (epsilon {scalar_epsilon:e})"
    );
}
