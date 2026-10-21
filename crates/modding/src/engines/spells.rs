//! Mana admission and retained cooldowns from `mod-spell-engine.ts`.

use std::collections::HashSet;
use std::sync::Arc;

use solomon_sim::modding::content::{PreparedModContentCatalog, PreparedModSpellDefinition};
use solomon_sim::modding::runtime::{ModSpellCheckpoint, ModSpellCooldown};

use super::{MAXIMUM_SAFE_INTEGER, duration_ticks, validate_player, validate_tick_rate};

pub struct ModSpellEngine {
    catalog: Arc<PreparedModContentCatalog>,
    ticks_per_second: u32,
    state: ModSpellCheckpoint,
}

pub struct ModSpellAdmission<'a> {
    pub mana_cost: f64,
    pub ready_tick: u64,
    pub spell: &'a PreparedModSpellDefinition,
}

impl ModSpellEngine {
    pub fn new(
        catalog: Arc<PreparedModContentCatalog>,
        ticks_per_second: u32,
    ) -> Result<Self, String> {
        validate_tick_rate(ticks_per_second, "spell")?;
        Ok(Self { catalog, ticks_per_second, state: ModSpellCheckpoint::default() })
    }

    pub fn revision(&self) -> u64 {
        self.state.revision
    }

    pub fn admit(
        &mut self,
        player_id: &str,
        content_id: &str,
        tick: u64,
        current_mana: f64,
        mana_cost: Option<f64>,
    ) -> Result<ModSpellAdmission<'_>, String> {
        let spell = self
            .catalog
            .spell(content_id)
            .ok_or_else(|| format!("mod spell is unavailable: {content_id}"))?;
        let cost = mana_cost.unwrap_or(spell.mana);
        if tick > MAXIMUM_SAFE_INTEGER
            || !current_mana.is_finite()
            || current_mana < 0.0
            || !cost.is_finite()
            || !(0.0..=1_000_000.0).contains(&cost)
        {
            return Err("mod spell admission input is invalid".into());
        }
        validate_player(player_id, "spell")?;
        let index = self
            .state
            .cooldowns
            .iter()
            .position(|row| row.player_id == player_id && row.content_id == content_id);
        if index.is_some_and(|index| self.state.cooldowns[index].ready_tick > tick) {
            return Err("mod spell is cooling down".into());
        }
        if current_mana < cost {
            return Err("mod spell has insufficient mana".into());
        }
        if index.is_none() && self.state.cooldowns.len() >= 4_096 {
            return Err("mod spell cooldown limit reached".into());
        }
        let ready_tick = tick + duration_ticks(spell.cooldown_ms, self.ticks_per_second);
        if let Some(index) = index {
            self.state.cooldowns[index].ready_tick = ready_tick;
        } else {
            self.state.cooldowns.push(ModSpellCooldown {
                content_id: content_id.into(),
                player_id: player_id.into(),
                ready_tick,
            });
        }
        self.state.revision += 1;
        Ok(ModSpellAdmission { mana_cost: cost, ready_tick, spell })
    }

    pub fn checkpoint(&self) -> ModSpellCheckpoint {
        self.state.clone()
    }

    pub fn project(&self, player_id: Option<&str>) -> Vec<ModSpellCooldown> {
        self.state
            .cooldowns
            .iter()
            .filter(|row| player_id.is_none_or(|id| row.player_id == id))
            .cloned()
            .collect()
    }

    pub fn restore(&mut self, checkpoint: ModSpellCheckpoint) -> Result<(), String> {
        if checkpoint.revision > MAXIMUM_SAFE_INTEGER || checkpoint.cooldowns.len() > 4_096 {
            return Err("mod spell checkpoint is invalid".into());
        }
        let mut keys = HashSet::new();
        for row in &checkpoint.cooldowns {
            validate_player(&row.player_id, "spell")?;
            if self.catalog.spell(&row.content_id).is_none()
                || !keys.insert((&row.player_id, &row.content_id))
                || row.ready_tick > MAXIMUM_SAFE_INTEGER
            {
                return Err("mod spell checkpoint contains an invalid cooldown".into());
            }
        }
        self.state = checkpoint;
        Ok(())
    }
}
