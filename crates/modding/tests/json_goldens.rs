//! Graph hashes include source collation and ECMAScript numeric spelling.
use serde_json::Value;
use solomon_modding::assets::sha256;
use solomon_modding::json::{SourceCollator, canonical_graph, stringify};
use solomon_sim::modding::LuaValue;

#[test]
fn graph_collation_and_json_numbers_preserve_source_digests() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-json-goldens.json"
    ))
    .unwrap();
    let collator = SourceCollator::default();
    let mut words: Vec<_> =
        fixture["words"].as_array().unwrap().iter().map(|word| word.as_str().unwrap()).collect();
    words.sort_by(|left, right| collator.compare(left, right));
    assert_eq!(serde_json::to_value(words).unwrap(), fixture["sorted"]);
    for case in fixture["numbers"].as_array().unwrap() {
        let bits = case["bits"].as_str().unwrap().parse::<u64>().unwrap();
        assert_eq!(
            stringify(&LuaValue::Number(f64::from_bits(bits))).unwrap(),
            case["json"].as_str().unwrap()
        );
    }
    for case in fixture["graphs"].as_array().unwrap() {
        let graph: LuaValue = serde_json::from_value(case["graph"].clone()).unwrap();
        let actual = stringify(&canonical_graph(&graph, &collator)).unwrap();
        assert_eq!(actual, case["canonicalJson"].as_str().unwrap());
        assert_eq!(sha256(actual.as_bytes()), case["graphSha256"].as_str().unwrap());
    }
}
