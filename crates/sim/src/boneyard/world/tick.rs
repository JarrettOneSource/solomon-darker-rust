//! Normal Boneyard world epoch from `core-server/boneyard-world.ts`.
//!
//! The session owns player resources, secondary/primary phases and the one painter allocator.
//! This epoch owns movement, loot, encounter/waves, enemy authority and real terminal rewards.

use std::cell::Cell;
use std::collections::BTreeMap;

use crate::actor_physics::resolve_actor_motion;
use crate::economy::HubInventoryItem;
use crate::enemies::config::EnemyArenaScalars;
use crate::enemies::model::{EnemyEvent, EnemyPlayerDamage, EnemyReward, EnemyTarget};
use crate::enemies::world::NativeEnemyMovementView;
use crate::enemies::world::{EnemyAbilityEffect, EnemyStepContext};
use crate::enemies::{EnemySpawnIntent, EnemyToken};
use crate::hub::PLAYER_CHARACTER_RADIUS;
use crate::loot::NativeLootModifiers;
use crate::painter::{PainterRegistration, WorldPainterOrder};
use crate::secondary::NativeSecondaryActorKind;
use crate::secondary::golem::NATIVE_GOLEM_RADIUS;
use crate::{HubWizard, Vec2};

use super::super::loot::{BoneyardLootParticipant, BoneyardLootStep};
use super::super::navigation::NavigationSet;
use super::super::projectile_world::BoneyardProjectileViewport;
use super::super::tutorial::{
    NATIVE_TUTORIAL_CAMERA_TARGET, native_tutorial_enemy_camera_position_is_allowed,
    native_tutorial_enemy_spawn_position_is_allowed, native_tutorial_hostile_scene_paused,
};
use super::super::waves::director::WaveTickContext;
use super::super::world_geometry::{
    BoneyardEnemyGeometry, BoneyardGeometryInput, BoneyardPhysicsWorld,
};
use super::super::world_light::{
    BoneyardLightEnvironment, WorldLightPlayer, WorldLightScene, boneyard_world_light_query,
};
use super::rewards::LootEnemySnapshot;
use super::targets::{boneyard_mouth_world_targets, inside_bounds};
use super::{
    BoneyardMovementInput, BoneyardMovementParticipant, BoneyardMovementStep, BoneyardPlayerBody,
    BoneyardPlayerStatus, BoneyardWorld,
};

/// Borrowed player values sampled by the session after Tutorial offers/XP and before movement.
/// These are projections of canonical profile/economy/resource state, never another player owner.
#[derive(Clone, Copy, Debug)]
pub struct BoneyardPlayerCombatStatus<'a> {
    pub status: BoneyardPlayerStatus<'a>,
    pub advanced_unlocks: &'a [bool; 8],
    pub inventory_has_health_potion: bool,
    pub level: u32,
    pub loot_modifiers: NativeLootModifiers,
    pub owned_recipe_indexes: &'a [usize],
}

pub struct BoneyardWorldParticipant<'a> {
    pub wizard: &'a mut HubWizard,
    pub combat: BoneyardPlayerCombatStatus<'a>,
    pub movement: Vec2,
    pub force_movement_facing: bool,
    pub viewport: Vec2,
    pub light_registration: Option<PainterRegistration>,
    pub overlay_effect_phase: f64,
}

pub struct BoneyardWorldTickContext<'a> {
    pub enhanced_effects: bool,
    pub tick: u64,
    pub participants: &'a mut [BoneyardWorldParticipant<'a>],
    pub light_environment: BoneyardLightEnvironment<'a>,
    pub external_spawn_intents: Vec<EnemySpawnIntent>,
    pub navigation: &'a NavigationSet,
    pub custom_loot: Option<&'a dyn Fn(u32, EnemyToken) -> Vec<HubInventoryItem>>,
}

#[derive(Clone, Debug)]
pub struct BoneyardWorldTickResult {
    pub movement: BTreeMap<String, BoneyardMovementStep>,
    pub loot: BoneyardLootStep,
    pub enemy_events: Vec<EnemyEvent>,
    pub player_damage: Vec<EnemyPlayerDamage>,
    pub rewards: Vec<EnemyReward>,
}

impl BoneyardWorld {
    /// The normal world path. Projectile registrations remain unresolved until the session
    /// finishes secondary/staff/primary work and commits them through the shared allocator.
    pub fn step(
        &mut self,
        context: BoneyardWorldTickContext<'_>,
        painter_order: &mut WorldPainterOrder,
    ) -> BoneyardWorldTickResult {
        self.enemies.assert_painter_registrations_committed();
        let paused = self.tutorial.as_ref().is_some_and(native_tutorial_hostile_scene_paused);
        let participants = context.participants;
        let light_camera_bounds = self.arena_camera_bounds();
        let movement = {
            let mut rows: Vec<_> = participants
                .iter_mut()
                .map(|participant| BoneyardMovementParticipant {
                    wizard: &mut *participant.wizard,
                    input: BoneyardMovementInput {
                        player: participant.combat.status,
                        movement: participant.movement,
                        force_movement_facing: participant.force_movement_facing,
                        tutorial_movement_paused: paused,
                        hostile_scene_paused: paused,
                    },
                })
                .collect();
            self.step_player_movements(&mut rows, paused)
        };
        let loot = {
            let rows: Vec<_> = participants
                .iter()
                .map(|participant| {
                    let player = participant.combat;
                    BoneyardLootParticipant {
                        advanced_unlocks: player.advanced_unlocks,
                        alive: player.status.alive,
                        connected: true,
                        heading_index: participant.wizard.heading_index,
                        level: player.level,
                        modifiers: player.loot_modifiers,
                        owned_recipe_indexes: player.owned_recipe_indexes,
                        player_id: player.status.id,
                        position: participant.wizard.position,
                    }
                })
                .collect();
            self.step_ground_loot(&rows, context.tick, painter_order)
        };
        let player_bodies: Vec<_> = participants
            .iter()
            .map(|participant| BoneyardPlayerBody {
                status: participant.combat.status,
                position: participant.wizard.position,
            })
            .collect();
        let encounter =
            self.step_encounter_and_arena(&player_bodies, context.tick, context.navigation);
        let reward_source = LootEnemySnapshot::new(&self.enemies);
        let environment = context.light_environment;
        let light_players: Vec<_> = participants
            .iter()
            .map(|participant| WorldLightPlayer {
                id: participant.combat.status.id,
                position: participant.wizard.position,
                heading_index: participant.wizard.heading_index,
                weapon_pulse: participant.wizard.primary_cast.weapon_pulse,
                overlay_effect_phase: participant.overlay_effect_phase,
                light_registration: participant.light_registration,
                viewport: Some(participant.viewport),
            })
            .collect();
        let world_light = boneyard_world_light_query(
            WorldLightScene {
                bounds: self.loaded.scene.bounds,
                camera_bounds: light_camera_bounds,
                lantern_position: self.lantern_position,
                lantern_light_registration: self.lantern_light_registration,
            },
            &self.loaded.run_id,
            &light_players,
            &self.enemies,
            context.tick,
            environment,
        );
        let world_key = format!("boneyard:{}", self.loaded.run_id);
        let mut players: Vec<_> = participants
            .iter()
            .map(|participant| {
                let status = participant.combat.status;
                EnemyTarget {
                    id: status.id.to_owned(),
                    summoned: false,
                    alive: status.alive,
                    collision_radius: PLAYER_CHARACTER_RADIUS,
                    connected: true,
                    eligible: status.eligible,
                    heading_deg: Some(participant.wizard.heading_degrees),
                    position: participant.wizard.position,
                    velocity_per_tick: Vec2::new(
                        participant.wizard.velocity.x * 0.01,
                        participant.wizard.velocity.y * 0.01,
                    ),
                }
            })
            .collect();
        let mut ability_effects = Vec::new();
        if let Some(secondary) = environment.secondary_abilities {
            for actor in &secondary.actors {
                if actor.kind == NativeSecondaryActorKind::Golem && actor.world_key == world_key {
                    players.push(EnemyTarget {
                        id: format!("golem:{}", actor.id),
                        summoned: true,
                        alive: true,
                        collision_radius: NATIVE_GOLEM_RADIUS,
                        connected: true,
                        eligible: true,
                        heading_deg: Some(0.0),
                        position: actor.position,
                        velocity_per_tick: Vec2::ZERO,
                    });
                }
            }
            for effect in &secondary.target_effects {
                if effect.world_key != world_key {
                    continue;
                }
                let row = (
                    effect.target_id,
                    EnemyAbilityEffect {
                        time_scale: effect.time_scale,
                        weaken_factor: effect.weaken_factor,
                        disrupted_ticks: effect.disrupted_ticks,
                        dazzle_ticks: effect.dazzle_ticks,
                        flee_ticks: effect.flee_ticks,
                    },
                );
                if let Some(existing) =
                    ability_effects.iter_mut().find(|(id, _)| *id == effect.target_id)
                {
                    *existing = row;
                } else {
                    ability_effects.push(row);
                }
            }
        }
        let mut focuses = Vec::new();
        let mut living_positions = Vec::new();
        let mut projectile_viewports = Vec::with_capacity(participants.len());
        for participant in participants.iter() {
            let status = participant.combat.status;
            if status.alive && status.eligible {
                living_positions.push(participant.wizard.position);
                if inside_bounds(participant.wizard.position, encounter.spawn_bounds) {
                    focuses.push(participant.wizard.position);
                }
            }
            projectile_viewports.push(BoneyardProjectileViewport {
                alternate_player: !status.alive,
                position: participant.wizard.position,
                viewport: participant.viewport,
            });
        }
        if focuses.is_empty() {
            focuses.push(Vec2::new(
                encounter.spawn_bounds.x + encounter.spawn_bounds.width / 2.0,
                encounter.spawn_bounds.y + encounter.spawn_bounds.height / 2.0,
            ));
        }
        let bodies = self.combat_bodies(&player_bodies);
        let puppet_targets = boneyard_mouth_world_targets(self, environment);
        let tutorial_present = self.tutorial.is_some();
        let camera_locked =
            self.tutorial.as_ref().is_some_and(|tutorial| tutorial.camera_lock_triggered);
        let spawn_domain = |position, radius| {
            native_tutorial_enemy_spawn_position_is_allowed(position, radius)
                && (!camera_locked
                    || native_tutorial_enemy_camera_position_is_allowed(position, radius))
        };
        let feedback = Cell::new(self.enemy_world_feedback);
        let spawn_camera_bounds =
            if tutorial_present { self.camera_bounds() } else { encounter.active_bounds };
        let dialogue_busy =
            self.encounter.as_ref().is_some_and(|encounter| encounter.voice_ticks_remaining > 0)
                || self.tutorial.as_ref().is_some_and(|tutorial| !tutorial.narration.idle());
        {
            let geometry = BoneyardEnemyGeometry::new(
                BoneyardGeometryInput {
                    active_bounds: encounter.active_bounds,
                    spawn_bounds: encounter.spawn_bounds,
                    spawn_camera_bounds,
                    static_collision: &self.collision,
                    collision: &self.dynamic_collision,
                    navigation: context.navigation,
                    world_light: &world_light,
                    spawn_policy_focuses: &focuses,
                    spawn_domain: if tutorial_present { Some(&spawn_domain) } else { None },
                    spawn_retry_bounds: camera_locked.then_some(NATIVE_TUTORIAL_CAMERA_TARGET),
                    projectile_viewports: &projectile_viewports,
                    feedback: &feedback,
                },
                bodies,
            );
            let native_visibility = |position| geometry.native_visibility(position);
            let projected_point_visible = |position| geometry.projected_point_visible(position);
            let light_at = |position| world_light.scalar_at(position);
            let mut external = context.external_spawn_intents;
            let has_encounter = self.encounter.is_some();
            let waves = &mut self.waves;
            let mut resolve_spawn_intents = |counts: crate::enemies::world::EnemyLiveCounts| {
                let mut intents = std::mem::take(&mut external);
                if !paused
                    && has_encounter
                    && let Some(waves) = waves
                {
                    let tick_context = WaveTickContext {
                        bounds: encounter.spawn_bounds,
                        live_boss_count: counts.bosses,
                        live_enemy_count: counts.enemies,
                        live_zombie_count: counts.zombies,
                        players: &living_positions,
                        tick: context.tick,
                    };
                    intents.extend(if encounter.waves_started {
                        waves.step_run_release(tick_context)
                    } else {
                        waves.step(tick_context)
                    });
                }
                intents
            };
            let loot_store = &mut self.loot;
            let mut roll_loot_seed = |bound| loot_store.roll_loot_seed(bound);
            self.enemies.begin_deferred_projectile_painters();
            self.enemies.step(
                EnemyStepContext {
                    enhanced_effects: context.enhanced_effects,
                    tick: context.tick,
                    paused,
                    players: &players,
                    geometry: &geometry,
                    ability_effects: &ability_effects,
                    arena_scalars: EnemyArenaScalars::default(),
                    dialogue_busy,
                    roll_loot_seed: Some(&mut roll_loot_seed),
                    native_visibility: Some(&native_visibility),
                    projected_point_visible: Some(&projected_point_visible),
                    native_view_bounds: geometry.native_view_bounds(),
                    puppet_targets: &puppet_targets,
                    light_at: Some(&light_at),
                    native_movement_view: Some(NativeEnemyMovementView {
                        arena_bounds: encounter.active_bounds,
                        cameras: &world_light.cameras,
                        enhanced_effects: context.enhanced_effects,
                    }),
                },
                Some(painter_order),
                Some(&mut resolve_spawn_intents),
            );
        }
        self.enemy_world_feedback = feedback.get();
        for participant in participants.iter_mut() {
            if let Some(position) = self.enemies.player_positions.get(participant.combat.status.id)
            {
                participant.wizard.position = *position;
            }
        }
        let rewards = self.enemies.rewards.clone();
        let combat: Vec<_> = participants.iter().map(|participant| participant.combat).collect();
        self.materialize_enemy_rewards(
            &rewards,
            &reward_source,
            &combat,
            context.tick,
            painter_order,
            context.custom_loot,
        );
        self.apply_player_knockbacks(participants);
        self.retire_goodies_after_sealing();
        self.ticks = context.tick;
        BoneyardWorldTickResult {
            movement,
            loot,
            enemy_events: self.enemies.events.clone(),
            player_damage: self.enemies.player_damage.clone(),
            rewards,
        }
    }

    fn apply_player_knockbacks(&mut self, participants: &mut [BoneyardWorldParticipant<'_>]) {
        if self.enemies.player_knockbacks.is_empty() {
            return;
        }
        let players: Vec<_> = participants
            .iter()
            .map(|participant| BoneyardPlayerBody {
                status: participant.combat.status,
                position: participant.wizard.position,
            })
            .collect();
        let mut bodies = self.combat_bodies(&players);
        for knockback in &self.enemies.player_knockbacks {
            let body_id = format!("player-{}", knockback.player_id);
            if !bodies.iter().any(|body| body.id == body_id) {
                continue;
            }
            for body in &mut bodies {
                body.driven = body.id == body_id;
                body.delta = if body.driven { knockback.delta } else { Vec2::ZERO };
            }
            let mut physics = BoneyardPhysicsWorld {
                bounds: self.active_bounds(),
                collision: &self.dynamic_collision,
            };
            resolve_actor_motion(&mut bodies, &mut physics, |_, _| true, None);
        }
        self.commit_enemy_positions(&bodies);
        for participant in participants {
            let body_id = format!("player-{}", participant.combat.status.id);
            if let Some(body) = bodies.iter().find(|body| body.id == body_id) {
                participant.wizard.position = body.position;
            }
        }
        if let Some(body) = bodies.iter().find(|body| body.id == "lantern") {
            self.lantern_position = Some(body.position);
        }
    }
}
