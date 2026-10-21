//! Public authoring checks and package tests from the actual current sdmod CLI.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use solomon_modding::authoring::{
    admit_prepared_package, bundle_checked_package, check_web_lua_package, package_summary,
    run_package_tests,
};
use solomon_sim::modding::LuaValue;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    cases: Vec<Case>,
    included_bundle_entry: String,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    input: Input,
    expected: Option<LuaValue>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct Input {
    manifest: LuaValue,
    entry: String,
    scripts: BTreeMap<String, String>,
    files: Vec<File>,
    tests: BTreeMap<String, String>,
    command: Option<String>,
}

#[derive(Deserialize)]
struct File {
    path: String,
    bytes: String,
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root =
            std::env::temp_dir().join(format!("solomon-authoring-{}-{time}", std::process::id()));
        fs::create_dir(&root).expect("create owned authoring scratch");
        Self(root)
    }

    fn package(&self, case: &Case) -> PathBuf {
        let root = self.0.join(&case.name);
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::write(
            root.join("manifest.json"),
            solomon_modding::json::stringify(&case.input.manifest).unwrap(),
        )
        .unwrap();
        let entry = case
            .input
            .manifest
            .get("runtime")
            .and_then(|value| value.get("entryScript"))
            .and_then(LuaValue::as_str);
        if let Some(entry) = entry.filter(|path| !path.contains("..") && !path.starts_with('/')) {
            let path = root.join(entry);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, &case.input.entry).unwrap();
        }
        for (path, code) in &case.input.scripts {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, code).unwrap();
        }
        for file in &case.input.files {
            let path = root.join(&file.path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, STANDARD.decode(&file.bytes).unwrap()).unwrap();
        }
        for (name, code) in &case.input.tests {
            let path = root.join("tests").join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, code).unwrap();
        }
        root
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove owned authoring scratch");
    }
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/sdmod-authoring-goldens.json"
    ))
    .expect("recorded source authoring fixture")
}

#[test]
fn current_authoring_and_real_lua_package_tests_match_source() {
    let scratch = Scratch::new();
    for case in fixture().cases {
        let root = scratch.package(&case);
        let result = (|| {
            let checked = check_web_lua_package(&root, &Default::default())?;
            admit_prepared_package(&checked)?;
            let LuaValue::Object(mut summary) = package_summary(&checked)? else { unreachable!() };
            if case.input.command.as_deref() == Some("test") {
                let tested = run_package_tests(&checked)?;
                assert!(tested.output.is_empty(), "{} unexpectedly printed", case.name);
                summary.insert("tests".into(), LuaValue::Number(tested.tests as f64));
            }
            Ok::<_, String>(LuaValue::Object(summary))
        })();
        match (result, case.expected, case.error) {
            (Ok(actual), Some(expected), None) => assert_eq!(actual, expected, "{}", case.name),
            (Err(actual), None, Some(expected)) => assert_eq!(actual, expected, "{}", case.name),
            (actual, expected, error) => panic!(
                "{}: unexpected admission: {actual:?}, expected {expected:?}/{error:?}",
                case.name
            ),
        }
    }
}

#[test]
fn packed_includes_keep_source_text_and_recompile_to_the_same_graph() {
    let fixture = fixture();
    let case = fixture.cases.iter().find(|case| case.name == "includes").unwrap();
    let scratch = Scratch::new();
    let root = scratch.package(case);
    let checked = check_web_lua_package(&root, &Default::default()).unwrap();
    admit_prepared_package(&checked).unwrap();
    assert_eq!(bundle_checked_package(&checked).unwrap(), fixture.included_bundle_entry);
}
