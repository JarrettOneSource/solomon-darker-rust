//! Definition admission from `host/web-mod-content.ts`.
//! Each temporary VM closes before its immutable compiled values leave this
//! worker, so the native authority can receive the result without Lua handles.

use solomon_assets::boneyard::boneyard_scene_source_value;
use solomon_sim::modding::LuaValue;
use solomon_sim::modding::definition::{CompiledWebLuaMod, WebLuaContentKind};

use super::{MaterializedWebSessionContent, project_mod_boneyard};
use crate::assets::sha256;
use crate::definition::compiler::{
    CompileWebLuaDefinitionOptions, WebLuaDefinitionDependency, WebLuaDefinitionDependencyContent,
    compile_web_lua_definition,
};
use crate::definition::runtime::{WebLuaDefinitionRuntime, WebLuaDefinitionRuntimeOptions};

pub fn compile_web_session_content_definitions(
    mut content: MaterializedWebSessionContent,
) -> Result<MaterializedWebSessionContent, String> {
    if content.mod_sources.is_empty() {
        return Ok(content);
    }
    let mut compiled_mods: Vec<CompiledWebLuaMod> = Vec::new();
    for source in &content.mod_sources {
        let mut runtime = WebLuaDefinitionRuntime::new(WebLuaDefinitionRuntimeOptions {
            entry_script: "scripts/main.lua".into(),
            identity: source.identity.clone(),
            scripts: Vec::new(),
        })?;
        let definition = runtime.run(&source.entry_script).map_err(|error| error.to_string())?;
        let mut dependencies = Vec::with_capacity(compiled_mods.len());
        for package in &compiled_mods {
            let mut rows = Vec::with_capacity(package.content.len());
            for definition in &package.content {
                rows.push(WebLuaDefinitionDependencyContent {
                    content_id: definition.content_id.clone(),
                    content_kind: definition.content_kind,
                    key: definition.key.clone(),
                });
            }
            dependencies.push(WebLuaDefinitionDependency {
                content: rows,
                id: package.identity.id.clone(),
            });
        }
        compiled_mods.push(
            compile_web_lua_definition(
                &source.identity,
                &definition,
                &CompileWebLuaDefinitionOptions { dependencies, ..Default::default() },
            )
            .map_err(|error| error.to_string())?,
        );
        runtime.close();
    }

    let mut boneyards = Vec::new();
    for package in &compiled_mods {
        for definition in &package.content {
            if definition.content_kind != WebLuaContentKind::Boneyard {
                continue;
            }
            let path = definition.fields.get("source").and_then(LuaValue::as_str);
            let source =
                content.mod_sources.iter().find(|source| source.identity.id == package.identity.id);
            let bytes = source.and_then(|source| path.and_then(|path| source.files.get(path)));
            let (Some(path), Some(bytes)) = (path, bytes) else {
                return Err(format!(
                    "{}:{} Boneyard source is not packaged",
                    package.identity.id, definition.key
                ));
            };
            let mut projected =
                project_mod_boneyard(&package.identity.id, &package.identity.name, path, bytes)?;
            let mode = definition
                .fields
                .get("environment")
                .and_then(LuaValue::as_object)
                .and_then(|environment| environment.get("mode"))
                .and_then(LuaValue::as_number)
                .filter(|mode| {
                    mode.is_finite() && mode.fract() == 0.0 && mode.abs() <= 9_007_199_254_740_991.0
                });
            if let Some(mode) = mode {
                // Invalid signed/oversized modes cannot be represented by a
                // scene. They are also rejected by the Source prepared catalog.
                if mode < 0.0 || mode > f64::from(u32::MAX) {
                    return Err(format!(
                        "{}.environment.mode must be an integer within 0..2",
                        definition.key
                    ));
                }
                projected.scene.environment_mode = mode as u32;
                let geometry =
                    crate::json::stringify(&boneyard_scene_source_value(&projected.scene))?;
                projected.geometry_sha256 = sha256(geometry.as_bytes());
            }
            if let Some(name) = definition.fields.get("name").and_then(LuaValue::as_str) {
                projected.choice.name = name.into();
            }
            projected.web_lua_content_id = Some(definition.content_id.clone());
            boneyards.push(projected);
        }
    }
    content.boneyards.extend(boneyards);
    for summary in &mut content.summary.mods {
        summary.graph_sha256 = compiled_mods
            .iter()
            .find(|package| package.identity.id == summary.id)
            .map(|package| package.graph_sha256.clone());
    }
    content.compiled_mods = compiled_mods;
    Ok(content)
}
