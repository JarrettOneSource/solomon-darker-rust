//! Exact JavaScript powers at the observed Fire operand and IEEE boundaries.

use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<[String; 3]>,
}

#[test]
fn powers_match_v8_at_fire_envelopes_and_ieee_boundaries() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/website/float-pow-goldens.json")).unwrap();
    for [base, exponent, expected] in fixture.cases {
        let bits = |word: &str| u64::from_str_radix(word, 16).unwrap();
        let actual =
            solomon_sim::float::pow(f64::from_bits(bits(&base)), f64::from_bits(bits(&exponent)));
        if expected == "NaN" {
            assert!(actual.is_nan(), "pow({base}, {exponent})");
        } else {
            assert_eq!(actual.to_bits(), bits(&expected), "pow({base}, {exponent})");
        }
    }
}
