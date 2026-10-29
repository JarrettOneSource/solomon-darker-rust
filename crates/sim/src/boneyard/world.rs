//! Owned Boneyard state and source-ordered scene phases.
//!
//! Reference: `core-server/boneyard-world.ts`, `boneyard-world-construction.ts` and
//! `boneyard-world-placement.ts`. Session owns identity/profile, vitals and the active painter
//! allocator. Static geometry and prepared navigation reconstruct outside checkpoints.

mod ether_drain;
mod impulses;
mod rewards;
mod snapshot;
mod targets;
mod tick;
mod tutorial;

pub use ether_drain::BoneyardEtherDrainForces;
pub use snapshot::BoneyardSnapshot;
pub use targets::{
    BoneyardSceneryTargetRetention, boneyard_goodie_spell_targets, boneyard_mouth_world_targets,
};
pub use tick::{
    BoneyardPlayerCombatStatus, BoneyardWorldParticipant, BoneyardWorldTickContext,
    BoneyardWorldTickResult,
};
pub use tutorial::BoneyardTutorialPlayer;

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use crate::actor_physics::{ActorPhysicsBody, resolve_actor_motion};
use crate::enemies::EnemySpawnIntent;
use crate::enemies::model::{EnemyBrain, EnemyEvent, EnemyLifeState};
use crate::enemies::targeting::{boneyard_enemy_actor_flags, boneyard_enemy_collision_radius};
use crate::enemies::webbed::native_webbed_movement_scale;
use crate::enemies::world::EnemyWorld;
use crate::hub::{HubWizardPlan, PLAYER_CHARACTER_RADIUS, plan_hub_wizard_tick};
use crate::loot::{NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS, NativeLootDropKind};
use crate::painter::{PainterRegistration, WorldPainterOrder};
use crate::{HubWizard, Vec2};

use super::arena::{ArenaPhase, ArenaTransition};
use super::collision::{BoneyardCollisionWorld, boneyard_body_collides, can_place_boneyard_body};
use super::encounter::{SolomonEncounter, SolomonPhase, SolomonPlayer};
use super::escape::{prepare_solomon_escape_navigation, resolve_solomon_escape_movement};
use super::feedback::NativeEnemyWorldFeedback;
use super::gate::{GateLeaf, create_gate_leaves};
use super::loot::{
    BoneyardGoodieInput, BoneyardLootEvent, BoneyardLootParticipant, BoneyardLootStep,
    BoneyardLootStore,
};
use super::navigation::NavigationSet;
use super::tutorial::{
    NATIVE_TUTORIAL_CAMERA_TARGET, NativeTutorialState, native_tutorial_camera_bounds,
    native_tutorial_dialogue_ticks,
};
use super::waves::director::{WaveCheckpoint, WaveDirector, WavePhase, WaveTickContext};
use super::world_geometry::{BoneyardPhysicsWorld, boneyard_lantern_body, enemy_collision_body};
use super::{BoneyardBounds, BoneyardScene, BoneyardSource, LoadedBoneyard};

#[derive(Clone, Copy, Debug)]
pub struct BoneyardPlayerStatus<'a> {
    pub id: &'a str,
    pub alive: bool,
    pub eligible: bool,
    pub collision_enabled: bool,
    pub movement_scale: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct BoneyardMovementInput<'a> {
    pub player: BoneyardPlayerStatus<'a>,
    pub movement: Vec2,
    pub force_movement_facing: bool,
    pub tutorial_movement_paused: bool,
    pub hostile_scene_paused: bool,
}

/// Borrow the canonical character for the movement epoch; the world retains no player copy.
pub struct BoneyardMovementParticipant<'a> {
    pub wizard: &'a mut HubWizard,
    pub input: BoneyardMovementInput<'a>,
}

/// A transient body/query projection sampled from session participants.
#[derive(Clone, Copy, Debug)]
pub struct BoneyardPlayerBody<'a> {
    pub status: BoneyardPlayerStatus<'a>,
    pub position: Vec2,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoneyardMovementContact {
    pub body_id: String,
    pub staff_hostile: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoneyardMovementStep {
    pub movement_active: bool,
    pub contacts: Vec<BoneyardMovementContact>,
    /// Logical lights capture the incoming arena camera; final physics uses the newly
    /// stepped active bounds. Rendering may query the current camera separately.
    pub light_camera_bounds: BoneyardBounds,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneyardEncounterStep {
    pub waves_started: bool,
    pub active_bounds: BoneyardBounds,
    pub spawn_bounds: BoneyardBounds,
}

#[derive(Clone, Debug)]
pub struct BoneyardWorld {
    pub loaded: LoadedBoneyard,
    pub ticks: u64,
    /// Immutable Region scenery for prepared routes.
    pub collision: BoneyardCollisionWorld,
    /// The same scenery with this tick's moving gate overlay for final collision.
    pub dynamic_collision: BoneyardCollisionWorld,
    pub gate_leaves: Vec<GateLeaf>,
    pub arena_transition: Option<ArenaTransition>,
    pub encounter: Option<SolomonEncounter>,
    pub waves: Option<WaveDirector>,
    pub tutorial: Option<NativeTutorialState>,
    /// Immutable scenery targets reconstruct from the scene; native cleanup permanently
    /// filters their bounds even if a later Tutorial safety check releases its camera lock.
    pub scenery_retained_bounds: Option<BoneyardBounds>,
    /// Website saves retain the actual three target tables, including legacy subsets
    /// that cannot be reconstructed from one cleanup rectangle. `None` keeps all targets.
    pub scenery_target_retention: Option<Arc<targets::BoneyardSceneryTargetRetention>>,
    pub enemies: EnemyWorld,
    pub death_weapons: crate::ComponentEntities<crate::death_animations::NativeDeathWeaponActor>,
    pub next_death_weapon_id: u64,
    pub loot: BoneyardLootStore,
    pub enemy_world_feedback: NativeEnemyWorldFeedback,
    pub enemy_events: Vec<EnemyEvent>,
    pub loot_events: Vec<BoneyardLootEvent>,
    pub player_ouch_deadline_tick: u64,
    pub lantern_position: Option<Vec2>,
    pub lantern_light_registration: Option<PainterRegistration>,
    pub solomon_painter_registration: Option<PainterRegistration>,
}

/// Domain data taken at a completed simulation tick. Immutable scenery, meshes and compiled
/// wave programs are omitted. The session checkpoints its shared painter allocator once.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct BoneyardCheckpoint {
    pub choice_id: String,
    // Earlier Rust checkpoints only contained the shipped default catalog.
    #[cfg_attr(feature = "persistence", serde(default))]
    pub choice_source: BoneyardSource,
    pub mod_id: Option<String>,
    pub mod_name: Option<String>,
    pub run_id: String,
    pub seed: String,
    pub source_sha256: String,
    pub geometry_sha256: String,
    pub ticks: u64,
    pub gate_leaves: Vec<GateLeaf>,
    pub arena_transition: Option<ArenaTransition>,
    pub encounter: Option<SolomonEncounter>,
    pub waves: Option<WaveCheckpoint>,
    pub tutorial: Option<NativeTutorialState>,
    pub scenery_retained_bounds: Option<BoneyardBounds>,
    #[cfg_attr(feature = "persistence", serde(default, skip_serializing_if = "Option::is_none"))]
    pub scenery_target_retention: Option<Arc<targets::BoneyardSceneryTargetRetention>>,
    pub enemies: EnemyWorld,
    pub death_weapons: crate::ComponentEntities<crate::death_animations::NativeDeathWeaponActor>,
    pub next_death_weapon_id: u64,
    pub loot: BoneyardLootStore,
    pub enemy_world_feedback: NativeEnemyWorldFeedback,
    pub enemy_events: Vec<EnemyEvent>,
    pub loot_events: Vec<BoneyardLootEvent>,
    pub player_ouch_deadline_tick: u64,
    pub lantern_position: Option<Vec2>,
    pub lantern_light_registration: Option<PainterRegistration>,
    pub solomon_painter_registration: Option<PainterRegistration>,
}

impl BoneyardWorld {
    #[must_use]
    pub fn new(loaded: LoadedBoneyard) -> Self {
        let scene = &loaded.scene;
        let collision = BoneyardCollisionWorld::from_scene(scene);
        let gate_leaves = create_gate_leaves(&scene.fences, &loaded.seed);
        let mut dynamic_collision = collision.clone();
        dynamic_collision.set_gate_leaves(&gate_leaves);
        let tutorial = loaded.choice_id == "stock-tutorial";
        let owns_solomon_encounter =
            loaded.choice_source == BoneyardSource::Default && scene.solomon_dig.is_some();
        let ordinary = owns_solomon_encounter && !tutorial;
        let arena_transition = if ordinary {
            Some(ArenaTransition::new(scene.bounds, scene.spawn.position))
        } else {
            None
        };
        let encounter = match &scene.solomon_dig {
            Some(dig) if owns_solomon_encounter && tutorial => Some(
                SolomonEncounter::new_tutorial(dig, &loaded.seed, native_tutorial_dialogue_ticks()),
            ),
            Some(dig) if ordinary => Some(SolomonEncounter::new(dig, &loaded.seed)),
            _ => None,
        };
        let waves = if ordinary {
            Some(WaveDirector::new(&loaded.seed, Some(&loaded.source_sha256)))
        } else {
            None
        };
        let tutorial =
            tutorial.then(|| NativeTutorialState::new(scene.spawn.position, 0, &loaded.seed));
        let enemies = EnemyWorld::new(&loaded.seed, native_enemy_registration_base(scene));
        let mut source_goodies = Vec::new();
        for (ordinal, object) in scene.objects.iter().enumerate() {
            if object.type_id != 2061 {
                continue;
            }
            source_goodies.push(BoneyardGoodieInput {
                eid: object.eid.clone(),
                position: object.position,
                reward_seed: None,
                scenery_registration_ordinal: Some(ordinal),
                subtype: Some(0),
            });
        }
        let loot = BoneyardLootStore::new(&loaded.seed, &source_goodies);
        let lantern_position = scene.solomon_dig.as_ref().map(|dig| dig.lantern_position);
        Self {
            loaded,
            ticks: 0,
            collision,
            dynamic_collision,
            gate_leaves,
            arena_transition,
            encounter,
            waves,
            tutorial,
            scenery_retained_bounds: None,
            scenery_target_retention: None,
            enemies,
            death_weapons: crate::ComponentEntities::default(),
            next_death_weapon_id: 1,
            loot,
            enemy_world_feedback: NativeEnemyWorldFeedback::default(),
            enemy_events: Vec::new(),
            loot_events: Vec::new(),
            player_ouch_deadline_tick: 0,
            lantern_position,
            lantern_light_registration: None,
            solomon_painter_registration: None,
        }
    }

    #[must_use]
    pub fn spawn_wizard(&self) -> HubWizard {
        let mut wizard = HubWizard::at(self.loaded.scene.spawn.position);
        wizard.set_facing(self.loaded.scene.spawn.facing_degrees);
        wizard
    }

    /// Focused pre-combat movement uses the same gate and actor solve without the
    /// encounter's input lock. The caller owns the character throughout the replay.
    pub fn step_movement_only(&mut self, wizard: &mut HubWizard, movement: Vec2) {
        let plan = plan_hub_wizard_tick(wizard.velocity, movement, 1.0);
        self.step_entrance();
        let camera = self.arena_camera_bounds();
        let mut participants = [BoneyardMovementParticipant {
            wizard,
            input: BoneyardMovementInput {
                player: BoneyardPlayerStatus {
                    id: "local",
                    alive: true,
                    eligible: true,
                    collision_enabled: true,
                    movement_scale: 1.0,
                },
                movement,
                force_movement_facing: false,
                tutorial_movement_paused: false,
                hostile_scene_paused: false,
            },
        }];
        self.resolve_player_plans(&mut participants, &[plan], camera, false);
    }

    /// One-player callers borrow the same canonical movement batch.
    pub fn step_player_movement(
        &mut self,
        wizard: &mut HubWizard,
        input: BoneyardMovementInput<'_>,
    ) -> BoneyardMovementStep {
        let mut participants = [BoneyardMovementParticipant { wizard, input }];
        self.step_player_movements(&mut participants, input.hostile_scene_paused)
            .remove(input.player.id)
            .unwrap()
    }

    pub fn step_player_movements(
        &mut self,
        participants: &mut [BoneyardMovementParticipant<'_>],
        hostile_scene_paused: bool,
    ) -> BTreeMap<String, BoneyardMovementStep> {
        let light_camera_bounds = self.arena_camera_bounds();
        self.step_entrance();
        let mut plans = Vec::with_capacity(participants.len());
        for participant in participants.iter() {
            let input = participant.input;
            let locked = input.tutorial_movement_paused
                || self
                    .encounter
                    .as_ref()
                    .is_some_and(|encounter| encounter.player_locked(input.player.id));
            let webbed_scale =
                native_webbed_movement_scale(self.enemies.webbed_players.get(input.player.id));
            let velocity = if locked { Vec2::ZERO } else { participant.wizard.velocity };
            let movement = if locked { Vec2::ZERO } else { input.movement };
            let scale = if locked { 0.0 } else { input.player.movement_scale * webbed_scale };
            let mut plan = plan_hub_wizard_tick(velocity, movement, scale);
            plan.face = input.force_movement_facing;
            plans.push(plan);
        }
        let movement = self.resolve_player_plans(
            participants,
            &plans,
            light_camera_bounds,
            hostile_scene_paused,
        );
        // Live combat recovers old blocked roots before using players as spawn
        // goals. The movement-only recording retains the narrow-phase result.
        for participant in participants {
            if participant.input.player.alive
                && participant.input.player.collision_enabled
                && boneyard_body_collides(
                    participant.wizard.position,
                    &self.dynamic_collision,
                    PLAYER_CHARACTER_RADIUS,
                )
            {
                participant.wizard.position = super::spawn::SpawnPlacement {
                    position: participant.wizard.position,
                    bounds: self.active_bounds(),
                    collision: &self.dynamic_collision,
                    radius: PLAYER_CHARACTER_RADIUS,
                }
                .resolve(None)
                .expect("Boneyard has a collision-safe player placement");
            }
        }
        movement
    }

    fn resolve_player_plans(
        &mut self,
        participants: &mut [BoneyardMovementParticipant<'_>],
        plans: &[HubWizardPlan],
        light_camera_bounds: BoneyardBounds,
        hostile_scene_paused: bool,
    ) -> BTreeMap<String, BoneyardMovementStep> {
        for (participant, plan) in participants.iter().zip(plans) {
            if !participant.input.player.collision_enabled {
                continue;
            }
            let requested = Vec2::new(
                participant.wizard.position.x + plan.delta.x,
                participant.wizard.position.y + plan.delta.y,
            );
            for leaf in &mut self.gate_leaves {
                if leaf.touches(requested, PLAYER_CHARACTER_RADIUS) {
                    leaf.apply_contact(plan.delta);
                }
            }
        }
        for leaf in &mut self.gate_leaves {
            leaf.step();
        }
        self.dynamic_collision.set_gate_leaves(&self.gate_leaves);
        let mut bodies = Vec::new();
        for (participant, plan) in participants.iter().zip(plans) {
            if participant.input.player.collision_enabled {
                bodies.push(player_body(
                    participant.input.player.id,
                    participant.wizard.position,
                    plan.delta,
                    true,
                ));
            }
        }
        bodies.extend(self.enemy_bodies());
        if let Some(position) = self.lantern_position {
            bodies.push(boneyard_lantern_body(position));
        }
        let mut physics = BoneyardPhysicsWorld {
            bounds: self.active_bounds(),
            collision: &self.dynamic_collision,
        };
        let mut contacts = Vec::new();
        resolve_actor_motion(&mut bodies, &mut physics, |_, _| true, Some(&mut contacts));
        let mut results = BTreeMap::new();
        for (participant, plan) in participants.iter_mut().zip(plans) {
            let id = participant.input.player.id;
            let body_id = format!("player-{id}");
            let position = if participant.input.player.collision_enabled {
                bodies
                    .iter()
                    .find(|body| body.id == body_id)
                    .expect("every collision-enabled participant has a body")
                    .position
            } else {
                participant.wizard.position
            };
            participant.wizard.commit(plan, position);
            let active = participant.input.player.collision_enabled && plan.movement_active;
            let mut movement_contacts = Vec::new();
            if active {
                for contact in &contacts {
                    if contact.mover_id == body_id {
                        movement_contacts.push(BoneyardMovementContact {
                            body_id: contact.other_id.clone(),
                            staff_hostile: self.staff_hostile_body(&contact.other_id),
                        });
                    }
                }
            }
            results.insert(
                id.to_owned(),
                BoneyardMovementStep {
                    movement_active: active,
                    contacts: movement_contacts,
                    light_camera_bounds,
                },
            );
        }
        if !hostile_scene_paused {
            self.commit_enemy_positions(&bodies);
        }
        if let Some(lantern) = bodies.iter().find(|body| body.id == "lantern") {
            self.lantern_position = Some(lantern.position);
        }
        self.ticks += 1;
        results
    }

    pub fn step_encounter_and_arena(
        &mut self,
        players: &[BoneyardPlayerBody<'_>],
        tick: u64,
        navigation: &NavigationSet,
    ) -> BoneyardEncounterStep {
        let living: Vec<_> = players
            .iter()
            .filter(|player| player.status.alive && player.status.eligible)
            .map(|player| SolomonPlayer { id: player.status.id, position: player.position })
            .collect();
        let mut waves_started = false;
        if let Some(encounter) = &mut self.encounter {
            let prior_run_event = encounter.run_event_id;
            if encounter.phase == SolomonPhase::Escaping {
                prepare_solomon_escape_navigation(
                    encounter,
                    self.loaded.scene.bounds,
                    &self.dynamic_collision,
                    &self.collision,
                    navigation,
                );
                let source = encounter.clone();
                encounter.step(&living, tick);
                resolve_solomon_escape_movement(
                    &source,
                    encounter,
                    self.loaded.scene.bounds,
                    &self.dynamic_collision,
                );
            } else {
                encounter.step(&living, tick);
            }
            if encounter.run_event_id > prior_run_event
                && let Some(waves) = &mut self.waves
            {
                waves.start();
                waves_started = true;
            }
        }
        self.enemy_world_feedback.step(1);
        if self.arena_transition.as_ref().is_some_and(|arena| arena.phase == ArenaPhase::Open)
            && self.waves.as_ref().is_some_and(|waves| waves.state.phase != WavePhase::Dormant)
        {
            let mut safety_bodies = Vec::new();
            for player in players {
                if player.status.alive && player.status.eligible {
                    safety_bodies.push((player.position, PLAYER_CHARACTER_RADIUS));
                }
            }
            for actor in self.enemies.actors.iter() {
                if boneyard_enemy_actor_flags(&actor) != 0 {
                    safety_bodies.push((actor.position, boneyard_enemy_collision_radius(&actor)));
                }
            }
            for maggot in self.enemies.maggots.iter() {
                safety_bodies.push((maggot.position, maggot.collision_radius));
            }
            for actor in self.loot.actors.iter() {
                if actor.kind == NativeLootDropKind::Sack {
                    safety_bodies.push((actor.position, NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS));
                }
            }
            let arena = self.arena_transition.as_mut().expect("open arena exists");
            if arena.safety_clear(&safety_bodies) {
                arena.start();
            }
        }
        BoneyardEncounterStep {
            waves_started,
            active_bounds: self.active_bounds(),
            spawn_bounds: self.spawn_bounds(),
        }
    }

    /// Ground items advance after player movement and before Solomon/arena safety. The
    /// placement query uses only current world bounds and scenery, as in the source.
    pub fn step_ground_loot(
        &mut self,
        participants: &[BoneyardLootParticipant<'_>],
        tick: u64,
        painter_order: &mut WorldPainterOrder,
    ) -> BoneyardLootStep {
        let bounds = self.active_bounds();
        let collision = &self.dynamic_collision;
        let placement =
            |position, radius| can_place_boneyard_body(position, bounds, collision, radius);
        self.loot.step(participants, &placement, tick, painter_order)
    }

    pub fn retire_goodies_after_sealing(&mut self) {
        if let Some(bounds) = self.pending_cleanup_bounds() {
            self.loot.retire_goodies_outside_bounds(bounds);
            self.scenery_retained_bounds = Some(match self.scenery_retained_bounds {
                None => bounds,
                Some(previous) => {
                    let x = previous.x.max(bounds.x);
                    let y = previous.y.max(bounds.y);
                    BoneyardBounds::new(
                        x,
                        y,
                        (previous.x + previous.width).min(bounds.x + bounds.width) - x,
                        (previous.y + previous.height).min(bounds.y + bounds.height) - y,
                    )
                }
            });
            if let Some(retained) = &mut self.scenery_target_retention {
                for (index, object) in self.loaded.scene.objects.iter().enumerate() {
                    if targets::inside_bounds(object.position, bounds) {
                        continue;
                    }
                    let id = format!("scenery:{}", object.eid);
                    if !retained.earthquake_ids.contains(&index)
                        && !retained.primary_ids.contains(&id)
                        && !retained.spell_ids.contains(&id)
                    {
                        continue;
                    }
                    let retained = Arc::make_mut(retained);
                    retained.earthquake_ids.remove(&index);
                    retained.primary_ids.remove(&id);
                    retained.spell_ids.remove(&id);
                }
            }
        }
    }

    /// Called once by the enemy store after its live actors/projectiles step. External births
    /// are consumed in the same native batch before wave births, including while paused.
    pub fn resolve_spawn_intents(
        &mut self,
        context: WaveTickContext<'_>,
        waves_started: bool,
        hostile_scene_paused: bool,
        mut external: Vec<EnemySpawnIntent>,
    ) -> Vec<EnemySpawnIntent> {
        if hostile_scene_paused || self.encounter.is_none() {
            return external;
        }
        if let Some(waves) = &mut self.waves {
            let births =
                if waves_started { waves.step_run_release(context) } else { waves.step(context) };
            external.extend(births);
        }
        external
    }

    #[must_use]
    pub fn checkpoint(&self) -> BoneyardCheckpoint {
        self.enemies.assert_painter_registrations_committed();
        BoneyardCheckpoint {
            choice_id: self.loaded.choice_id.clone(),
            choice_source: self.loaded.choice_source,
            mod_id: self.loaded.mod_id.clone(),
            mod_name: self.loaded.mod_name.clone(),
            run_id: self.loaded.run_id.clone(),
            seed: self.loaded.seed.clone(),
            source_sha256: self.loaded.source_sha256.clone(),
            geometry_sha256: self.loaded.geometry_sha256.clone(),
            ticks: self.ticks,
            gate_leaves: self.gate_leaves.clone(),
            arena_transition: self.arena_transition,
            encounter: self.encounter.clone(),
            waves: self.waves.as_ref().map(WaveDirector::checkpoint),
            tutorial: self.tutorial.clone(),
            scenery_retained_bounds: self.scenery_retained_bounds,
            scenery_target_retention: self.scenery_target_retention.clone(),
            enemies: self.enemies.clone(),
            death_weapons: self.death_weapons.clone(),
            next_death_weapon_id: self.next_death_weapon_id,
            loot: self.loot.clone(),
            enemy_world_feedback: self.enemy_world_feedback,
            enemy_events: self.enemy_events.clone(),
            loot_events: self.loot_events.clone(),
            player_ouch_deadline_tick: self.player_ouch_deadline_tick,
            lantern_position: self.lantern_position,
            lantern_light_registration: self.lantern_light_registration,
            solomon_painter_registration: self.solomon_painter_registration,
        }
    }

    pub fn restore(loaded: LoadedBoneyard, checkpoint: BoneyardCheckpoint) -> Result<Self, String> {
        let mut world = Self::new(loaded);
        world.apply_checkpoint(checkpoint)?;
        Ok(world)
    }

    #[must_use]
    pub fn active_bounds(&self) -> BoneyardBounds {
        self.arena_transition
            .as_ref()
            .map_or(self.loaded.scene.bounds, ArenaTransition::active_bounds)
    }

    #[must_use]
    pub fn spawn_bounds(&self) -> BoneyardBounds {
        if let Some(arena) = &self.arena_transition
            && self.waves.as_ref().is_some_and(|waves| waves.state.phase != WavePhase::Dormant)
        {
            arena.combat_bounds
        } else {
            self.active_bounds()
        }
    }

    #[must_use]
    pub fn camera_origin(&self, position: Vec2, logical_view: Vec2, scale: f64) -> Vec2 {
        boneyard_camera_origin(position, self.camera_bounds(), logical_view, scale)
    }

    #[must_use]
    pub fn camera_bounds(&self) -> BoneyardBounds {
        self.tutorial
            .as_ref()
            .and_then(native_tutorial_camera_bounds)
            .unwrap_or_else(|| self.arena_camera_bounds())
    }

    fn arena_camera_bounds(&self) -> BoneyardBounds {
        self.arena_transition.as_ref().map_or(self.loaded.scene.bounds, |arena| arena.camera_bounds)
    }

    /// Bounds of the retained gameplay target tables. Cleanup applies at the end of the
    /// world epoch, after enemy mouth queries, and never resurrects discarded scenery rows.
    #[must_use]
    pub fn cleanup_bounds(&self) -> Option<BoneyardBounds> {
        self.scenery_retained_bounds
    }

    fn pending_cleanup_bounds(&self) -> Option<BoneyardBounds> {
        if let Some(arena) = &self.arena_transition
            && arena.phase == ArenaPhase::Sealed
        {
            return Some(arena.combat_bounds);
        }
        self.tutorial
            .as_ref()
            .filter(|tutorial| {
                tutorial.camera_lock_triggered && tutorial.camera_lock_ticks_remaining == 0
            })
            .map(|_| NATIVE_TUTORIAL_CAMERA_TARGET)
    }

    #[must_use]
    pub fn enemy_bodies(&self) -> Vec<ActorPhysicsBody> {
        let mut bodies = Vec::new();
        for actor in self.enemies.actors.iter() {
            if !matches!(actor.brain, EnemyBrain::Cocoon(_))
                && boneyard_enemy_actor_flags(&actor) != 0
            {
                let mut body = enemy_collision_body(
                    format!("enemy-{}", actor.id),
                    actor.position,
                    boneyard_enemy_collision_radius(&actor),
                );
                body.collision_recipient = match &actor.brain {
                    EnemyBrain::Wraith(brain) => brain.collision_recipient,
                    _ => true,
                };
                body.push_resistance = match actor.brain {
                    EnemyBrain::Portal(_) => 150.0,
                    EnemyBrain::Coffin(_) => 1.0,
                    _ => 0.0,
                };
                bodies.push(body);
            }
        }
        for maggot in self.enemies.maggots.iter() {
            if maggot.life_state == EnemyLifeState::Alive {
                let mut body = enemy_collision_body(
                    format!("enemy-{}", maggot.id),
                    maggot.position,
                    maggot.collision_radius,
                );
                // Both airborne and grounded native Maggots can move as roots,
                // but never participate as another body's collision recipient.
                body.collision_recipient = false;
                bodies.push(body);
            }
        }
        bodies
    }

    #[must_use]
    pub fn combat_bodies(&self, players: &[BoneyardPlayerBody<'_>]) -> Vec<ActorPhysicsBody> {
        let mut bodies = Vec::new();
        for player in players {
            if player.status.collision_enabled {
                bodies.push(player_body(player.status.id, player.position, Vec2::ZERO, false));
            }
        }
        bodies.extend(self.enemy_bodies());
        if let Some(position) = self.lantern_position {
            bodies.push(boneyard_lantern_body(position));
        }
        bodies
    }

    fn step_entrance(&mut self) {
        if let Some(arena) = &mut self.arena_transition {
            arena.step();
        }
    }

    fn commit_enemy_positions(&mut self, bodies: &[ActorPhysicsBody]) {
        for mut actor in self.enemies.actors.iter_mut() {
            let id = format!("enemy-{}", actor.id);
            if let Some(body) = bodies.iter().find(|body| body.id == id) {
                actor.position = body.position;
            }
        }
        for mut maggot in self.enemies.maggots.iter_mut() {
            let id = format!("enemy-{}", maggot.id);
            if let Some(body) = bodies.iter().find(|body| body.id == id) {
                maggot.position = body.position;
            }
        }
    }

    fn staff_hostile_body(&self, body_id: &str) -> bool {
        self.enemies.actors.iter().any(|actor| {
            body_id == format!("enemy-{}", actor.id)
                && boneyard_enemy_actor_flags(&actor) & 0x2 != 0
        }) || self.enemies.maggots.iter().any(|maggot| {
            body_id == format!("enemy-{}", maggot.id)
                && maggot.life_state == EnemyLifeState::Alive
                && maggot.combat_active
        })
    }
}

fn player_body(id: &str, position: Vec2, delta: Vec2, driven: bool) -> ActorPhysicsBody {
    ActorPhysicsBody {
        collision_recipient: true,
        id: format!("player-{id}"),
        position,
        delta,
        radius: PLAYER_CHARACTER_RADIUS,
        push_resistance: 10.0,
        push_strength: 12.0,
        driven,
        push_enabled: true,
    }
}

fn native_enemy_registration_base(scene: &BoneyardScene) -> usize {
    let mut posts = HashSet::new();
    for fence in &scene.fences {
        if fence.code() == 3 {
            continue;
        }
        for point in fence.points.iter().take(2) {
            // JavaScript string keys identify both signed zeros as the same endpoint.
            let x = if point.x == 0.0 { 0 } else { point.x.to_bits() };
            let y = if point.y == 0.0 { 0 } else { point.y.to_bits() };
            posts.insert((x, y));
        }
    }
    scene.objects.len() + posts.len()
}

/// `renderer/boneyard-render-contract.ts::boneyardCamera`, as a top-left logical origin.
#[must_use]
pub fn boneyard_camera_origin(
    position: Vec2,
    bounds: BoneyardBounds,
    logical_view: Vec2,
    scale: f64,
) -> Vec2 {
    let half_width = logical_view.x / 2.0 / scale;
    let half_height = logical_view.y / 2.0 / scale;
    let clamp_axis = |position: f64, start: f64, size: f64, half_view: f64| {
        if size <= half_view * 2.0 {
            return start + size / 2.0;
        }
        position.max(start + half_view).min(start + size - half_view)
    };
    Vec2::new(
        clamp_axis(position.x, bounds.x, bounds.width, half_width) - half_width,
        clamp_axis(position.y, bounds.y, bounds.height, half_height) - half_height,
    )
}
