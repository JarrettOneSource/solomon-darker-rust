//! `player-staff-combat-system.ts`: one ordered host over the canonical transient queue.

use super::*;
use crate::Vec2;
use crate::boneyard::collision::first_boneyard_path_block_progress;
use crate::boneyard::{BoneyardMovementStep, BoneyardWorld};
use crate::enemies::EnemyFlag;
use crate::enemies::model::{EnemyBrain, EnemyDamageRequest, EnemyLifeState};
use crate::enemies::targeting::{boneyard_enemy_actor_flags, boneyard_enemy_collision_radius};
use crate::painter::WorldManagerLane;
use crate::profile::runtime::{PlayerStaffAttackOutcome, PlayerStaffDamageLane};
use crate::profile::{WizardElement, native_hagatha_boss_damage_factor, player_staff_damage};
use crate::spells::model::PrimarySpellTransient;
use crate::spells::staff::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct StaffTickResult {
    pub acting_player_ids: BTreeSet<String>,
    pub camera_displacements: Vec<crate::secondary::NativeSecondaryCameraDisplacement>,
    pub displacements: Vec<(usize, Vec2)>,
    pub dazzle_requests: Vec<(usize, u64)>,
    pub heading_perturbations: Vec<(usize, f64)>,
    pub pike_break_feedback: Vec<(String, Vec2)>,
}

struct StaffCombatTarget {
    row: NativeStaffTarget,
    actor_id: usize,
    heading_degrees: f64,
    native_type_id: u16,
    pike: bool,
}
impl AsRef<NativeStaffTarget> for StaffCombatTarget {
    fn as_ref(&self) -> &NativeStaffTarget {
        &self.row
    }
}

struct StaffKnockbackWorld<'world, 'player> {
    world: &'world mut BoneyardWorld,
    players: &'world [crate::boneyard::world::BoneyardPlayerBody<'player>],
}
impl crate::knockback::NativeKnockbackAreaWorld<String> for StaffKnockbackWorld<'_, '_> {
    fn position(&self, id: &String) -> Option<Vec2> {
        staff_targets(self.world)
            .into_iter()
            .find(|target| &target.row.id == id)
            .map(|target| target.row.position)
    }
    fn move_actor(&mut self, id: &String, delta: Vec2) -> Vec<String> {
        let Some(id) = enemy_id(id) else {
            return Vec::new();
        };
        self.world
            .move_knockback_area_target(id, delta, self.players)
            .into_iter()
            .map(|id| format!("enemy:{id}"))
            .collect()
    }
}

impl GameSession {
    pub(super) fn step_staff(
        &mut self,
        inputs: &BTreeMap<String, GameSessionInput>,
        movements: &BTreeMap<String, BoneyardMovementStep>,
        tick: u64,
    ) -> StaffTickResult {
        let mut output = StaffTickResult::default();
        let mut retained = Vec::new();
        let mut spawned = Vec::new();
        let mut existing_actions = BTreeSet::new();
        let transients = std::mem::take(&mut self.spells.transients);
        for transient in transients {
            let PrimarySpellTransient::Staff(actor) = transient else {
                retained.push(transient);
                continue;
            };
            match actor {
                NativePlayerStaffTransient::Contact(event) => {
                    if let Some(event) = step_native_staff_contact_event(&event) {
                        retained.push(PrimarySpellTransient::Staff(
                            NativePlayerStaffTransient::Contact(event),
                        ));
                    }
                }
                NativePlayerStaffTransient::ContactKnockback(actor) => {
                    let id = enemy_id(&actor.target_id);
                    let exists = id.is_some_and(|id| {
                        staff_targets(self.boneyard.as_ref().unwrap())
                            .iter()
                            .any(|target| target.actor_id == id)
                    });
                    let stepped = step_native_staff_contact_knockback(&actor, exists);
                    if let (Some(id), Some(delta)) = (id, stepped.displacement) {
                        self.boneyard.as_mut().unwrap().enemies.release_skeleton_pike(id);
                        output.displacements.push((id, delta));
                    }
                    if let Some(actor) = stepped.actor {
                        retained.push(PrimarySpellTransient::Staff(
                            NativePlayerStaffTransient::ContactKnockback(actor),
                        ));
                    }
                }
                NativePlayerStaffTransient::PikeBreak(vfx) => {
                    if let Some(vfx) = step_native_staff_pike_break_vfx(&vfx) {
                        retained.push(PrimarySpellTransient::Staff(
                            NativePlayerStaffTransient::PikeBreak(vfx),
                        ));
                    }
                }
                NativePlayerStaffTransient::Vfx(vfx) => {
                    if let Some(vfx) = step_native_player_staff_vfx(&vfx) {
                        retained.push(PrimarySpellTransient::Staff(
                            NativePlayerStaffTransient::Vfx(vfx),
                        ));
                    }
                }
                NativePlayerStaffTransient::Knockback(actor) => {
                    let bodies = super::secondary::boneyard_player_bodies(
                        &self.participants,
                        &self.participant_order,
                        &self.run,
                    );
                    let mut world = StaffKnockbackWorld {
                        world: self.boneyard.as_mut().unwrap(),
                        players: &bodies,
                    };
                    let stepped =
                        step_native_staff_knockback(&actor, &mut world, &mut self.secondary.rng);
                    output.camera_displacements.push(
                        crate::secondary::NativeSecondaryCameraDisplacement {
                            displacement: stepped.camera_displacement,
                            tick,
                            world_key: actor.common.world_key.clone(),
                        },
                    );
                    for target in stepped.dazzled_target_ids {
                        if let Some(id) = enemy_id(&target) {
                            output.dazzle_requests.push((id, NATIVE_STAFF_KNOCKBACK_DAZZLE_TICKS));
                        }
                    }
                    for perturbation in stepped.heading_perturbations {
                        if let Some(id) = enemy_id(&perturbation.target_id) {
                            output.heading_perturbations.push((id, perturbation.heading_degrees));
                        }
                    }
                    if let Some(actor) = stepped.actor {
                        retained.push(PrimarySpellTransient::Staff(
                            NativePlayerStaffTransient::Knockback(actor),
                        ));
                    }
                }
                NativePlayerStaffTransient::Action(action) => {
                    let owner = action.common().owner_id.clone();
                    let Some(participant) = self.participants.get_mut(&owner) else {
                        continue;
                    };
                    existing_actions.insert(owner.clone());
                    output.acting_player_ids.insert(owner);
                    let stepped =
                        step_native_player_staff_action(&action, participant.wizard.position);
                    participant.wizard.set_facing(stepped.sample.base().heading_degrees);
                    if stepped.contact {
                        self.apply_staff_contact(&stepped.sample, tick, &mut spawned, &mut output);
                    }
                    // A retiring Spin still emitted the contact from its sampled terminal frame.
                    if let Some(action) = stepped.action {
                        retained.push(PrimarySpellTransient::Staff(
                            NativePlayerStaffTransient::Action(action),
                        ));
                    }
                }
            }
        }
        for id in self.participant_order.clone() {
            let participant = self.participants.get_mut(&id).unwrap();
            let player = &mut participant.player;
            let world = self.boneyard.as_ref().unwrap();
            let derived = player.derived_stats();
            let input = inputs[&id];
            let movement = &movements[&id];
            let secondary_acting = self.secondary.players.get(&id).is_some_and(|player| {
                player.cast_action.is_some() || player.cast_spin_ticks_remaining > 0
            });
            let admission =
                world.encounter.as_ref().is_none_or(|encounter| encounter.combat_enabled());
            if !admission
                || existing_actions.contains(&id)
                || secondary_acting
                || player.combat.life_state != crate::combat::PlayerLifeState::Alive
                || player.progression.pending_offer.is_some()
                || !derived.staff_equipped
                || participant.wizard.primary_cast.action_tick.is_some()
                || participant.wizard.primary_cast.channel_active
                || input.primary
                || input.quickbar.is_some()
                || !movement.movement_active
            {
                continue;
            }
            let has_target = if movement.contacts.is_empty() {
                let targets = staff_targets(world);
                let rows: Vec<_> = targets.iter().map(|target| target.row.clone()).collect();
                native_staff_admission_target(
                    &NativeStaffPlayer {
                        collision_radius: 25.0,
                        heading_degrees: participant.wizard.heading_degrees,
                        position: participant.wizard.position,
                    },
                    &rows,
                )
                .is_some()
            } else {
                movement.contacts.iter().any(|contact| contact.staff_hostile)
            };
            if !has_target {
                continue;
            }
            let lane = if player.skill_runtime.staff_melee_alternate {
                PlayerStaffDamageLane::Secondary
            } else {
                PlayerStaffDamageLane::Primary
            };
            let world_key = format!("boneyard:{}", world.loaded.run_id);
            let action = create_native_player_staff_action(
                NativePlayerStaffActionInput {
                    derived: &derived,
                    heading_degrees: participant.wizard.heading_degrees,
                    id: self.spells.next_id,
                    lane,
                    origin: participant.wizard.position,
                    owner_id: &id,
                    world_key: &world_key,
                },
                &mut self.secondary.rng,
            );
            self.spells.next_id += 1;
            if matches!(action, NativePlayerStaffAction::Melee(_)) {
                player.skill_runtime.staff_melee_alternate =
                    !player.skill_runtime.staff_melee_alternate;
            }
            spawned.push(PrimarySpellTransient::Staff(NativePlayerStaffTransient::Action(action)));
            output.acting_player_ids.insert(id);
        }
        retained.extend(spawned);
        self.spells.transients = retained;
        output
    }

    fn apply_staff_contact(
        &mut self,
        action: &NativePlayerStaffAction,
        tick: u64,
        spawned: &mut Vec<PrimarySpellTransient>,
        output: &mut StaffTickResult,
    ) {
        let owner = &action.common().owner_id;
        let participant = &self.participants[owner];
        let element = participant.player.selection.element;
        let derived = participant.player.derived_stats();
        let total = player_staff_damage(
            &participant.player.skill_runtime,
            &derived,
            &participant.player.progression,
            action.base().outcome,
        );
        let split_damage = participant.player.book.effective_ranks[65] > 0;
        // All physical and Knockback candidates are captured before damage. A lethal hit
        // cannot remove its body from this same callback's constructor/RNG sequence.
        let before = staff_targets(self.boneyard.as_ref().unwrap());
        let mut selected = native_staff_damage_targets(action, &before);
        if !matches!(
            action.base().outcome,
            PlayerStaffAttackOutcome::CriticalHit | PlayerStaffAttackOutcome::Whirl
        ) && !split_damage
            && !selected.is_empty()
        {
            let index = self.secondary.rng.draw_integer(selected.len() as i32) as usize;
            selected = vec![selected[index]];
        }
        let physical_rows: Vec<_> = before
            .iter()
            .map(|target| NativeStaffPhysicalTarget {
                target: target.row.clone(),
                pike: target.pike,
            })
            .collect();
        let physical = native_staff_physical_contact_targets(
            &NativeStaffPlayer {
                collision_radius: 25.0,
                heading_degrees: action.base().heading_degrees,
                position: action.base().origin,
            },
            &physical_rows,
        );
        let physical: Vec<_> = physical.into_iter().cloned().collect();
        let impacts = resolve_native_staff_physical_contacts(
            action,
            &physical,
            element,
            derived.staff_damage_secondary,
            &mut self.secondary.rng,
        );
        let mut pitches = Vec::new();
        let mut pike_sounds = Vec::new();
        for impact in impacts {
            pitches.push(impact.sound_pitch);
            let Some(id) = enemy_id(&impact.target_id) else {
                continue;
            };
            self.boneyard
                .as_mut()
                .unwrap()
                .enemies
                .apply_staff_impact_vertical_velocity(id, impact.vertical_velocity);
            if let Some(rng) = impact.pike_break_presentation_rng {
                let target = before.iter().find(|target| target.actor_id == id);
                if self.boneyard.as_mut().unwrap().enemies.break_skeleton_pike(id)
                    && let Some(target) = target
                {
                    pike_sounds.push(pitches.len() - 1);
                    output.pike_break_feedback.push((owner.clone(), target.row.position));
                    let effect = create_native_staff_pike_break_vfx(
                        self.spells.next_id,
                        action,
                        &target.row,
                        rng,
                        target.heading_degrees,
                        self.enhanced_effects,
                    );
                    self.spells.next_id += 1;
                    self.push_staff_birth(NativePlayerStaffTransient::PikeBreak(effect), spawned);
                }
            }
            if let Some(delta) = impact.contact_knockback_delta {
                let delta = Vec2::new(
                    crate::float::fround(delta.x * derived.push_strength_factor),
                    crate::float::fround(delta.y * derived.push_strength_factor),
                );
                let effect = create_native_staff_contact_knockback(
                    self.spells.next_id,
                    action,
                    &impact.target_id,
                    delta,
                );
                self.spells.next_id += 1;
                self.push_staff_birth(
                    NativePlayerStaffTransient::ContactKnockback(effect),
                    spawned,
                );
            }
        }
        let mut accepted = Vec::new();
        // The source damage host returns on an empty selection. Physical contacts
        // above still run, and the positive-count kernel must not receive zero.
        if !selected.is_empty() {
            let damage = native_staff_contact_damage_per_target(
                total,
                selected.len(),
                action.base().outcome == PlayerStaffAttackOutcome::Whirl,
            );
            for target in selected {
                let amount = damage
                    * native_hagatha_boss_damage_factor(
                        &self.participants[owner].player.economy.owned_perk_selectors,
                        u32::from(target.native_type_id),
                    );
                let result = self.damage_enemy(EnemyDamageRequest {
                    enhanced_effects: Some(self.enhanced_effects),
                    actor_id: target.actor_id,
                    amount,
                    ether_drain_capture: false,
                    has_magic_damage: false,
                    hit_strength: 1.0,
                    magic: false,
                    source_player_id: Some(owner.clone()),
                    suppress_hit_reaction: false,
                    suppress_hurt_sound: false,
                    tick,
                });
                if result.accepted {
                    accepted.push(target);
                    if action.base().outcome == PlayerStaffAttackOutcome::DisablingHit {
                        self.boneyard
                            .as_mut()
                            .unwrap()
                            .enemies
                            .apply_staff_disable(target.actor_id);
                    }
                }
            }
        }
        if !accepted.is_empty() {
            let world = self.boneyard.as_ref().unwrap();
            let targets = native_staff_knockback_targets(action, &before);
            let ids: Vec<_> = targets
                .into_iter()
                .filter(|target| {
                    first_boneyard_path_block_progress(
                        action.base().origin,
                        target.row.position,
                        world.active_bounds(),
                        &world.dynamic_collision,
                        0.0,
                        &[],
                    )
                    .is_none_or(|progress| progress >= 1.0)
                })
                .map(|target| target.row.id.clone())
                .collect();
            if let Some(effect) = create_native_staff_knockback(
                self.spells.next_id,
                action,
                &ids,
                derived.push_strength_factor,
            ) {
                self.spells.next_id += 1;
                self.push_staff_birth(NativePlayerStaffTransient::Knockback(effect), spawned);
            }
        }
        let target_mean = if accepted.is_empty() {
            action.base().origin
        } else {
            let mut sum = Vec2::ZERO;
            for target in &accepted {
                sum.x += target.row.position.x;
                sum.y += target.row.position.y;
            }
            Vec2::new(sum.x / accepted.len() as f64, sum.y / accepted.len() as f64)
        };
        let target_ids: Vec<_> = accepted.iter().map(|target| target.row.id.clone()).collect();
        let presentation = create_native_staff_contact_presentation(
            NativeStaffContactPresentationInput {
                first_id: self.spells.next_id,
                action,
                target_ids: &target_ids,
                target_mean,
                element_tint: element_tint(element),
                impact_sound_pitches: &pitches,
                pike_break_sound_indexes: &pike_sounds,
            },
            &mut self.secondary.rng,
        );
        self.spells.next_id = presentation.next_id;
        for effect in presentation.vfx {
            self.push_staff_birth(NativePlayerStaffTransient::Vfx(effect), spawned);
        }
        self.push_staff_birth(NativePlayerStaffTransient::Contact(presentation.event), spawned);
    }

    fn push_staff_birth(
        &mut self,
        mut actor: NativePlayerStaffTransient,
        output: &mut Vec<PrimarySpellTransient>,
    ) {
        if matches!(
            actor,
            NativePlayerStaffTransient::PikeBreak(_) | NativePlayerStaffTransient::Vfx(_)
        ) {
            actor.common_mut().painter_registrations =
                vec![self.painter_order.register(WorldManagerLane::Actor)];
        }
        output.push(PrimarySpellTransient::Staff(actor));
    }
}

fn staff_targets(world: &BoneyardWorld) -> Vec<StaffCombatTarget> {
    let mut rows = Vec::new();
    for actor in world.enemies.actors.iter() {
        if boneyard_enemy_actor_flags(&actor) & 2 == 0 {
            continue;
        }
        rows.push(StaffCombatTarget {
            actor_id: actor.id,
            row: NativeStaffTarget {
                collision_radius: boneyard_enemy_collision_radius(&actor),
                id: format!("enemy:{}", actor.id),
                position: actor.position,
                registration_order: Some(actor.native_registration_order),
            },
            heading_degrees: actor.heading_deg,
            native_type_id: actor.config.native_type_id,
            pike: matches!(actor.brain, EnemyBrain::Skeleton(_))
                && actor.config.flags.contains(&EnemyFlag::Pike),
        });
    }
    for actor in &world.enemies.maggots {
        if actor.life_state != EnemyLifeState::Alive || !actor.combat_active {
            continue;
        }
        rows.push(StaffCombatTarget {
            actor_id: actor.id,
            row: NativeStaffTarget {
                collision_radius: actor.collision_radius,
                id: format!("enemy:{}", actor.id),
                position: actor.position,
                registration_order: Some(actor.native_registration_order),
            },
            heading_degrees: actor.heading_deg,
            native_type_id: 0,
            pike: false,
        });
    }
    rows.sort_by_key(|target| target.actor_id);
    rows
}
fn enemy_id(id: &str) -> Option<usize> {
    id.strip_prefix("enemy:")?.parse::<usize>().ok().filter(|id| *id > 0)
}
fn element_tint(element: WizardElement) -> u32 {
    match element {
        WizardElement::Air => 0xa0c3c3,
        WizardElement::Earth => 0x90b390,
        WizardElement::Ether => 0x886688,
        WizardElement::Fire => 0x998077,
        WizardElement::Water => 0x5e6e81,
    }
}
