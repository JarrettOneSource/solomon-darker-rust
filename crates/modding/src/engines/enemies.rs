//! Authored chase/attack/tombstone authority from `mod-enemy-engine.ts`.

use std::collections::HashSet;
use std::sync::Arc;

use solomon_sim::Vec2;
use solomon_sim::float::{atan2, hypot};
use solomon_sim::modding::content::PreparedModContentCatalog;
use solomon_sim::modding::runtime::{
    ActiveModEnemy, ModActorPlayer, ModEnemyAttack, ModEnemyCheckpoint, ModEnemyLifeState,
};

use super::{MAXIMUM_SAFE_INTEGER, duration_ticks, validate_tick, validate_tick_rate};
use crate::json::SourceCollator;

pub struct ModEnemyEngine {
    catalog: Arc<PreparedModContentCatalog>,
    ticks_per_second: u32,
    state: ModEnemyCheckpoint,
    collator: SourceCollator,
}

impl ModEnemyEngine {
    pub fn new(
        catalog: Arc<PreparedModContentCatalog>,
        ticks_per_second: u32,
    ) -> Result<Self, String> {
        validate_tick_rate(ticks_per_second, "enemy")?;
        Ok(Self {
            catalog,
            ticks_per_second,
            state: ModEnemyCheckpoint::default(),
            collator: SourceCollator::default(),
        })
    }

    pub fn revision(&self) -> u64 {
        self.state.revision
    }
    pub fn checkpoint(&self) -> ModEnemyCheckpoint {
        self.state.clone()
    }
    pub fn project(&self) -> &[ActiveModEnemy] {
        &self.state.enemies
    }

    pub fn collision_radius(&self, content_id: &str) -> Result<f64, String> {
        self.catalog
            .enemy(content_id)
            .map(|definition| definition.collision_radius)
            .ok_or_else(|| format!("mod enemy is unavailable: {content_id}"))
    }

    pub fn clear(&mut self) {
        if self.state.enemies.is_empty() {
            return;
        }
        self.state.enemies.clear();
        self.state.revision += 1;
    }

    pub fn damage(
        &mut self,
        id: u64,
        amount: f64,
        tick: u64,
        source_player_id: Option<&str>,
    ) -> Result<Option<ActiveModEnemy>, String> {
        if !amount.is_finite() || amount < 0.0 || tick > MAXIMUM_SAFE_INTEGER {
            return Err("mod enemy damage is invalid".into());
        }
        let Some(enemy) = self
            .state
            .enemies
            .iter_mut()
            .find(|enemy| enemy.id == id && enemy.life_state == ModEnemyLifeState::Alive)
        else {
            return Ok(None);
        };
        enemy.current_health = (enemy.current_health - amount).max(0.0);
        enemy.death_tick = (enemy.current_health == 0.0).then_some(tick);
        if let Some(player_id) = source_player_id {
            enemy.last_damaged_by_player_id = Some(player_id.into())
        }
        if enemy.current_health == 0.0 {
            enemy.life_state = ModEnemyLifeState::Dying;
            enemy.moving = false
        }
        self.state.revision += 1;
        Ok(Some(enemy.clone()))
    }

    pub fn restore(&mut self, checkpoint: ModEnemyCheckpoint) -> Result<(), String> {
        if checkpoint.next_id == 0
            || checkpoint.next_id > MAXIMUM_SAFE_INTEGER
            || checkpoint.revision > MAXIMUM_SAFE_INTEGER
            || checkpoint.enemies.len() > 2_048
        {
            return Err("mod enemy checkpoint is invalid".into());
        }
        let mut ids = HashSet::new();
        for enemy in &checkpoint.enemies {
            let definition = self.catalog.enemy(&enemy.content_id);
            let alive = enemy.current_health > 0.0 && enemy.death_tick.is_none();
            let dying = enemy.current_health == 0.0 && enemy.death_tick.is_some();
            if definition.is_none_or(|definition| definition.health != enemy.maximum_health)
                || enemy.id == 0
                || enemy.id > MAXIMUM_SAFE_INTEGER
                || !ids.insert(enemy.id)
                || !enemy.current_health.is_finite()
                || enemy.current_health < 0.0
                || enemy.current_health > enemy.maximum_health
                || !enemy.x.is_finite()
                || !enemy.y.is_finite()
                || enemy.heading_index > 15
                || enemy
                    .last_attack_tick
                    .is_some_and(|tick| tick < enemy.spawned_tick || tick > MAXIMUM_SAFE_INTEGER)
                || enemy.next_attack_tick < enemy.spawned_tick
                || enemy.next_attack_tick > MAXIMUM_SAFE_INTEGER
                || (enemy.life_state == ModEnemyLifeState::Alive) != alive
                || (enemy.life_state == ModEnemyLifeState::Dying) != dying
            {
                return Err("mod enemy checkpoint contains an invalid enemy".into());
            }
        }
        self.state = checkpoint;
        Ok(())
    }

    pub fn spawn(
        &mut self,
        content_id: &str,
        position: Vec2,
        tick: u64,
    ) -> Result<ActiveModEnemy, String> {
        let definition = self
            .catalog
            .enemy(content_id)
            .ok_or_else(|| format!("mod enemy is unavailable: {content_id}"))?;
        if !position.x.is_finite() || !position.y.is_finite() || tick > MAXIMUM_SAFE_INTEGER {
            return Err("mod enemy spawn is invalid".into());
        }
        if self.state.enemies.len() >= 2_048 {
            return Err("active mod enemy limit reached".into());
        }
        let enemy = ActiveModEnemy {
            content_id: content_id.into(),
            current_health: definition.health,
            death_tick: None,
            heading_index: 0,
            id: self.state.next_id,
            last_attack_tick: None,
            last_damaged_by_player_id: None,
            life_state: ModEnemyLifeState::Alive,
            maximum_health: definition.health,
            moving: false,
            next_attack_tick: tick
                + duration_ticks(definition.attack_cooldown_ms, self.ticks_per_second).max(1),
            spawned_tick: tick,
            target_player_id: None,
            x: position.x,
            y: position.y,
        };
        self.state.next_id += 1;
        self.state.revision += 1;
        self.state.enemies.push(enemy.clone());
        Ok(enemy)
    }

    pub fn tick(
        &mut self,
        tick: u64,
        players: &[ModActorPlayer],
        mut movement: impl FnMut(Vec2, Vec2, f64) -> Vec2,
    ) -> Result<Vec<ModEnemyAttack>, String> {
        validate_tick(tick, "enemy")?;
        let mut attacks = Vec::new();
        let mut retained: Vec<ActiveModEnemy> = Vec::with_capacity(self.state.enemies.len());
        let mut changed = false;
        for enemy in &self.state.enemies {
            if enemy.life_state == ModEnemyLifeState::Dying {
                if tick as f64 - enemy.death_tick.expect("dying enemy has a death tick") as f64
                    >= 50.0
                {
                    changed = true
                } else {
                    retained.push(enemy.clone())
                }
                continue;
            }
            let target = players.iter().min_by(|left, right| {
                let difference = distance_squared(enemy.x, enemy.y, left.x, left.y)
                    - distance_squared(enemy.x, enemy.y, right.x, right.y);
                difference
                    .partial_cmp(&0.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| self.collator.compare(&left.id, &right.id))
            });
            let Some(target) = target else {
                let mut next = enemy.clone();
                next.target_player_id = None;
                next.moving = false;
                changed |= enemy.target_player_id.is_some() || enemy.moving;
                retained.push(next);
                continue;
            };
            let definition = self
                .catalog
                .enemy(&enemy.content_id)
                .expect("active enemy references accepted content");
            let dx = target.x - enemy.x;
            let dy = target.y - enemy.y;
            let distance = hypot(dx, dy);
            let can_attack = distance <= definition.attack_range;
            let origin = Vec2::new(enemy.x, enemy.y);
            let requested = if can_attack || distance == 0.0 {
                origin
            } else {
                movement(
                    origin,
                    Vec2::new(
                        enemy.x + dx / distance * definition.speed.min(distance),
                        enemy.y + dy / distance * definition.speed.min(distance),
                    ),
                    definition.collision_radius,
                )
            };
            let blocked = retained.iter().any(|other| {
                if other.life_state != ModEnemyLifeState::Alive {
                    return false;
                }
                let radius = definition.collision_radius
                    + self
                        .catalog
                        .enemy(&other.content_id)
                        .expect("active enemy references accepted content")
                        .collision_radius;
                distance_squared(requested.x, requested.y, other.x, other.y) < radius * radius
            });
            let position = if blocked { origin } else { requested };
            let attacked = can_attack && tick >= enemy.next_attack_tick;
            if attacked {
                attacks.push(ModEnemyAttack {
                    amount: definition.attack_damage,
                    enemy_id: enemy.id,
                    player_id: target.id.clone(),
                })
            }
            let mut next = enemy.clone();
            let turn = (atan2(dy, dx) / (std::f64::consts::PI * 2.0) + 1.0) % 1.0;
            next.heading_index = (turn * 16.0 + 0.5).floor() as u64 % 16;
            if attacked {
                next.last_attack_tick = Some(tick);
                next.next_attack_tick = tick
                    + duration_ticks(definition.attack_cooldown_ms, self.ticks_per_second).max(1)
            }
            next.moving = position.x != enemy.x || position.y != enemy.y;
            next.target_player_id = Some(target.id.clone());
            next.x = position.x;
            next.y = position.y;
            changed |= next.x != enemy.x
                || next.y != enemy.y
                || next.heading_index != enemy.heading_index
                || next.moving != enemy.moving
                || next.next_attack_tick != enemy.next_attack_tick
                || next.target_player_id != enemy.target_player_id;
            retained.push(next);
        }
        self.state.enemies = retained;
        if changed || !attacks.is_empty() {
            self.state.revision += 1
        }
        Ok(attacks)
    }
}

fn distance_squared(x: f64, y: f64, other_x: f64, other_y: f64) -> f64 {
    let dx = other_x - x;
    let dy = other_y - y;
    dx * dx + dy * dy
}
