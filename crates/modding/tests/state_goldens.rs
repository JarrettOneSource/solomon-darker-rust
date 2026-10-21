//! Scoped cells use actual Lua migrations and exact Source ordered JSON budgets.

mod common;
use serde::Deserialize;
use serde_json::{Value, json};
use solomon_modding::definition::runtime::{
    WebLuaDefinitionRuntime, WebLuaDefinitionRuntimeOptions,
};
use solomon_modding::state::{ModStateStore, schema_default, validate_schema_value};
use solomon_sim::modding::LuaValue;
use solomon_sim::modding::definition::{WebLuaModIdentity, WebLuaSchemaDefinition};
use solomon_sim::modding::runtime::{ModStateCellDefinition, ModStateCheckpoint, ModStateScope};
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Fixture {
    identity: WebLuaModIdentity,
    code: String,
    definitions: Vec<Definition>,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Definition {
    name: String,
    definition: ModStateCellDefinition,
}
#[derive(Deserialize)]
struct Case {
    input: Input,
    output: Value,
    error: Option<String>,
    checkpoint: Option<ModStateCheckpoint>,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum Input {
    Cell { name: String, definition: String, key: Option<String>, scope: ModStateScope },
    Set { name: String, value: LuaValue },
    Get { name: String },
    Snapshot { scopes: Option<Vec<ModStateScope>> },
    Restore { checkpoint: ModStateCheckpoint, definitions: Vec<String> },
    Rollback { checkpoint: ModStateCheckpoint },
    CloseScope { scope: ModStateScope },
    Default { schema: WebLuaSchemaDefinition, field: String },
    Validate { schema: WebLuaSchemaDefinition, value: LuaValue, field: String },
    Close,
}

#[test]
fn scoped_cells_migrate_validate_rollback_and_enforce_exact_source_byte_budgets() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/mod-state-goldens.json"
    ))
    .unwrap();
    let mut runtime = WebLuaDefinitionRuntime::new(WebLuaDefinitionRuntimeOptions {
        entry_script: "scripts/main.lua".into(),
        identity: fixture.identity,
        scripts: Vec::new(),
    })
    .unwrap();
    runtime.run(&fixture.code).unwrap();
    let definitions: BTreeMap<_, _> =
        fixture.definitions.into_iter().map(|row| (row.name, row.definition)).collect();
    let mut store = ModStateStore::new();
    let mut cells = BTreeMap::new();
    for (index, case) in fixture.cases.into_iter().enumerate() {
        let result: Result<Value, String> = match case.input {
            Input::Cell { name, definition, key, scope } => {
                let mut definition = definitions[&definition].clone();
                if let Some(key) = key {
                    definition.key = key
                }
                store.cell(definition, scope).and_then(|id| {
                    cells.insert(name, id.clone());
                    store.read(&id).map(|value| json!(value))
                })
            }
            Input::Set { name, value } => store.set(&cells[&name], value).map(|value| json!(value)),
            Input::Get { name } => store.read(&cells[&name]).map(|value| json!(value)),
            Input::Snapshot { scopes } => {
                store.snapshot(scopes.as_deref()).map(|value| json!(value))
            }
            Input::Restore { checkpoint, definitions: keys } => {
                let definitions: Vec<_> = keys.iter().map(|key| definitions[key].clone()).collect();
                store
                    .restore(&checkpoint, &definitions, |definition, version, value| {
                        runtime.invoke_migration(&definition.key, version, value)
                    })
                    .map(|()| Value::Null)
            }
            Input::Rollback { checkpoint } => store.rollback(&checkpoint).map(|()| Value::Null),
            Input::CloseScope { scope } => store.close_scope(&scope).map(|value| json!(value)),
            Input::Default { schema, field } => {
                schema_default(&schema, &field).map(|value| json!(value))
            }
            Input::Validate { schema, value, field } => {
                validate_schema_value(&schema, &value, &field).map(|value| json!(value))
            }
            Input::Close => {
                store.close();
                Ok(Value::Null)
            }
        };
        match result {
            Ok(output) => {
                assert!(case.error.is_none(), "transaction{index} unexpectedly accepted");
                common::assert_json_values(&output, &case.output, &format!("transaction{index}"));
            }
            Err(error) => assert_eq!(Some(error), case.error, "transaction{index} rejection"),
        }
        match store.snapshot(None) {
            Ok(checkpoint) => common::assert_json_values(
                &json!(checkpoint),
                &json!(case.checkpoint.expect("open source store")),
                &format!("transaction{index}.checkpoint"),
            ),
            Err(error) => assert_eq!(error, "mod state store is closed"),
        }
    }
}
