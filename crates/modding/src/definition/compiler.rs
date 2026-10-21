//! Immutable graph compilation from `web-lua-definition-compiler.ts`.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use solomon_sim::modding::definition::*;
use solomon_sim::modding::{LuaObject, LuaValue};

use super::error::{
    WebLuaDefinitionError, WebLuaDefinitionErrorCode as Code, WebLuaDefinitionIssue as Issue,
};
use super::schemas;
use super::suggestions::did_you_mean;
use crate::assets::sha256;
use crate::json::{SourceCollator, canonical_graph, lua_value, stringify};

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebLuaDefinitionDependencyContent {
    pub content_id: String,
    pub content_kind: WebLuaContentKind,
    pub key: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct WebLuaDefinitionDependency {
    pub content: Vec<WebLuaDefinitionDependencyContent>,
    pub id: String,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileWebLuaDefinitionOptions {
    #[serde(default)]
    pub dependencies: Vec<WebLuaDefinitionDependency>,
    pub limits: Option<WebLuaDefinitionLimits>,
    #[serde(default)]
    pub stock_content: Vec<WebLuaDefinitionDependencyContent>,
}

struct CompileContext<'a> {
    assets: BTreeMap<&'a str, &'a WebLuaAssetDefinition>,
    content: BTreeMap<&'a str, &'a WebLuaContentDefinition>,
    dependencies: BTreeMap<&'a str, &'a WebLuaDefinitionDependency>,
    identity: &'a WebLuaModIdentity,
    issues: Vec<Issue>,
    limits: WebLuaDefinitionLimits,
    stock_content: &'a [WebLuaDefinitionDependencyContent],
}

pub fn compile_web_lua_definition(
    identity: &WebLuaModIdentity,
    definition: &WebLuaModDefinition,
    options: &CompileWebLuaDefinitionOptions,
) -> Result<CompiledWebLuaMod, WebLuaDefinitionError> {
    let mut context = CompileContext {
        assets: BTreeMap::new(),
        content: BTreeMap::new(),
        dependencies: BTreeMap::new(),
        identity,
        issues: Vec::new(),
        limits: options.limits.unwrap_or_default(),
        stock_content: &options.stock_content,
    };
    if definition.api != WEB_LUA_DEFINITION_API_VERSION {
        context.issues.push(Issue::new(
            Code::EApiVersion,
            "api",
            format!("expected {WEB_LUA_DEFINITION_API_VERSION}, received {}", definition.api),
            None,
        ));
    }
    for (path, count, maximum) in [
        ("assets", definition.assets.len(), context.limits.maximum_assets),
        ("content", definition.content.len(), context.limits.maximum_content),
        ("rules", definition.rules.len(), context.limits.maximum_rules),
        ("reducers", definition.reducers.len(), context.limits.maximum_reducers),
    ] {
        if count > maximum {
            context.issues.push(Issue::new(
                Code::EBudget,
                path,
                format!("{path} has {count} entries; maximum is {maximum}"),
                None,
            ));
        }
    }
    for (index, asset) in definition.assets.iter().enumerate() {
        validate_key(
            &asset.key,
            &format!("assets[{index}].key"),
            &asset.source,
            &mut context.issues,
        );
        if context.assets.contains_key(asset.key.as_str()) {
            context.issues.push(Issue::new(
                Code::EDuplicate,
                format!("assets[{index}].key"),
                format!("duplicate asset key: {}", asset.key),
                Some(&asset.source),
            ));
        } else {
            context.assets.insert(&asset.key, asset);
        }
    }
    let mut ids = BTreeMap::new();
    for (index, content) in definition.content.iter().enumerate() {
        validate_key(
            &content.key,
            &format!("content[{index}].key"),
            &content.source,
            &mut context.issues,
        );
        let id = stable_web_lua_content_id(&identity.id, &content.key);
        if let Some(previous) = ids.insert(id, &content.key)
            && previous != &content.key
        {
            context.issues.push(Issue::new(
                Code::EDuplicate,
                format!("content[{index}].key"),
                format!("content identity collision between {previous} and {}", content.key),
                Some(&content.source),
            ));
        }
        if context.content.contains_key(content.key.as_str()) {
            context.issues.push(Issue::new(
                Code::EDuplicate,
                format!("content[{index}].key"),
                format!("content key is already used by another family: {}", content.key),
                Some(&content.source),
            ));
        } else {
            context.content.insert(&content.key, content);
        }
    }
    for (index, dependency) in options.dependencies.iter().enumerate() {
        if context.dependencies.contains_key(dependency.id.as_str()) {
            context.issues.push(Issue::new(
                Code::EDuplicate,
                format!("dependencies[{index}].id"),
                format!("duplicate dependency: {}", dependency.id),
                None,
            ));
        } else {
            context.dependencies.insert(&dependency.id, dependency);
        }
    }
    for (index, asset) in definition.assets.iter().enumerate() {
        schemas::validate_asset(asset, index, &mut context.issues);
    }
    for (index, content) in definition.content.iter().enumerate() {
        schemas::validate_content(content, index, &mut context.issues);
    }
    schemas::validate_nodes(definition, &mut context.issues);
    validate_budgets(definition, &mut context);
    validate_cycles(definition, &mut context);
    validate_mounts(definition, &mut context);

    let collator = SourceCollator::default();
    let mut assets = Vec::with_capacity(definition.assets.len());
    for (index, asset) in definition.assets.iter().enumerate() {
        assets.push(CompiledWebLuaAsset {
            asset_kind: asset.asset_kind,
            fields: context.resolve_record(
                &asset.fields,
                &format!("assets[{index}].fields"),
                &asset.source,
            ),
            key: asset.key.clone(),
        });
    }
    assets.sort_by(|left, right| collator.compare(&left.key, &right.key));
    let mut content = Vec::with_capacity(definition.content.len());
    for (index, entry) in definition.content.iter().enumerate() {
        content.push(CompiledWebLuaContent {
            content_id: stable_web_lua_content_id(&identity.id, &entry.key),
            content_kind: entry.content_kind,
            fields: context.resolve_record(
                &entry.fields,
                &format!("content[{index}].fields"),
                &entry.source,
            ),
            key: entry.key.clone(),
        });
    }
    content.sort_by(|left, right| collator.compare(&left.key, &right.key));
    let mut reducers = Vec::with_capacity(definition.reducers.len());
    for reducer in &definition.reducers {
        reducers.push(CompiledWebLuaReducer {
            key: reducer.key.clone(),
            on: reducer.on.clone(),
            schema_version: reducer.schema_version,
            scope: reducer.scope,
            state: reducer.state.clone(),
        });
    }
    reducers.sort_by(|left, right| collator.compare(&left.key, &right.key));
    let mut rules = Vec::with_capacity(definition.rules.len());
    for (index, rule) in definition.rules.iter().enumerate() {
        rules.push(WebLuaRuleDefinition {
            fields: context.resolve_record(
                &rule.fields,
                &format!("rules[{index}].fields"),
                &rule.source,
            ),
            operation: rule.operation.clone(),
            source: rule.source.clone(),
        });
    }
    if !context.issues.is_empty() {
        return Err(WebLuaDefinitionError::new(context.issues));
    }
    let mut graph = LuaObject::new();
    graph.insert("apiVersion".into(), LuaValue::String(WEB_LUA_DEFINITION_API_VERSION.into()));
    graph.insert("assets".into(), typed_value(&assets));
    graph.insert("content".into(), typed_value(&content));
    graph.insert("identity".into(), typed_value(identity));
    graph.insert("reducers".into(), typed_value(&reducers));
    graph.insert("rules".into(), typed_value(&rules));
    let canonical_json = stringify(&canonical_graph(&LuaValue::Object(graph), &collator))
        .expect("definition values have already been checked for finite JSON data");
    Ok(CompiledWebLuaMod {
        api_version: WEB_LUA_DEFINITION_API_VERSION.into(),
        assets,
        graph_sha256: sha256(canonical_json.as_bytes()),
        canonical_json,
        capabilities: infer_capabilities(definition),
        content,
        identity: identity.clone(),
        reducers,
        rules,
    })
}

impl CompileContext<'_> {
    fn resolve_record(
        &mut self,
        value: &LuaObject,
        path: &str,
        source: &WebLuaDefinitionSource,
    ) -> LuaObject {
        let mut output = LuaObject::new();
        for (key, value) in value.iter() {
            output.insert(
                key.to_owned(),
                self.resolve_value(value, &format!("{path}.{key}"), source),
            );
        }
        output
    }

    fn resolve_value(
        &mut self,
        value: &LuaValue,
        path: &str,
        source: &WebLuaDefinitionSource,
    ) -> LuaValue {
        if schemas::is_token(Some(value), "content-reference") {
            return self.resolve_reference(value, path, source);
        }
        if schemas::is_token(Some(value), "asset-reference") {
            let Some(key) = value.get("key").and_then(LuaValue::as_str) else {
                self.issues.push(Issue::new(
                    Code::EReference,
                    path,
                    "asset reference key must be text",
                    Some(source),
                ));
                return value.clone();
            };
            if !self.assets.contains_key(key) {
                let hint = did_you_mean(key, self.assets.keys().copied());
                self.issues.push(Issue::new(
                    Code::EReference,
                    path,
                    format!("unknown asset: {key}{hint}"),
                    Some(source),
                ));
            }
            return value.clone();
        }
        match value {
            LuaValue::Array(values) => {
                let mut output = Vec::with_capacity(values.len());
                for (index, value) in values.iter().enumerate() {
                    output.push(self.resolve_value(value, &format!("{path}[{index}]"), source));
                }
                LuaValue::Array(output)
            }
            LuaValue::Object(value) => LuaValue::Object(self.resolve_record(value, path, source)),
            _ => value.clone(),
        }
    }

    fn resolve_reference(
        &mut self,
        reference: &LuaValue,
        path: &str,
        source: &WebLuaDefinitionSource,
    ) -> LuaValue {
        let key = reference.get("key").and_then(LuaValue::as_str);
        let target = reference
            .get("targetKind")
            .and_then(LuaValue::as_str)
            .and_then(WebLuaContentKind::parse);
        let (Some(key), Some(target)) = (key, target) else {
            self.issues.push(Issue::new(
                Code::EReference,
                path,
                "content reference requires a key and supported family",
                Some(source),
            ));
            return reference.clone();
        };
        let mod_id = reference.get("modId").and_then(LuaValue::as_str).unwrap_or(&self.identity.id);
        let id = if mod_id == self.identity.id {
            self.content
                .get(key)
                .filter(|entry| entry.content_kind == target)
                .map(|entry| stable_web_lua_content_id(mod_id, &entry.key))
        } else if mod_id == "stock" {
            self.stock_content
                .iter()
                .rev()
                .find(|entry| entry.content_kind == target && entry.key == key)
                .map(|entry| entry.content_id.clone())
        } else {
            self.dependencies
                .get(mod_id)
                .and_then(|dependency| {
                    dependency
                        .content
                        .iter()
                        .find(|entry| entry.key == key && entry.content_kind == target)
                })
                .map(|entry| entry.content_id.clone())
        };
        if id.is_none() {
            let hint = self.reference_hint(key, target, mod_id);
            self.issues.push(Issue::new(
                Code::EReference,
                path,
                format!("unknown {} reference {mod_id}:{key}{hint}", target.as_str()),
                Some(source),
            ));
        }
        let mut resolved = LuaObject::new();
        resolved.insert("contentId".into(), LuaValue::String(id.unwrap_or_else(|| "0".into())));
        resolved.insert("key".into(), LuaValue::String(key.to_owned()));
        resolved.insert("kind".into(), LuaValue::String("resolved-content-reference".into()));
        resolved.insert("modId".into(), LuaValue::String(mod_id.to_owned()));
        resolved.insert("targetKind".into(), LuaValue::String(target.as_str().into()));
        LuaValue::Object(resolved)
    }

    fn reference_hint(&self, key: &str, target: WebLuaContentKind, mod_id: &str) -> String {
        let mut candidates = Vec::new();
        if mod_id == self.identity.id {
            if let Some(entry) = self.content.get(key)
                && entry.content_kind != target
            {
                return format!(
                    "; {key} is a {}, not a {}",
                    entry.content_kind.as_str(),
                    target.as_str()
                );
            }
            for entry in self.content.values() {
                if entry.content_kind == target {
                    candidates.push(entry.key.as_str());
                }
            }
        } else if mod_id == "stock" {
            for entry in self.stock_content {
                if entry.content_kind == target {
                    candidates.push(entry.key.as_str());
                }
            }
        } else {
            let Some(dependency) = self.dependencies.get(mod_id) else {
                return format!("; {mod_id} is not a dependency of this mod");
            };
            for entry in &dependency.content {
                if entry.content_kind == target {
                    candidates.push(entry.key.as_str());
                }
            }
        }
        did_you_mean(key, candidates)
    }
}

fn validate_key(key: &str, path: &str, source: &WebLuaDefinitionSource, issues: &mut Vec<Issue>) {
    if !valid_web_lua_content_key(key) {
        issues.push(Issue::new(Code::EContentKey, path, "use 1..128 lowercase letters, digits, periods, underscores, or hyphens without edge separators", Some(source)));
    }
}

fn validate_budgets(definition: &WebLuaModDefinition, context: &mut CompileContext<'_>) {
    let mut root = LuaObject::new();
    let mut assets = Vec::new();
    for asset in &definition.assets {
        assets.push(ordered_node(asset, &["assetKind", "fields", "key", "kind", "source"]));
    }
    root.insert("assets".into(), LuaValue::Array(assets));
    let mut content = Vec::new();
    for entry in &definition.content {
        content.push(ordered_node(entry, &["contentKind", "fields", "key", "kind", "source"]));
    }
    root.insert("content".into(), LuaValue::Array(content));
    let mut rules = Vec::new();
    for rule in &definition.rules {
        rules.push(ordered_node(rule, &["fields", "kind", "operation", "source"]));
    }
    root.insert("rules".into(), LuaValue::Array(rules));
    let mut reducers = Vec::new();
    for reducer in &definition.reducers {
        let mut value = LuaObject::new();
        value.insert("key".into(), LuaValue::String(reducer.key.clone()));
        value.insert("on".into(), typed_value(&reducer.on));
        value.insert("schemaVersion".into(), LuaValue::Number(reducer.schema_version as f64));
        value.insert("scope".into(), LuaValue::String(reducer.scope.as_str().into()));
        value.insert(
            "state".into(),
            ordered_node(&reducer.state, &["fields", "kind", "schemaKind", "source"]),
        );
        reducers.push(LuaValue::Object(value));
    }
    root.insert("reducers".into(), LuaValue::Array(reducers));
    let mut nodes = 0;
    visit_budget(&LuaValue::Object(root), "definition", 0, &mut nodes, context);
    if nodes > context.limits.maximum_nodes {
        context.issues.push(Issue::new(
            Code::EBudget,
            "definition",
            format!("definition exceeds {} nodes", context.limits.maximum_nodes),
            None,
        ));
    }
}

fn ordered_node<T: serde::Serialize>(value: &T, keys: &[&str]) -> LuaValue {
    // Serde emits an internally tagged enum's discriminant first. The source
    // walks its constructor's field order when reporting budget violations.
    let mut unordered = match typed_value(value) {
        LuaValue::Object(value) => value,
        _ => unreachable!("a definition node serializes as an object"),
    };
    let mut ordered = LuaObject::new();
    for key in keys {
        let value = unordered.remove(key).expect("the source node declares this field");
        ordered.insert((*key).to_owned(), value);
    }
    LuaValue::Object(ordered)
}

fn visit_budget(
    value: &LuaValue,
    path: &str,
    depth: usize,
    nodes: &mut usize,
    context: &mut CompileContext<'_>,
) {
    *nodes += 1;
    if *nodes > context.limits.maximum_nodes {
        return;
    }
    if depth > context.limits.maximum_depth {
        context.issues.push(Issue::new(
            Code::EBudget,
            path,
            format!("definition nesting exceeds {}", context.limits.maximum_depth),
            None,
        ));
        return;
    }
    match value {
        LuaValue::String(value) if value.len() > context.limits.maximum_string_bytes => {
            context.issues.push(Issue::new(
                Code::EBudget,
                path,
                format!("string exceeds {} bytes", context.limits.maximum_string_bytes),
                None,
            ));
        }
        LuaValue::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                visit_budget(value, &format!("{path}[{index}]"), depth + 1, nodes, context);
            }
        }
        LuaValue::Object(values) => {
            for (key, value) in values.iter() {
                visit_budget(value, &format!("{path}.{key}"), depth + 1, nodes, context);
            }
        }
        _ => {}
    }
}

fn validate_cycles(definition: &WebLuaModDefinition, context: &mut CompileContext<'_>) {
    let mut edges = BTreeMap::new();
    let mut keys = Vec::new();
    for content in &definition.content {
        let mut references = Vec::new();
        collect_local_references(
            &LuaValue::Object(content.fields.clone()),
            &context.identity.id,
            &mut references,
        );
        if !edges.contains_key(content.key.as_str()) {
            keys.push(content.key.as_str());
        }
        edges.insert(content.key.as_str(), references);
    }
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    let mut path = Vec::new();
    for key in keys {
        visit_cycle(key, &edges, &mut visiting, &mut visited, &mut path, &mut context.issues);
    }
}

fn collect_local_references(value: &LuaValue, mod_id: &str, references: &mut Vec<String>) {
    if schemas::is_token(Some(value), "content-reference") {
        let owner = value.get("modId").and_then(LuaValue::as_str);
        if owner.is_none_or(|owner| owner == mod_id)
            && let Some(key) = value.get("key").and_then(LuaValue::as_str)
            && !references.iter().any(|existing| existing == key)
        {
            references.push(key.to_owned());
        }
        return;
    }
    match value {
        LuaValue::Array(values) => {
            for value in values {
                collect_local_references(value, mod_id, references);
            }
        }
        LuaValue::Object(values) => {
            for (_, value) in values.iter() {
                collect_local_references(value, mod_id, references);
            }
        }
        _ => {}
    }
}

fn visit_cycle<'a>(
    key: &'a str,
    edges: &'a BTreeMap<&str, Vec<String>>,
    visiting: &mut HashSet<&'a str>,
    visited: &mut HashSet<&'a str>,
    path: &mut Vec<&'a str>,
    issues: &mut Vec<Issue>,
) {
    if visited.contains(key) {
        return;
    }
    if visiting.contains(key) {
        let start = path.iter().position(|entry| *entry == key).unwrap_or(0);
        let mut cycle = path[start..].to_vec();
        cycle.push(key);
        issues.push(Issue::new(
            Code::ECycle,
            format!("content.{key}"),
            format!("content reference cycle: {}", cycle.join(" -> ")),
            None,
        ));
        return;
    }
    visiting.insert(key);
    path.push(key);
    for target in &edges[key] {
        if edges.contains_key(target.as_str()) {
            visit_cycle(target, edges, visiting, visited, path, issues);
        }
    }
    path.pop();
    visiting.remove(key);
    visited.insert(key);
}

fn validate_mounts(definition: &WebLuaModDefinition, context: &mut CompileContext<'_>) {
    let mut owners = BTreeMap::new();
    for (index, content) in definition.content.iter().enumerate() {
        let Some(mount) = content.fields.get("mount").and_then(LuaValue::as_object) else {
            continue;
        };
        if mount.get("exclusive").and_then(LuaValue::as_bool) == Some(false) {
            continue;
        }
        let scene = mount.get("scene").and_then(LuaValue::as_str).filter(|value| !value.is_empty());
        let anchor =
            mount.get("anchor").and_then(LuaValue::as_str).filter(|value| !value.is_empty());
        let (Some(scene), Some(anchor)) = (scene, anchor) else { continue };
        let claim = format!("{scene}:{anchor}");
        if let Some(previous) = owners.get(&claim) {
            context.issues.push(Issue::new(
                Code::EMountConflict,
                format!("content[{index}].fields.mount"),
                format!("{} and {previous} both claim exclusive mount {claim}", content.key),
                Some(&content.source),
            ));
        } else {
            owners.insert(claim, &content.key);
        }
    }
}

fn infer_capabilities(definition: &WebLuaModDefinition) -> Vec<String> {
    let mut capabilities = BTreeSet::new();
    for asset in &definition.assets {
        capabilities.insert(format!("assets.{}", asset.asset_kind.as_str()));
    }
    for content in &definition.content {
        capabilities.insert(format!("content.{}", content.content_kind.as_str()));
        visit_capabilities(&LuaValue::Object(content.fields.clone()), &mut capabilities);
    }
    if !definition.reducers.is_empty() {
        capabilities.insert("reducers.authority".into());
    }
    for rule in &definition.rules {
        visit_capabilities(&typed_value(rule), &mut capabilities);
    }
    for reducer in &definition.reducers {
        visit_capabilities(&typed_value(&reducer.state), &mut capabilities);
    }
    capabilities.into_iter().collect()
}

fn visit_capabilities(value: &LuaValue, capabilities: &mut BTreeSet<String>) {
    match value {
        LuaValue::Object(object) => {
            for (kind, field, prefix) in [
                ("rule-definition", "operation", ""),
                ("intent-definition", "intentKind", "intent."),
                ("schema-definition", "schemaKind", "schema."),
            ] {
                if schemas::is_token(Some(value), kind)
                    && let Some(value) = object.get(field).and_then(LuaValue::as_str)
                {
                    capabilities.insert(format!("{prefix}{value}"));
                }
            }
            for (_, value) in object.iter() {
                visit_capabilities(value, capabilities);
            }
        }
        LuaValue::Array(values) => {
            for value in values {
                visit_capabilities(value, capabilities);
            }
        }
        _ => {}
    }
}

fn typed_value<T: serde::Serialize>(value: &T) -> LuaValue {
    lua_value(value).expect("typed definitions contain JSON-compatible data")
}
