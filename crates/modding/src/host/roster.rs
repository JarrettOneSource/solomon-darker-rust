//! Authored initial/wave roster placement from `prepared-mod-host.ts`.

use solomon_sim::Vec2;
use solomon_sim::enemies::EnemyToken;

use crate::host_intents::{resolve_spawn, resolved_reference};

use super::*;

impl PreparedModHost {
    pub(super) fn spawn_boneyard_roster(
        &mut self,
        content_id: &str,
        tick: u64,
        wave: Option<f64>,
        game: &GameSession,
    ) -> Result<(), String> {
        let world = game.boneyard.as_ref().ok_or("Boneyard roster requires an active Boneyard")?;
        let definition = self
            .runtime
            .content
            .boneyard(content_id)
            .ok_or("active Web Lua Boneyard is unavailable")?;
        let mut roster = Vec::new();
        if let Some(wave) = wave {
            for row in &definition.waves {
                if row.get("wave").and_then(LuaValue::as_number) == Some(wave)
                    || row.get("ordinal").and_then(LuaValue::as_number) == Some(wave)
                {
                    let Some(LuaValue::Array(entries)) = row.get("roster") else {
                        return Err("Boneyard wave roster is invalid".into());
                    };
                    roster.extend(entries.iter());
                }
            }
        } else {
            roster.extend(definition.roster.iter());
        }
        let entry = definition.anchors.get("entry").and_then(LuaValue::as_object);
        let anchor = entry
            .and_then(|entry| {
                let x = entry.get("x")?.as_number()?;
                let y = entry.get("y")?.as_number()?;
                (x.is_finite() && y.is_finite()).then_some(Vec2::new(x, y))
            })
            .unwrap_or(world.loaded.scene.spawn.position);
        for (index, entry) in roster.iter().enumerate() {
            let angle = index as f64 / roster.len().max(1) as f64 * std::f64::consts::PI * 2.0;
            let radius = 48.0 + (index / 8) as f64 * 42.0;
            let requested = Vec2::new(
                anchor.x + solomon_sim::float::cos(angle) * radius,
                anchor.y + solomon_sim::float::sin(angle) * radius,
            );
            if matches!(entry, LuaValue::Object(_)) {
                let reference = resolved_reference(Some(entry), "Boneyard roster enemy")?;
                if reference.target_kind != "enemy" {
                    return Err("Boneyard roster entry must reference an enemy".into());
                }
                let radius = self.runtime.enemies.collision_radius(reference.content_id)?;
                let position = resolve_spawn(world, requested, radius)?;
                self.runtime.enemies.spawn(reference.content_id, position, tick)?;
                continue;
            }
            let value = entry.as_str().ok_or("Boneyard roster entry is invalid")?;
            let normalized =
                value.strip_prefix("stock.").unwrap_or(value).replace('_', "").to_uppercase();
            let token = EnemyToken::from_id(&normalized)
                .filter(|token| *token != EnemyToken::Portal)
                .ok_or_else(|| format!("Boneyard stock enemy is unavailable: {value}"))?;
            // Source evaluates this arithmetic as a JS number before remainder.
            let id = 0x5000_0000
                + ((tick as f64 * 131.0 + index as f64 + self.enemy_spawns.len() as f64)
                    % f64::from(0x0fff_ffff_u32)) as usize;
            let ordinal = wave.map(|wave| wave as u64).unwrap_or_else(|| {
                world.waves.as_ref().map_or(0, |waves| waves.state.wave_ordinal)
            });
            self.enemy_spawns.push(EnemySpawnIntent::new(token, requested, id, tick + 1, ordinal));
        }
        Ok(())
    }
}
