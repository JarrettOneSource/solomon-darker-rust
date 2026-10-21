//! Actual area/channel/projectile simulation from `mod-spell-effect-engine.ts`.

use std::collections::HashSet;

use solomon_sim::Vec2;
use solomon_sim::float::hypot;
use solomon_sim::modding::runtime::{
    ActiveModSpellEffect, ModActorPlayer, ModEffectTemplate, ModSpellEffectBatch,
    ModSpellEffectCheckpoint, ModSpellEffectIntent, ModSpellEffectKind, ModSpellEffectTarget,
    ModStateScope,
};
use solomon_sim::modding::{LuaObject, LuaValue};

use super::{MAXIMUM_SAFE_INTEGER, validate_tick, validate_tick_rate};
use crate::duration::duration_ticks_value;

pub struct ModSpellEffectEngine {
    ticks_per_second: u32,
    state: ModSpellEffectCheckpoint,
}

pub struct ModSpellEffectSpawn<'a> {
    pub content_id: &'a str,
    pub fields: &'a LuaObject,
    pub mod_id: &'a str,
    pub origin: Vec2,
    pub owner_player_id: &'a str,
    pub scope: ModStateScope,
    pub target: Vec2,
    pub tick: u64,
}

impl ModSpellEffectEngine {
    pub fn new(ticks_per_second: u32) -> Result<Self, String> {
        validate_tick_rate(ticks_per_second, "spell effect")?;
        Ok(Self { ticks_per_second, state: ModSpellEffectCheckpoint::default() })
    }
    pub fn revision(&self) -> u64 {
        self.state.revision
    }
    pub fn checkpoint(&self) -> ModSpellEffectCheckpoint {
        self.state.clone()
    }
    pub fn project(&self) -> &[ActiveModSpellEffect] {
        &self.state.effects
    }

    pub fn restore(&mut self, checkpoint: ModSpellEffectCheckpoint) -> Result<(), String> {
        if checkpoint.next_id == 0
            || checkpoint.next_id > MAXIMUM_SAFE_INTEGER
            || checkpoint.next_sequence == 0
            || checkpoint.next_sequence > MAXIMUM_SAFE_INTEGER
            || checkpoint.revision > MAXIMUM_SAFE_INTEGER
            || checkpoint.effects.len() > 1_024
        {
            return Err("mod spell effect checkpoint is invalid".into());
        }
        let mut ids = HashSet::new();
        for effect in &checkpoint.effects {
            validate_effect(effect)?;
            if !ids.insert(effect.id) {
                return Err("mod spell effect checkpoint contains duplicates".into());
            }
        }
        self.state = checkpoint;
        Ok(())
    }

    pub fn retire(&mut self, id: u64) -> bool {
        let Some(index) = self.state.effects.iter().position(|effect| effect.id == id) else {
            return false;
        };
        self.state.effects.remove(index);
        self.state.revision += 1;
        true
    }

    pub fn spawn(
        &mut self,
        input: ModSpellEffectSpawn<'_>,
    ) -> Result<ActiveModSpellEffect, String> {
        if self.state.effects.len() >= 1_024 {
            return Err("active mod spell effect limit reached".into());
        }
        let kind = match input.fields.get("prefab").and_then(LuaValue::as_str) {
            Some("area") => ModSpellEffectKind::Area,
            Some("channel") => ModSpellEffectKind::Channel,
            Some("projectile") => ModSpellEffectKind::Projectile,
            _ => return Err("mod spell effect prefab is invalid".into()),
        };
        let allowed: &[&str] = match kind {
            ModSpellEffectKind::Area => {
                &["duration", "effects", "every", "prefab", "radius", "spell_content_id"]
            }
            ModSpellEffectKind::Channel => {
                &["duration", "effects", "every", "prefab", "spell_content_id", "width"]
            }
            ModSpellEffectKind::Projectile => {
                &["duration", "effects", "prefab", "radius", "speed", "spell_content_id"]
            }
        };
        let unknown: Vec<_> =
            input.fields.iter().map(|(key, _)| key).filter(|key| !allowed.contains(key)).collect();
        if !unknown.is_empty() {
            return Err(format!("spell effect contains unknown fields: {}", unknown.join(", ")));
        }
        let default_duration = LuaValue::String(
            match kind {
                ModSpellEffectKind::Area => "100ms",
                ModSpellEffectKind::Projectile => "2s",
                ModSpellEffectKind::Channel => "1s",
            }
            .into(),
        );
        let default_interval = LuaValue::String(
            if kind == ModSpellEffectKind::Projectile { "10ms" } else { "100ms" }.into(),
        );
        let duration = duration_ticks_value(
            Some(nullish(input.fields.get("duration")).unwrap_or(&default_duration)),
            self.ticks_per_second,
            "effect duration",
        )?;
        let interval = duration_ticks_value(
            Some(nullish(input.fields.get("every")).unwrap_or(&default_interval)),
            self.ticks_per_second,
            "effect interval",
        )?;
        let default_radius = LuaValue::Number(32.0);
        let radius = finite(
            nullish(input.fields.get("radius"))
                .or_else(|| nullish(input.fields.get("width")))
                .unwrap_or(&default_radius),
            1.0,
            1_024.0,
            "effect radius",
        )?;
        let length = hypot(input.target.x - input.origin.x, input.target.y - input.origin.y);
        let speed_per_tick = if kind == ModSpellEffectKind::Projectile {
            finite(
                nullish(input.fields.get("speed")).unwrap_or(&LuaValue::Number(600.0)),
                1.0,
                10_000.0,
                "projectile speed",
            )? / f64::from(self.ticks_per_second)
        } else {
            0.0
        };
        let position = if kind == ModSpellEffectKind::Area { input.target } else { input.origin };
        let effect = ActiveModSpellEffect {
            content_id: input.content_id.into(),
            effects: templates(input.fields.get("effects"))?,
            expires_tick: input.tick + duration,
            hit_targets: Vec::new(),
            id: self.state.next_id,
            interval_ticks: interval,
            kind,
            last_tick: input.tick,
            mod_id: input.mod_id.into(),
            next_pulse_tick: input.tick,
            owner_player_id: input.owner_player_id.into(),
            radius,
            scope: input.scope,
            speed_per_tick,
            started_tick: input.tick,
            target_x: input.target.x,
            target_y: input.target.y,
            x: position.x,
            y: position.y,
        };
        // Source increments the instance sequence before the distinct-target
        // admission check. A rejected zero-length projectile still spends it.
        self.state.next_id += 1;
        if kind == ModSpellEffectKind::Projectile && length == 0.0 {
            return Err("projectile requires a distinct target".into());
        }
        self.state.effects.push(effect.clone());
        self.state.revision += 1;
        Ok(effect)
    }

    pub fn tick(
        &mut self,
        tick: u64,
        players: &[ModActorPlayer],
        targets: &[ModSpellEffectTarget],
    ) -> Result<Vec<ModSpellEffectBatch>, String> {
        validate_tick(tick, "spell effect")?;
        let mut batches = Vec::new();
        let mut retained = Vec::new();
        let effects = std::mem::take(&mut self.state.effects);
        for source in effects {
            if source.expires_tick <= tick
                || source.kind == ModSpellEffectKind::Channel
                    && !players.iter().any(|player| player.id == source.owner_player_id)
            {
                self.state.revision += 1;
                continue;
            }
            let mut effect = advance(&source, players, tick);
            let mut retired = false;
            let mut pulsed = false;
            if effect.kind == ModSpellEffectKind::Projectile {
                let mut candidates: Vec<_> = targets
                    .iter()
                    .filter(|target| {
                        !effect.hit_targets.contains(&target_id(target))
                            && in_projectile_path(&source, &effect, target)
                    })
                    .collect();
                candidates.sort_by(|left, right| {
                    path_progress(&source, &effect, left)
                        .partial_cmp(&path_progress(&source, &effect, right))
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| left.kind.as_str().cmp(right.kind.as_str()))
                        .then(left.id.cmp(&right.id))
                });
                if let Some(target) = candidates.first() {
                    batches.push(self.batch(&effect, &[target]));
                    effect.hit_targets = vec![target_id(target)];
                    retired = true
                } else if effect.x == effect.target_x && effect.y == effect.target_y {
                    retired = true
                }
            } else if effect.next_pulse_tick <= tick {
                let matching: Vec<_> = targets
                    .iter()
                    .filter(|target| {
                        if effect.kind == ModSpellEffectKind::Area {
                            distance_squared(effect.x, effect.y, target.x, target.y)
                                <= effect.radius * effect.radius
                        } else {
                            in_channel(&effect, target)
                        }
                    })
                    .take(256)
                    .collect();
                batches.push(self.batch(&effect, &matching));
                effect.next_pulse_tick += effect.interval_ticks;
                pulsed = true;
            }
            if effect.kind != ModSpellEffectKind::Area || pulsed || retired {
                self.state.revision += 1
            }
            if !retired {
                retained.push(effect)
            }
        }
        self.state.effects = retained;
        Ok(batches)
    }

    fn batch(
        &mut self,
        effect: &ActiveModSpellEffect,
        targets: &[&ModSpellEffectTarget],
    ) -> ModSpellEffectBatch {
        let mut intents = Vec::new();
        for template in &effect.effects {
            let targeted = matches!(
                template.fields.get("target").and_then(LuaValue::as_str),
                Some("hostiles_in_area" | "hostiles_in_channel" | "target_enemy")
            );
            let rows: Vec<_> = if targeted {
                targets.iter().map(|target| Some(*target)).collect()
            } else {
                vec![None]
            };
            for target in rows {
                let mut fields = template.fields.clone();
                fields.insert("effect_id".into(), LuaValue::Number(effect.id as f64));
                if let Some(target) = target {
                    let mut row = LuaObject::new();
                    row.insert("id".into(), LuaValue::Number(target.id as f64));
                    row.insert("kind".into(), LuaValue::String(target.kind.as_str().into()));
                    fields.insert("target".into(), LuaValue::Object(row));
                }
                fields.insert("x".into(), LuaValue::Number(effect.x));
                fields.insert("y".into(), LuaValue::Number(effect.y));
                intents.push(ModSpellEffectIntent {
                    fields,
                    kind: template.kind.clone(),
                    mod_id: effect.mod_id.clone(),
                    owner: format!("spell-effect.{}", effect.id),
                    scope: effect.scope.clone(),
                    sequence: self.state.next_sequence,
                });
                self.state.next_sequence += 1;
            }
        }
        let mut context = LuaObject::new();
        for (key, value) in [
            ("effect_id", LuaValue::Number(effect.id as f64)),
            ("participant_id", LuaValue::String(effect.owner_player_id.clone())),
            ("target_x", LuaValue::Number(effect.target_x)),
            ("target_y", LuaValue::Number(effect.target_y)),
        ] {
            context.insert(key.into(), value);
        }
        ModSpellEffectBatch { context, intents, scope: effect.scope.clone() }
    }
}

fn nullish(value: Option<&LuaValue>) -> Option<&LuaValue> {
    value.filter(|value| !matches!(value, LuaValue::Null))
}
fn finite(value: &LuaValue, minimum: f64, maximum: f64, field: &str) -> Result<f64, String> {
    value
        .as_number()
        .filter(|number| number.is_finite() && *number >= minimum && *number <= maximum)
        .ok_or_else(|| format!("{field} must be finite within {minimum}..{maximum}"))
}

fn templates(value: Option<&LuaValue>) -> Result<Vec<ModEffectTemplate>, String> {
    let Some(LuaValue::Array(values)) = value.filter(
        |value| matches!(value,LuaValue::Array(values) if !values.is_empty() && values.len()<=32),
    ) else {
        return Err("spell effect prefab requires 1..32 effects".into());
    };
    let mut result = Vec::new();
    for value in values {
        let row = value.as_object().ok_or("spell effect entry is invalid")?;
        let operation = row
            .get("operation")
            .and_then(LuaValue::as_str)
            .and_then(|operation| operation.strip_prefix("effect."));
        let fields = row.get("fields").and_then(LuaValue::as_object);
        let (Some(kind), Some(fields)) = (operation, fields) else {
            return Err("spell effect entry must be created by sd.effect".into());
        };
        result.push(ModEffectTemplate { fields: fields.clone(), kind: kind.into() });
    }
    Ok(result)
}

fn advance(
    source: &ActiveModSpellEffect,
    players: &[ModActorPlayer],
    tick: u64,
) -> ActiveModSpellEffect {
    let mut effect = source.clone();
    match source.kind {
        ModSpellEffectKind::Area => {}
        ModSpellEffectKind::Channel => {
            if let Some(owner) = players.iter().find(|player| player.id == source.owner_player_id) {
                effect.last_tick = tick;
                effect.x = owner.x;
                effect.y = owner.y;
            }
        }
        ModSpellEffectKind::Projectile => {
            let elapsed = tick as f64 - source.last_tick as f64;
            let dx = source.target_x - source.x;
            let dy = source.target_y - source.y;
            let length = hypot(dx, dy);
            let distance = length.min(source.speed_per_tick * elapsed);
            effect.last_tick = tick;
            if length != 0.0 {
                effect.x += dx / length * distance;
                effect.y += dy / length * distance
            }
        }
    }
    effect
}

fn path_progress(
    source: &ActiveModSpellEffect,
    effect: &ActiveModSpellEffect,
    target: &ModSpellEffectTarget,
) -> f64 {
    let dx = effect.x - source.x;
    let dy = effect.y - source.y;
    let length = dx * dx + dy * dy;
    if length == 0.0 {
        0.0
    } else {
        (((target.x - source.x) * dx + (target.y - source.y) * dy) / length).clamp(0.0, 1.0)
    }
}
fn in_projectile_path(
    source: &ActiveModSpellEffect,
    effect: &ActiveModSpellEffect,
    target: &ModSpellEffectTarget,
) -> bool {
    let progress = path_progress(source, effect, target);
    distance_squared(
        source.x + (effect.x - source.x) * progress,
        source.y + (effect.y - source.y) * progress,
        target.x,
        target.y,
    ) <= effect.radius * effect.radius
}
fn in_channel(effect: &ActiveModSpellEffect, target: &ModSpellEffectTarget) -> bool {
    let dx = effect.target_x - effect.x;
    let dy = effect.target_y - effect.y;
    let length = dx * dx + dy * dy;
    let progress = if length == 0.0 {
        0.0
    } else {
        (((target.x - effect.x) * dx + (target.y - effect.y) * dy) / length).clamp(0.0, 1.0)
    };
    distance_squared(effect.x + dx * progress, effect.y + dy * progress, target.x, target.y)
        <= effect.radius * effect.radius
}
fn distance_squared(x: f64, y: f64, other_x: f64, other_y: f64) -> f64 {
    let dx = other_x - x;
    let dy = other_y - y;
    dx * dx + dy * dy
}
fn target_id(target: &ModSpellEffectTarget) -> String {
    format!("{}:{}", target.kind.as_str(), target.id)
}

fn validate_effect(effect: &ActiveModSpellEffect) -> Result<(), String> {
    if effect.id == 0
        || effect.id > MAXIMUM_SAFE_INTEGER
        || effect.started_tick > MAXIMUM_SAFE_INTEGER
        || effect.expires_tick > MAXIMUM_SAFE_INTEGER
        || effect.expires_tick <= effect.started_tick
        || effect.last_tick > MAXIMUM_SAFE_INTEGER
        || effect.last_tick < effect.started_tick
        || effect.next_pulse_tick > MAXIMUM_SAFE_INTEGER
        || effect.next_pulse_tick < effect.started_tick
        || effect.interval_ticks == 0
        || effect.interval_ticks > MAXIMUM_SAFE_INTEGER
        || effect.content_id.is_empty()
        || effect.mod_id.is_empty()
        || effect.owner_player_id.is_empty()
        || effect.scope.id.is_empty()
        || [
            effect.x,
            effect.y,
            effect.target_x,
            effect.target_y,
            effect.radius,
            effect.speed_per_tick,
        ]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err("mod spell effect checkpoint contains an invalid effect".into());
    }
    if effect.effects.is_empty() || effect.effects.len() > 32 {
        return Err("spell effect prefab requires 1..32 effects".into());
    }
    Ok(())
}
