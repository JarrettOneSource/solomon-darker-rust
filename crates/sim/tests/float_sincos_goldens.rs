//! Independent pinned-V8 scalar primitives, including the real imported-Hub wander operand.

use serde::Deserialize;
use solomon_sim::float::{cos, sin, sin_cos};

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<[String; 3]>,
}

fn bits(value: &str) -> u64 {
    u64::from_str_radix(value, 16).unwrap()
}

#[test]
fn sincos_matches_v8_at_kernel_reduction_and_ieee_boundaries() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/website/float-sincos-goldens.json")).unwrap();
    for [input, expected_sin, expected_cos] in fixture.cases {
        let value = f64::from_bits(bits(&input));
        let (paired_sin, paired_cos) = sin_cos(value);
        for (function, actual, expected) in [
            ("sin", sin(value), expected_sin.as_str()),
            ("cos", cos(value), expected_cos.as_str()),
            ("sin_cos sine", paired_sin, expected_sin.as_str()),
            ("sin_cos cosine", paired_cos, expected_cos.as_str()),
        ] {
            if expected == "NaN" {
                assert!(actual.is_nan(), "{function}({input})");
            } else {
                assert_eq!(actual.to_bits(), bits(expected), "{function}({input})");
            }
        }
    }
}
