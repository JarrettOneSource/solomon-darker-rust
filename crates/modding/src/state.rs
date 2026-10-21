//! Scoped reducer cells from `modding/runtime/mod-state-store.ts`.
//! Migration callbacks are borrowed from the real VM during restore; saved cells
//! retain only schema identity, scope and ordinary ordered values.

use solomon_sim::modding::definition::WebLuaSchemaDefinition;
use solomon_sim::modding::runtime::{
    ModStateCellDefinition, ModStateCellSnapshot, ModStateCheckpoint, ModStateScope, ModStateUpdate,
};
use solomon_sim::modding::{LuaObject, LuaValue};
use std::collections::HashSet;

use crate::definition::values::typed_value;
use crate::json::SourceCollator;

#[derive(Clone)]
struct StoredCell {
    id: String,
    definition: ModStateCellDefinition,
    scope: ModStateScope,
    value: LuaValue,
}

pub struct ModStateStore {
    cells: Vec<StoredCell>,
    closed: bool,
    revision: u64,
    collator: SourceCollator,
}

impl Default for ModStateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ModStateStore {
    pub fn new() -> Self {
        Self { cells: Vec::new(), closed: false, revision: 0, collator: SourceCollator::default() }
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn cell(
        &mut self,
        definition: ModStateCellDefinition,
        scope: ModStateScope,
    ) -> Result<String, String> {
        self.require_open()?;
        validate_cell_definition(&definition)?;
        validate_scope(&scope)?;
        if definition.scope != scope.kind {
            return Err(format!(
                "state cell {}:{} requires {} scope",
                definition.mod_id,
                definition.key,
                definition.scope.as_str()
            ));
        }
        let id = cell_id(&definition.mod_id, &definition.key, &scope);
        if let Some(stored) = self.cells.iter().find(|cell| cell.id == id) {
            if !same_definition(&stored.definition, &definition)? {
                return Err(format!(
                    "state cell definition changed during one runtime: {}:{}",
                    definition.mod_id, definition.key
                ));
            }
            return Ok(id);
        }
        if self.cells.len() >= 4_096 {
            return Err("mod state cell limit reached".into());
        }
        let field = format!("state {}:{}", definition.mod_id, definition.key);
        let initial = match &definition.default_value {
            None => schema_default(&definition.schema, &field)?,
            Some(value) => {
                validate_schema_value(&definition.schema, value, &format!("{field} default"))?
            }
        };
        let mod_id = definition.mod_id.clone();
        self.cells.push(StoredCell { id: id.clone(), definition, scope, value: initial });
        if let Err(error) = self.enforce_mod_budget(&mod_id) {
            self.cells.pop();
            return Err(error);
        }
        Ok(id)
    }

    pub fn snapshot(&self, scopes: Option<&[ModStateScope]>) -> Result<ModStateCheckpoint, String> {
        self.require_open()?;
        let selected: Option<HashSet<_>> =
            scopes.map(|scopes| scopes.iter().map(scope_key).collect());
        let mut cells: Vec<_> = self
            .cells
            .iter()
            .filter(|cell| {
                selected.as_ref().is_none_or(|selected| selected.contains(&scope_key(&cell.scope)))
            })
            .collect();
        self.sort_cells(&mut cells);
        Ok(ModStateCheckpoint {
            cells: cells
                .into_iter()
                .map(|cell| ModStateCellSnapshot {
                    key: cell.definition.key.clone(),
                    mod_id: cell.definition.mod_id.clone(),
                    schema_version: cell.definition.schema_version,
                    scope: cell.scope.clone(),
                    value: cell.value.clone(),
                })
                .collect(),
            revision: self.revision,
        })
    }

    pub fn restore(
        &mut self,
        checkpoint: &ModStateCheckpoint,
        definitions: &[ModStateCellDefinition],
        mut migrate: impl FnMut(&ModStateCellDefinition, u64, &LuaValue) -> Result<LuaValue, String>,
    ) -> Result<(), String> {
        self.require_open()?;
        if checkpoint.revision > 9_007_199_254_740_991 {
            return Err("mod state checkpoint revision is invalid".into());
        }
        let mut candidate: Vec<StoredCell> = Vec::new();
        for snapshot in &checkpoint.cells {
            validate_scope(&snapshot.scope)?;
            // Source Map construction retains the last definition of an equal key.
            let Some(definition) = definitions.iter().rev().find(|definition| {
                definition.mod_id == snapshot.mod_id
                    && definition.key == snapshot.key
                    && definition.scope == snapshot.scope.kind
            }) else {
                continue;
            };
            validate_cell_definition(definition)?;
            let mut value = snapshot.value.clone();
            let mut version = snapshot.schema_version;
            if version == 0
                || version > 9_007_199_254_740_991
                || version > definition.schema_version
            {
                return Err(format!(
                    "state checkpoint has invalid schema version for {}:{}",
                    snapshot.mod_id, snapshot.key
                ));
            }
            while version < definition.schema_version {
                if !definition.migration_versions.contains(&version) {
                    return Err(format!(
                        "state cell {}:{} has no migration from version {version}",
                        snapshot.mod_id, snapshot.key
                    ));
                }
                value = migrate(definition, version, &value)?;
                version += 1;
            }
            value = validate_schema_value(
                &definition.schema,
                &value,
                &format!("state checkpoint {}:{}", snapshot.mod_id, snapshot.key),
            )?;
            let id = cell_id(&snapshot.mod_id, &snapshot.key, &snapshot.scope);
            let stored = StoredCell {
                id: id.clone(),
                definition: definition.clone(),
                scope: snapshot.scope.clone(),
                value,
            };
            if let Some(index) = candidate.iter().position(|cell| cell.id == id) {
                candidate[index] = stored
            } else {
                candidate.push(stored)
            }
        }
        let previous = std::mem::replace(&mut self.cells, candidate);
        for definition in definitions {
            if let Err(error) = self.enforce_mod_budget(&definition.mod_id) {
                self.cells = previous;
                return Err(error);
            }
        }
        self.revision = self.revision.max(checkpoint.revision);
        Ok(())
    }

    pub fn rollback(&mut self, checkpoint: &ModStateCheckpoint) -> Result<(), String> {
        self.require_open()?;
        let mut snapshots = Vec::new();
        for snapshot in &checkpoint.cells {
            let id = cell_id(&snapshot.mod_id, &snapshot.key, &snapshot.scope);
            if let Some(index) = snapshots.iter().position(|(key, _)| *key == id) {
                snapshots[index] = (id, snapshot)
            } else {
                snapshots.push((id, snapshot))
            }
        }
        self.cells.retain(|cell| snapshots.iter().any(|(id, _)| *id == cell.id));
        for (id, snapshot) in snapshots {
            let Some(cell) = self.cells.iter_mut().find(|cell| {
                cell.id == id && cell.definition.schema_version == snapshot.schema_version
            }) else {
                return Err(format!(
                    "cannot roll back missing mod state cell {}:{}",
                    snapshot.mod_id, snapshot.key
                ));
            };
            cell.value = validate_schema_value(
                &cell.definition.schema,
                &snapshot.value,
                &format!("rollback {}:{}", snapshot.mod_id, snapshot.key),
            )?;
        }
        self.revision = checkpoint.revision;
        Ok(())
    }

    pub fn close_scope(&mut self, scope: &ModStateScope) -> Result<usize, String> {
        let key = scope_key(scope);
        self.close_scopes(|scope| scope_key(scope) == key)
    }
    pub fn close_scopes(
        &mut self,
        mut predicate: impl FnMut(&ModStateScope) -> bool,
    ) -> Result<usize, String> {
        self.require_open()?;
        let before = self.cells.len();
        self.cells.retain(|cell| !predicate(&cell.scope));
        let removed = before - self.cells.len();
        if removed > 0 {
            self.revision += 1
        }
        Ok(removed)
    }
    pub fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.cells.clear();
    }

    pub fn read(&self, id: &str) -> Result<LuaValue, String> {
        self.require_open()?;
        self.cells
            .iter()
            .find(|cell| cell.id == id)
            .map(|cell| cell.value.clone())
            .ok_or_else(|| "mod state cell is stale".into())
    }

    pub fn set(&mut self, id: &str, value: LuaValue) -> Result<ModStateUpdate, String> {
        self.update(id, |_| Ok(value))
    }

    pub fn update(
        &mut self,
        id: &str,
        reducer: impl FnOnce(LuaValue) -> Result<LuaValue, String>,
    ) -> Result<ModStateUpdate, String> {
        self.require_open()?;
        let index =
            self.cells.iter().position(|cell| cell.id == id).ok_or("mod state cell is stale")?;
        let cell = &self.cells[index];
        let before = cell.value.clone();
        let next = validate_schema_value(
            &cell.definition.schema,
            &reducer(before.clone())?,
            &format!("state {}:{}", cell.definition.mod_id, cell.definition.key),
        )?;
        let mod_id = cell.definition.mod_id.clone();
        self.cells[index].value = next.clone();
        if let Err(error) = self.enforce_mod_budget(&mod_id) {
            self.cells[index].value = before;
            return Err(error);
        }
        if !same_lua_value(&before, &next)? {
            self.revision += 1
        }
        Ok(ModStateUpdate { revision: self.revision, value: next })
    }

    fn enforce_mod_budget(&self, mod_id: &str) -> Result<(), String> {
        let mut cells: Vec<_> =
            self.cells.iter().filter(|cell| cell.definition.mod_id == mod_id).collect();
        self.sort_cells(&mut cells);
        let mut values = Vec::new();
        for cell in cells {
            let mut row = LuaObject::new();
            row.insert("key".into(), LuaValue::String(cell.definition.key.clone()));
            row.insert("scope".into(), crate::json::lua_value(&cell.scope)?);
            row.insert("value".into(), cell.value.clone());
            values.push(LuaValue::Object(row));
        }
        if crate::json::stringify(&LuaValue::Array(values))?.len() > 64 * 1_024 {
            return Err(format!("mod state exceeds 65536 bytes for {mod_id}"));
        }
        Ok(())
    }

    fn sort_cells(&self, cells: &mut [&StoredCell]) {
        cells.sort_by(|left, right| {
            self.collator
                .compare(&left.definition.mod_id, &right.definition.mod_id)
                .then_with(|| self.collator.compare(&left.definition.key, &right.definition.key))
                .then_with(|| {
                    self.collator.compare(left.scope.kind.as_str(), right.scope.kind.as_str())
                })
                .then_with(|| self.collator.compare(&left.scope.id, &right.scope.id))
        });
    }
    fn require_open(&self) -> Result<(), String> {
        if self.closed { Err("mod state store is closed".into()) } else { Ok(()) }
    }
}

pub fn validate_schema_value(
    schema: &WebLuaSchemaDefinition,
    value: &LuaValue,
    field: &str,
) -> Result<LuaValue, String> {
    validate_schema_at(schema, value, field, 0)
}

fn validate_schema_at(
    schema: &WebLuaSchemaDefinition,
    value: &LuaValue,
    field: &str,
    depth: usize,
) -> Result<LuaValue, String> {
    if depth > 32 {
        return Err(format!("{field} exceeds schema depth"));
    }
    match schema.schema_kind.as_str() {
        "boolean" => {
            if matches!(value, LuaValue::Boolean(_)) {
                Ok(value.clone())
            } else {
                Err(format!("{field} must be a boolean"))
            }
        }
        "integer" | "number" => {
            let integer = schema.schema_kind == "integer";
            let number = value
                .as_number()
                .filter(|number| {
                    number.is_finite()
                        && (!integer
                            || number.fract() == 0.0 && number.abs() <= 9_007_199_254_740_991.0)
                })
                .ok_or_else(|| {
                    if integer {
                        format!("{field} must be a safe integer")
                    } else {
                        format!("{field} must be finite")
                    }
                })?;
            let bound = if integer { 9_007_199_254_740_991.0 } else { f64::MAX };
            let minimum = schema_number(schema, "min", -bound);
            let maximum = schema_number(schema, "max", bound);
            if number < minimum || number > maximum {
                return Err(format!(
                    "{field} is outside {}..{}",
                    number_text(minimum),
                    number_text(maximum)
                ));
            }
            Ok(value.clone())
        }
        "string" => {
            let text = value.as_str().ok_or_else(|| format!("{field} must be text"))?;
            let maximum = schema_number(schema, "max_bytes", 16_384.0);
            if text.len() as f64 > maximum {
                return Err(format!("{field} exceeds {} bytes", number_text(maximum)));
            }
            Ok(value.clone())
        }
        "enum" => {
            let values = schema_array(schema, "values")?;
            for candidate in values {
                if same_lua_value(candidate, value)? {
                    return Ok(value.clone());
                }
            }
            Err(format!("{field} is not an accepted enum value"))
        }
        "array" => {
            let LuaValue::Array(values) = value else {
                return Err(format!("{field} must be an array"));
            };
            let item = schema
                .fields
                .get("item")
                .filter(|value| is_schema(value))
                .ok_or_else(|| format!("{field} array schema has no item definition"))?;
            let maximum = schema_number(schema, "max_items", 256.0);
            if values.len() as f64 > maximum {
                return Err(format!("{field} exceeds {} items", number_text(maximum)));
            }
            let item = typed_value(item)?;
            let mut output = Vec::new();
            for (index, value) in values.iter().enumerate() {
                output.push(validate_schema_at(
                    &item,
                    value,
                    &format!("{field}[{index}]"),
                    depth + 1,
                )?);
            }
            Ok(LuaValue::Array(output))
        }
        "object" => {
            let values = value.as_object().ok_or_else(|| format!("{field} must be an object"))?;
            let fields = schema
                .fields
                .get("fields")
                .and_then(LuaValue::as_object)
                .ok_or_else(|| format!("{field} object schema has no fields"))?;
            let unknown: Vec<_> =
                values.iter().map(|(key, _)| key).filter(|key| fields.get(key).is_none()).collect();
            if !unknown.is_empty() {
                return Err(format!("{field} has unknown fields: {}", unknown.join(", ")));
            }
            let mut output = LuaObject::new();
            for (key, child) in fields.iter() {
                if !is_schema(child) {
                    return Err(format!("{field}.{key} has an invalid schema"));
                }
                let child = typed_value(child)?;
                let candidate =
                    match values.get(key).filter(|value| !matches!(value, LuaValue::Null)) {
                        Some(value) => value.clone(),
                        None => schema_default(&child, &format!("{field}.{key}"))?,
                    };
                output.insert(
                    key.into(),
                    validate_schema_at(&child, &candidate, &format!("{field}.{key}"), depth + 1)?,
                );
            }
            Ok(LuaValue::Object(output))
        }
        _ => Err(format!("{field} uses unsupported schema kind {}", schema.schema_kind)),
    }
}

pub fn schema_default(schema: &WebLuaSchemaDefinition, field: &str) -> Result<LuaValue, String> {
    if let Some(candidate) = schema.fields.get("default").filter(|value| !is_schema(value)) {
        return validate_schema_value(schema, candidate, &format!("{field} default"));
    }
    match schema.schema_kind.as_str() {
        "object" => {
            let fields = schema
                .fields
                .get("fields")
                .and_then(LuaValue::as_object)
                .ok_or_else(|| format!("{field} object schema has no fields"))?;
            let mut output = LuaObject::new();
            for (key, child) in fields.iter() {
                if !is_schema(child) {
                    return Err(format!("{field}.{key} has an invalid schema"));
                }
                output.insert(
                    key.into(),
                    schema_default(&typed_value(child)?, &format!("{field}.{key}"))?,
                );
            }
            Ok(LuaValue::Object(output))
        }
        "array" => Ok(LuaValue::Array(Vec::new())),
        "boolean" => Ok(LuaValue::Boolean(false)),
        "integer" | "number" => Ok(LuaValue::Number(0.0)),
        "string" => Ok(LuaValue::String(String::new())),
        "enum" => Ok(schema_array(schema, "values")?.first().cloned().unwrap_or(LuaValue::Null)),
        _ => Err(format!("{field} has no default")),
    }
}

fn validate_cell_definition(definition: &ModStateCellDefinition) -> Result<(), String> {
    if definition.mod_id.is_empty() || definition.key.is_empty() {
        return Err("mod state cell identity is invalid".into());
    }
    if definition.schema_version == 0 || definition.schema_version > 9_007_199_254_740_991 {
        return Err("mod state cell schema version must be positive".into());
    }
    for version in 1..definition.schema_version {
        if !definition.migration_versions.contains(&version) {
            return Err(format!("mod state cell is missing migration from version {version}"));
        }
    }
    Ok(())
}
fn validate_scope(scope: &ModStateScope) -> Result<(), String> {
    if scope.id.is_empty() { Err("mod state scope is invalid".into()) } else { Ok(()) }
}
fn cell_id(mod_id: &str, key: &str, scope: &ModStateScope) -> String {
    format!("{mod_id}\0{key}\0{}\0{}", scope.kind.as_str(), scope.id)
}
fn scope_key(scope: &ModStateScope) -> String {
    format!("{}\0{}", scope.kind.as_str(), scope.id)
}
fn same_definition(
    left: &ModStateCellDefinition,
    right: &ModStateCellDefinition,
) -> Result<bool, String> {
    Ok(left.mod_id == right.mod_id
        && left.key == right.key
        && left.scope == right.scope
        && left.schema_version == right.schema_version
        && crate::json::stringify(&crate::json::lua_value(&left.schema)?)?
            == crate::json::stringify(&crate::json::lua_value(&right.schema)?)?)
}
fn same_lua_value(left: &LuaValue, right: &LuaValue) -> Result<bool, String> {
    Ok(crate::json::stringify(left)? == crate::json::stringify(right)?)
}
fn schema_number(schema: &WebLuaSchemaDefinition, key: &str, fallback: f64) -> f64 {
    schema
        .fields
        .get(key)
        .and_then(LuaValue::as_number)
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}
fn schema_array<'a>(
    schema: &'a WebLuaSchemaDefinition,
    key: &str,
) -> Result<&'a [LuaValue], String> {
    match schema.fields.get(key) {
        Some(LuaValue::Array(values)) => Ok(values),
        _ => Err(format!("{} schema {key} must be an array", schema.schema_kind)),
    }
}
fn is_schema(value: &LuaValue) -> bool {
    value.as_object().is_some_and(|fields| {
        fields.get("kind").and_then(LuaValue::as_str) == Some("schema-definition")
    })
}
fn number_text(value: f64) -> String {
    crate::json::stringify(&LuaValue::Number(value)).expect("schema bounds are finite")
}
