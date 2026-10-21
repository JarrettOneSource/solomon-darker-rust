//! Eight-lane Hurricane programs, fast-distance force and native damage cadence.

use serde_json::{Value, json};
use solomon_sim::hurricane::*;
use solomon_sim::{NativeRng, Vec2};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn rng(value: &NativeRng) -> Value {
    json!({"indexA":value.index_a,"indexB":value.index_b,"words":&value.words[..]})
}
fn program(value: &NativeHurricanePresentationProgram) -> Value {
    json!({"phaseDegrees":value.phase_degrees,"lanes":value.lanes.map(|lane|json!({"angleDegrees":lane.angle_degrees,
        "angularVelocityDegrees":lane.angular_velocity_degrees,"radius":lane.radius,"verticalOffset":lane.vertical_offset}))})
}
fn assert_snapshot(actual: &Value, expected: &Value, label: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64(), "{label}"),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{label}");
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                assert_snapshot(a, b, &format!("{label}[{index}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{label}");
            for (key, b) in b {
                assert_snapshot(&a[key], b, &format!("{label}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{label}"),
    }
}

#[test]
fn hurricane_program_force_damage_and_cadence_replay_the_website() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/hurricane-goldens.json")).unwrap();
    for case in fixture["programs"].as_array().unwrap() {
        let mut random = NativeRng::new(case["seed"].as_i64().unwrap());
        let mut current = create_native_hurricane_presentation(&mut random);
        assert_snapshot(
            &json!({"program":program(&current),"rng":rng(&random)}),
            &case["initial"],
            "Hurricane birth",
        );
        let mut samples = case["samples"].as_array().unwrap().iter().peekable();
        for tick in 1..=2000 {
            let charge = if tick % 400 < 200 {
                solomon_sim::float::fround(0.0015 + f64::from(tick % 200) / 200.0)
            } else {
                1.0
            };
            step_native_hurricane_presentation(&mut current, charge, &mut random);
            if samples.peek().is_some_and(|sample| sample["tick"].as_i64() == Some(tick.into())) {
                assert_snapshot(
                    &json!({"tick":tick,"program":program(&current),"rng":rng(&random)}),
                    samples.next().unwrap(),
                    "Hurricane presentation",
                );
            }
        }
    }
    for case in fixture["forces"].as_array().unwrap() {
        let force = native_hurricane_orbit_force(
            point(&case["source"]),
            point(&case["target"]),
            case["charge"].as_f64().unwrap(),
            case["movementStep"].as_u64().unwrap(),
        );
        let expected = case["force"].as_object().map(|_| point(&case["force"]));
        assert_eq!(force, expected, "Hurricane orbit force");
    }
    for case in fixture["damages"].as_array().unwrap() {
        let mut random = NativeRng::new(case["seed"].as_i64().unwrap());
        for sample in case["samples"].as_array().unwrap() {
            let result = draw_native_hurricane_damage(
                &mut random,
                sample["charge"].as_f64().unwrap(),
                sample["minimum"].as_f64().unwrap(),
                sample["maximum"].as_f64().unwrap(),
            );
            assert_snapshot(
                &json!({"damage":result.damage,"suppressHitSound":result.suppress_hit_sound,"rng":rng(&random)}),
                &sample["result"],
                "Hurricane damage",
            );
        }
    }
    for case in fixture["cadence"].as_array().unwrap() {
        assert_eq!(
            native_hurricane_movement_due(
                case["serial"].as_u64().unwrap(),
                case["tick"].as_u64().unwrap(),
                case["step"].as_u64().unwrap()
            ),
            case["due"].as_bool().unwrap()
        );
    }
}
