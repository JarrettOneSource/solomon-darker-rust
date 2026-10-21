//! Status stacking, expiry and ordered modifiers from `mod-status-engine.ts`.

use std::collections::HashSet;
use std::sync::Arc;

use solomon_sim::modding::LuaValue;
use solomon_sim::modding::content::{
    ModStatusStacking, PreparedModContentCatalog, PreparedModStatusDefinition,
};
use solomon_sim::modding::runtime::{ActiveModStatus, ModStatusCheckpoint};

use super::{MAXIMUM_SAFE_INTEGER, duration_ticks, validate_tick, validate_tick_rate};

pub struct ModStatusEngine {
    catalog: Arc<PreparedModContentCatalog>,
    ticks_per_second: u32,
    state: ModStatusCheckpoint,
}

impl ModStatusEngine {
    pub fn new(
        catalog: Arc<PreparedModContentCatalog>,
        ticks_per_second: u32,
    ) -> Result<Self, String> {
        validate_tick_rate(ticks_per_second, "status")?;
        Ok(Self { catalog, ticks_per_second, state: ModStatusCheckpoint::default() })
    }

    pub fn revision(&self) -> u64 {
        self.state.revision
    }

    pub fn apply(
        &mut self,
        content_id: &str,
        target_id: &str,
        tick: u64,
        duration_ms: Option<f64>,
    ) -> Result<bool, String> {
        let definition = self
            .catalog
            .status(content_id)
            .ok_or_else(|| format!("mod status is unavailable: {content_id}"))?;
        validate_target(target_id)?;
        validate_tick(tick, "status")?;
        let duration =
            duration_ticks(duration_ms.unwrap_or(definition.duration_ms), self.ticks_per_second)
                .max(1);
        let matching = self
            .state
            .instances
            .iter()
            .filter(|instance| {
                instance.content_id == content_id
                    && instance.target_id == target_id
                    && instance.expires_tick > tick
            })
            .count();
        if definition.stacking == ModStatusStacking::Ignore && matching > 0 {
            return Ok(false);
        }
        if definition.stacking == ModStatusStacking::Stack && matching >= 8 {
            return Ok(false);
        }
        if definition.stacking != ModStatusStacking::Stack {
            self.state.instances.retain(|instance| {
                instance.content_id != content_id || instance.target_id != target_id
            });
        }
        if self.state.instances.len() >= 4_096 {
            return Err("active mod status limit reached".into());
        }
        self.state.instances.push(ActiveModStatus {
            content_id: content_id.into(),
            expires_tick: tick + duration,
            instance_id: self.state.next_instance_id,
            mod_id: definition.common.mod_id.clone(),
            started_tick: tick,
            target_id: target_id.into(),
        });
        self.state.next_instance_id += 1;
        self.state.revision += 1;
        Ok(true)
    }

    pub fn checkpoint(&self) -> ModStatusCheckpoint {
        self.state.clone()
    }

    pub fn filter_damage(&self, target_id: &str, amount: f64, tick: u64) -> Result<f64, String> {
        self.modify(target_id, "incoming_damage", amount, tick)
    }

    pub fn filter_mana(&self, target_id: &str, delta: f64, tick: u64) -> Result<f64, String> {
        if delta < 0.0 {
            Ok(-self.modify(target_id, "mana_spend", -delta, tick)?)
        } else {
            Ok(delta)
        }
    }

    pub fn project(&self, target_id: Option<&str>) -> Vec<ActiveModStatus> {
        let mut instances: Vec<_> = self
            .state
            .instances
            .iter()
            .filter(|instance| target_id.is_none_or(|id| instance.target_id == id))
            .cloned()
            .collect();
        instances.sort_by_key(|instance| instance.instance_id);
        instances
    }

    pub fn restore(&mut self, checkpoint: ModStatusCheckpoint) -> Result<(), String> {
        if checkpoint.revision > MAXIMUM_SAFE_INTEGER
            || checkpoint.next_instance_id == 0
            || checkpoint.next_instance_id > MAXIMUM_SAFE_INTEGER
            || checkpoint.instances.len() > 4_096
        {
            return Err("mod status checkpoint is invalid".into());
        }
        let mut ids = HashSet::new();
        for instance in &checkpoint.instances {
            validate_target(&instance.target_id)?;
            validate_tick(instance.started_tick, "status")?;
            validate_tick(instance.expires_tick, "status")?;
            let definition = self.catalog.status(&instance.content_id);
            if instance.instance_id == 0
                || instance.instance_id > MAXIMUM_SAFE_INTEGER
                || instance.expires_tick <= instance.started_tick
                || definition.is_none_or(|definition| instance.mod_id != definition.common.mod_id)
                || !ids.insert(instance.instance_id)
            {
                return Err("mod status checkpoint contains an invalid instance".into());
            }
        }
        self.state = checkpoint;
        Ok(())
    }

    pub fn tick(&mut self, tick: u64) -> Result<usize, String> {
        validate_tick(tick, "status")?;
        let before = self.state.instances.len();
        self.state.instances.retain(|instance| instance.expires_tick > tick);
        let removed = before - self.state.instances.len();
        if removed > 0 {
            self.state.revision += 1
        }
        Ok(removed)
    }

    fn modify(&self, target_id: &str, lane: &str, source: f64, tick: u64) -> Result<f64, String> {
        if !source.is_finite() || source < 0.0 {
            return Err(format!("mod status {lane} input is invalid"));
        }
        validate_tick(tick, "status")?;
        let mut value = source;
        for instance in &self.state.instances {
            if instance.target_id != target_id || instance.expires_tick <= tick {
                continue;
            }
            let Some(definition) = self.catalog.status(&instance.content_id) else { continue };
            if let Some(modifier) = definition.modifiers.get(lane) {
                value = apply_modifier(value, modifier, definition, lane)?;
            }
        }
        // Math.max propagates NaN; Rust's f64::max would silently choose 0.
        Ok(if value.is_nan() { value } else { value.max(0.0) })
    }
}

fn apply_modifier(
    source: f64,
    modifier: &LuaValue,
    definition: &PreparedModStatusDefinition,
    lane: &str,
) -> Result<f64, String> {
    if let LuaValue::Number(number) = modifier
        && number.is_finite()
    {
        return Ok(source * number);
    }
    let label = format!("{}:{} {lane} modifier", definition.common.mod_id, definition.common.key);
    let Some(fields) = modifier.as_object() else { return Err(format!("{label} is invalid")) };
    if fields.len() != 1 {
        return Err(format!("{label} is ambiguous"));
    }
    let (operation, value) = fields.iter().next().expect("one modifier field");
    let Some(amount) = value.as_number().filter(|number| number.is_finite()) else {
        return Err(format!("{label} amount is invalid"));
    };
    match operation {
        "multiply" => Ok(source * amount),
        "add" => Ok(source + amount),
        "set" => Ok(amount),
        _ => Err(format!("{label} operation is invalid")),
    }
}

fn validate_target(value: &str) -> Result<(), String> {
    if value.is_empty() || value.encode_utf16().count() > 128 {
        return Err("mod status target is invalid".into());
    }
    Ok(())
}
