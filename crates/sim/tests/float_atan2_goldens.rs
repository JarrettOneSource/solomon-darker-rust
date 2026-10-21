//! Exact JavaScript atan2 primitives behind source heading and navigation decisions.

use serde::Deserialize;
use solomon_sim::float::atan2;

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<[String; 3]>,
}

fn bits(value: &str) -> u64 {
    u64::from_str_radix(value, 16).unwrap()
}

#[test]
fn atan2_matches_v8_at_reduction_boundaries_and_across_ieee_exponents() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/website/float-atan2-goldens.json")).unwrap();
    for [y, x, expected] in fixture.cases {
        let actual = atan2(f64::from_bits(bits(&y)), f64::from_bits(bits(&x)));
        if expected == "NaN" {
            assert!(actual.is_nan(), "atan2({y}, {x})");
        } else {
            assert_eq!(actual.to_bits(), bits(&expected), "atan2({y}, {x})");
        }
    }
}
