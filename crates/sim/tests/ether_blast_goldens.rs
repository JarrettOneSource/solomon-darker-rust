use serde::Deserialize;
use solomon_sim::spells::ether::*;
use solomon_sim::{NativeRng, Vec2};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChargeCase {
    start: f64,
    capacity: u32,
    full_mana: bool,
    plane: bool,
    charge: f64,
    crossed_integer: bool,
    released: u32,
}

#[derive(Deserialize)]
struct DamageCase {
    charges: u32,
    health: f64,
    damage: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RngState {
    index_a: usize,
    index_b: usize,
    words: Vec<u32>,
}

#[derive(Deserialize)]
struct Program {
    particles: Vec<[f64; 12]>,
    rng: RngState,
}

#[derive(Deserialize)]
struct Frames {
    age: f64,
    frames: Vec<[f64; 3]>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Goldens {
    charge_cases: Vec<ChargeCase>,
    damage_cases: Vec<DamageCase>,
    program: Program,
    frames: Vec<Frames>,
    origin: Point,
}

#[derive(Deserialize)]
struct Point {
    x: f64,
    y: f64,
}

#[test]
fn ether_blast_charge_health_pulse_and_particle_program_replay_the_website() {
    let fixture: Goldens =
        serde_json::from_str(include_str!("fixtures/website/ether-blast-goldens.json")).unwrap();
    for sample in fixture.charge_cases {
        let actual = advance_native_ether_blast_charge(
            sample.start,
            sample.capacity,
            sample.full_mana,
            sample.plane,
        );
        assert_eq!(actual.charge, sample.charge);
        assert_eq!(actual.crossed_integer, sample.crossed_integer);
        assert_eq!(native_ether_blast_release_charges(actual.charge), sample.released);
    }
    for sample in fixture.damage_cases {
        assert_eq!(native_ether_blast_damage(sample.charges, sample.health), sample.damage);
    }
    assert_eq!(
        native_ether_blast_pulse_origin(Vec2::new(100.1, -200.2), Vec2::new(0.6, 0.8)),
        Vec2::new(fixture.origin.x, fixture.origin.y)
    );
    let mut rng = NativeRng::new(7);
    let particles = create_native_ether_blast_particle_program(&mut rng);
    assert_eq!(rng.index_a, fixture.program.rng.index_a);
    assert_eq!(rng.index_b, fixture.program.rng.index_b);
    assert_eq!(rng.words.as_slice(), fixture.program.rng.words);
    assert_eq!(particles.len(), fixture.program.particles.len());
    for (particle, expected) in particles.iter().zip(fixture.program.particles) {
        assert_eq!(
            [
                particle.alpha_loss,
                particle.blue,
                particle.damping,
                particle.green,
                particle.offset.x,
                particle.offset.y,
                particle.red,
                particle.rotation_degrees,
                particle.scale,
                particle.sprite_record as f64,
                particle.velocity.x,
                particle.velocity.y,
            ],
            expected
        );
    }
    for sample in fixture.frames {
        for (particle, expected) in particles.iter().zip(sample.frames) {
            let actual = native_ether_blast_particle_frame(*particle, sample.age);
            assert_eq!([actual.alpha, actual.position.x, actual.position.y], expected);
        }
    }
}
