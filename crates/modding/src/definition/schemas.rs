//! Authored definition schemas from `web-lua-definition-schemas.ts`.
//! Validation appends all source issues in source traversal order.

use std::collections::HashSet;

use solomon_sim::modding::definition::*;
use solomon_sim::modding::{LuaObject, LuaValue};

use super::error::{WebLuaDefinitionErrorCode as Code, WebLuaDefinitionIssue as Issue};
use super::suggestions::{did_you_mean, list_choices};

pub fn content_schema(
    kind: WebLuaContentKind,
) -> (&'static [&'static str], &'static [&'static str]) {
    use WebLuaContentKind::*;
    match kind {
        Affix => (&["applies_to", "modifiers"], &["name", "modifiers"]),
        AffixPool => (&["applies_to", "entries", "rng_domain", "rolls"], &["entries"]),
        Boneyard => (
            &["anchors", "art", "environment", "roster", "source", "triggers", "waves"],
            &["name", "source"],
        ),
        Boast => (
            &[
                "art",
                "fail_on",
                "instruction",
                "random_skill_choices",
                "response",
                "score_multiplier",
                "statement",
                "stock_icon",
                "success_wave",
            ],
            &["instruction", "name", "response", "statement"],
        ),
        Enemy => (&["art", "loot", "stats"], &["name"]),
        Item => (&["art", "equipment", "stack", "use"], &["name"]),
        Potion => (
            &["art", "duration", "loot", "on_use", "presentation", "status"],
            &["duration", "name", "on_use"],
        ),
        Powerup => (&["art", "effect", "pickup"], &["effect", "name"]),
        Room => (&["art", "geometry", "props"], &["geometry"]),
        Scene => (&["art", "rooms"], &["rooms"]),
        SceneExtension => (&["features", "scene"], &["features", "scene"]),
        Shop => (
            &["art", "mount", "npc", "restock", "services", "stock", "stock_scope"],
            &["name", "stock"],
        ),
        Skill => (
            &["art", "grants", "maximum_rank", "offer", "parent", "prerequisites", "ranks"],
            &["name", "ranks"],
        ),
        Spell => (&["art", "behavior", "cooldown", "mana", "slot"], &["behavior", "name", "slot"]),
        Status => (&["duration", "modifiers", "stacking"], &[]),
        Ui => (
            &["accessible_name", "actions", "bindings", "mount", "view", "visible"],
            &["mount", "view"],
        ),
    }
}

pub fn validate_asset(definition: &WebLuaAssetDefinition, index: usize, issues: &mut Vec<Issue>) {
    let allowed: &[&str] = match definition.asset_kind {
        WebLuaAssetKind::Boneyard | WebLuaAssetKind::Scene => &["path"],
        WebLuaAssetKind::Music => &["bus", "loop", "path", "volume"],
        WebLuaAssetKind::Sheet => &["animations", "frame", "headings", "image"],
        WebLuaAssetKind::Sound => &["bus", "path", "volume"],
        WebLuaAssetKind::Sprite => &["frame", "frames", "path"],
    };
    let path = format!("assets[{index}].fields");
    let source = Some(&definition.source);
    validate_unknown_fields(&definition.fields, allowed, &path, source, issues);
    let file_key = if definition.asset_kind == WebLuaAssetKind::Sheet { "image" } else { "path" };
    if definition.fields.get(file_key).and_then(LuaValue::as_str).is_none_or(str::is_empty) {
        issues.push(Issue::new(
            Code::EAsset,
            path,
            format!("{} asset requires an owned package path", definition.asset_kind.as_str()),
            source,
        ));
    }
}

pub fn validate_content(
    definition: &WebLuaContentDefinition,
    index: usize,
    issues: &mut Vec<Issue>,
) {
    let fields = &definition.fields;
    let source = Some(&definition.source);
    let (allowed, required) = content_schema(definition.content_kind);
    let mut allowed = allowed.to_vec();
    allowed.extend(["description", "name"]);
    let path = format!("content[{index}].fields");
    validate_unknown_fields(fields, &allowed, &path, source, issues);
    require_fields(fields, required, definition.content_kind.as_str(), &path, source, issues);
    for (field, maximum) in [("name", 128), ("description", 1_024)] {
        if let Some(value) = fields.get(field) {
            validate_text(Some(value), &format!("{path}.{field}"), source, maximum, issues);
        }
    }
    if definition.content_kind == WebLuaContentKind::Boast {
        validate_boast(fields, &path, source, issues);
    }
    for field in ["duration", "cooldown"] {
        if let Some(value) = fields.get(field) {
            validate_duration(Some(value), &format!("{path}.{field}"), source, issues);
        }
    }
    if let Some(stacking) = fields.get("stacking")
        && !stacking
            .as_str()
            .is_some_and(|value| ["ignore", "refresh", "replace", "stack"].contains(&value))
    {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.stacking"),
            "expected \"refresh\", \"stack\", \"replace\", or \"ignore\"",
            source,
        ));
    }
    if let Some(loot) = fields.get("loot").and_then(LuaValue::as_object) {
        for field in ["ordinary", "boss"] {
            if let Some(value) = loot.get(field)
                && !value
                    .as_number()
                    .is_some_and(|value| value.is_finite() && (0.0..=1.0).contains(&value))
            {
                issues.push(Issue::new(
                    Code::ESchema,
                    format!("{path}.loot.{field}"),
                    "expected a probability from 0 through 1",
                    source,
                ));
            }
        }
    }
}

fn validate_boast(
    fields: &LuaObject,
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    issues: &mut Vec<Issue>,
) {
    for field in ["instruction", "response", "statement"] {
        validate_text(fields.get(field), &format!("{path}.{field}"), source, 1_024, issues);
    }
    let art = fields.get("art").and_then(LuaValue::as_object);
    let custom_icon = art.and_then(|art| art.get("icon")).is_some_and(truthy);
    if art.is_some_and(|art| art.iter().any(|(field, _)| field != "icon")) {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.art"),
            "boast art supports only the icon slot",
            source,
        ));
    }
    if fields.get("stock_icon").is_none() == !custom_icon {
        issues.push(Issue::new(
            Code::ESchema,
            path,
            "boast requires exactly one of stock_icon or art.icon",
            source,
        ));
    }
    if let Some(value) = fields.get("stock_icon")
        && !safe_integer_in(Some(value), 0.0, 7.0)
    {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.stock_icon"),
            "boast stock_icon must be an integer within 0..7",
            source,
        ));
    }
    if let Some(value) = fields.get("fail_on") {
        let producers = ["magical-equipment", "mana-underflow", "potion-use", "secondary-cast"];
        let valid = value.as_array().is_some_and(|values| {
            let mut seen = HashSet::new();
            values.len() <= 4
                && values.iter().all(|value| {
                    value
                        .as_str()
                        .is_some_and(|value| producers.contains(&value) && seen.insert(value))
                })
        });
        if !valid {
            issues.push(Issue::new(
                Code::ESchema,
                format!("{path}.fail_on"),
                "boast fail_on contains unsupported or duplicate producers",
                source,
            ));
        }
    }
    if fields.get("random_skill_choices").is_some_and(|value| value.as_bool().is_none()) {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.random_skill_choices"),
            "boast random_skill_choices must be boolean",
            source,
        ));
    }
    if let Some(value) = fields.get("success_wave")
        && !safe_integer_in(Some(value), 1.0, 10_000.0)
    {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.success_wave"),
            "boast success_wave must be an integer within 1..10000",
            source,
        ));
    }
    if fields.get("score_multiplier").is_some_and(|value| {
        !value.as_number().is_some_and(|value| value.is_finite() && (1.0..=10.0).contains(&value))
    }) {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.score_multiplier"),
            "boast score_multiplier must be finite within 1..10",
            source,
        ));
    }
}

pub fn validate_nodes(definition: &WebLuaModDefinition, issues: &mut Vec<Issue>) {
    for (index, entry) in definition.content.iter().enumerate() {
        visit_nodes(
            &LuaValue::Object(entry.fields.clone()),
            &format!("content[{index}].fields"),
            definition,
            issues,
        );
    }
    for (index, rule) in definition.rules.iter().enumerate() {
        visit_nodes(
            &crate::json::lua_value(rule).expect("a typed rule is JSON-compatible"),
            &format!("rules[{index}]"),
            definition,
            issues,
        );
    }
    for (index, reducer) in definition.reducers.iter().enumerate() {
        visit_nodes(
            &crate::json::lua_value(&reducer.state).expect("a typed schema is JSON-compatible"),
            &format!("reducers[{index}].state"),
            definition,
            issues,
        );
    }
}

fn visit_nodes(
    value: &LuaValue,
    path: &str,
    definition: &WebLuaModDefinition,
    issues: &mut Vec<Issue>,
) {
    match value {
        LuaValue::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                visit_nodes(value, &format!("{path}[{index}]"), definition, issues);
            }
        }
        LuaValue::Object(object) => {
            let source = object.get("source").and_then(source_value);
            let fields = object.get("fields").and_then(LuaValue::as_object);
            match object.get("kind").and_then(LuaValue::as_str) {
                Some("rule-definition") => {
                    if let (Some(fields), Some(operation)) =
                        (fields, object.get("operation").and_then(LuaValue::as_str))
                    {
                        validate_rule(operation, fields, path, source.as_ref(), issues);
                        if operation == "effect.present"
                            && let Some(sound) = fields
                                .get("sound")
                                .filter(|value| is_token(Some(value), "asset-reference"))
                            && let Some(key) = sound.get("key").and_then(LuaValue::as_str)
                        {
                            let kind = definition
                                .assets
                                .iter()
                                .rev()
                                .find(|asset| asset.key == key)
                                .map(|asset| asset.asset_kind);
                            if kind != Some(WebLuaAssetKind::Sound) {
                                issues.push(Issue::new(
                                    Code::ESchema,
                                    format!("{path}.fields.sound"),
                                    "effect.present requires an sd.art.sound reference",
                                    source.as_ref(),
                                ));
                            }
                        }
                    }
                }
                Some("schema-definition") => {
                    if let (Some(fields), Some(kind)) =
                        (fields, object.get("schemaKind").and_then(LuaValue::as_str))
                    {
                        validate_schema(kind, fields, path, source.as_ref(), issues);
                    }
                }
                Some("intent-definition") => {
                    if let (Some(fields), Some(kind)) =
                        (fields, object.get("intentKind").and_then(LuaValue::as_str))
                    {
                        validate_rule(
                            &format!("effect.{kind}"),
                            fields,
                            path,
                            source.as_ref(),
                            issues,
                        );
                    }
                }
                _ => {}
            }
            for (key, child) in object.iter() {
                visit_nodes(child, &format!("{path}.{key}"), definition, issues);
            }
        }
        _ => {}
    }
}

fn validate_rule(
    operation: &str,
    fields: &LuaObject,
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    issues: &mut Vec<Issue>,
) {
    let (allowed, required): (&[&str], &[&str]) = match operation {
        "rules.on" => (&["event", "node"], &["event", "node"]),
        "rules.all" | "rules.first" => (&["nodes"], &["nodes"]),
        "rules.when" => (&["no", "predicate", "yes"], &["predicate", "yes"]),
        "rules.after" => (&["duration", "node"], &["duration", "node"]),
        "rules.every" => (&["interval", "node", "times"], &["interval", "node", "times"]),
        "prefab.area" => (
            &["duration", "effects", "every", "radius"],
            &["duration", "effects", "every", "radius"],
        ),
        "prefab.channel" => {
            (&["duration", "effects", "every", "width"], &["duration", "effects", "every", "width"])
        }
        "prefab.projectile" => (
            &["duration", "effects", "radius", "speed"],
            &["duration", "effects", "radius", "speed"],
        ),
        "prefab.minimap" => (&["layers", "range", "size"], &["range", "size"]),
        "prefab.portal" => {
            (&["destination", "policy", "prompt", "selector"], &["destination", "selector"])
        }
        "effect.damage" => (&["amount", "modifier", "target"], &["amount", "target"]),
        "effect.resource" => (&["experience", "gold", "health", "mana", "target"], &["target"]),
        "effect.status" => (&["status", "target"], &["status", "target"]),
        "effect.spawn" => (&["content", "enemy", "token", "x", "y"], &[]),
        "effect.grant" => (&["item", "quantity", "target"], &["item", "target"]),
        "effect.state" => (&["clear", "key", "value"], &["key"]),
        "effect.present" => (&["sound"], &["sound"]),
        "effect.input" => (&["aim", "movement", "primary", "quickbar", "release"], &[]),
        "effect.select_skill" => (
            &["choice_index", "offer_sequence", "skill_id"],
            &["choice_index", "offer_sequence", "skill_id"],
        ),
        _ => {
            issues.push(Issue::new(
                Code::ESchema,
                path,
                format!("unsupported Web Lua rule operation: {operation}"),
                source,
            ));
            return;
        }
    };
    let fields_path = format!("{path}.fields");
    validate_unknown_fields(fields, allowed, &fields_path, source, issues);
    require_fields(fields, required, operation, &fields_path, source, issues);
    match operation {
        "rules.on" => {
            require_text(fields.get("event"), &format!("{fields_path}.event"), source, issues);
            validate_event(fields.get("event"), &format!("{fields_path}.event"), source, issues);
            require_rule(fields.get("node"), &format!("{fields_path}.node"), source, issues);
        }
        "rules.all" | "rules.first" => require_rules(
            fields.get("nodes"),
            &format!("{fields_path}.nodes"),
            source,
            false,
            issues,
        ),
        "rules.when" => {
            validate_predicate(
                fields.get("predicate"),
                &format!("{fields_path}.predicate"),
                source,
                0,
                issues,
            );
            require_rule(fields.get("yes"), &format!("{fields_path}.yes"), source, issues);
            if let Some(value) = fields.get("no") {
                require_rule(Some(value), &format!("{fields_path}.no"), source, issues);
            }
        }
        "rules.after" | "rules.every" => {
            let duration = if operation == "rules.after" { "duration" } else { "interval" };
            validate_duration(
                fields.get(duration),
                &format!("{fields_path}.{duration}"),
                source,
                issues,
            );
            require_rule(fields.get("node"), &format!("{fields_path}.node"), source, issues);
            if operation == "rules.every" && !safe_integer_in(fields.get("times"), 1.0, 1_024.0) {
                issues.push(Issue::new(
                    Code::ESchema,
                    format!("{fields_path}.times"),
                    "rules.every times must be an integer within 1..1024",
                    source,
                ));
            }
        }
        "prefab.area" | "prefab.channel" | "prefab.projectile" => {
            for field in ["duration", "every"] {
                if let Some(value) = fields.get(field) {
                    validate_duration(
                        Some(value),
                        &format!("{fields_path}.{field}"),
                        source,
                        issues,
                    );
                }
            }
            for field in ["radius", "speed", "width"] {
                if fields.get(field).is_some_and(|value| !positive(Some(value))) {
                    issues.push(Issue::new(
                        Code::ESchema,
                        format!("{fields_path}.{field}"),
                        format!("{operation} {field} must be positive"),
                        source,
                    ));
                }
            }
            require_rules(
                fields.get("effects"),
                &format!("{fields_path}.effects"),
                source,
                true,
                issues,
            );
        }
        "prefab.minimap" => validate_minimap(fields, &fields_path, source, issues),
        "prefab.portal" => validate_portal(fields, &fields_path, source, issues),
        _ => validate_effect(operation, fields, &fields_path, source, issues),
    }
}

fn validate_effect(
    operation: &str,
    fields: &LuaObject,
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    issues: &mut Vec<Issue>,
) {
    let (required_one, message): (&[&str], &str) = match operation {
        "effect.resource" => {
            (&["experience", "gold", "health", "mana"], "effect.resource requires a resource value")
        }
        "effect.spawn" => {
            (&["content", "enemy", "token"], "effect.spawn requires content, enemy, or token")
        }
        _ => (&[], ""),
    };
    if !required_one.is_empty() && !required_one.iter().any(|field| fields.get(field).is_some()) {
        issues.push(Issue::new(Code::ESchema, path, message, source));
    }
    if operation == "effect.state"
        && fields.get("value").is_some()
            == (fields.get("clear").and_then(LuaValue::as_bool) == Some(true))
    {
        issues.push(Issue::new(
            Code::ESchema,
            path,
            "effect.state requires exactly one of value or clear = true",
            source,
        ));
    }
    if operation == "effect.damage" && !positive(fields.get("amount")) {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.amount"),
            "effect.damage amount must be positive",
            source,
        ));
    }
    for (owner, field, kinds, message) in [
        ("effect.status", "status", &["status"][..], "effect.status requires a status reference"),
        (
            "effect.grant",
            "item",
            &["item", "potion"][..],
            "effect.grant requires an item or potion reference",
        ),
    ] {
        if operation == owner && !is_reference(fields.get(field), kinds) {
            issues.push(Issue::new(Code::ESchema, format!("{path}.{field}"), message, source));
        }
    }
    if operation == "effect.present" && !is_token(fields.get("sound"), "asset-reference") {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.sound"),
            "effect.present requires a sound asset reference",
            source,
        ));
    }
    if operation == "effect.spawn" {
        if fields.get("content").is_some() && !is_reference(fields.get("content"), &["powerup"]) {
            issues.push(Issue::new(
                Code::ESchema,
                format!("{path}.content"),
                "effect.spawn content requires a powerup reference",
                source,
            ));
        }
        if let Some(enemy) = fields.get("enemy")
            && enemy.as_str().is_none()
            && !is_reference(Some(enemy), &["enemy"])
        {
            issues.push(Issue::new(
                Code::ESchema,
                format!("{path}.enemy"),
                "effect.spawn enemy requires an enemy reference or stock token",
                source,
            ));
        }
    }
}

fn validate_schema(
    kind: &str,
    fields: &LuaObject,
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    issues: &mut Vec<Issue>,
) {
    let allowed: &[&str] = match kind {
        "array" => &["item", "max_items"],
        "boolean" => &["default"],
        "enum" => &["values"],
        "integer" | "number" => &["default", "max", "min"],
        "object" => &["fields"],
        "string" => &["default", "max_bytes"],
        _ => {
            issues.push(Issue::new(
                Code::ESchema,
                path,
                format!("unsupported schema kind: {kind}"),
                source,
            ));
            return;
        }
    };
    validate_unknown_fields(fields, allowed, &format!("{path}.fields"), source, issues);
    let invalid = match kind {
        "array" => !is_token(fields.get("item"), "schema-definition"),
        "object" => fields.get("fields").and_then(LuaValue::as_object).is_none(),
        "enum" => {
            fields.get("values").and_then(LuaValue::as_array).is_none_or(<[LuaValue]>::is_empty)
        }
        _ => false,
    };
    if invalid {
        let field = match kind {
            "array" => "item",
            "object" => "fields",
            _ => "values",
        };
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.fields.{field}"),
            format!("{kind} schema requires {field}"),
            source,
        ));
    }
}

fn validate_predicate(
    value: Option<&LuaValue>,
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    depth: usize,
    issues: &mut Vec<Issue>,
) {
    if value.and_then(LuaValue::as_bool).is_some() {
        return;
    }
    let Some(fields) = value.and_then(LuaValue::as_object) else {
        issues.push(Issue::new(Code::ESchema, path, "rules.when predicate must be true, false, or a table such as {context = \"wave\", at_least = 5}", source));
        return;
    };
    if depth > 8 {
        issues.push(Issue::new(Code::ESchema, path, "predicate nests too deeply", source));
        return;
    }
    const NUMERIC: [&str; 4] = ["above", "at_least", "at_most", "below"];
    const COMPARISONS: [&str; 6] =
        ["above", "at_least", "at_most", "below", "equals", "not_equals"];
    const GROUPS: [&str; 3] = ["all", "any", "none"];
    validate_unknown_fields(fields, &WEB_LUA_PREDICATE_FIELDS, path, source, issues);
    let has_event = fields.get("event").is_some();
    let has_context = fields.get("context").is_some();
    let groups: Vec<_> = GROUPS.into_iter().filter(|field| fields.get(field).is_some()).collect();
    if has_event {
        validate_event(fields.get("event"), &format!("{path}.event"), source, issues);
    }
    if has_context && fields.get("context").and_then(LuaValue::as_str).is_none_or(str::is_empty) {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.context"),
            "context must be nonempty text",
            source,
        ));
    }
    if usize::from(has_event) + usize::from(has_context) + groups.len() != 1 {
        issues.push(Issue::new(
            Code::ESchema,
            path,
            "predicate requires exactly one of event or context, or one group of all, any, or none",
            source,
        ));
        return;
    }
    let comparisons: Vec<_> =
        COMPARISONS.into_iter().filter(|field| fields.get(field).is_some()).collect();
    if comparisons.len() > 1 {
        issues.push(Issue::new(
            Code::ESchema,
            path,
            format!("predicate may use only one comparison; found {}", comparisons.join(", ")),
            source,
        ));
    }
    if let Some(first) = comparisons.first().filter(|_| !has_context) {
        issues.push(Issue::new(
            Code::ESchema,
            path,
            format!("{first} is valid only with context"),
            source,
        ));
    }
    for comparison in comparisons {
        let value = fields.get(comparison).expect("comparison was found in the object");
        if NUMERIC.contains(&comparison) && value.as_number().is_none() {
            issues.push(Issue::new(
                Code::ESchema,
                format!("{path}.{comparison}"),
                format!("{comparison} must be a number"),
                source,
            ));
        }
        if matches!(comparison, "equals" | "not_equals")
            && !matches!(value, LuaValue::Boolean(_) | LuaValue::Number(_) | LuaValue::String(_))
        {
            issues.push(Issue::new(
                Code::ESchema,
                format!("{path}.{comparison}"),
                format!("{comparison} must be boolean, number, or string"),
                source,
            ));
        }
    }
    for group in groups {
        let values = fields.get(group).and_then(LuaValue::as_array);
        let Some(values) = values.filter(|values| !values.is_empty()) else {
            issues.push(Issue::new(
                Code::ESchema,
                format!("{path}.{group}"),
                format!("{group} must be a nonempty list of predicates"),
                source,
            ));
            continue;
        };
        for (index, value) in values.iter().enumerate() {
            validate_predicate(
                Some(value),
                &format!("{path}.{group}[{index}]"),
                source,
                depth + 1,
                issues,
            );
        }
    }
}

fn validate_minimap(
    fields: &LuaObject,
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    issues: &mut Vec<Issue>,
) {
    if let Some(layers) = fields.get("layers")
        && !layers.as_array().is_some_and(|values| {
            values.iter().all(|value| {
                value
                    .as_str()
                    .is_some_and(|value| ["party", "visible_hostiles", "powerups"].contains(&value))
            })
        })
    {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.layers"),
            "minimap layers support party, visible_hostiles, and powerups",
            source,
        ));
    }
    if !positive(fields.get("range")) {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.range"),
            "minimap range must be positive",
            source,
        ));
    }
    if let Some(size) = fields.get("size").and_then(LuaValue::as_object) {
        validate_unknown_fields(
            size,
            &["height", "width"],
            &format!("{path}.size"),
            source,
            issues,
        );
        if !positive(size.get("width")) || !positive(size.get("height")) {
            issues.push(Issue::new(
                Code::ESchema,
                format!("{path}.size"),
                "minimap width and height must be positive",
                source,
            ));
        }
    } else if !positive(fields.get("size")) {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.size"),
            "minimap size must be a number or width/height table",
            source,
        ));
    }
}

fn validate_portal(
    fields: &LuaObject,
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    issues: &mut Vec<Issue>,
) {
    if let Some(selector) = fields.get("selector").and_then(LuaValue::as_object) {
        validate_unknown_fields(
            selector,
            &["object_kind"],
            &format!("{path}.selector"),
            source,
            issues,
        );
        if selector.get("object_kind").and_then(LuaValue::as_str) != Some("monument") {
            issues.push(Issue::new(
                Code::ESchema,
                format!("{path}.selector.object_kind"),
                "Web Lua 1.0 portals attach to monument objects",
                source,
            ));
        }
    } else {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.selector"),
            "portal selector must be a table",
            source,
        ));
    }
    if fields.get("policy").is_some_and(|value| {
        !value.as_str().is_some_and(|value| ["leader_confirms", "any_member"].contains(&value))
    }) {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.policy"),
            "portal policy must be leader_confirms or any_member",
            source,
        ));
    }
    if fields.get("prompt").is_some_and(|value| value.as_str().is_none_or(str::is_empty)) {
        issues.push(Issue::new(
            Code::ESchema,
            format!("{path}.prompt"),
            "portal prompt must be nonempty text",
            source,
        ));
    }
}

pub fn validate_unknown_fields(
    fields: &LuaObject,
    allowed: &[&str],
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    issues: &mut Vec<Issue>,
) {
    let mut choices = allowed.to_vec();
    choices.sort();
    for (key, _) in fields.iter() {
        if allowed.contains(&key) {
            continue;
        }
        let mut hint = did_you_mean(key, choices.iter().copied());
        if hint.is_empty() && !choices.is_empty() && choices.len() <= 12 {
            hint = format!("; the fields here are {}", list_choices(choices.iter().copied()));
        }
        issues.push(Issue::new(
            Code::EUnknownField,
            format!("{path}.{key}"),
            format!("field is not supported by this 1.0 definition{hint}"),
            source,
        ));
    }
}

fn require_fields(
    fields: &LuaObject,
    required: &[&str],
    operation: &str,
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    issues: &mut Vec<Issue>,
) {
    for field in required {
        if fields.get(field).is_none_or(|value| matches!(value, LuaValue::Null)) {
            issues.push(Issue::new(
                Code::ESchema,
                format!("{path}.{field}"),
                format!("{operation} requires {field}"),
                source,
            ));
        }
    }
}

fn require_text(
    value: Option<&LuaValue>,
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    issues: &mut Vec<Issue>,
) {
    if value.and_then(LuaValue::as_str).is_none_or(str::is_empty) {
        issues.push(Issue::new(Code::ESchema, path, "expected nonempty text", source));
    }
}

fn validate_text(
    value: Option<&LuaValue>,
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    maximum: usize,
    issues: &mut Vec<Issue>,
) {
    if !value
        .and_then(LuaValue::as_str)
        .is_some_and(|value| !value.is_empty() && value.len() <= maximum)
    {
        issues.push(Issue::new(
            Code::ESchema,
            path,
            format!("expected nonempty text of at most {maximum} bytes"),
            source,
        ));
    }
}

fn validate_event(
    value: Option<&LuaValue>,
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    issues: &mut Vec<Issue>,
) {
    if value
        .and_then(LuaValue::as_str)
        .is_some_and(|value| WEB_LUA_RULE_EVENT_NAMES.contains(&value))
    {
        return;
    }
    let label = value.and_then(LuaValue::as_str).map_or_else(String::new, |value| {
        format!(" \"{value}\"{}", did_you_mean(value, WEB_LUA_RULE_EVENT_NAMES))
    });
    issues.push(Issue::new(
        Code::ESchema,
        path,
        format!("unknown event{label}; expected one of {}", WEB_LUA_RULE_EVENT_NAMES.join(", ")),
        source,
    ));
}

fn require_rule(
    value: Option<&LuaValue>,
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    issues: &mut Vec<Issue>,
) {
    if !is_token(value, "rule-definition") {
        issues.push(Issue::new(
            Code::ESchema,
            path,
            "expected an sd.rules, sd.effect, or sd.prefab node",
            source,
        ));
    }
}

fn require_rules(
    value: Option<&LuaValue>,
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    effects_only: bool,
    issues: &mut Vec<Issue>,
) {
    let valid = value.and_then(LuaValue::as_array).is_some_and(|values| {
        !values.is_empty()
            && values.iter().all(|value| {
                is_token(Some(value), "rule-definition")
                    && (!effects_only
                        || value
                            .get("operation")
                            .and_then(LuaValue::as_str)
                            .is_some_and(|operation| operation.starts_with("effect.")))
            })
    });
    if !valid {
        let message = if effects_only {
            "expected a nonempty array of sd.effect nodes"
        } else {
            "expected a nonempty array of rule nodes"
        };
        issues.push(Issue::new(Code::ESchema, path, message, source));
    }
}

fn validate_duration(
    value: Option<&LuaValue>,
    path: &str,
    source: Option<&WebLuaDefinitionSource>,
    issues: &mut Vec<Issue>,
) {
    if safe_integer_in(value, 0.0, 9_007_199_254_740_991.0) {
        return;
    }
    let valid = value.and_then(LuaValue::as_str).is_some_and(|text| {
        let number = ["ms", "s", "m", "h"].into_iter().find_map(|suffix| text.strip_suffix(suffix));
        let Some(number) = number else { return false };
        let (whole, fraction) = number
            .split_once('.')
            .map_or((number, None), |(whole, fraction)| (whole, Some(fraction)));
        !whole.is_empty()
            && (whole == "0" || !whole.starts_with('0'))
            && whole.bytes().all(|byte| byte.is_ascii_digit())
            && fraction.is_none_or(|fraction| {
                !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit())
            })
    });
    if !valid {
        issues.push(Issue::new(
            Code::ESchema,
            path,
            "expected nonnegative milliseconds or a duration such as \"180s\" or \"3m\"",
            source,
        ));
    }
}

fn positive(value: Option<&LuaValue>) -> bool {
    value.and_then(LuaValue::as_number).is_some_and(|value| value.is_finite() && value > 0.0)
}

fn safe_integer_in(value: Option<&LuaValue>, minimum: f64, maximum: f64) -> bool {
    value
        .and_then(LuaValue::as_number)
        .is_some_and(|value| value.fract() == 0.0 && (minimum..=maximum).contains(&value))
}

pub fn is_token(value: Option<&LuaValue>, kind: &str) -> bool {
    value.and_then(|value| value.get("kind")).and_then(LuaValue::as_str) == Some(kind)
}

fn is_reference(value: Option<&LuaValue>, kinds: &[&str]) -> bool {
    is_token(value, "content-reference")
        && value
            .and_then(|value| value.get("targetKind"))
            .and_then(LuaValue::as_str)
            .is_some_and(|kind| kinds.contains(&kind))
}

pub fn truthy(value: &LuaValue) -> bool {
    match value {
        LuaValue::Null => false,
        LuaValue::Boolean(value) => *value,
        LuaValue::Number(value) => *value != 0.0 && !value.is_nan(),
        LuaValue::String(value) => !value.is_empty(),
        _ => true,
    }
}

fn source_value(value: &LuaValue) -> Option<WebLuaDefinitionSource> {
    let file = value.get("file")?.as_str()?.to_owned();
    let integer = |key| value.get(key).and_then(LuaValue::as_number).map(|value| value as u32);
    Some(WebLuaDefinitionSource {
        file,
        column: integer("column"),
        line: integer("line"),
        node: integer("node"),
    })
}
