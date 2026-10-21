//! Ordered pickup candidates and short collection events from `mod-powerup-engine.ts`.

use std::collections::HashSet;
use std::sync::Arc;

use solomon_sim::Vec2;
use solomon_sim::modding::content::PreparedModContentCatalog;
use solomon_sim::modding::runtime::{
    ActiveModPowerup, ModActorPlayer, ModPowerupCheckpoint, ModPowerupCollectionEvent,
};

use super::{MAXIMUM_SAFE_INTEGER, validate_tick};

pub struct ModPowerupEngine {
    catalog: Arc<PreparedModContentCatalog>,
    state: ModPowerupCheckpoint,
    collections: Vec<ModPowerupCollectionEvent>,
}

pub struct ModPowerupCandidate<'a> {
    pub instance: &'a ActiveModPowerup,
    pub player_id: &'a str,
}

impl ModPowerupEngine {
    pub fn new(catalog: Arc<PreparedModContentCatalog>) -> Self {
        Self { catalog, state: ModPowerupCheckpoint::default(), collections: Vec::new() }
    }
    pub fn revision(&self) -> u64 {
        self.state.revision
    }
    pub fn checkpoint(&self) -> ModPowerupCheckpoint {
        self.state.clone()
    }
    pub fn project(&self) -> &[ActiveModPowerup] {
        &self.state.instances
    }
    pub fn project_collections(&self) -> &[ModPowerupCollectionEvent] {
        &self.collections
    }

    pub fn collect(
        &mut self,
        id: u64,
        player_id: &str,
        tick: u64,
    ) -> Result<Option<ModPowerupCollectionEvent>, String> {
        if tick > MAXIMUM_SAFE_INTEGER {
            return Err("mod powerup collection tick is invalid".into());
        }
        let Some(index) = self.state.instances.iter().position(|instance| instance.id == id) else {
            return Ok(None);
        };
        let instance = self.state.instances.remove(index);
        let event = ModPowerupCollectionEvent {
            content_id: instance.content_id,
            id: instance.id,
            player_id: player_id.into(),
            tick,
            x: instance.x,
            y: instance.y,
        };
        self.collections.push(event.clone());
        if self.collections.len() > 256 {
            self.collections.remove(0);
        }
        self.state.revision += 1;
        Ok(Some(event))
    }

    pub fn clear(&mut self) {
        if self.state.instances.is_empty() && self.collections.is_empty() {
            return;
        }
        self.state.instances.clear();
        self.collections.clear();
        self.state.revision += 1;
    }

    pub fn candidates<'a>(&'a self, players: &'a [ModActorPlayer]) -> Vec<ModPowerupCandidate<'a>> {
        let mut candidates = Vec::new();
        for instance in &self.state.instances {
            let definition = self
                .catalog
                .powerup(&instance.content_id)
                .expect("active powerup references accepted content");
            let maximum = definition.pickup_radius * definition.pickup_radius;
            if let Some(player) = players.iter().find(|player| {
                let dx = player.x - instance.x;
                let dy = player.y - instance.y;
                dx * dx + dy * dy < maximum
            }) {
                candidates.push(ModPowerupCandidate { instance, player_id: &player.id })
            }
        }
        candidates
    }

    pub fn restore(&mut self, checkpoint: ModPowerupCheckpoint) -> Result<(), String> {
        if checkpoint.next_id == 0
            || checkpoint.next_id > MAXIMUM_SAFE_INTEGER
            || checkpoint.revision > MAXIMUM_SAFE_INTEGER
            || checkpoint.instances.len() > 1_024
        {
            return Err("mod powerup checkpoint is invalid".into());
        }
        let mut ids = HashSet::new();
        for instance in &checkpoint.instances {
            let definition = self.catalog.powerup(&instance.content_id);
            if instance.id == 0
                || instance.id > MAXIMUM_SAFE_INTEGER
                || !ids.insert(instance.id)
                || definition.is_none_or(|definition| definition.common.mod_id != instance.mod_id)
                || !instance.x.is_finite()
                || !instance.y.is_finite()
                || instance.spawned_tick > MAXIMUM_SAFE_INTEGER
            {
                return Err("mod powerup checkpoint contains an invalid instance".into());
            }
        }
        self.state = checkpoint;
        self.collections.clear();
        Ok(())
    }

    pub fn spawn(
        &mut self,
        content_id: &str,
        position: Vec2,
        tick: u64,
    ) -> Result<ActiveModPowerup, String> {
        let definition = self
            .catalog
            .powerup(content_id)
            .ok_or_else(|| format!("mod powerup is unavailable: {content_id}"))?;
        if !position.x.is_finite() || !position.y.is_finite() || tick > MAXIMUM_SAFE_INTEGER {
            return Err("mod powerup spawn is invalid".into());
        }
        if self.state.instances.len() >= 1_024 {
            return Err("active mod powerup limit reached".into());
        }
        let instance = ActiveModPowerup {
            content_id: content_id.into(),
            id: self.state.next_id,
            mod_id: definition.common.mod_id.clone(),
            spawned_tick: tick,
            x: position.x,
            y: position.y,
        };
        self.state.next_id += 1;
        self.state.revision += 1;
        self.state.instances.push(instance.clone());
        Ok(instance)
    }

    pub fn tick(&mut self, tick: u64) -> Result<bool, String> {
        validate_tick(tick, "powerup")?;
        let before = self.collections.len();
        self.collections.retain(|event| (tick as f64 - event.tick as f64) < 50.0);
        let changed = before != self.collections.len();
        if changed {
            self.state.revision += 1
        }
        Ok(changed)
    }
}
