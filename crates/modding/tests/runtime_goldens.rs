//! Real Lua executes the source's constructors, includes and retained callbacks.

mod common;

use serde::Deserialize;
use solomon_modding::definition::WebLuaDefinitionIssue;
use solomon_modding::definition::runtime::{
    WebLuaDefinitionRuntime, WebLuaDefinitionRuntimeError, WebLuaDefinitionRuntimeOptions,
};
use solomon_modding::definition::{CompileWebLuaDefinitionOptions, compile_web_lua_definition};
use solomon_sim::modding::LuaValue;
use solomon_sim::modding::definition::{WebLuaModDefinition, WebLuaModIdentity};

#[derive(Deserialize)]
struct Fixture {
    identity: WebLuaModIdentity,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    input: Input,
    definition: Option<WebLuaModDefinition>,
    issues: Vec<WebLuaDefinitionIssue>,
    actions: Vec<ActionOutput>,
    logs: Vec<String>,
    closed_memory_bytes: usize,
}

#[derive(Deserialize)]
struct Input {
    code: String,
    scripts: Vec<(String, String)>,
    actions: Vec<Action>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Action {
    Reduce { key: String, state: LuaValue, event: LuaValue, context: LuaValue },
    Migrate { key: String, version: u64, value: LuaValue },
    Run { code: String },
}

#[derive(Deserialize)]
struct ActionOutput {
    output: Option<LuaValue>,
    error: Option<String>,
}

#[test]
fn actual_lua_definitions_includes_reducers_and_sandbox_match_the_source() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-runtime-goldens.json"
    ))
    .unwrap();
    for case in fixture.cases {
        let mut runtime = WebLuaDefinitionRuntime::new(WebLuaDefinitionRuntimeOptions {
            entry_script: "scripts/main.lua".into(),
            identity: fixture.identity.clone(),
            scripts: case.input.scripts,
        })
        .unwrap();
        let (definition, issues) = match runtime.run(&case.input.code) {
            Ok(definition) => (Some(definition), Vec::new()),
            Err(WebLuaDefinitionRuntimeError::Definition(error)) => (None, error.issues),
            Err(error) => panic!("{} unexpected error: {error}", case.name),
        };
        assert_eq!(issues, case.issues, "{} issues", case.name);
        if let (Some(mut actual), Some(mut expected)) = (definition, case.definition) {
            let options = CompileWebLuaDefinitionOptions::default();
            let actual_graph = compile_web_lua_definition(&fixture.identity, &actual, &options);
            let expected_graph = compile_web_lua_definition(&fixture.identity, &expected, &options);
            match (actual_graph, expected_graph) {
                (Ok(actual), Ok(expected)) => common::assert_json_values(
                    &serde_json::to_value(actual).unwrap(),
                    &serde_json::to_value(expected).unwrap(),
                    &format!("{} compiled", case.name),
                ),
                (Err(actual), Err(expected)) => {
                    assert_eq!(actual, expected, "{} graph rejection", case.name)
                }
                _ => panic!("{} changed graph admission", case.name),
            }
            // The Source sd.mod assets map crosses Lua's private hash order.
            // Independent Source VMs return both [worn,icon] and [icon,worn];
            // the verified compiler consumes their same canonical graph above.
            actual.assets.sort_by_key(|asset| asset.source.node);
            expected.assets.sort_by_key(|asset| asset.source.node);
            common::assert_json_values(
                &serde_json::to_value(&actual).unwrap(),
                &serde_json::to_value(&expected).unwrap(),
                &case.name,
            );
        } else {
            assert!(!case.issues.is_empty(), "{} missing accepted definition", case.name);
        }
        assert_eq!(case.input.actions.len(), case.actions.len(), "{} callback count", case.name);
        for (index, (action, expected)) in
            case.input.actions.into_iter().zip(case.actions).enumerate()
        {
            let result = match action {
                Action::Reduce { key, state, event, context } => {
                    runtime.invoke_reducer(&key, &state, &event, &context).map(|result| {
                        let mut fields = solomon_sim::modding::LuaObject::new();
                        fields.insert("intents".into(), result.intents);
                        fields.insert("state".into(), result.state);
                        LuaValue::Object(fields)
                    })
                }
                Action::Migrate { key, version, value } => {
                    runtime.invoke_migration(&key, version, &value)
                }
                Action::Run { code } => runtime
                    .run(&code)
                    .map_err(|error| error.to_string())
                    .and_then(|definition| solomon_modding::json::lua_value(&definition)),
            };
            match result {
                Ok(output) => {
                    assert!(
                        expected.error.is_none(),
                        "{} action {index} unexpected success",
                        case.name
                    );
                    common::assert_json_values(
                        &serde_json::to_value(output).unwrap(),
                        &serde_json::to_value(expected.output.expect("accepted callback output"))
                            .unwrap(),
                        &format!("{} action {index}", case.name),
                    );
                }
                Err(error) => {
                    assert_eq!(Some(error), expected.error, "{} action {index}", case.name)
                }
            }
        }
        assert!(runtime.memory_bytes() > 0, "{} live VM memory", case.name);
        assert_eq!(runtime.drain_logs(), case.logs, "{} logs", case.name);
        runtime.close();
        assert_eq!(
            runtime.memory_bytes(),
            case.closed_memory_bytes,
            "{} closed VM memory",
            case.name
        );
        assert!(runtime.run("sd.mod{}").unwrap_err().to_string().contains("closed"));
    }
}
