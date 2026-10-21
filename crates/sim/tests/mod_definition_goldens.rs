//! Definition identities and the complete enabled content vocabulary.

use serde_json::Value;
use solomon_sim::modding::definition::*;

#[test]
fn authored_content_identities_and_enabled_families_match_the_source() {
    let golden: Value =
        serde_json::from_str(include_str!("fixtures/website/mod-definition-goldens.json")).unwrap();
    for case in golden["ids"].as_array().unwrap() {
        assert_eq!(
            stable_web_lua_content_id(
                case["modId"].as_str().unwrap(),
                case["key"].as_str().unwrap()
            ),
            case["contentId"].as_str().unwrap(),
        );
    }
    for case in golden["keys"].as_array().unwrap() {
        assert_eq!(
            valid_web_lua_content_key(case["key"].as_str().unwrap()),
            case["valid"].as_bool().unwrap(),
            "key {:?}",
            case["key"],
        );
    }
    let content_kinds: Vec<_> =
        WebLuaContentKind::ALL.into_iter().map(|kind| kind.as_str()).collect();
    assert_eq!(serde_json::to_value(content_kinds).unwrap(), golden["contentKinds"]);
    for kind in WebLuaContentKind::ALL {
        assert_eq!(
            serde_json::to_value(kind.art_slots()).unwrap(),
            golden["artSlots"][kind.as_str()]
        );
    }
    let asset_kinds: Vec<_> = WebLuaAssetKind::ALL.into_iter().map(|kind| kind.as_str()).collect();
    let scope_kinds: Vec<_> = WebLuaScopeKind::ALL.into_iter().map(|kind| kind.as_str()).collect();
    assert_eq!(serde_json::to_value(asset_kinds).unwrap(), golden["assetKinds"]);
    assert_eq!(serde_json::to_value(scope_kinds).unwrap(), golden["scopeKinds"]);
    for (actual, field) in [
        (WEB_LUA_RULE_EVENT_NAMES.as_slice(), "eventNames"),
        (WEB_LUA_RULE_NAMES.as_slice(), "ruleNames"),
        (WEB_LUA_EFFECT_NAMES.as_slice(), "effectNames"),
        (WEB_LUA_PREFAB_NAMES.as_slice(), "prefabNames"),
        (WEB_LUA_SCHEMA_NAMES.as_slice(), "schemaNames"),
        (WEB_LUA_ART_ALIAS_NAMES.as_slice(), "artAliasNames"),
    ] {
        assert_eq!(serde_json::to_value(actual).unwrap(), golden[field]);
    }
    let limits = WebLuaDefinitionLimits::default();
    assert_eq!(limits.maximum_assets, golden["limits"]["maximumAssets"].as_u64().unwrap() as usize);
    assert_eq!(
        limits.maximum_content,
        golden["limits"]["maximumContent"].as_u64().unwrap() as usize
    );
    assert_eq!(limits.maximum_depth, golden["limits"]["maximumDepth"].as_u64().unwrap() as usize);
    assert_eq!(limits.maximum_nodes, golden["limits"]["maximumNodes"].as_u64().unwrap() as usize);
    assert_eq!(
        limits.maximum_reducers,
        golden["limits"]["maximumReducers"].as_u64().unwrap() as usize
    );
    assert_eq!(limits.maximum_rules, golden["limits"]["maximumRules"].as_u64().unwrap() as usize);
    assert_eq!(
        limits.maximum_string_bytes,
        golden["limits"]["maximumStringBytes"].as_u64().unwrap() as usize
    );
}
