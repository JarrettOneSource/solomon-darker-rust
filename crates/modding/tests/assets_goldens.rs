//! Accepted asset bytes and rejected package changes replay the actual compiler.
use serde::Deserialize;
use serde_json::Value;
use solomon_modding::assets::{ModAssetSource, compile_mod_assets};
use solomon_sim::modding::assets::{GameModAsset, PreparedModAsset};
use solomon_sim::modding::definition::CompiledWebLuaMod;

#[derive(Deserialize)]
struct Input {
    assets: Vec<GameModAsset>,
    mods: Vec<CompiledWebLuaMod>,
    sources: Vec<ModAssetSource>,
}

#[test]
fn all_six_asset_families_and_package_rejections_match_source() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-assets-goldens.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let input: Input = serde_json::from_value(case["input"].clone()).unwrap();
        match compile_mod_assets(&input.assets, &input.mods, &input.sources) {
            Ok(actual) => {
                assert!(case["error"].is_null(), "{} unexpectedly accepted", case["name"]);
                let expected: Vec<PreparedModAsset> =
                    serde_json::from_value(case["output"].clone()).unwrap();
                assert_eq!(actual.all(), expected, "{}", case["name"]);
            }
            Err(error) => assert_eq!(error, case["error"].as_str().unwrap(), "{}", case["name"]),
        }
    }
}
