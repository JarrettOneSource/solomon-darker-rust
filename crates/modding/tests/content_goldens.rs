//! Prepared authored families and package admission errors replay the pinned catalog.
use serde::Deserialize;
use solomon_modding::assets::PreparedModAssetCatalog;
use solomon_modding::content::compile_mod_content_catalog;
use solomon_sim::modding::assets::PreparedModAsset;
use solomon_sim::modding::content::PreparedModContentDefinition;
use solomon_sim::modding::definition::CompiledWebLuaMod;

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    mods: Vec<CompiledWebLuaMod>,
    assets: Vec<PreparedModAsset>,
    output: Option<Vec<PreparedModContentDefinition>>,
    error: Option<String>,
}

#[test]
fn all_authored_content_families_and_rejections_match_source() {
    // Decode directly: Lua objects retain source property insertion order.
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-content-goldens.json"
    ))
    .unwrap();
    for case in fixture.cases {
        let assets = PreparedModAssetCatalog::new(case.assets).unwrap();
        match compile_mod_content_catalog(&case.mods, &assets) {
            Ok(actual) => {
                assert!(case.error.is_none(), "{} unexpectedly accepted", case.name);
                assert_eq!(actual.all(), case.output.unwrap(), "{}", case.name);
            }
            Err(error) => assert_eq!(Some(error), case.error, "{}", case.name),
        }
    }
}
