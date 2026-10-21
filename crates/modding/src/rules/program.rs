//! Indexed immutable rule evaluation from `mod-rule-engine.ts`.

use solomon_sim::modding::definition::{
    CompiledWebLuaMod, WebLuaContentKind, WebLuaDefinitionSource, WebLuaRuleDefinition,
};
use solomon_sim::modding::runtime::{
    ModIntent, ModRuleDispatchInput, ModRuleTimerCheckpoint, ModStateScope,
};
use solomon_sim::modding::{LuaObject, LuaValue};

use crate::definition::values::typed_value;
use crate::duration::duration_ticks_value;

pub(super) struct RuleProgram {
    pub nodes: Vec<IndexedRule>,
    pub roots: Vec<usize>,
}
pub(super) struct IndexedRule {
    pub rule: WebLuaRuleDefinition,
    pub children: Vec<(String, usize)>,
}
pub(super) struct EvaluationState<'a> {
    pub mod_id: &'a str,
    pub next_intent_sequence: &'a mut u64,
    pub next_timer_id: &'a mut u64,
    pub timers: &'a mut Vec<ModRuleTimerCheckpoint>,
    pub ticks_per_second: u32,
}

impl RuleProgram {
    pub fn new(compiled: &CompiledWebLuaMod) -> Result<Self, String> {
        let mut rules = compiled.rules.clone();
        rules.extend(content_rules(compiled)?);
        let mut program = Self { nodes: Vec::new(), roots: Vec::new() };
        for rule in rules {
            let index = program.index_rule(rule)?;
            program.roots.push(index);
        }
        Ok(program)
    }

    fn index_rule(&mut self, rule: WebLuaRuleDefinition) -> Result<usize, String> {
        let index = self.nodes.len();
        let fields = rule.fields.clone();
        self.nodes.push(IndexedRule { rule, children: Vec::new() });
        let mut children = Vec::new();
        for (key, value) in fields.iter() {
            self.index_value(value, key, &mut children)?;
        }
        self.nodes[index].children = children;
        Ok(index)
    }
    fn index_value(
        &mut self,
        value: &LuaValue,
        path: &str,
        children: &mut Vec<(String, usize)>,
    ) -> Result<(), String> {
        if is_rule(value) {
            let index = self.index_rule(typed_value(value)?)?;
            children.push((path.into(), index));
        } else {
            match value {
                LuaValue::Array(values) => {
                    for (index, value) in values.iter().enumerate() {
                        self.index_value(value, &format!("{path}[{index}]"), children)?;
                    }
                }
                LuaValue::Object(fields) => {
                    for (key, value) in fields.iter() {
                        self.index_value(value, &format!("{path}.{key}"), children)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub fn child(&self, index: usize, path: &str) -> Option<usize> {
        self.nodes[index].children.iter().find(|(key, _)| key == path).map(|(_, index)| *index)
    }
}

pub(super) fn evaluate_rule(
    program: &RuleProgram,
    index: usize,
    input: &ModRuleDispatchInput,
    state: &mut EvaluationState<'_>,
) -> Result<Vec<ModIntent>, String> {
    let rule = &program.nodes[index].rule;
    match rule.operation.as_str() {
        "rules.on" => {
            if rule.fields.get("event").and_then(LuaValue::as_str) != Some(input.event.as_str()) {
                return Ok(Vec::new());
            }
            match program.child(index, "node") {
                Some(child) => evaluate_rule(program, child, input, state),
                None => Ok(Vec::new()),
            }
        }
        "rules.all" | "rules.first" => {
            let Some(LuaValue::Array(values)) = rule.fields.get("nodes").filter(
                |value| matches!(value,LuaValue::Array(values) if values.iter().all(is_rule)),
            ) else {
                return Err(format!("{} has invalid nodes", rule.operation));
            };
            let mut result = Vec::new();
            for ordinal in 0..values.len() {
                let child =
                    program.child(index, &format!("nodes[{ordinal}]")).expect("indexed rule child");
                let next = evaluate_rule(program, child, input, state)?;
                if rule.operation == "rules.first" && !next.is_empty() {
                    return Ok(next);
                }
                result.extend(next);
            }
            Ok(result)
        }
        "rules.when" => {
            let branch =
                if predicate(rule.fields.get("predicate"), input, 0)? { "yes" } else { "no" };
            match program.child(index, branch) {
                Some(child) => evaluate_rule(program, child, input, state),
                None => Ok(Vec::new()),
            }
        }
        "rules.after" | "rules.every" => {
            schedule(program, index, input, state)?;
            Ok(Vec::new())
        }
        "prefab.area" | "prefab.channel" | "prefab.projectile" => {
            let mut fields = rule.fields.clone();
            fields.insert("prefab".into(), LuaValue::String(rule.operation[7..].into()));
            fields.insert(
                "spell_content_id".into(),
                input
                    .payload
                    .as_object()
                    .and_then(|fields| fields.get("content_id"))
                    .cloned()
                    .unwrap_or(LuaValue::Null),
            );
            let sequence = *state.next_intent_sequence;
            *state.next_intent_sequence += 1;
            Ok(vec![intent(state.mod_id, "spell-effect", &fields, "rule", &input.scope, sequence)?])
        }
        operation => {
            if let Some(kind) = operation.strip_prefix("effect.") {
                let sequence = *state.next_intent_sequence;
                *state.next_intent_sequence += 1;
                Ok(vec![intent(state.mod_id, kind, &rule.fields, "rule", &input.scope, sequence)?])
            } else {
                Ok(Vec::new())
            }
        }
    }
}

fn schedule(
    program: &RuleProgram,
    index: usize,
    input: &ModRuleDispatchInput,
    state: &mut EvaluationState<'_>,
) -> Result<(), String> {
    if state.timers.len() >= 256 {
        return Err("mod rule timer limit reached".into());
    }
    let rule = &program.nodes[index].rule;
    if !rule.fields.get("node").is_some_and(is_rule) {
        return Err(format!("{} requires a rule node", rule.operation));
    }
    let child = program
        .child(index, "node")
        .ok_or_else(|| format!("{} rule node is not part of the compiled graph", rule.operation))?;
    let repeated = rule.operation == "rules.every";
    let delay = duration_ticks_value(
        rule.fields.get(if repeated { "interval" } else { "duration" }),
        state.ticks_per_second,
        if repeated { "rules.every interval" } else { "rules.after duration" },
    )?;
    let remaining = if repeated {
        rule.fields
            .get("times")
            .and_then(LuaValue::as_number)
            .filter(|value| {
                value.is_finite() && value.fract() == 0.0 && (1.0..=1_024.0).contains(value)
            })
            .ok_or("rules.every times must be an integer within 1..1024")? as u64
    } else {
        1
    };
    let context = normalize_object(&input.context, "mod timer context")?;
    let payload = normalize_value(&input.payload, "mod timer payload", 0)?;
    state.timers.push(ModRuleTimerCheckpoint {
        context,
        due_tick: input.tick + delay,
        event: input.event.clone(),
        id: *state.next_timer_id,
        interval_ticks: repeated.then_some(delay),
        mod_id: state.mod_id.into(),
        node_id: format!("r{child}"),
        payload,
        remaining,
        scope: input.scope.clone(),
    });
    *state.next_timer_id += 1;
    Ok(())
}

pub(super) fn intent(
    mod_id: &str,
    kind: &str,
    fields: &LuaObject,
    owner: &str,
    scope: &ModStateScope,
    sequence: u64,
) -> Result<ModIntent, String> {
    Ok(ModIntent {
        fields: normalize_object(fields, &format!("{mod_id}:{owner} {kind} intent"))?,
        kind: kind.into(),
        mod_id: mod_id.into(),
        owner: owner.into(),
        scope: scope.clone(),
        sequence,
    })
}

pub(super) fn normalize_object(fields: &LuaObject, field: &str) -> Result<LuaObject, String> {
    match normalize_value(&LuaValue::Object(fields.clone()), field, 0)? {
        LuaValue::Object(fields) => Ok(fields),
        _ => unreachable!("object normalization retains its shape"),
    }
}

fn normalize_value(value: &LuaValue, field: &str, depth: usize) -> Result<LuaValue, String> {
    if depth > 32 {
        return Err(format!("{field} exceeds its depth limit"));
    }
    match value {
        LuaValue::Number(value) if !value.is_finite() => Err(format!("{field} must be finite")),
        LuaValue::Array(values) => {
            let mut output = Vec::new();
            for (index, value) in values.iter().enumerate() {
                output.push(normalize_value(value, &format!("{field}[{index}]"), depth + 1)?);
            }
            Ok(LuaValue::Array(output))
        }
        LuaValue::Object(fields) => {
            let mut output = LuaObject::new();
            for (key, value) in fields.iter() {
                output.insert(
                    key.into(),
                    normalize_value(value, &format!("{field}.{key}"), depth + 1)?,
                );
            }
            Ok(LuaValue::Object(output))
        }
        _ => Ok(value.clone()),
    }
}

pub(super) fn is_rule(value: &LuaValue) -> bool {
    value.as_object().is_some_and(|fields| {
        fields.get("kind").and_then(LuaValue::as_str) == Some("rule-definition")
    })
}

fn content_rules(compiled: &CompiledWebLuaMod) -> Result<Vec<WebLuaRuleDefinition>, String> {
    let mut rules = Vec::new();
    for content in &compiled.content {
        if content.content_kind == WebLuaContentKind::Boneyard {
            if let Some(LuaValue::Array(triggers)) = content.fields.get("triggers") {
                for node in triggers.iter().filter(|value| is_rule(value)) {
                    let mut predicate = LuaObject::new();
                    predicate.insert(
                        "context".into(),
                        LuaValue::String("active_boneyard_content_id".into()),
                    );
                    predicate.insert("equals".into(), LuaValue::String(content.content_id.clone()));
                    let mut fields = LuaObject::new();
                    fields.insert("no".into(), LuaValue::Null);
                    fields.insert("predicate".into(), LuaValue::Object(predicate));
                    fields.insert("yes".into(), node.clone());
                    rules.push(WebLuaRuleDefinition {
                        fields,
                        operation: "rules.when".into(),
                        source: node_source(node)?,
                    });
                }
            }
            continue;
        }
        let (trigger, field) = match content.content_kind {
            WebLuaContentKind::Potion => ("use", "on_use"),
            WebLuaContentKind::Item => ("use", "use"),
            WebLuaContentKind::Powerup => ("pickup", "effect"),
            WebLuaContentKind::Spell => ("cast", "behavior"),
            _ => continue,
        };
        let Some(node) = content.fields.get(field).filter(|value| is_rule(value)) else { continue };
        let mut fields = LuaObject::new();
        fields.insert(
            "event".into(),
            LuaValue::String(format!("action.content.{trigger}.{}", content.content_id)),
        );
        fields.insert("node".into(), node.clone());
        rules.push(WebLuaRuleDefinition {
            fields,
            operation: "rules.on".into(),
            source: node_source(node)?,
        });
    }
    Ok(rules)
}

fn node_source(value: &LuaValue) -> Result<WebLuaDefinitionSource, String> {
    typed_value(value.get("source").ok_or("rule source is unavailable")?)
}

fn predicate(
    value: Option<&LuaValue>,
    input: &ModRuleDispatchInput,
    depth: usize,
) -> Result<bool, String> {
    if let Some(LuaValue::Boolean(value)) = value {
        return Ok(*value);
    }
    let source = value.and_then(LuaValue::as_object).ok_or("rules.when predicate is invalid")?;
    if depth > 8 {
        return Err("rules.when predicate nests too deeply".into());
    }
    let allowed = [
        "above",
        "all",
        "any",
        "at_least",
        "at_most",
        "below",
        "context",
        "equals",
        "event",
        "none",
        "not_equals",
    ];
    let unknown: Vec<_> =
        source.iter().map(|(key, _)| key).filter(|key| !allowed.contains(key)).collect();
    if !unknown.is_empty() {
        return Err(format!("rules.when predicate has unknown fields: {}", unknown.join(", ")));
    }
    if let Some(event) = source.get("event").and_then(LuaValue::as_str) {
        return Ok(event == input.event);
    }
    if let Some(context) = source.get("context").and_then(LuaValue::as_str) {
        let actual = input.context.get(context);
        if let Some(expected) = source.get("equals") {
            return Ok(strict_equal(actual, expected));
        }
        if let Some(expected) = source.get("not_equals") {
            return Ok(!strict_equal(actual, expected));
        }
        for comparison in ["above", "at_least", "at_most", "below"] {
            if let Some(expected) = source.get(comparison) {
                let numbers = actual
                    .and_then(LuaValue::as_number)
                    .zip(expected.as_number())
                    .filter(|(actual, expected)| actual.is_finite() && expected.is_finite());
                return Ok(numbers.is_some_and(|(actual, expected)| match comparison {
                    "above" => actual > expected,
                    "at_least" => actual >= expected,
                    "at_most" => actual <= expected,
                    _ => actual < expected,
                }));
            }
        }
        return Ok(actual.is_some_and(truthy));
    }
    for group in ["all", "any", "none"] {
        if let Some(LuaValue::Array(values)) = source.get(group) {
            let mut any = false;
            let mut all = true;
            for value in values {
                let result = predicate(Some(value), input, depth + 1)?;
                any |= result;
                all &= result;
                if group == "all" && !result || group != "all" && result {
                    break;
                }
            }
            return Ok(match group {
                "all" => all,
                "any" => any,
                _ => !any,
            });
        }
    }
    Err("rules.when predicate requires event, context, all, any, or none".into())
}

fn strict_equal(actual: Option<&LuaValue>, expected: &LuaValue) -> bool {
    match (actual, expected) {
        (Some(LuaValue::Null), LuaValue::Null) => true,
        (Some(LuaValue::Boolean(actual)), LuaValue::Boolean(expected)) => actual == expected,
        (Some(LuaValue::Number(actual)), LuaValue::Number(expected)) => actual == expected,
        (Some(LuaValue::String(actual)), LuaValue::String(expected)) => actual == expected,
        _ => false,
    }
}
fn truthy(value: &LuaValue) -> bool {
    match value {
        LuaValue::Null => false,
        LuaValue::Boolean(value) => *value,
        LuaValue::Number(value) => *value != 0.0 && !value.is_nan(),
        LuaValue::String(value) => !value.is_empty(),
        LuaValue::Array(_) | LuaValue::Object(_) => true,
    }
}
