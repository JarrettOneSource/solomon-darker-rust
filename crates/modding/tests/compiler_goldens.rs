//! Source graph resolution, digest and admission diagnostics share one compiler.

use serde::Deserialize;
use solomon_modding::definition::{
    CompileWebLuaDefinitionOptions, WebLuaDefinitionIssue, compile_web_lua_definition,
};
use solomon_sim::modding::definition::{CompiledWebLuaMod, WebLuaModDefinition, WebLuaModIdentity};

#[derive(Deserialize)]
struct Fixture {
    identity: WebLuaModIdentity,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    definition: WebLuaModDefinition,
    options: CompileWebLuaDefinitionOptions,
    output: Option<CompiledWebLuaMod>,
    issues: Option<Vec<WebLuaDefinitionIssue>>,
    error: Option<String>,
}

#[test]
fn definition_graphs_resolve_references_hash_and_report_source_admission_issues() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-compiler-goldens.json"
    ))
    .unwrap();
    for case in fixture.cases {
        match compile_web_lua_definition(&fixture.identity, &case.definition, &case.options) {
            Ok(actual) => {
                assert!(case.error.is_none(), "{} unexpectedly accepted", case.name);
                assert_eq!(actual, case.output.unwrap(), "{}", case.name);
            }
            Err(error) => {
                let expected = case.issues.unwrap();
                assert_eq!(error.issues.len(), expected.len(), "{} issue count", case.name);
                for (index, (actual, expected)) in error.issues.iter().zip(&expected).enumerate() {
                    assert_eq!(actual, expected, "{} issue {index}", case.name);
                }
                assert_eq!(Some(error.to_string()), case.error, "{}", case.name);
            }
        }
    }
}
