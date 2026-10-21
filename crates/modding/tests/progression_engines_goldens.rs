//! Real authored offers, purchases and scoped values retain their Source state.

mod common;

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::engines::{
    semantic::ModSemanticStateEngine, shops::ModShopEngine, skills::ModSkillEngine,
};
use solomon_sim::modding::LuaValue;
use solomon_sim::modding::content::{PreparedModContentCatalog, PreparedModContentDefinition};
use solomon_sim::modding::runtime::{
    ModSemanticStateCheckpoint, ModShopCheckpoint, ModSkillCheckpoint, ModStateScope,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    catalog: Vec<PreparedModContentDefinition>,
    mod_ids: Vec<String>,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    input: Input,
    output: Value,
    error: Option<String>,
    skills: ModSkillCheckpoint,
    shops: ModShopCheckpoint,
    semantic: ModSemanticStateCheckpoint,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
enum Input {
    Offer { player_id: String, level: u64, seed: u64, count: u32 },
    Choose { player_id: String, content_id: String, offer_sequence: u64 },
    Bind { player_id: String, slot: u32, content_id: Option<String> },
    Unlocked { player_id: String, content_id: String },
    Filter { player_id: String, key: String, source: f64 },
    SkillRestore { checkpoint: ModSkillCheckpoint },
    Purchase { player_id: String, shop_content_id: String, row: usize, gold: u64, tick: u64 },
    Remaining { player_id: String, shop_content_id: String, row: usize, tick: u64 },
    ShopTick { tick: u64 },
    ShopRestore { checkpoint: ModShopCheckpoint },
    Set { mod_id: String, scope: ModStateScope, key: String, value: LuaValue },
    Clear { mod_id: String, scope: ModStateScope, key: String },
    SemanticProject { viewer_id: String },
    SemanticRestore { checkpoint: ModSemanticStateCheckpoint },
}

#[test]
fn authored_skill_offers_quickbars_shops_and_scoped_state_match_source_transactions() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-progression-engines-goldens.json"
    ))
    .unwrap();
    let catalog = Arc::new(PreparedModContentCatalog::from_prepared(fixture.catalog).unwrap());
    let mut skills = ModSkillEngine::new(Arc::clone(&catalog));
    let mut shops = ModShopEngine::new(catalog, 100).unwrap();
    let mut semantic = ModSemanticStateEngine::new(fixture.mod_ids);
    for (index, case) in fixture.cases.into_iter().enumerate() {
        let result: Result<Value, String> = match case.input {
            Input::Offer { player_id, level, seed, count } => {
                skills.offer(&player_id, level, seed, count).map(|skills| {
                    json!(skills.iter().map(|skill| &skill.common.content_id).collect::<Vec<_>>())
                })
            }
            Input::Choose { player_id, content_id, offer_sequence } => {
                skills.choose(&player_id, &content_id, offer_sequence).map(|value| json!(value))
            }
            Input::Bind { player_id, slot, content_id } => {
                skills.bind(&player_id, slot, content_id.as_deref()).map(|()| Value::Null)
            }
            Input::Unlocked { player_id, content_id } => {
                skills.spell_unlocked(&player_id, &content_id).map(|value| json!(value))
            }
            Input::Filter { player_id, key, source } => {
                skills.filter(&player_id, &key, source).map(|value| json!(value))
            }
            Input::SkillRestore { checkpoint } => skills.restore(checkpoint).map(|()| Value::Null),
            Input::Purchase { player_id, shop_content_id, row, gold, tick } => shops
                .purchase(&player_id, &shop_content_id, row, gold, tick)
                .map(|value| json!(value)),
            Input::Remaining { player_id, shop_content_id, row, tick } => {
                shops.remaining(&player_id, &shop_content_id, row, tick).map(|value| json!(value))
            }
            Input::ShopTick { tick } => shops.tick(tick).map(|value| json!(value)),
            Input::ShopRestore { checkpoint } => shops.restore(checkpoint).map(|()| Value::Null),
            Input::Set { mod_id, scope, key, value } => {
                semantic.set(&mod_id, scope, &key, value).map(|()| Value::Null)
            }
            Input::Clear { mod_id, scope, key } => Ok(json!(semantic.clear(&mod_id, &scope, &key))),
            Input::SemanticProject { viewer_id } => Ok(json!(semantic.project(Some(&viewer_id)))),
            Input::SemanticRestore { checkpoint } => {
                semantic.restore(checkpoint).map(|()| Value::Null)
            }
        };
        match result {
            Ok(output) => {
                assert!(case.error.is_none(), "transaction{index} unexpectedly accepted");
                common::assert_json_values(&output, &case.output, &format!("transaction{index}"));
            }
            Err(error) => assert_eq!(Some(error), case.error, "transaction{index} error"),
        }
        assert_eq!(skills.checkpoint(), case.skills, "transaction{index}.skills");
        assert_eq!(shops.checkpoint(), case.shops, "transaction{index}.shops");
        common::assert_json_values(
            &json!(semantic.checkpoint()),
            &json!(case.semantic),
            &format!("transaction{index}.semantic"),
        );
    }
}
