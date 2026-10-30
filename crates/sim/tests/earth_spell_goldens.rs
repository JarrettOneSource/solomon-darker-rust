use serde::Deserialize;
use solomon_sim::Vec2;
use solomon_sim::spells::earth::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContactInput {
    release_base_damage: f64,
    release_charge: f64,
    remaining_pool: f64,
    target_health: f64,
    toughness: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Contact {
    input: ContactInput,
    charge: Option<f64>,
    continue_traversal: bool,
    damage: f64,
    depleted: bool,
    remaining_pool: f64,
}
#[derive(Deserialize)]
struct Point {
    x: f64,
    y: f64,
    z: f64,
}
#[derive(Deserialize)]
struct OrientationSample {
    direction: [f64; 2],
    step: u64,
    value: [f64; 9],
    point: Point,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Seed {
    birth_tick: u64,
    charge: f64,
    enhanced_effects: Option<bool>,
    id: u64,
}
#[derive(Deserialize)]
struct FragmentFrame {
    age: f64,
    fragments: Vec<[f64; 11]>,
}
#[derive(Deserialize)]
struct FragmentSample {
    seed: Seed,
    count: usize,
    lifetime: u64,
    frames: Vec<FragmentFrame>,
}
#[derive(Deserialize)]
struct Goldens {
    #[serde(rename = "sourceRevision")]
    source_revision: String,
    growth: Vec<[f64; 5]>,
    release: Vec<[f64; 4]>,
    contacts: Vec<Contact>,
    orientations: Vec<OrientationSample>,
    fragments: Vec<FragmentSample>,
}

#[test]
fn no_bind_boulder_contact_exhausts_the_pool_for_lethal_and_nonlethal_targets() {
    for (target_health, damage) in [(1.0, 1.0), (2.0, 2.0), (20.0, 2.0)] {
        let result = consume_native_earth_boulder_contact(NativeEarthBoulderContactInput {
            release_base_damage: 8.0,
            release_charge: 0.5,
            remaining_pool: 2.0,
            target_health,
            toughness: 0.0,
        });
        assert_eq!(
            result,
            NativeEarthBoulderContactResult {
                charge: 0.32499998807907104,
                continue_traversal: false,
                damage,
                depleted: true,
                remaining_pool: 0.0,
            }
        );
    }
}

#[test]
fn boulder_contact_rejects_negative_and_nonfinite_toughness() {
    for toughness in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            std::panic::catch_unwind(|| {
                consume_native_earth_boulder_contact(NativeEarthBoulderContactInput {
                    release_base_damage: 8.0,
                    release_charge: 0.5,
                    remaining_pool: 2.0,
                    target_health: 1.0,
                    toughness,
                })
            })
            .is_err()
        );
    }
}

#[test]
fn boulder_growth_pool_orientation_and_lit_fragments_match_the_website() {
    let fixture: Goldens =
        serde_json::from_str(include_str!("fixtures/website/earth-spell-goldens.json")).unwrap();
    assert_eq!(fixture.source_revision, "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    for [charge, growth, maximum, speed, expected] in fixture.growth {
        assert_eq!(advance_native_earth_boulder_charge(charge, growth, maximum, speed), expected);
    }
    for [damage, charge, factor, expected] in fixture.release {
        assert_eq!(native_earth_boulder_released_damage(damage, charge, factor), expected);
    }
    for sample in fixture.contacts {
        let input = sample.input;
        let result = consume_native_earth_boulder_contact(NativeEarthBoulderContactInput {
            release_base_damage: input.release_base_damage,
            release_charge: input.release_charge,
            remaining_pool: input.remaining_pool,
            target_health: input.target_health,
            toughness: input.toughness,
        });
        if let Some(charge) = sample.charge {
            assert_eq!(result.charge, charge);
        } else {
            assert!(result.charge.is_nan(), "the source preserves 0/0 at an empty release pool");
        }
        assert_eq!(result.damage, sample.damage);
        assert_eq!(result.continue_traversal, sample.continue_traversal);
        assert_eq!(result.depleted, sample.depleted);
        assert_eq!(result.remaining_pool, sample.remaining_pool);
    }
    for sample in fixture.orientations {
        let direction = Vec2::new(sample.direction[0], sample.direction[1]);
        let mut value = EARTH_BOULDER_IDENTITY_ORIENTATION;
        for step in 1..=sample.step {
            value = if step <= 5 {
                earth_boulder_held_orientation_step(value, direction)
            } else {
                earth_boulder_flight_orientation_step(value, direction, Vec2::new(1.8, 2.4), 1.0)
            };
        }
        assert_eq!(value, sample.value);
        let point =
            earth_boulder_transform_point(EarthBoulderVector3 { x: 0.3, y: 0.4, z: 0.5 }, value);
        assert_eq!([point.x, point.y, point.z], [sample.point.x, sample.point.y, sample.point.z]);
    }
    for sample in fixture.fragments {
        let seed = EarthImpactSeed {
            birth_tick: sample.seed.birth_tick,
            charge: sample.seed.charge,
            enhanced_effects: sample.seed.enhanced_effects.unwrap_or(true),
            id: sample.seed.id,
        };
        assert_eq!(earth_impact_fragment_count(seed.charge), sample.count);
        assert_eq!(earth_impact_lifetime_ticks(seed), sample.lifetime);
        for frame in sample.frames {
            let fragments = earth_impact_fragments_at_age(seed, frame.age);
            assert_eq!(fragments.len(), frame.fragments.len());
            for (fragment, expected) in fragments.into_iter().zip(frame.fragments) {
                let actual = [
                    fragment.alpha,
                    fragment.height,
                    fragment.index as f64,
                    fragment.position.x,
                    fragment.position.y,
                    fragment.record as f64,
                    fragment.rotation,
                    fragment.scale,
                    fragment.velocity.x,
                    fragment.velocity.y,
                    fragment.vertical_velocity,
                ];
                assert_eq!(
                    actual, expected,
                    "impact {} age {} fragment {}",
                    seed.id, frame.age, fragment.index
                );
            }
        }
    }
}
