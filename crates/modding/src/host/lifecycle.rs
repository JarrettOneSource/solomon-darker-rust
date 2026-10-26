//! Ordered outer-host epochs from `host/prepared-mod-host.ts::step/tick`.

use solomon_sim::boneyard::collision::resolve_boneyard_movement;
use solomon_sim::combat::PlayerLifeState;
use solomon_sim::enemies::model::EnemyLifeState;
use solomon_sim::modding::extensions::{
    GameSimulationDamageFilterInput, GameSimulationDamageKind, GameSimulationExtensions,
};

use crate::intents::{ModIntentAdapter, ModIntentExecutionContext};

use super::*;

impl PreparedModHost {
    pub fn activate_boneyard(
        &mut self,
        content_id: Option<&str>,
        restore_existing: bool,
        game: &GameSession,
    ) -> Result<(), String> {
        self.require_open()?;
        self.reported_enemy_health.clear();
        if restore_existing {
            if content_id.is_some_and(|id| self.runtime.content.boneyard(id).is_none()) {
                return Err("restored Web Lua Boneyard is unavailable".into());
            }
            self.active_boneyard_content_id = content_id.map(str::to_owned);
            return Ok(());
        }
        let checkpoint = self.runtime.enemies.checkpoint();
        let spawn_count = self.enemy_spawns.len();
        self.runtime.enemies.clear();
        self.active_boneyard_content_id = content_id.map(str::to_owned);
        if let Some(id) = content_id
            && let Err(error) = self.spawn_boneyard_roster(id, game.ticks, None, game)
        {
            self.runtime.enemies.restore(checkpoint)?;
            self.enemy_spawns.truncate(spawn_count);
            self.active_boneyard_content_id = None;
            return Err(error);
        }
        Ok(())
    }

    pub fn step(
        &mut self,
        events: &[PreparedModDerivedEvent],
        tick: u64,
        scope_id: &str,
        context: &LuaObject,
        game: &mut GameSession,
    ) -> Result<PreparedModStepResult, String> {
        self.require_open()?;
        for event in events {
            let Some(payload) = event.payload.as_object() else { continue };
            if event.name == "level.up"
                && let Some(id) = payload.get("player_id").and_then(LuaValue::as_str)
                && let Some(level) = payload.get("level").and_then(safe_integer)
                && level > 0
            {
                self.runtime.skills.offer(id, level, tick, 3)?;
            }
            if event.name == "wave.started"
                && let Some(id) = self.active_boneyard_content_id.clone()
                && let Some(wave) = payload.get("wave").and_then(LuaValue::as_number)
                && wave.is_finite()
                && wave.fract() == 0.0
                && wave.abs() <= 9_007_199_254_740_991.0
            {
                self.spawn_boneyard_roster(&id, tick, Some(wave), game)?;
            }
        }
        let mut inputs = Vec::with_capacity(events.len());
        for event in events {
            let mut fields = context.clone();
            fields.insert(
                "active_boneyard_content_id".into(),
                self.active_boneyard_content_id.clone().map_or(LuaValue::Null, LuaValue::String),
            );
            fields.insert("event".into(), LuaValue::String(event.name.clone()));
            inputs.push(PreparedModEvent {
                context: fields,
                event: event.name.clone(),
                payload: event.payload.clone(),
                scope: ModStateScope { id: scope_id.into(), kind: WebLuaScopeKind::PartyRun },
            });
        }
        let result = self.step_raw(&PreparedModStepInput { events: inputs, tick }, game)?;
        if events.iter().any(|event| event.name == "run.ended") {
            self.session.close_run(scope_id)?;
        }
        Ok(result)
    }

    pub fn tick(&mut self, tick: u64, game: &mut GameSession) -> Result<bool, String> {
        self.require_open()?;
        let before_revision = self.tick_revision();
        self.runtime.statuses.tick(tick)?;
        self.presentation.retain(|event| tick as f64 - (event.tick as f64) < 100.0);
        self.runtime.shops.tick(tick)?;
        self.runtime.powerups.tick(tick)?;

        // Source samples living player positions once, before custom attacks.
        // Effects and pickups use that same sample even if an attack is lethal.
        let mut players = Vec::new();
        for id in &game.participant_order {
            let participant = &game.participants.get(id).expect("participant exists");
            if participant.player.combat.life_state == PlayerLifeState::Alive {
                players.push(ModActorPlayer {
                    id: id.clone(),
                    x: participant.wizard.position.x,
                    y: participant.wizard.position.y,
                });
            }
        }
        let mut native_targets = Vec::new();
        if let Some(world) = &game.boneyard {
            for actor in world.enemies.actors.iter() {
                if actor.life_state == EnemyLifeState::Alive {
                    native_targets.push(ModSpellEffectTarget {
                        id: actor.id as u64,
                        kind: ModSpellEffectTargetKind::NativeEnemy,
                        x: actor.position.x,
                        y: actor.position.y,
                    });
                }
            }
            let attacks =
                if world.encounter.as_ref().is_none_or(|encounter| encounter.combat_enabled()) {
                    let mut collision = world.collision.clone();
                    collision.set_gate_leaves(&world.gate_leaves);
                    self.runtime.enemies.tick(tick, &players, |start, requested, radius| {
                        resolve_boneyard_movement(
                            start,
                            requested,
                            world.loaded.scene.bounds,
                            &collision,
                            radius,
                        )
                    })?
                } else {
                    Vec::new()
                };
            if !attacks.is_empty() {
                // Source writes the full attack group only after every filter
                // succeeds. Never leave a partial damage batch on an error.
                let mut attacked = game.clone();
                for attack in attacks {
                    let participant = attacked
                        .participants
                        .get_mut(&attack.player_id)
                        .expect("sampled living player remains in this synchronous epoch");
                    let amount = self.runtime.filter_damage(
                        GameSimulationDamageFilterInput {
                            amount: attack.amount,
                            damage_kind: GameSimulationDamageKind::Physical,
                            source_actor_id: None,
                            target_player_id: &attack.player_id,
                            tick,
                        },
                        &participant.player.economy.equipment,
                    )?;
                    attacked.damage_player(&attack.player_id, amount, tick);
                }
                *game = attacked;
            }
        } else {
            self.runtime.enemies.clear();
            self.runtime.powerups.clear();
            self.active_boneyard_content_id = None;
        }
        let mut targets = native_targets;
        for enemy in self.runtime.enemies.project() {
            if enemy.life_state == ModEnemyLifeState::Alive {
                targets.push(ModSpellEffectTarget {
                    id: enemy.id,
                    kind: ModSpellEffectTargetKind::ModEnemy,
                    x: enemy.x,
                    y: enemy.y,
                });
            }
        }
        for batch in self.runtime.spell_effects.tick(tick, &players, &targets)? {
            if batch.intents.is_empty() {
                continue;
            }
            let context =
                ModIntentExecutionContext { context: batch.context, scope: batch.scope, tick };
            let failed_commit = {
                let mut adapter = HostIntentAdapter {
                    game,
                    runtime: &mut self.runtime,
                    controls: &mut self.controls,
                    spawns: &mut self.enemy_spawns,
                    presentation: &mut self.presentation,
                };
                // A prepare failure is fatal in Source. Only a failed commit
                // retires this effect and logs its rejected transaction.
                let mut transaction = adapter.prepare(&batch.intents, &context)?;
                match transaction.commit() {
                    Ok(()) => None,
                    Err(reason) => {
                        transaction.rollback(&reason)?;
                        Some(reason)
                    }
                }
            };
            if let Some(reason) = failed_commit {
                if let Some(id) = context.context.get("effect_id").and_then(LuaValue::as_number) {
                    self.runtime.spell_effects.retire(id as u64);
                }
                self.logs.push(format!(
                    "{}:{}: {reason}",
                    context.scope.kind.as_str(),
                    context.scope.id
                ));
            }
        }
        let candidates: Vec<_> = self
            .runtime
            .powerups
            .candidates(&players)
            .iter()
            .map(|candidate| (candidate.player_id.to_owned(), candidate.instance.clone()))
            .collect();
        for (player_id, instance) in candidates {
            let input = PreparedModActionInput {
                action: "content.pickup".into(),
                context: object([
                    ("participant_id", LuaValue::String(player_id.clone())),
                    ("powerup_id", LuaValue::Number(instance.id as f64)),
                ]),
                payload: object_value([
                    ("content_id", LuaValue::String(instance.content_id)),
                    ("participant_id", LuaValue::String(player_id.clone())),
                    ("powerup_id", LuaValue::Number(instance.id as f64)),
                ]),
                request_id: instance.id,
                scope: participant_scope(game, &player_id),
                tick,
            };
            let mut adapter = HostIntentAdapter {
                game,
                runtime: &mut self.runtime,
                controls: &mut self.controls,
                spawns: &mut self.enemy_spawns,
                presentation: &mut self.presentation,
            };
            // Source pickup actions intentionally do not use report().
            let result = self.session.act(&input, &mut adapter)?;
            if result.accepted {
                self.runtime.powerups.collect(instance.id, &player_id, tick)?;
            }
        }
        let enemies = self.runtime.enemies.project().to_vec();
        for enemy in &enemies {
            if self
                .reported_enemy_health
                .get(&enemy.id)
                .is_some_and(|previous| enemy.current_health < *previous)
            {
                let mut context = object([("mod_enemy_id", LuaValue::Number(enemy.id as f64))]);
                if let Some(id) = &enemy.last_damaged_by_player_id {
                    context.insert("participant_id".into(), LuaValue::String(id.clone()));
                }
                let event = PreparedModEvent {
                    context,
                    event: "mod.enemy.damaged".into(),
                    payload: object_value([
                        ("content_id", LuaValue::String(enemy.content_id.clone())),
                        ("current_health", LuaValue::Number(enemy.current_health)),
                        ("enemy_id", LuaValue::Number(enemy.id as f64)),
                        ("maximum_health", LuaValue::Number(enemy.maximum_health)),
                    ]),
                    scope: ModStateScope {
                        id: format!("{}:{}", enemy.content_id, enemy.id),
                        kind: WebLuaScopeKind::Entity,
                    },
                };
                let mut events = vec![event];
                if enemy.current_health == 0.0 {
                    let mut death = events[0].clone();
                    death.event = "mod.enemy.died".into();
                    events.push(death);
                }
                self.step_raw(&PreparedModStepInput { events, tick }, game)?;
            }
            self.reported_enemy_health.insert(enemy.id, enemy.current_health);
        }
        self.reported_enemy_health.retain(|id, _| enemies.iter().any(|enemy| enemy.id == *id));
        Ok(before_revision != self.tick_revision())
    }

    fn tick_revision(&self) -> u64 {
        self.runtime.statuses.revision()
            + self.runtime.powerups.revision()
            + self.runtime.skills.revision()
            + self.runtime.semantic_state.revision()
            + self.runtime.shops.revision()
            + self.runtime.spell_effects.revision()
    }
}

fn safe_integer(value: &LuaValue) -> Option<u64> {
    value
        .as_number()
        .filter(|value| {
            value.is_finite()
                && value.fract() == 0.0
                && *value >= 0.0
                && *value <= 9_007_199_254_740_991.0
        })
        .map(|value| value as u64)
}
