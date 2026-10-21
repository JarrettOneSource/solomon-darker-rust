//! Native enemy births from Website `core-server/enemies/construction.ts`.
//!
//! Constructor draws remain in source order, including inherited family constructors,
//! rejected Imp births, grouped placements, private action state and locomotion phases.

use super::config::{
    EnemyClassification, EnemyConfig, EnemyConfigRandom, EnemyFamilyConfig, SkeletonWeapon,
    evaluate_enemy_config,
};
use super::crow::create_native_crow;
use super::demon_skull_controller::create_native_demon_skull;
use super::faculty_controller::{create_native_faculty, register_native_faculty};
use super::model::*;
use super::path::{NativeEnemyPathState, native_enemy_target_refresh_ticks};
use super::ranged::{RangedFamily, construct_native_ranged_attack_range};
use super::targeting::nearest_eligible_target;
use super::world::{
    EnemyGeometry, EnemySpawnContext, EnemySpawnPlacementRequest, EnemyWorld,
    FacultyVoiceController,
};
use super::{EnemyFlag, EnemySpawnIntent, EnemyToken};
use crate::Vec2;
use crate::float;
use crate::float::fround;
use crate::heading::heading_degrees_from_vector;
use crate::painter::{WorldManagerLane, WorldPainterOrder};
use std::collections::BTreeMap;

impl EnemyWorld {
    pub(super) fn materialize_step_intents(
        &mut self,
        intents: &[EnemySpawnIntent],
        context: &mut super::world::EnemyStepContext<'_>,
        split_depth: Option<i64>,
        order: &mut WorldPainterOrder,
    ) -> Vec<usize> {
        let shared = context
            .roll_loot_seed
            .as_mut()
            .map(|callback| &mut **callback as &mut dyn FnMut(u32) -> u32);
        self.materialize_spawn_intents(
            intents,
            EnemySpawnContext {
                players: context.players,
                tick: context.tick,
                arena_scalars: context.arena_scalars,
                roll_loot_seed: shared,
            },
            context.geometry,
            split_depth,
            order,
        )
    }

    /// `materializeSpawnIntents`: draw the constructor envelope even when the live Imp
    /// cap later rejects a birth. A placement group reuses its first accepted position.
    pub fn spawn(
        &mut self,
        intents: &[EnemySpawnIntent],
        context: EnemySpawnContext<'_>,
        geometry: &(impl EnemyGeometry + ?Sized),
        painter_order: Option<&mut WorldPainterOrder>,
    ) -> Vec<usize> {
        let mut standalone;
        let order = if let Some(order) = painter_order {
            order
        } else {
            standalone = self.standalone_painter_order();
            &mut standalone
        };
        self.materialize_spawn_intents(intents, context, geometry, None, order)
    }

    pub(super) fn materialize_spawn_intents(
        &mut self,
        intents: &[EnemySpawnIntent],
        mut context: EnemySpawnContext<'_>,
        geometry: &(impl EnemyGeometry + ?Sized),
        imp_split_depth_override: Option<i64>,
        painter_order: &mut WorldPainterOrder,
    ) -> Vec<usize> {
        let mut spawned = Vec::new();
        let mut placement_groups = BTreeMap::new();
        let mut imp_count =
            self.actors.iter().filter(|actor| actor.config.enemy_token == EnemyToken::Imp).count();
        for intent in intents {
            let base_speed_unit = self.draw_unit();
            let collision_radius_unit = self.draw_unit();
            let armor = self.draw_unit();
            let split = self.draw_unit();
            let split_many_unit =
                if intent.flags.contains(&EnemyFlag::SplitMany) { self.draw_unit() } else { 0.0 };
            // The source constructor forwards strafe only through the authored Archer
            // family recipe; its separate custom intent field is retained but not read.
            let mut config_intent = intent.clone();
            config_intent.archer_strafing = false;
            let mut config = evaluate_enemy_config(
                &config_intent,
                context.arena_scalars,
                EnemyConfigRandom {
                    base_speed_unit,
                    collision_radius_unit,
                    random_armor: armor >= 0.5,
                    split_many_gate_unit: split,
                    split_many_unit,
                    split_unit: u8::from(split >= 0.5),
                },
            );
            if let Some(split_depth) = imp_split_depth_override {
                let EnemyFamilyConfig::Imp { split_depth: depth } = &mut config.family else {
                    panic!("only child Imps can inherit reduced split state");
                };
                *depth = split_depth;
            }
            if let Some(ejection) = intent.portal_ejection {
                assert_eq!(
                    config.enemy_token,
                    EnemyToken::Imp,
                    "only an Imp can consume a Portal ejection payload"
                );
                config.primary_damage = Some(ejection.inherited_primary_damage);
            }
            if let Some(damage) = intent.green_imp_spit_damage {
                assert!(
                    config.enemy_token == EnemyToken::Imp && intent.native_type_id == 2044,
                    "GreenImp requires its native Imp payload"
                );
                config.maximum_health = 1.0;
                config.native_type_id = 2044;
                config.primary_damage = Some(damage / 20.0);
                config.experience = damage / 20.0;
            }
            if config.enemy_token == EnemyToken::Imp
                && intent.portal_ejection.is_none()
                && imp_count >= 70
            {
                continue;
            }
            let raw_position = intent.portal_ejection.map_or(intent.position, |ejection| {
                portal_child_position(
                    ejection.parent_position,
                    ejection.parent_heading_deg,
                    ejection.child_heading_deg,
                    config.collision_radius,
                )
            });
            let cached_position =
                intent.placement_group_id.and_then(|id| placement_groups.get(&id).copied());
            let position = if let Some(position) = cached_position {
                position
            } else {
                let position = geometry.resolve_spawn_placement(
                    EnemySpawnPlacementRequest {
                        actor_id: self.next_actor_id,
                        navigation_clearance: intent.navigation_clearance.unwrap_or(
                            if config.enemy_token == EnemyToken::Demon { 50.0 } else { 25.0 },
                        ),
                        position: raw_position,
                        position_policy: intent.position_policy,
                        radius: intent.placement_radius.unwrap_or(config.collision_radius),
                        reachability_radius: intent
                            .reachability_radius
                            .unwrap_or(config.collision_radius),
                    },
                    &mut self.steering_rng,
                );
                if let Some(id) = intent.placement_group_id {
                    placement_groups.insert(id, position);
                }
                position
            };
            let target_player_id =
                nearest_eligible_target(position, context.players).map(|target| target.id.clone());
            let stride_phase_deg = if config.enemy_token == EnemyToken::Zombie {
                self.locomotion_rng.draw_scaled_float(360.0, false)
            } else {
                0.0
            };
            let gait_pose = self.locomotion_rng.draw_scaled_float(4.0, false);
            let body_gait_phase = self.locomotion_rng.draw_scaled_float(4.0, false);
            let mut path = NativeEnemyPathState::new(&mut self.steering_rng);
            let heading_deg = intent.portal_ejection.map_or_else(
                || {
                    context
                        .players
                        .iter()
                        .find(|target| Some(&target.id) == target_player_id.as_ref())
                        .map_or(0.0, |target| {
                            heading_degrees_from_vector(
                                target.position.x - position.x,
                                target.position.y - position.y,
                            )
                        })
                },
                |ejection| ejection.child_heading_deg,
            );
            if intent.enable_discorporeal_health_gates {
                self.demon_skull_encounter.health_triggers_enabled = true;
            }
            let faculty_member_count = self
                .actors
                .iter()
                .filter(|actor| {
                    matches!(actor.brain, EnemyBrain::Faculty(_))
                        && actor.life_state == EnemyLifeState::Alive
                })
                .count()
                + 1;
            let mut brain = self.create_brain(
                &config,
                BrainOwner {
                    actor_id: self.next_actor_id,
                    faculty_member_count,
                    heading_deg,
                    position,
                    spawn_tick: intent.spawn_tick,
                    cocoon_target_player_id: intent.cocoon_target_player_id.clone(),
                },
            );
            if let Some(ejection) = intent.portal_ejection {
                let EnemyBrain::Imp(imp) = &mut brain else {
                    panic!("only an Imp brain can consume a Portal ejection payload");
                };
                imp.base_horizontal_speed = 4.5;
                imp.horizontal_speed = 6.75;
                imp.vertical_offset = -0.1;
                imp.vertical_velocity = ejection.vertical_velocity;
            }
            let rest_body_pose = if config.enemy_token == EnemyToken::SkeletonMage {
                f64::from(self.locomotion_rng.draw_integer(2))
            } else {
                0.0
            };
            let hurricane_contact_cooldown = f64::from(self.draw_integer(100));
            let registration_ordinal =
                painter_order.register(WorldManagerLane::Actor).registration_ordinal;
            let loot_seed = self.draw_loot_seed(10_000_000, &mut context.roll_loot_seed);
            if config.enemy_token == EnemyToken::DireFaculty {
                path.flank_radius = 300.0;
            }
            let actor = EnemyActor {
                ether_drain_captured: false,
                hit_feedback: PuppetHit { strength: 1.0, tick: context.tick, timer: 0.0 },
                hit_reaction_timer: 0.0,
                blizzard_push_accumulator: 0.0,
                blizzard_push_last_tick: None,
                body_gait_phase,
                body_pose: rest_body_pose,
                brain,
                current_health: config.maximum_health,
                config,
                death_epoch: None,
                death_presentation_started: false,
                death_started_tick: None,
                death_tick: 0,
                gait_pose,
                head_facing_offset: 0,
                heading_deg,
                hurricane_contact_cooldown,
                id: self.next_actor_id,
                last_damaged_by_player_id: None,
                last_damage_tick: None,
                last_movement_tick: None,
                lethal_magic_damage: false,
                life_state: EnemyLifeState::Alive,
                registration_ordinal,
                lighting: EnemyLighting { charge: 0.0, glow: 0.0, provider_copies: 0 },
                loot_seed,
                next_movement_tick: context.tick + 2,
                next_target_refresh_tick: context.tick
                    + native_enemy_target_refresh_ticks(intent.pathfinding_mode),
                native_cell_binding_order: self.next_native_cell_binding_order,
                native_registration_order: self.next_native_registration_order,
                path,
                position,
                reward_granted: false,
                rest_body_pose,
                shield_health: 0.0,
                shadow_lateral_offset: 0.0,
                shield_maximum_health: 0.0,
                shield_pulse: 0.0,
                shield_sound_cooldown_ticks: 0,
                source_spawn_intent_id: intent.id,
                spawn_tick: intent.spawn_tick,
                staff_action_factor: 1.0,
                staff_movement_factor: 1.0,
                stride_phase_deg,
                target_player_id: target_player_id.clone(),
                terminal_emitted: false,
                wave_ordinal: intent.wave_ordinal,
            };
            self.next_native_cell_binding_order += 1;
            self.next_native_registration_order += 1;
            self.next_actor_id += 1;
            if self.featured_boss_id.is_none()
                && matches!(
                    actor.config.classification,
                    EnemyClassification::Boss | EnemyClassification::MultipleBoss
                )
            {
                self.featured_boss_id = Some(actor.id);
            }
            if actor.config.enemy_token == EnemyToken::Imp {
                imp_count += 1;
            }
            spawned.push(actor.id);
            self.spawned_actor_ids.push(actor.id);
            let mut event = EnemyEvent::new(EnemyEventKind::EnemySpawned, actor.id, context.tick);
            event.target_player_id = Some(target_player_id);
            self.emit_event(event);
            self.actors.push(actor);
        }
        spawned
    }

    fn create_brain(&mut self, config: &EnemyConfig, owner: BrainOwner) -> EnemyBrain {
        match &config.family {
            EnemyFamilyConfig::Skeleton { weapon, .. } => EnemyBrain::Skeleton(SkeletonBrain {
                action_progress: 0.0,
                marker_emitted: false,
                vertical_offset: 0.0,
                vertical_velocity: 0.0,
                action: match weapon {
                    SkeletonWeapon::Claw => SkeletonAction::Claw,
                    SkeletonWeapon::Pike => SkeletonAction::Pike,
                    _ => SkeletonAction::Weapon,
                },
                contact_target_player_id: None,
                pike: None,
                phase: SkeletonPhase::Approach,
            }),
            EnemyFamilyConfig::Archer { range_mode, .. } => {
                let direction = self.steering_rng.draw_sign(1.0);
                let range = construct_native_ranged_attack_range(
                    RangedFamily::Archer,
                    *range_mode,
                    &mut self.steering_rng,
                );
                EnemyBrain::Archer(ArcherBrain {
                    action_progress: 0.0,
                    marker_emitted: false,
                    vertical_offset: 0.0,
                    vertical_velocity: 0.0,
                    aim_seed: 0,
                    attack_range: range.range,
                    phase: ArcherPhase::RangeControl,
                    range_easy_pending: range.range_easy_pending,
                    strafe: ArcherStrafeState {
                        direction: if direction == -1.0 { -1 } else { 1 },
                        limb_heading_deg: owner.heading_deg,
                        movement_ramp: 0.0,
                        turn_blend: 0.0,
                    },
                })
            }
            EnemyFamilyConfig::Mage { range_mode, shield_interval, .. } => {
                // The Mage constructor first consumes its inherited Archer range draw.
                let _ = construct_native_ranged_attack_range(
                    RangedFamily::Archer,
                    0,
                    &mut self.steering_rng,
                );
                let range = construct_native_ranged_attack_range(
                    RangedFamily::Mage,
                    *range_mode,
                    &mut self.steering_rng,
                );
                EnemyBrain::Mage(MageBrain {
                    action_progress: 0.0,
                    marker_emitted: false,
                    vertical_offset: 0.0,
                    vertical_velocity: 0.0,
                    attack_range: range.range,
                    cast_program: MageCastProgram::Short,
                    cast_roll: 0.0,
                    disabled_primary_ticks: 0,
                    lightning_target_player_id: None,
                    lightning_target_position: None,
                    lightning_ticks_remaining: 0,
                    phase: MagePhase::RangeControl,
                    range_easy_pending: range.range_easy_pending,
                    shield_ticks_remaining: if *shield_interval > 0.0 {
                        (*shield_interval * 100.0).round().max(1.0) as u64
                    } else {
                        0
                    },
                })
            }
            EnemyFamilyConfig::Imp { .. } => {
                let effect_phase = self.draw_unit() * 10.0;
                let body_variant = self.draw_integer(4) as usize;
                let rotation = self.draw_unit() * 45.0;
                let body_rotation_deg = if self.draw_unit() < 0.5 { -rotation } else { rotation };
                EnemyBrain::Imp(ImpBrain {
                    base_horizontal_speed: config.base_speed,
                    body_rotation_deg,
                    body_variant,
                    effect_alpha: 0.0,
                    effect_phase,
                    horizontal_speed: config.base_speed,
                    vertical_offset: 0.0,
                    vertical_velocity: 0.0,
                    escape_heading_deg: None,
                    phase: FlightPhase::Flight,
                    visual_rng_state: self.rng_state,
                })
            }
            EnemyFamilyConfig::Zombie { body_type, .. } => {
                let body_phase_deg = self.draw_unit() * 360.0;
                let head_base_rotation_deg = (self.draw_unit() * 2.0 - 1.0) * 65.0;
                let head_phase_deg = self.draw_unit() * 360.0;
                self.draw_integer(2);
                let rear_arm_base_rotation_deg = self.draw_unit() * 20.0;
                let front_arm_base_rotation_deg = self.draw_unit() * 20.0;
                let attack_side = self.draw_integer(2) as u8;
                let body = self.draw_integer(3) as u8;
                let head_roll = self.draw_integer(4);
                let head = if head_roll == 3 { self.draw_integer(2) as u8 + 1 } else { 0 };
                EnemyBrain::Zombie(ZombieBrain {
                    action_progress: 0.0,
                    action_rate: 0.0,
                    action_swing: 0.0,
                    angular_offset_deg: 0.0,
                    attack_side,
                    body_phase_deg,
                    body_type: if *body_type == 3 { 3 } else { body },
                    contact_target_player_id: None,
                    front_arm_base_rotation_deg,
                    head_base_rotation_deg,
                    head_phase_deg,
                    head_type: if *body_type == 3 { 3 } else { head },
                    impact_state_ticks_remaining: 0,
                    marker_emitted: false,
                    phase: ZombiePhase::Approach,
                    phase_ticks_remaining: 0,
                    rear_arm_base_rotation_deg,
                    vertical_offset: 0.0,
                    vertical_velocity: 0.0,
                    visual_rng_state: self.rng_state,
                })
            }
            EnemyFamilyConfig::Wraith => {
                self.draw_unit();
                let resting = self.draw_unit();
                let flyby_offset = self.draw_integer(601);
                let initial = self.draw_unit();
                EnemyBrain::Wraith(super::wraith::create_native_wraith_flight_state(
                    config.chase_speed,
                    resting,
                    initial,
                    u64::from(flyby_offset),
                ))
            }
            EnemyFamilyConfig::Demon { .. } => EnemyBrain::Demon(DemonBrain {
                action_progress: 0.0,
                articulation: super::demon::create_demon_articulation(
                    owner.actor_id,
                    owner.spawn_tick,
                    owner.position,
                    owner.heading_deg,
                    config.scale,
                ),
                marker_emitted: false,
                phase: DemonPhase::Approach,
            }),
            EnemyFamilyConfig::Coffin { .. } => {
                let hidden = self.draw_integer(2);
                let gate = self.draw_integer(50);
                let launch_scale = if self.draw_unit() < 0.5 { -1 } else { 1 };
                let launch_rotation_deg = (self.draw_unit() * 2.0 - 1.0) * 15.0;
                EnemyBrain::Coffin(CoffinBrain {
                    anchor_position: owner.position,
                    launch_rotation_deg,
                    launch_scale,
                    maggot_charge: 0.0,
                    phase: CoffinPhase::Hidden,
                    phase_tick: 0,
                    phase_ticks_remaining: if hidden == 0 { 180 } else { 360 } + u64::from(gate),
                })
            }
            EnemyFamilyConfig::Portal { frequency } => {
                let fixed_scale = fround(1.25 + self.draw_unit() * 0.5);
                let upper = portal_frequency_bounds(*frequency).1;
                let ticks_until_ejection = 450 + u64::from(self.draw_integer(upper / 3));
                EnemyBrain::Portal(PortalBrain {
                    anchor_position: None,
                    age_ticks: 0,
                    alpha: 0.0,
                    aura_phase: 0.0,
                    body_phase: 0.0,
                    fixed_scale,
                    ticks_until_ejection,
                    hurt_ticks_remaining: 0,
                    phase: PortalPhase::Active,
                })
            }
            EnemyFamilyConfig::DemonSkull { capabilities } => EnemyBrain::DemonSkull(
                create_native_demon_skull(&mut self.steering_rng, config.scale, *capabilities),
            ),
            EnemyFamilyConfig::Faculty(_) => {
                let mut brain = create_native_faculty(&mut self.steering_rng);
                if self.faculty_voice_controller.is_none() {
                    self.faculty_voice_controller =
                        Some(FacultyVoiceController::new(&mut self.steering_rng));
                }
                register_native_faculty(
                    &mut brain,
                    &mut self.steering_rng,
                    owner.faculty_member_count,
                );
                brain.body_heading_deg = owner.heading_deg;
                EnemyBrain::Faculty(brain)
            }
            EnemyFamilyConfig::Heartmonger { crow_count, .. } => {
                let mut orbit_phase_deg = self.steering_rng.draw_scaled_float(360.0, false);
                let mut crows = Vec::with_capacity(*crow_count);
                for index in 0..*crow_count {
                    let mut crow = create_native_crow(
                        self.next_death_effect_id + index,
                        &mut self.steering_rng,
                    );
                    crow.orbit_phase_deg = orbit_phase_deg;
                    orbit_phase_deg = fround(orbit_phase_deg + 360.0 / (*crow_count as f64));
                    crows.push(crow);
                }
                self.next_death_effect_id += crow_count;
                EnemyBrain::Heartmonger(HeartmongerBrain {
                    activated: false,
                    crows,
                    crow_cooldown_ticks: 100,
                    head_variant: 0,
                    summon_ticks_remaining: 200,
                    phase: HeartmongerPhase::Approach,
                    leg_phase: 0.0,
                    torso_phase: 0.0,
                })
            }
            EnemyFamilyConfig::Spider { .. } => {
                let distance = self.steering_rng.draw_scaled_float(1.0, false);
                EnemyBrain::Spider(SpiderBrain {
                    action_state: 0,
                    preferred_distance: fround(
                        (180.0 + fround(distance * 100.0)) / (1.1_f32 as f64),
                    ),
                    body_heading_deg: -1.0,
                    move_ticks_remaining: 0,
                    strafe_sign: 0.0,
                    strafe_ticks_remaining: 0,
                    action_ticks_remaining: 0,
                    frame: 0.0,
                    spit_ticks_remaining: 0,
                    attached: false,
                    vertical_offset: 0.0,
                    attachment_angle_deg: 0.0,
                    attachment_distance: 0.0,
                    suck_ticks: 0,
                    phase: SpiderPhase::Active,
                })
            }
            EnemyFamilyConfig::Cocoon => EnemyBrain::Cocoon(CocoonBrain {
                phase: CocoonPhase::Active,
                owner_player_id: owner.cocoon_target_player_id,
                owner_position: Vec2::new(owner.position.x, owner.position.y - 1.0),
            }),
        }
    }
}

struct BrainOwner {
    actor_id: usize,
    faculty_member_count: usize,
    heading_deg: f64,
    position: Vec2,
    spawn_tick: u64,
    cocoon_target_player_id: Option<String>,
}

pub(super) fn portal_frequency_bounds(frequency: u8) -> (u32, u32) {
    match frequency {
        0 => (800, 1000),
        1 => (600, 800),
        2 => (300, 400),
        3 => (200, 300),
        4 => (100, 200),
        5 => (25, 50),
        _ => panic!("Portal frequency must be within 0..5"),
    }
}

pub(super) fn portal_child_position(
    position: Vec2,
    portal_heading: f64,
    child_heading: f64,
    radius: f64,
) -> Vec2 {
    let child = child_heading * std::f64::consts::PI / 180.0;
    let parent = portal_heading * std::f64::consts::PI / 180.0;
    Vec2::new(
        fround(position.x + float::sin(child) * 30.0 + float::sin(parent) * (5.0 + radius)),
        fround(position.y - float::cos(child) * 30.0 - float::cos(parent) * (5.0 + radius)),
    )
}
