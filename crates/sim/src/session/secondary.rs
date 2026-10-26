//! Secondary snapshot epoch and ordered mutations from `game-simulation.ts` and
//! `native-secondary-world.ts`. Geometry callbacks borrow the actual active world.

use super::secondary_world::SessionSecondaryWorld;
use super::*;
use crate::Vec2;
use crate::boneyard::projectile_world::BoneyardProjectileViewport;
use crate::boneyard::world::{BoneyardPlayerBody, BoneyardPlayerStatus};
use crate::combat::PlayerLifeState;
use crate::enemies::model::EnemyDamageRequest;
use crate::enemies::projectiles::EnemyProjectileRead;
use crate::float::fround;
use crate::hub::interaction::BoastFailureProducer;
use crate::player::PlayerState;
use crate::profile::native_hagatha_boss_damage_factor;
use crate::secondary::*;
use crate::spells::casting::NativePlayerWeaponKind;
use crate::spells::contacts::{
    BoneyardSpecialDeathInput, BoneyardSpecialDeathResult, resolve_boneyard_special_death_payload,
};
use std::collections::BTreeMap;

impl GameSession {
    pub(super) fn activate_secondary_belt_skill(
        &mut self,
        id: &str,
        ability: NativeSecondaryAbilityId,
        slot: usize,
        aim: Option<Vec2>,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) -> Result<(), String> {
        self.fail_boast_with_extensions(id, BoastFailureProducer::SecondaryCast, extensions);
        let previous_mindstars = self
            .secondary
            .players
            .iter()
            .filter(|(_, player)| player.mindstar)
            .map(|(id, _)| id.clone())
            .collect();
        let positions: Vec<_> = self
            .participant_order
            .iter()
            .map(|id| self.participants.get(id).expect("participant exists").wizard.position)
            .collect();
        let key = self.world_key(id).expect("an admitted secondary owner has its actual world");
        let teleport_bodies = SessionSecondaryWorld::collect_teleport_bodies(
            self.boneyard.as_ref(),
            &positions,
            &self.secondary.actors,
        );
        let world = SessionSecondaryWorld {
            boneyard: self.boneyard.as_mut(),
            players: boneyard_player_bodies(&self.participants, &self.participant_order, &self.run),
            ether_drain: None,
            regions: self
                .participants
                .iter()
                .map(|(id, participant)| (id.clone(), participant.wizard.region))
                .collect(),
            teleport_bodies,
            // Source supplies null secondaryProjectileVisible for this operation.
            viewports: Vec::new(),
        };
        let participant = &self.participants.get(id).expect("participant exists");
        let player = &participant.player;
        let derived = player.derived_stats();
        let mut authority = NativeSecondaryPlayerAuthority::from_snapshot_with_stat_book(
            NativeSecondaryPlayerSnapshot {
                alive: player.combat.life_state == PlayerLifeState::Alive,
                weapon_kind: player_weapon(player),
                belt: &player.belt,
                character: participant.wizard,
                current_mana: player.combat.current_mana,
                maximum_mana: player.combat.maximum_mana,
                eligible: player.combat.can_cast() && player.progression.pending_offer.is_none(),
                input: NativeSecondaryInput { aim, ..NativeSecondaryInput::default() },
                skill_book: &player.book,
                world_key: &key,
            },
            &player.stat_book,
            &derived,
            derived.offensive_factors(&player.skill_runtime),
        );
        authority.enhanced_effects = self.enhanced_effects;
        let players = BTreeMap::from([(id.to_owned(), authority)]);
        let first_event = self.secondary.next_event_id;
        let result = self.secondary.activate_belt_skill(
            id,
            ability,
            slot,
            &NativeSecondaryTickContext {
                enhanced_effects: Some(self.enhanced_effects),
                players: &players,
                tick: self.ticks,
            },
            &world,
            Some(&mut self.painter_order),
        );
        self.apply_secondary_player_outcomes(&result, &previous_mindstars, self.ticks, extensions)?;
        let first_enemy_event = self.boneyard.as_ref().unwrap().enemies.next_event_id;
        // Source immediate casts return damage:[]; shield dispel has no terminal
        // payload. Only the fixed-tick receiver lane can return deferred newborns.
        let _ = self.apply_secondary_contacts(&result, self.ticks);
        if !result.target_heading_changes.is_empty()
            || !result.dampened_caster_target_ids.is_empty()
            || !result.dispelled_shield_target_ids.is_empty()
            || !result.removed_projectile_ids.is_empty()
        {
            let events = self
                .boneyard
                .as_ref()
                .unwrap()
                .enemies
                .events
                .iter()
                .filter(|event| event.event_id >= first_enemy_event)
                .cloned()
                .collect::<Vec<_>>();
            self.retain_world_events(&events, Vec::new(), &[], self.ticks);
        }
        let mut flashes = self.screen_flashes.writer(self.ticks);
        self.secondary.write_screen_flashes_since(first_event, &mut flashes);
        self.screen_flashes = flashes.finish();
        Ok(())
    }

    pub(super) fn step_secondary(
        &mut self,
        inputs: &BTreeMap<String, GameSessionInput>,
        tick: u64,
        unstepped_ids: &[u64],
        extensions: Option<&dyn GameSimulationExtensions>,
    ) -> Result<NativeSecondaryTickResult, String> {
        let unstepped = self.secondary.actors.extract_if(|actor| unstepped_ids.contains(&actor.id));
        let previous_mindstars: std::collections::BTreeSet<_> = self
            .secondary
            .players
            .iter()
            .filter(|(_, player)| player.mindstar)
            .map(|(id, _)| id.clone())
            .collect();
        let positions: Vec<_> = self
            .participant_order
            .iter()
            .map(|id| self.participants.get(id).expect("participant exists").wizard.position)
            .collect();
        let ether_drain = super::ether_drain::SessionEtherDrainSnapshot::capture(self);
        let teleport_bodies = SessionSecondaryWorld::collect_teleport_bodies(
            self.boneyard.as_ref(),
            &positions,
            &self.secondary.actors,
        );
        let keys: BTreeMap<_, _> = self
            .participant_order
            .iter()
            .map(|id| (id.clone(), self.world_key(id).unwrap()))
            .collect();
        let mut players = BTreeMap::new();
        for id in &self.participant_order {
            let participant = &self.participants.get(id).expect("participant exists");
            let player = &participant.player;
            let derived = player.derived_stats();
            let input = inputs[id];
            let snapshot = NativeSecondaryPlayerSnapshot {
                alive: player.combat.life_state == PlayerLifeState::Alive,
                weapon_kind: player_weapon(player),
                belt: &player.belt,
                character: participant.wizard,
                current_mana: player.combat.current_mana,
                maximum_mana: player.combat.maximum_mana,
                eligible: player.combat.can_cast() && player.progression.pending_offer.is_none(),
                input: NativeSecondaryInput {
                    aim: input.aim,
                    primary: input.primary,
                    quickbar: input.quickbar,
                },
                skill_book: &player.book,
                world_key: &keys[id],
            };
            let mut authority = NativeSecondaryPlayerAuthority::from_snapshot_with_stat_book(
                snapshot,
                &player.stat_book,
                &derived,
                derived.offensive_factors(&player.skill_runtime),
            );
            authority.enhanced_effects = self.enhanced_effects;
            players.insert(id.clone(), authority);
        }
        let mut world = SessionSecondaryWorld {
            boneyard: self.boneyard.as_mut(),
            players: boneyard_player_bodies(&self.participants, &self.participant_order, &self.run),
            ether_drain,
            regions: self
                .participants
                .iter()
                .map(|(id, participant)| (id.clone(), participant.wizard.region))
                .collect(),
            teleport_bodies,
            viewports: self
                .participant_order
                .iter()
                .map(|id| BoneyardProjectileViewport {
                    position: self
                        .participants
                        .get(id)
                        .expect("participant exists")
                        .wizard
                        .position,
                    viewport: inputs[id].viewport,
                    alternate_player: false,
                })
                .collect(),
        };
        let first_event_id = self.secondary.next_event_id;
        let result = self.secondary.step(
            &NativeSecondaryTickContext {
                enhanced_effects: Some(self.enhanced_effects),
                players: &players,
                tick,
            },
            &mut world,
            Some(&mut self.painter_order),
        );
        let mut flashes = self.screen_flashes.writer(tick);
        self.secondary.write_screen_flashes_since(first_event_id, &mut flashes);
        self.screen_flashes = flashes.finish();
        self.secondary.actors.extend(unstepped);
        self.secondary.actors.sort_by_key(|actor| actor.id);
        self.apply_ether_drain_world_animation_forces(&result.ether_drain_world_animation_contacts);
        self.apply_ether_drain_forces_and_player_contacts(
            &result.ether_drain_contacts,
            tick,
            extensions,
        )?;
        self.apply_secondary_player_outcomes(&result, &previous_mindstars, tick, extensions)?;
        Ok(result)
    }

    /// `applySecondaryPlayerOutcomes` is shared by tick and immediate belt casts.
    fn apply_secondary_player_outcomes(
        &mut self,
        result: &NativeSecondaryTickResult,
        previous_mindstars: &std::collections::BTreeSet<String>,
        tick: u64,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) -> Result<(), String> {
        for (id, position) in &result.relocated_players {
            if let Some(participant) = self.participants.get_mut(id) {
                participant.wizard.position = *position;
            }
        }
        for (id, heading) in &result.facing_heading_degrees_by_player {
            if let Some(participant) = self.participants.get_mut(id) {
                participant.wizard.set_facing(*heading);
            }
        }
        for (id, amount) in &result.mana_recovered {
            if let Some(participant) = self.participants.get_mut(id) {
                participant.player.combat.restore_mana(*amount);
            }
        }
        for (id, amount) in &result.health_recovered {
            if let Some(participant) = self.participants.get_mut(id) {
                participant.player.combat.restore_health(*amount);
            }
        }
        for (id, amount) in &result.mana_spent {
            if *amount > 0.0
                && let Some(participant) = self.participants.get_mut(id)
            {
                super::extensions::apply_filtered_mana_delta(
                    participant.player,
                    id,
                    -*amount,
                    crate::modding::extensions::GameSimulationManaSource::SecondaryCast,
                    tick,
                    extensions,
                    Some("secondary ability mana authority diverged"),
                )?;
            }
        }
        for id in &result.mana_underflow_player_ids {
            self.fail_boast_with_extensions(id, BoastFailureProducer::ManaUnderflow, extensions);
        }
        for id in &result.overloaded_player_ids {
            if let Some(participant) = self.participants.get_mut(id) {
                let cost = participant.player.combat.current_mana;
                super::extensions::apply_filtered_mana_delta(
                    participant.player,
                    id,
                    -cost,
                    crate::modding::extensions::GameSimulationManaSource::Overload,
                    tick,
                    extensions,
                    None,
                )?;
            }
        }
        for id in &self.participant_order {
            let participant = self.participants.get_mut(id).unwrap();
            let next = self.secondary.players.get(id).is_some_and(|player| player.mindstar);
            if previous_mindstars.contains(id) != next {
                participant.player.set_mindstar(next);
            }
            if let Some(secondary) = self.secondary.players.get_mut(id) {
                secondary.reserved_mana = secondary
                    .mana_reserve(&participant.player.book, participant.player.combat.maximum_mana);
            }
        }
        Ok(())
    }

    /// Resolve the admitted secondary receiver lane. Returned newborns publish
    /// after the ordinary primary pass, so their age-zero area cannot replay.
    pub fn apply_secondary_contacts(
        &mut self,
        result: &NativeSecondaryTickResult,
        tick: u64,
    ) -> Vec<BoneyardSpecialDeathResult> {
        let Some(world) = &mut self.boneyard else {
            return Vec::new();
        };
        let mut secondary_deaths = Vec::new();
        world
            .enemies
            .projectiles
            .retain(|projectile| !result.removed_projectile_ids.contains(&projectile.state().id));
        world
            .enemies
            .boss_spells
            .retain(|spell| !result.removed_projectile_ids.contains(&spell.owner.id));
        world.enemies.dampen_casters(
            &result.dampened_caster_target_ids,
            tick,
            Some(&mut self.painter_order),
        );
        for id in &result.dispelled_shield_target_ids {
            let amount = self
                .boneyard
                .as_ref()
                .unwrap()
                .enemies
                .actors
                .iter()
                .find(|actor| actor.id == *id)
                .map_or(0.0, |actor| actor.shield_health);
            if amount > 0.0 {
                self.damage_enemy(EnemyDamageRequest {
                    enhanced_effects: Some(self.enhanced_effects),
                    actor_id: *id,
                    amount,
                    ether_drain_capture: false,
                    has_magic_damage: false,
                    hit_strength: 1.0,
                    magic: false,
                    source_player_id: None,
                    suppress_hit_reaction: false,
                    suppress_hurt_sound: false,
                    tick,
                });
            }
        }
        let world_key = format!("boneyard:{}", self.boneyard.as_ref().unwrap().loaded.run_id);
        let fields: Vec<_> = self
            .secondary
            .actors
            .iter()
            .filter(|actor| {
                actor.kind == NativeSecondaryActorKind::EtherDrain && actor.world_key == world_key
            })
            .map(|actor| actor.position)
            .collect();
        for contact in &result.damage {
            let world = self.boneyard.as_ref().unwrap();
            let actor = world.enemies.actors.iter().find(|actor| actor.id == contact.target_id);
            let multiplier = actor.map_or(1.0, |actor| {
                self.participants.get(&contact.owner_id).map_or(1.0, |participant| {
                    native_hagatha_boss_damage_factor(
                        &participant.player.economy.owned_perk_selectors,
                        u32::from(actor.config.native_type_id),
                    )
                })
            });
            let capture = contact.ether_drain
                && actor.is_some_and(|actor| {
                    fields.iter().any(|position| {
                        let x = position.x - actor.position.x;
                        let y = position.y - actor.position.y;
                        x * x + y * y < 1600.0
                    })
                });
            let victim = actor.map(|actor| (actor.id, actor.position)).or_else(|| {
                world
                    .enemies
                    .maggots
                    .iter()
                    .find(|actor| actor.id == contact.target_id)
                    .map(|actor| (actor.id, actor.position))
            });
            let amount = contact.amount * multiplier;
            // Rescue/Knockback own movement independently of their legal zero debit.
            if amount == 0.0 && contact.amount.is_finite() && contact.amount >= 0.0 {
                continue;
            }
            let magic = contact.kind != NativeSecondaryDamageKind::Physical;
            let damaged = self.damage_enemy(EnemyDamageRequest {
                enhanced_effects: Some(self.enhanced_effects),
                actor_id: contact.target_id,
                amount,
                ether_drain_capture: capture,
                has_magic_damage: magic,
                hit_strength: contact.hit_strength.unwrap_or(1.0),
                magic,
                source_player_id: Some(contact.owner_id.clone()),
                suppress_hit_reaction: contact.suppress_hit_reaction,
                suppress_hurt_sound: contact.suppress_hurt_sound,
                tick,
            });
            if damaged.special_death_accepted
                && let Some(payload) = contact.special_death_payload
                && let Some((victim_id, position)) = victim
            {
                let death = resolve_boneyard_special_death_payload(
                    &mut self.boneyard.as_mut().unwrap().enemies,
                    BoneyardSpecialDeathInput {
                        payload,
                        victim_id,
                        position,
                        owner_id: &contact.owner_id,
                        next_spell_id: self.spells.next_id,
                        tick,
                        world_key: &world_key,
                        enhanced_effects: self.enhanced_effects,
                    },
                    &mut self.combat_rng,
                    &mut self.painter_order,
                );
                self.spells.next_id = death.spells.next_id;
                self.record_spell_damage_notices(&death.combat.damage_notices);
                secondary_deaths.push(death);
            }
        }
        self.apply_ether_drain_enemy_contacts(&result.ether_drain_contacts, tick);
        let world = self.boneyard.as_mut().unwrap();
        for change in &result.target_heading_changes {
            if let Some(mut actor) =
                world.enemies.actors.iter_mut().find(|actor| actor.id == change.target_id)
            {
                let heading = if change.mode == NativeSecondaryHeadingMode::Absolute {
                    change.degrees
                } else {
                    actor.heading_deg + change.degrees
                };
                actor.heading_deg = fround((heading % 360.0 + 360.0) % 360.0);
                if change.mode == NativeSecondaryHeadingMode::Absolute {
                    actor.path.wander_heading_deg = actor.heading_deg;
                }
            } else if let Some(mut actor) =
                world.enemies.maggots.iter_mut().find(|actor| actor.id == change.target_id)
            {
                let heading = if change.mode == NativeSecondaryHeadingMode::Absolute {
                    change.degrees
                } else {
                    actor.heading_deg + change.degrees
                };
                actor.heading_deg = fround((heading % 360.0 + 360.0) % 360.0);
            }
        }
        secondary_deaths
    }

    pub(super) fn apply_secondary_knockbacks(
        &mut self,
        result: &NativeSecondaryTickResult,
        previous_owners: &[(u64, String)],
    ) {
        if self.boneyard.is_none() || result.knockbacks.is_empty() {
            return;
        }
        let mut impulses = Vec::with_capacity(result.knockbacks.len());
        for knockback in &result.knockbacks {
            // A secondary actor can retire during this epoch and still emit its final push.
            let owner = self
                .secondary
                .actors
                .iter()
                .find(|actor| actor.id == knockback.source_actor_id)
                .map(|actor| &actor.common.owner_id)
                .or_else(|| {
                    previous_owners
                        .iter()
                        .find(|(id, _)| *id == knockback.source_actor_id)
                        .map(|(_, owner)| owner)
                });
            let factor = owner
                .and_then(|id| self.participants.get(id))
                .map_or(1.0, |participant| participant.player.derived_stats().push_strength_factor);
            let delta = if factor == 1.0 {
                knockback.delta
            } else {
                Vec2::new(fround(knockback.delta.x * factor), fround(knockback.delta.y * factor))
            };
            impulses.push((knockback.target_id, delta));
        }
        let players =
            boneyard_player_bodies(&self.participants, &self.participant_order, &self.run);
        self.boneyard.as_mut().unwrap().apply_secondary_enemy_knockbacks(&impulses, &players);
    }

    pub fn fail_boast(&mut self, id: &str, producer: BoastFailureProducer) {
        self.fail_boast_with_extensions(id, producer, None);
    }

    pub fn fail_boast_with_extensions(
        &mut self,
        id: &str,
        producer: BoastFailureProducer,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) {
        let Some(participant) = self.participants.get_mut(id) else {
            return;
        };
        super::extensions::fail_player_boast(participant.player, producer, extensions);
    }
}

pub(super) fn player_weapon(player: &PlayerState) -> Option<NativePlayerWeaponKind> {
    match player.economy.equipment.weapon.as_ref().and_then(|item| item.equipment_type) {
        Some(crate::economy::EquipmentType::Staff) => Some(NativePlayerWeaponKind::Staff),
        Some(crate::economy::EquipmentType::Wand) => Some(NativePlayerWeaponKind::Wand),
        _ => None,
    }
}

pub(super) fn boneyard_player_bodies<'a>(
    participants: &'a Participants,
    order: &[String],
    run: &crate::game_run::GameRunLifecycle,
) -> Vec<BoneyardPlayerBody<'a>> {
    order
        .iter()
        .map(|id| {
            let participant = participants.get(id).expect("participant exists");
            BoneyardPlayerBody {
                status: BoneyardPlayerStatus {
                    id: participant.id,
                    alive: participant.player.combat.life_state == PlayerLifeState::Alive,
                    eligible: run.eligible_player_ids.contains(id),
                    collision_enabled: participant.player.combat.collision_enabled_after_tick(),
                    movement_scale: participant.player.combat.movement_scale()
                        * participant.player.derived_stats().movement_factor,
                },
                position: participant.wizard.position,
            }
        })
        .collect()
}
