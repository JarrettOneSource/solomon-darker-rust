//! Creation identity, shorthand inference and free rules match the source assembly.

use serde::Deserialize;
use solomon_modding::definition::WebLuaDefinitionIssue;
use solomon_modding::definition::assembly::{
    AssembledWebLuaDefinition, WebLuaAssemblyInput, WebLuaExplicitLists,
    assemble_web_lua_definition,
};

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    input: WebLuaAssemblyInput,
    explicit: Option<WebLuaExplicitLists>,
    output: AssembledWebLuaDefinition,
    issues: Vec<WebLuaDefinitionIssue>,
}

#[test]
fn friendly_definitions_lower_actual_nodes_assets_references_and_free_rules() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-assembly-goldens.json"
    ))
    .unwrap();
    for case in fixture.cases {
        let mut issues = Vec::new();
        let actual = assemble_web_lua_definition(&case.input, case.explicit.as_ref(), &mut issues);
        assert_eq!(issues, case.issues, "{} issues", case.name);
        assert_eq!(actual.assets, case.output.assets, "{} assets", case.name);
        assert_eq!(actual.content.len(), case.output.content.len(), "{} content count", case.name);
        for (index, (actual, expected)) in
            actual.content.iter().zip(&case.output.content).enumerate()
        {
            assert_eq!(actual, expected, "{} content {index}", case.name);
        }
        assert_eq!(actual.rules, case.output.rules, "{} root rules", case.name);
    }
}
