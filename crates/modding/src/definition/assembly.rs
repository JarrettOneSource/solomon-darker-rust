//! Friendly definition lowering from `web-lua-definition-assembly.ts`.
//! Creation registries remain the source of node identity; explicit lists only
//! change their initial order, and never silently discard unlisted definitions.

use std::collections::{BTreeMap, HashSet};

use solomon_sim::enemies::EnemyToken;
use solomon_sim::modding::definition::*;
use solomon_sim::modding::{LuaObject, LuaValue};

use super::error::{WebLuaDefinitionErrorCode as Code, WebLuaDefinitionIssue as Issue};
use super::schemas::{content_schema, is_token};
use super::suggestions::{did_you_mean, suggest_web_lua_name};

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct WebLuaExplicitLists {
    pub assets: Vec<WebLuaExplicitAsset>,
    pub content: Vec<u32>,
    pub rules: Vec<u32>,
    pub systems: Vec<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct WebLuaExplicitAsset {
    pub key: String,
    pub node: u32,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct WebLuaAssemblyInput {
    pub assets: Vec<WebLuaAssetDefinition>,
    pub content: Vec<WebLuaContentDefinition>,
    pub identity: WebLuaModIdentity,
    pub rules: Vec<WebLuaRuleDefinition>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AssembledWebLuaDefinition {
    pub assets: Vec<WebLuaAssetDefinition>,
    pub content: Vec<WebLuaContentDefinition>,
    pub rules: Vec<WebLuaRuleDefinition>,
}

pub fn assemble_web_lua_definition(
    input: &WebLuaAssemblyInput,
    explicit: Option<&WebLuaExplicitLists>,
    issues: &mut Vec<Issue>,
) -> AssembledWebLuaDefinition {
    Assembly::new(input, issues).build(explicit)
}

struct Assembly<'a> {
    input: &'a WebLuaAssemblyInput,
    issues: &'a mut Vec<Issue>,
    assets: Vec<WebLuaAssetDefinition>,
    asset_keys: BTreeMap<u32, String>,
    assets_by_key: BTreeMap<String, String>,
    assets_by_shape: BTreeMap<String, String>,
    consumed: HashSet<u32>,
    local: Vec<&'a WebLuaContentDefinition>,
}

impl<'a> Assembly<'a> {
    fn new(input: &'a WebLuaAssemblyInput, issues: &'a mut Vec<Issue>) -> Self {
        Self {
            input,
            issues,
            assets: Vec::new(),
            asset_keys: BTreeMap::new(),
            assets_by_key: BTreeMap::new(),
            assets_by_shape: BTreeMap::new(),
            consumed: HashSet::new(),
            local: Vec::new(),
        }
    }

    fn build(mut self, explicit: Option<&WebLuaExplicitLists>) -> AssembledWebLuaDefinition {
        if let Some(explicit) = explicit {
            for asset in &explicit.assets {
                self.declare_asset(asset.node, &asset.key);
            }
        }
        for asset in &self.input.assets {
            self.declare_asset(node(&asset.source), "");
        }
        let mut content_nodes = Vec::new();
        let mut seen = HashSet::new();
        if let Some(explicit) = explicit {
            for &node in &explicit.content {
                if seen.insert(node) {
                    content_nodes.push(node);
                }
            }
        }
        for entry in &self.input.content {
            let node = node(&entry.source);
            if seen.insert(node) {
                content_nodes.push(node);
            }
        }
        for &node in &content_nodes {
            let definition = self.content_node(node);
            if self.local_definition(definition.content_kind, &definition.key).is_none() {
                self.local.push(definition);
            }
        }
        let mut content = Vec::new();
        for node in content_nodes {
            let definition = self.content_node(node).clone();
            content.push(self.lower_content(&definition));
        }
        let mut rules = Vec::new();
        let mut roots = HashSet::new();
        if let Some(explicit) = explicit {
            for &node in &explicit.rules {
                if roots.insert(node) {
                    let rule = self.rule_node(node).clone();
                    rules.push(self.lower_rule(&rule, "rules"));
                }
            }
        }
        for rule in &self.input.rules {
            let node = node(&rule.source);
            if roots.contains(&node)
                || self.consumed.contains(&node)
                || rule.operation != "rules.on"
            {
                continue;
            }
            roots.insert(node);
            rules.push(self.lower_rule(rule, "rules"));
        }
        for rule in &self.input.rules {
            let node = node(&rule.source);
            if roots.contains(&node) || self.consumed.contains(&node) {
                continue;
            }
            let name = rule
                .operation
                .strip_prefix("rules.")
                .map_or_else(|| format!("sd.{}", rule.operation), |name| format!("sd.{name}"));
            self.issue(Code::EGraph, "rules", format!("{name} was created but never attached to anything; put it inside sd.on(event, ...), a potion's on_use, or another rule, or list it under sd.mod rules"), &rule.source);
        }
        AssembledWebLuaDefinition { assets: self.assets, content, rules }
    }

    fn content_node(&self, node: u32) -> &'a WebLuaContentDefinition {
        self.input
            .content
            .iter()
            .find(|entry| entry.source.node == Some(node))
            .expect("sd.mod content nodes belong to this creation registry")
    }

    fn rule_node(&self, node: u32) -> &'a WebLuaRuleDefinition {
        self.input
            .rules
            .iter()
            .find(|entry| entry.source.node == Some(node))
            .expect("sd.mod rule nodes belong to this creation registry")
    }

    fn local_definition(
        &self,
        kind: WebLuaContentKind,
        key: &str,
    ) -> Option<&'a WebLuaContentDefinition> {
        self.local.iter().copied().find(|entry| entry.content_kind == kind && entry.key == key)
    }

    fn reference(&self, target_kind: WebLuaContentKind, key: &str) -> LuaValue {
        let mut reference = LuaObject::new();
        reference.insert("key".into(), LuaValue::String(key.to_owned()));
        reference.insert("kind".into(), LuaValue::String("content-reference".into()));
        reference.insert("modId".into(), LuaValue::String(self.input.identity.id.clone()));
        reference.insert("targetKind".into(), LuaValue::String(target_kind.as_str().into()));
        LuaValue::Object(reference)
    }

    fn union_reference(
        &self,
        value: &str,
        kinds: &[WebLuaContentKind],
        default_kind: WebLuaContentKind,
    ) -> LuaValue {
        for &kind in kinds {
            if self.local_definition(kind, value).is_some() {
                return self.reference(kind, value);
            }
        }
        let candidates = self
            .local
            .iter()
            .filter(|entry| kinds.contains(&entry.content_kind))
            .map(|entry| entry.key.as_str());
        let suggestion = suggest_web_lua_name(value, candidates);
        let target = suggestion
            .and_then(|key| {
                kinds.iter().copied().find(|&kind| self.local_definition(kind, key).is_some())
            })
            .unwrap_or(default_kind);
        self.reference(target, value)
    }

    fn enemy(
        &mut self,
        value: &str,
        path: &str,
        source: &WebLuaDefinitionSource,
        preserve_stock_name: bool,
    ) -> LuaValue {
        if !value.starts_with("stock.")
            && self.local_definition(WebLuaContentKind::Enemy, value).is_some()
        {
            return self.reference(WebLuaContentKind::Enemy, value);
        }
        if let Some(token) = stock_enemy(value) {
            return LuaValue::String(if preserve_stock_name { value } else { token }.to_owned());
        }
        if value.starts_with("stock.") || is_uppercase_token(value) {
            self.unknown_stock_enemy(value, path, source);
            return LuaValue::String(value.to_owned());
        }
        self.reference(WebLuaContentKind::Enemy, value)
    }

    fn declare_auto_asset(
        &mut self,
        kind: WebLuaAssetKind,
        fields: LuaObject,
        source: &WebLuaDefinitionSource,
    ) -> LuaValue {
        let shape = asset_shape(kind, &fields);
        if let Some(key) = self.assets_by_shape.get(&shape) {
            return asset_reference(key);
        }
        let key = self.derive_asset_key(kind, &fields);
        let definition = WebLuaAssetDefinition {
            asset_kind: kind,
            fields,
            key: key.clone(),
            source: without_node(source),
        };
        self.push_asset(definition, shape);
        asset_reference(&key)
    }

    fn declare_asset(&mut self, node: u32, requested_key: &str) {
        if self.asset_keys.contains_key(&node) {
            return;
        }
        let Some(definition) =
            self.input.assets.iter().find(|entry| entry.source.node == Some(node))
        else {
            return;
        };
        let shape = asset_shape(definition.asset_kind, &definition.fields);
        let mut key = if requested_key.is_empty() {
            definition.key.clone()
        } else {
            requested_key.to_owned()
        };
        if key.is_empty() {
            if let Some(existing) = self.assets_by_shape.get(&shape) {
                self.asset_keys.insert(node, existing.clone());
                return;
            }
            key = self.derive_asset_key(definition.asset_kind, &definition.fields);
        } else if let Some(previous) = self.assets_by_key.get(&key) {
            if previous != &shape {
                self.issue(
                    Code::EDuplicate,
                    &format!("assets.{key}"),
                    format!("two different assets use the key {key}; give one of them another key"),
                    &definition.source,
                );
            }
            self.asset_keys.insert(node, key);
            return;
        }
        self.asset_keys.insert(node, key.clone());
        let mut definition = definition.clone();
        definition.key = key;
        self.push_asset(definition, shape);
    }

    fn push_asset(&mut self, definition: WebLuaAssetDefinition, shape: String) {
        self.assets_by_key.insert(definition.key.clone(), shape.clone());
        self.assets_by_shape.entry(shape).or_insert_with(|| definition.key.clone());
        self.assets.push(definition);
    }

    fn derive_asset_key(&self, kind: WebLuaAssetKind, fields: &LuaObject) -> String {
        let path = fields
            .get("image")
            .and_then(LuaValue::as_str)
            .or_else(|| fields.get("path").and_then(LuaValue::as_str))
            .unwrap_or("");
        let basename = path.rsplit('/').next().unwrap_or("");
        let stem = basename.rsplit_once('.').map_or(basename, |(stem, _)| stem);
        let root: String = slug_key(stem).chars().take(96).collect();
        let root = if root.is_empty() { kind.as_str() } else { &root };
        for key in [root.to_owned(), format!("{root}.{}", kind.as_str())] {
            if !self.assets_by_key.contains_key(&key) {
                return key;
            }
        }
        for ordinal in 2_u64.. {
            let key = format!("{root}.{}.{ordinal}", kind.as_str());
            if !self.assets_by_key.contains_key(&key) {
                return key;
            }
        }
        unreachable!("the finite definition budget leaves an unused asset key")
    }

    fn asset_token_reference(
        &mut self,
        token: &LuaValue,
        path: &str,
        source: &WebLuaDefinitionSource,
    ) -> LuaValue {
        let key =
            token.get("source").and_then(source_node).and_then(|node| self.asset_keys.get(&node));
        if let Some(key) = key {
            return asset_reference(key);
        }
        self.issue(
            Code::EReference,
            path,
            "this asset was not created by sd.art inside this script",
            source,
        );
        asset_reference(token.get("key").and_then(LuaValue::as_str).unwrap_or(""))
    }

    fn content_token_reference(
        &mut self,
        token: &LuaValue,
        path: &str,
        source: &WebLuaDefinitionSource,
    ) -> LuaValue {
        let kind =
            token.get("contentKind").and_then(LuaValue::as_str).and_then(WebLuaContentKind::parse);
        let key = token.get("key").and_then(LuaValue::as_str);
        if let (Some(kind), Some(key)) = (kind, key) {
            return self.reference(kind, key);
        }
        self.issue(
            Code::EReference,
            path,
            "this content token was not created by sd.kit inside this script",
            source,
        );
        LuaValue::Null
    }

    fn lower_content(&mut self, definition: &WebLuaContentDefinition) -> WebLuaContentDefinition {
        let kind = definition.content_kind;
        let path = format!("content.{}", definition.key);
        let mut fields = definition.fields.clone();
        let (schema_fields, _) = content_schema(kind);
        let mut art = fields.get("art").and_then(LuaValue::as_object).cloned().unwrap_or_default();
        let mut art_touched = false;
        for &slot in kind.art_slots() {
            if schema_fields.contains(&slot) || fields.get(slot).is_none() {
                continue;
            }
            if art.get(slot).is_some() {
                self.issue(
                    Code::ESchema,
                    &format!("{path}.{slot}"),
                    format!("{slot} is set both at the top level and inside art; keep one of them"),
                    &definition.source,
                );
            }
            art.insert(slot.to_owned(), fields.remove(slot).expect("shorthand art field exists"));
            art_touched = true;
        }
        if kind == WebLuaContentKind::Boneyard
            && let Some(source) = fields.get("source").and_then(LuaValue::as_str)
            && art.get("layout").is_none()
        {
            art.insert("layout".into(), LuaValue::String(source.to_owned()));
            art_touched = true;
        }
        if art_touched && fields.get("art").is_some_and(|value| value.as_object().is_none()) {
            self.issue(
                Code::ESchema,
                &format!("{path}.art"),
                "art must be a table when shorthand art fields such as icon or sound are also used",
                &definition.source,
            );
        }
        if art_touched || fields.get("art").and_then(LuaValue::as_object).is_some() {
            let previous = art.into_fields();
            art = LuaObject::new();
            for (slot, value) in previous {
                let slot_path = format!("{path}.art.{slot}");
                let lowered = match &value {
                    LuaValue::String(value) => {
                        self.art_asset_from_path(kind, &slot, value, &slot_path, &definition.source)
                    }
                    _ if is_token(Some(&value), "asset-definition") => {
                        Some(self.asset_token_reference(&value, &slot_path, &definition.source))
                    }
                    _ => None,
                };
                art.insert(slot, lowered.unwrap_or(value));
            }
            fields.insert("art".into(), LuaValue::Object(art));
        }
        let owner = WalkOwner::Content(kind);
        let mut lowered = LuaObject::new();
        for (field, value) in fields.iter() {
            let value = if field == "art" {
                value.clone()
            } else {
                self.lower_value(value, field, &owner, &path, &definition.source)
            };
            lowered.insert(field.to_owned(), value);
        }
        if kind == WebLuaContentKind::Potion
            && let Some(status) =
                lowered.get("status").filter(|value| is_token(Some(value), "content-reference"))
            && status.get("targetKind").and_then(LuaValue::as_str) == Some("status")
            && let Some(key) = status.get("key").and_then(LuaValue::as_str)
        {
            let status = status.clone();
            let duration = self
                .local_definition(WebLuaContentKind::Status, key)
                .and_then(|entry| entry.fields.get("duration"))
                .cloned();
            if lowered.get("on_use").is_none() {
                let mut fields = LuaObject::new();
                fields.insert("status".into(), status);
                fields.insert("target".into(), LuaValue::String("user".into()));
                lowered.insert(
                    "on_use".into(),
                    rule_value(&synthetic_rule("effect.status", fields, &definition.source)),
                );
            }
            if lowered.get("duration").is_none()
                && let Some(duration) = duration
            {
                lowered.insert("duration".into(), duration);
            }
        }
        WebLuaContentDefinition {
            content_kind: kind,
            fields: lowered,
            key: definition.key.clone(),
            source: definition.source.clone(),
        }
    }

    fn art_asset_from_path(
        &mut self,
        kind: WebLuaContentKind,
        slot: &str,
        value: &str,
        path: &str,
        source: &WebLuaDefinitionSource,
    ) -> Option<LuaValue> {
        if slot == "atlas" {
            self.issue(Code::ESchema, path, format!("{} atlas needs sd.sheet(path, {{frame = {{width = ..., height = ...}}, animations = {{...}}}}) so the game knows its animation frames", kind.as_str()), source);
            return None;
        }
        let mut fields = LuaObject::new();
        let asset_kind = match slot {
            "worn" | "worn_trim" => {
                fields.insert(
                    "animations".into(),
                    object([("wearable", LuaValue::Array(vec![LuaValue::Number(1.0)]))]),
                );
                fields.insert(
                    "frame".into(),
                    object([
                        ("height", LuaValue::Number(170.0)),
                        ("width", LuaValue::Number(170.0)),
                    ]),
                );
                fields.insert("image".into(), LuaValue::String(value.to_owned()));
                WebLuaAssetKind::Sheet
            }
            _ => {
                fields.insert("path".into(), LuaValue::String(value.to_owned()));
                match slot {
                    "sound" | "attack_sound" | "death_sound" => WebLuaAssetKind::Sound,
                    "ambience" | "loop" | "music" => WebLuaAssetKind::Music,
                    "layout" if value.ends_with(".boneyard") => WebLuaAssetKind::Boneyard,
                    "layout" => WebLuaAssetKind::Scene,
                    _ => WebLuaAssetKind::Sprite,
                }
            }
        };
        Some(self.declare_auto_asset(asset_kind, fields, source))
    }

    fn lower_rule(&mut self, rule: &WebLuaRuleDefinition, path: &str) -> WebLuaRuleDefinition {
        if let Some(node) = rule.source.node {
            self.consumed.insert(node);
        }
        let owner = WalkOwner::Rule(&rule.operation);
        let mut fields = LuaObject::new();
        for (field, value) in rule.fields.iter() {
            fields.insert(
                field.to_owned(),
                self.lower_value(value, field, &owner, path, &rule.source),
            );
        }
        WebLuaRuleDefinition {
            fields,
            operation: rule.operation.clone(),
            source: rule.source.clone(),
        }
    }

    fn lower_value(
        &mut self,
        value: &LuaValue,
        slot: &str,
        owner: &WalkOwner<'_>,
        path: &str,
        source: &WebLuaDefinitionSource,
    ) -> LuaValue {
        let location = format!("{path}.{slot}");
        if is_token(Some(value), "content-definition") {
            return self.content_token_reference(value, &location, source);
        }
        if is_token(Some(value), "asset-definition") {
            return self.asset_token_reference(value, &location, source);
        }
        if is_token(Some(value), "rule-definition") {
            let rule = source_rule(value);
            return rule_value(&self.lower_rule(&rule, &location));
        }
        match value {
            LuaValue::String(value) => self
                .lower_string(value, slot, owner, &location, source)
                .unwrap_or_else(|| LuaValue::String(value.clone())),
            LuaValue::Array(values) => {
                if owner.single_rule_field(slot)
                    && !values.is_empty()
                    && values.iter().all(|value| is_token(Some(value), "rule-definition"))
                {
                    let mut fields = LuaObject::new();
                    fields.insert("nodes".into(), LuaValue::Array(values.clone()));
                    let rule = synthetic_rule("rules.all", fields, source);
                    return rule_value(&self.lower_rule(&rule, &location));
                }
                let mut lowered = Vec::new();
                for value in values {
                    lowered.push(self.lower_value(
                        value,
                        &format!("{slot}[]"),
                        owner,
                        path,
                        source,
                    ));
                }
                LuaValue::Array(lowered)
            }
            LuaValue::Object(fields) => {
                let mut lowered = LuaObject::new();
                for (key, value) in fields.iter() {
                    lowered.insert(
                        key.to_owned(),
                        self.lower_value(value, &format!("{slot}.{key}"), owner, path, source),
                    );
                }
                LuaValue::Object(lowered)
            }
            _ => value.clone(),
        }
    }

    fn lower_string(
        &mut self,
        value: &str,
        slot: &str,
        owner: &WalkOwner<'_>,
        path: &str,
        source: &WebLuaDefinitionSource,
    ) -> Option<LuaValue> {
        use WebLuaContentKind as Kind;
        let target = match (owner, slot) {
            (WalkOwner::Content(Kind::AffixPool), "entries[].affix") => Some(Kind::Affix),
            (WalkOwner::Content(Kind::Potion), "status") => Some(Kind::Status),
            (WalkOwner::Content(Kind::Scene), "rooms[]") => Some(Kind::Room),
            (WalkOwner::Content(Kind::Shop), "services[].pool") => Some(Kind::AffixPool),
            (WalkOwner::Content(Kind::Skill), "parent" | "prerequisites[]") => Some(Kind::Skill),
            (WalkOwner::Rule("effect.status"), "status") => Some(Kind::Status),
            (WalkOwner::Rule("effect.spawn"), "content") => Some(Kind::Powerup),
            (WalkOwner::Rule("prefab.portal"), "destination") => Some(Kind::Scene),
            _ => None,
        };
        if let Some(target) = target {
            return Some(self.reference(target, value));
        }
        match (owner, slot) {
            (WalkOwner::Content(Kind::Boneyard), "roster[]" | "waves[].roster[]") => {
                Some(self.enemy(value, path, source, true))
            }
            (WalkOwner::Content(Kind::Shop), "stock[].item")
            | (WalkOwner::Rule("effect.grant"), "item") => {
                Some(self.union_reference(value, &[Kind::Item, Kind::Potion], Kind::Item))
            }
            (
                WalkOwner::Content(Kind::Skill),
                "grants[]" | "ranks[].grant" | "ranks[].grants[]",
            ) => Some(self.union_reference(value, &[Kind::Spell, Kind::Ui], Kind::Spell)),
            (WalkOwner::Rule("effect.spawn"), "enemy") => {
                Some(self.enemy(value, path, source, false))
            }
            (WalkOwner::Rule("effect.spawn"), "token") => {
                if let Some(token) = stock_enemy(value) {
                    return Some(LuaValue::String(token.to_owned()));
                }
                self.unknown_stock_enemy(value, path, source);
                Some(LuaValue::String(value.to_owned()))
            }
            (WalkOwner::Rule("effect.present"), "sound") => {
                let mut fields = LuaObject::new();
                fields.insert("path".into(), LuaValue::String(value.to_owned()));
                Some(self.declare_auto_asset(WebLuaAssetKind::Sound, fields, source))
            }
            _ => None,
        }
    }

    fn unknown_stock_enemy(&mut self, value: &str, path: &str, source: &WebLuaDefinitionSource) {
        let mut candidates = Vec::new();
        for (key, token) in STOCK_ENEMIES {
            candidates.extend([format!("stock.{key}"), key.to_owned(), token.id().to_owned()]);
        }
        let hint = did_you_mean(value, candidates.iter().map(String::as_str));
        self.issue(Code::EReference, path, format!("unknown stock enemy {value}{hint}"), source);
    }

    fn issue(
        &mut self,
        code: Code,
        path: &str,
        message: impl Into<String>,
        source: &WebLuaDefinitionSource,
    ) {
        self.issues.push(Issue::new(code, path, message, Some(&without_node(source))));
    }
}

enum WalkOwner<'a> {
    Content(WebLuaContentKind),
    Rule(&'a str),
}

impl WalkOwner<'_> {
    fn single_rule_field(&self, slot: &str) -> bool {
        matches!(
            (self, slot),
            (Self::Content(WebLuaContentKind::Item), "use")
                | (Self::Content(WebLuaContentKind::Potion), "on_use")
                | (Self::Content(WebLuaContentKind::Powerup), "effect")
                | (Self::Rule("rules.after" | "rules.every" | "rules.on"), "node")
                | (Self::Rule("rules.when"), "no" | "yes")
        )
    }
}

fn node(source: &WebLuaDefinitionSource) -> u32 {
    source.node.expect("creation registry definitions have a source node")
}

fn source_node(source: &LuaValue) -> Option<u32> {
    source.get("node").and_then(LuaValue::as_number).map(|value| value as u32)
}

pub(super) fn without_node(source: &WebLuaDefinitionSource) -> WebLuaDefinitionSource {
    WebLuaDefinitionSource {
        column: source.column,
        file: source.file.clone(),
        line: source.line,
        node: None,
    }
}

fn synthetic_rule(
    operation: &str,
    fields: LuaObject,
    source: &WebLuaDefinitionSource,
) -> WebLuaRuleDefinition {
    WebLuaRuleDefinition { fields, operation: operation.to_owned(), source: without_node(source) }
}

pub(super) fn rule_value(rule: &WebLuaRuleDefinition) -> LuaValue {
    object([
        ("fields", LuaValue::Object(rule.fields.clone())),
        ("kind", LuaValue::String("rule-definition".into())),
        ("operation", LuaValue::String(rule.operation.clone())),
        (
            "source",
            crate::json::lua_value(&rule.source).expect("source location is JSON-compatible"),
        ),
    ])
}

fn source_rule(value: &LuaValue) -> WebLuaRuleDefinition {
    let source = value
        .get("source")
        .and_then(LuaValue::as_object)
        .map(|source| {
            // Lua's source node is a Number. A JSON round trip spells it as
            // `4.0`, which serde correctly refuses as a u32 and would lose the
            // registration identity used by child-rule consumption.
            let integer = |key| {
                source
                    .get(key)
                    .and_then(LuaValue::as_number)
                    .filter(|value| {
                        value.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(value)
                    })
                    .map(|value| value as u32)
            };
            WebLuaDefinitionSource {
                column: integer("column"),
                file: source.get("file").and_then(LuaValue::as_str).unwrap_or("").to_owned(),
                line: integer("line"),
                node: integer("node"),
            }
        })
        .unwrap_or(WebLuaDefinitionSource {
            column: Some(0),
            file: String::new(),
            line: Some(0),
            node: None,
        });
    WebLuaRuleDefinition {
        fields: value.get("fields").and_then(LuaValue::as_object).cloned().unwrap_or_default(),
        operation: value.get("operation").and_then(LuaValue::as_str).unwrap_or("").to_owned(),
        source,
    }
}

fn asset_reference(key: &str) -> LuaValue {
    object([
        ("key", LuaValue::String(key.to_owned())),
        ("kind", LuaValue::String("asset-reference".into())),
    ])
}

fn object<const N: usize>(fields: [(&str, LuaValue); N]) -> LuaValue {
    LuaValue::Object(fields.into_iter().map(|(key, value)| (key.to_owned(), value)).collect())
}

fn asset_shape(kind: WebLuaAssetKind, fields: &LuaObject) -> String {
    format!("{}\n{}", kind.as_str(), canonical_text(&LuaValue::Object(fields.clone())))
}

fn canonical_text(value: &LuaValue) -> String {
    match value {
        LuaValue::Array(values) => {
            format!("[{}]", values.iter().map(canonical_text).collect::<Vec<_>>().join(","))
        }
        LuaValue::Object(fields) => {
            let mut entries: Vec<_> = fields.iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.encode_utf16().cmp(right.encode_utf16()));
            let mut pairs = Vec::new();
            for (key, value) in entries {
                pairs.push(format!(
                    "{}:{}",
                    serde_json::to_string(key).expect("a string serializes"),
                    canonical_text(value)
                ));
            }
            format!("{{{}}}", pairs.join(","))
        }
        _ => crate::json::stringify(value).expect("cloned definition values are finite"),
    }
}

pub(super) fn slug_key(value: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for character in value.to_lowercase().chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            if separator && !result.is_empty() {
                result.push('_');
            }
            result.push(character);
            separator = false;
        } else {
            separator = true;
        }
    }
    result
}

fn is_uppercase_token(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

const STOCK_ENEMIES: [(&str, EnemyToken); 11] = [
    ("coffin", EnemyToken::Coffin),
    ("demon_skull", EnemyToken::DemonSkull),
    ("demon", EnemyToken::Demon),
    ("dire_faculty", EnemyToken::DireFaculty),
    ("heartmonger", EnemyToken::Heartmonger),
    ("imp", EnemyToken::Imp),
    ("skeleton", EnemyToken::Skeleton),
    ("skeleton_archer", EnemyToken::SkeletonArcher),
    ("skeleton_mage", EnemyToken::SkeletonMage),
    ("wraith", EnemyToken::Wraith),
    ("zombie", EnemyToken::Zombie),
];

fn stock_enemy(value: &str) -> Option<&'static str> {
    for (key, token) in STOCK_ENEMIES {
        if value == key || value == token.id() || value.strip_prefix("stock.") == Some(key) {
            return Some(token.id());
        }
    }
    None
}
