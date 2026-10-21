//! Native compiler/test process used by the standalone Node authoring CLI.

use std::path::Path;

use solomon_modding::authoring::{
    admit_prepared_package, authoring_reference_inventory, bundle_checked_package,
    check_web_lua_package, checked_package_value, package_summary, run_package_tests,
};
use solomon_sim::modding::LuaValue;

fn run() -> Result<LuaValue, String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.as_slice() == ["reference"] {
        return authoring_reference_inventory();
    }
    let Some(command) = arguments.first().map(String::as_str) else { return Err(usage().into()) };
    if arguments.len() != 2 || !["inspect", "check", "test", "bundle"].contains(&command) {
        return Err(usage().into());
    }
    let checked = check_web_lua_package(Path::new(&arguments[1]), &Default::default())?;
    if command != "inspect" {
        admit_prepared_package(&checked)?;
    }
    if command == "test" {
        let tested = run_package_tests(&checked)?;
        let LuaValue::Object(mut summary) = package_summary(&checked)? else { unreachable!() };
        summary.insert("tests".into(), LuaValue::Number(tested.tests as f64));
        summary.insert("output".into(), solomon_modding::json::lua_value(&tested.output)?);
        return Ok(LuaValue::Object(summary));
    }
    let LuaValue::Object(mut value) = checked_package_value(&checked)? else { unreachable!() };
    if command == "bundle" {
        value.insert(
            "bundledEntryScript".into(),
            LuaValue::String(bundle_checked_package(&checked)?),
        );
    }
    Ok(LuaValue::Object(value))
}

fn usage() -> &'static str {
    "usage: solomon-sdmod reference | inspect <directory> | check <directory> | test <directory> | bundle <directory>"
}

fn main() {
    let result = run().and_then(|value| solomon_modding::json::stringify(&value));
    match result {
        Ok(output) => println!("{output}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
