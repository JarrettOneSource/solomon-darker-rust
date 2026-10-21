//! Enemy-to-player orchestration from `core-server/player-contact-system.ts`.
//! Reflections and adjusted status rows are returned for the session's later
//! source phases. Immediate resources, facing, web ownership and births live here.

use std::collections::BTreeMap;

use super::PlayerLightingState;
use super::harden::PendingPlayerHardenChip;
use super::player_feedback::{
    PlayerDamageSoundRequest, PlayerStatusBurstRequest, PlayerStatusKind,
    emit_boneyard_player_damage_sound, emit_player_status_burst, native_wizard_ouch_cooldown_ready,
    player_sound_event,
};
use crate::boneyard::BoneyardWorld;
use crate::boneyard::encounter::SolomonPhase;
use crate::combat::{PlayerCombatState, PlayerLifeState, player_poison_health_damage};
use crate::crow_blindness::native_crow_blindness;
use crate::enemies::model::{
    EnemyEvent, EnemyEventKind, EnemyPlayerDamage, EnemyPlayerDamageSource, EnemySound, EnemyTarget,
};
use crate::enemies::targeting::boneyard_enemy_collision_radius;
use crate::enemies::webbed::apply_native_webbed;
use crate::float::fround;
use crate::harmful::{
    NATIVE_FLASH_RESPONSE_RADIUS, PlayerHarmfulContactContext, PlayerIncomingDamage,
    player_deflect_reflection_source_in_range, resolve_player_flash_response,
    resolve_player_harmful_contact,
};
use crate::heading::heading_degrees_from_vector;
use crate::hub::PLAYER_CHARACTER_RADIUS;
use crate::hub::interaction::BoastFailureProducer;
use crate::modding::extensions::{
    GameSimulationDamageFilterInput, GameSimulationDamageKind, GameSimulationExtensions,
    GameSimulationManaSource, finite_mod_mutation,
};
use crate::painter::WorldPainterOrder;
use crate::player::PlayerState;
use crate::player_rescue::native_rescue_damage;
use crate::profile::player_poison_duration_seconds;
use crate::secondary::NativePlayerFlashResponseInput;
use crate::secondary::golem::{NATIVE_GOLEM_REFLECT_DISTANCE_SQUARED, NativeGolemIncomingDamage};
use crate::secondary::{
    NativeSecondaryActorKind, NativeSecondaryPlayerDamagePresentation,
    NativeSecondarySimulationState,
};
use crate::spells::boneyard::boneyard_native_secondary_targets;
use crate::{HubWizard, Vec2};

/// Mutable participant components borrowed from the session's canonical rows.
pub struct PlayerContactPlayer<'a> {
    pub player_id: &'a str,
    pub player: &'a mut PlayerState,
    pub wizard: &'a mut HubWizard,
    pub lighting: &'a mut PlayerLightingState,
}

pub struct PlayerContactContext<'a> {
    pub enhanced_effects: bool,
    pub players: &'a mut [PlayerContactPlayer<'a>],
    pub world: &'a mut BoneyardWorld,
    pub secondary: &'a mut NativeSecondarySimulationState,
    pub tick: u64,
    pub painter_order: &'a mut WorldPainterOrder,
    pub screen_flashes: &'a mut crate::screen_flash::NativeScreenFlashState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReflectedEnemyDamage {
    pub actor_id: usize,
    pub amount: f64,
    pub player_id: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerContactResult {
    pub harden_chips: Vec<PendingPlayerHardenChip>,
    pub applied_player_damage: Vec<EnemyPlayerDamage>,
    pub reflected_enemy_damage: Vec<ReflectedEnemyDamage>,
    pub deflect_pitches_by_event_id: BTreeMap<usize, f64>,
    pub player_damage_sound_events: Vec<EnemyEvent>,
}

/// Applies one global packet stream. Grouping by participant changes random
/// draws and semantic event order, and would apply Golem packets more than once.
pub fn apply_player_contacts(
    context: PlayerContactContext<'_>,
    damage: &[EnemyPlayerDamage],
) -> PlayerContactResult {
    apply_player_contacts_with_extensions(context, damage, None)
        .expect("stock enemy contacts have finite native mutations")
}

pub fn apply_player_contacts_with_extensions(
    mut context: PlayerContactContext<'_>,
    damage: &[EnemyPlayerDamage],
    extensions: Option<&dyn GameSimulationExtensions>,
) -> Result<PlayerContactResult, String> {
    if context.world.tutorial.as_ref().is_some_and(|tutorial| tutorial.damage_protection) {
        return Ok(PlayerContactResult::default());
    }
    let world_key = format!("boneyard:{}", context.world.loaded.run_id);
    let mut result = PlayerContactResult::default();
    let mut flashes = context.screen_flashes.writer(context.tick);
    let mut admitted_statuses = BTreeMap::new();
    for damage in damage {
        if let Some(id) = damage.player_id.strip_prefix("golem:") {
            if let Ok(id) = id.parse::<u64>() {
                apply_golem_contact(&mut context, damage, id, &mut result);
            }
            continue;
        }
        let Some(participant) = context
            .players
            .iter_mut()
            .find(|participant| participant.player_id == damage.player_id)
        else {
            continue;
        };
        ContactStep {
            enhanced_effects: context.enhanced_effects,
            participant,
            world: context.world,
            secondary: context.secondary,
            tick: context.tick,
            painter_order: context.painter_order,
            result: &mut result,
            world_key: &world_key,
            admitted_statuses: &mut admitted_statuses,
            extensions,
            flashes: &mut flashes,
        }
        .apply_player_contact(damage.clone())?;
    }
    *context.screen_flashes = flashes.finish();
    Ok(result)
}

#[derive(Clone, Copy)]
struct AdmittedStatusKinds {
    cold: bool,
    poison: bool,
}

struct ContactStep<'a, 'player> {
    enhanced_effects: bool,
    participant: &'a mut PlayerContactPlayer<'player>,
    world: &'a mut BoneyardWorld,
    secondary: &'a mut NativeSecondarySimulationState,
    tick: u64,
    painter_order: &'a mut WorldPainterOrder,
    result: &'a mut PlayerContactResult,
    world_key: &'a str,
    admitted_statuses: &'a mut BTreeMap<String, AdmittedStatusKinds>,
    extensions: Option<&'a dyn GameSimulationExtensions>,
    flashes: &'a mut crate::screen_flash::NativeScreenFlashWriter,
}

fn apply_golem_contact(
    context: &mut PlayerContactContext<'_>,
    damage: &EnemyPlayerDamage,
    id: u64,
    result: &mut PlayerContactResult,
) {
    let Some(index) = context
        .secondary
        .actors
        .iter()
        .position(|actor| actor.id == id && actor.kind == NativeSecondaryActorKind::Golem)
    else {
        return;
    };
    let actor = &mut context.secondary.actors[index];
    if damage.tragic_circle
        && let Some(golem) = &mut actor.golem
    {
        golem.circle_slow_ticks = 20;
        return;
    }
    let position = actor.position;
    let source = player_contact_source(context.world, damage);
    let in_range = source.is_some_and(|source| {
        squared_distance(source.position, position) < NATIVE_GOLEM_REFLECT_DISTANCE_SQUARED
    });
    let received = context.secondary.apply_golem_damage(
        id,
        NativeGolemIncomingDamage {
            primary_damage: damage.physical_damage,
            reflectable_physical_source_in_range: damage.physical_damage > 0.0 && in_range,
            secondary_damage: damage.magic_damage,
        },
        context.tick,
        context.enhanced_effects,
        Some(context.painter_order),
    );
    if received.reflected_damage > 0.0
        && let (Some(player_id), Some(actor_id)) =
            (received.owner_id, source.and_then(|source| source.reflectable_actor_id))
    {
        result.reflected_enemy_damage.push(ReflectedEnemyDamage {
            actor_id,
            amount: received.reflected_damage,
            player_id,
        });
    }
}

impl ContactStep<'_, '_> {
    fn apply_player_contact(&mut self, mut damage: EnemyPlayerDamage) -> Result<(), String> {
        let before = self.participant.player.combat.clone();
        let position = self.participant.wizard.position;
        let derived = self.participant.player.derived_stats();
        let source = player_contact_source(self.world, &damage);
        if damage.webbed_strength.is_some() && self.shield_remaining() > 0.0 {
            damage.physical_damage = 25.0;
        }
        if damage.tragic_circle {
            if before.life_state != PlayerLifeState::Alive {
                return Ok(());
            }
            self.drain_mana(fround(derived.mana_recovery_per_tick * 18.0))?;
            self.participant.player.combat.circle_slow_ticks_remaining = 20;
            return Ok(());
        }
        let in_reflect_range = source.is_some_and(|source| {
            player_deflect_reflection_source_in_range(
                position,
                PLAYER_CHARACTER_RADIUS,
                source.position,
                source.collision_radius,
            )
        });
        let contact = resolve_player_harmful_contact(
            PlayerHarmfulContactContext {
                runtime: &self.participant.player.skill_runtime,
                derived: &derived,
                progression: &self.participant.player.progression,
                rescue_protection: &self.participant.player.combat.rescue_protection,
                reflection_source_in_range: in_reflect_range,
                position,
                enhanced_effects: self.enhanced_effects,
            },
            PlayerIncomingDamage {
                physical_damage: damage.physical_damage,
                magic_damage: damage.magic_damage,
            },
            &mut self.secondary.rng,
        );
        if let Some(chip) = contact.harden_chip {
            self.result.harden_chips.push(PendingPlayerHardenChip {
                chip,
                owner_id: self.participant.player_id.into(),
                position,
                world_key: self.world_key.into(),
            });
        }
        if let Some(pitch) = contact.deflect_pitch {
            self.result.deflect_pitches_by_event_id.insert(damage.event_id, pitch);
            if let Some(source) = source {
                self.participant.wizard.set_facing(heading_degrees_from_vector(
                    source.position.x - position.x,
                    source.position.y - position.y,
                ));
                if contact.reflected_damage > 0.0
                    && let Some(actor_id) = source.reflectable_actor_id
                {
                    self.result.reflected_enemy_damage.push(ReflectedEnemyDamage {
                        actor_id,
                        amount: contact.reflected_damage,
                        player_id: self.participant.player_id.into(),
                    });
                }
            }
            self.apply_crow_blindness(&damage);
            return Ok(());
        }
        let shield_active = self.shield_remaining() > 0.0;
        let first_shield_event = self.secondary.next_event_id;
        let intercepted = self.secondary.apply_player_damage(
            self.participant.player_id,
            contact.shield_damage,
            self.tick,
            position,
            self.world_key,
            NativeSecondaryPlayerDamagePresentation {
                physical: damage.physical_damage > 0.0 && damage.magic_damage <= 0.0,
                enhanced_effects: self.enhanced_effects,
            },
        );
        self.secondary.write_screen_flashes_since(first_shield_event, self.flashes);
        let physical =
            if intercepted.absorbed_damage > 0.0 { 0.0 } else { contact.physical_damage };
        let magic = if intercepted.absorbed_damage > 0.0 { 0.0 } else { contact.magic_damage };
        let mut filtered_health_damage = 0.0;
        for (amount, kind) in [
            (physical, GameSimulationDamageKind::Physical),
            (magic, GameSimulationDamageKind::Magic),
        ] {
            if amount != 0.0 {
                filtered_health_damage += self
                    .filter_damage(amount, kind, damage.actor_id, "filtered enemy damage")?
                    .max(0.0);
            }
        }
        let poison_contact_damage =
            if self.participant.player.progression.poison_immunity_ticks_remaining > 0 {
                0.0
            } else {
                native_rescue_damage(
                    fround(
                        damage.poison_contact_damage.unwrap_or(0.0)
                            * derived.poison_damage_factor
                            * derived.incoming_damage_factor,
                    ),
                    &self.participant.player.combat.rescue_protection,
                )
            };
        let filtered_poison_damage = if poison_contact_damage > 0.0 {
            self.filter_damage(
                poison_contact_damage,
                GameSimulationDamageKind::Poison,
                damage.actor_id,
                "filtered enemy poison damage",
            )?
        } else {
            poison_contact_damage
        };
        let filtered_poison_damage =
            finite_mod_mutation(filtered_poison_damage, "filtered enemy poison damage")?.max(0.0);
        let capped_poison_damage =
            player_poison_health_damage(before.current_health, filtered_poison_damage);
        let health_damage = finite_mod_mutation(filtered_health_damage, "filtered enemy damage")?
            .max(0.0)
            + capped_poison_damage;
        let rejected = physical + magic + poison_contact_damage > 0.0
            && filtered_health_damage <= 0.0
            && filtered_poison_damage == 0.0;
        let stoneskin = self
            .secondary
            .players
            .get(self.participant.player_id)
            .is_some_and(|player| player.stoneskin_ticks_remaining > 0);
        if !rejected
            && intercepted.absorbed_damage == 0.0
            && !stoneskin
            && let Some(fraction) = damage.mana_damage_maximum_fraction
        {
            self.drain_mana(fround(before.maximum_mana * fraction))?;
        }
        if !rejected && (shield_active || !stoneskin || poison_contact_damage > 0.0) {
            if self.participant.player.progression.poison_immunity_ticks_remaining > 0 {
                damage.poison_damage = 0.0;
            }
            damage.poison_duration =
                player_poison_duration_seconds(&derived, damage.poison_duration);
            self.result.applied_player_damage.push(damage.clone());
            // Native Shield rejects Webbed using membership before this hit,
            // including a shield that the same contact just broke.
            if !shield_active {
                self.apply_web_contact(&damage);
            }
            let admitted = self
                .admitted_statuses
                .entry(self.participant.player_id.into())
                .or_insert(AdmittedStatusKinds {
                    cold: before.cold_slow_ticks_remaining > 0,
                    poison: before.poison_ticks_remaining > 0,
                });
            for status in [PlayerStatusKind::Cold, PlayerStatusKind::Poison] {
                let admitted = match status {
                    PlayerStatusKind::Cold => &mut admitted.cold,
                    PlayerStatusKind::Poison => &mut admitted.poison,
                };
                let active = match status {
                    PlayerStatusKind::Cold => damage.cold_slow_ticks > 0,
                    PlayerStatusKind::Poison => damage.poison_damage > 0.0,
                };
                if *admitted || !active {
                    continue;
                }
                *admitted = true;
                let event = emit_player_status_burst(
                    &mut self.world.enemies,
                    PlayerStatusBurstRequest {
                        actor_id: damage.actor_id,
                        player_id: self.participant.player_id,
                        position,
                        status,
                        tick: self.tick,
                    },
                    &mut self.secondary.rng,
                );
                self.result.player_damage_sound_events.push(event);
            }
        }
        let received = self.participant.player.damage_with_result(
            health_damage,
            self.tick,
            true,
            !damage.suppress_hit_response,
            damage.hit_strength.unwrap_or(1.0),
            self.participant.lighting.blindness_ticks_remaining,
        );
        self.result.player_damage_sound_events.extend(
            super::player_feedback::materialize_player_rescue_feedback(
                self.world,
                self.secondary,
                self.painter_order,
                self.flashes,
                super::player_feedback::PlayerRescueFeedbackInput {
                    player_id: self.participant.player_id,
                    position,
                    light_registration: self.participant.lighting.light_registration,
                    tick: self.tick,
                    world_key: self.world_key,
                    received,
                },
            ),
        );
        self.play_hurt_response(&damage, before, capped_poison_damage, filtered_health_damage);
        if !rejected
            && self.participant.player.combat.life_state == PlayerLifeState::Alive
            && capped_poison_damage == 0.0
            && !damage.suppress_flash
            && (intercepted.absorbed_damage == 0.0 || poison_contact_damage > 0.0)
            && (!stoneskin || poison_contact_damage > 0.0)
            && let Some(response) = resolve_player_flash_response(&derived, &mut self.secondary.rng)
        {
            let ids = boneyard_native_secondary_targets(
                &self.world.enemies,
                position,
                NATIVE_FLASH_RESPONSE_RADIUS,
            )
            .iter()
            .map(|target| target.id)
            .collect::<Vec<_>>();
            let first_flash_event = self.secondary.next_event_id;
            self.secondary.materialize_player_flash_response(NativePlayerFlashResponseInput {
                owner_id: self.participant.player_id,
                position,
                response: &response,
                target_ids: &ids,
                tick: self.tick,
                world_key: self.world_key,
            });
            self.secondary.write_screen_flashes_since(first_flash_event, self.flashes);
        }
        self.apply_crow_blindness(&damage);
        Ok(())
    }

    fn shield_remaining(&self) -> f64 {
        self.secondary
            .players
            .get(self.participant.player_id)
            .map_or(0.0, |player| player.magic_shield_absorb)
    }

    fn filter_damage(
        &self,
        amount: f64,
        damage_kind: GameSimulationDamageKind,
        actor_id: usize,
        field: &str,
    ) -> Result<f64, String> {
        let Some(extensions) = self.extensions else { return Ok(amount) };
        finite_mod_mutation(
            extensions.filter_damage(
                GameSimulationDamageFilterInput {
                    amount,
                    damage_kind,
                    source_actor_id: Some(actor_id),
                    target_player_id: self.participant.player_id,
                    tick: self.tick,
                },
                &self.participant.player.economy.equipment,
            )?,
            field,
        )
    }

    fn drain_mana(&mut self, amount: f64) -> Result<(), String> {
        let player = &mut self.participant.player;
        let delta = super::extensions::filter_mana_delta(
            player,
            self.participant.player_id,
            -amount,
            GameSimulationManaSource::EnemyContact,
            self.tick,
            self.extensions,
            "filtered enemy mana damage",
        )?;
        if delta >= 0.0 {
            player.combat.set_mana(
                fround(player.combat.current_mana + delta).min(player.combat.maximum_mana),
            );
            return Ok(());
        }
        if -delta > player.combat.current_mana {
            let before = player.economy.npc.boast.clone();
            let rules = before
                .selected
                .as_ref()
                .and_then(|selection| super::extensions::boast_rules(selection, self.extensions));
            player.economy.npc.boast.fail_with_rules(BoastFailureProducer::ManaUnderflow, rules);
            if player.economy.npc.boast != before {
                player.economy.revision += 1;
            }
        }
        if !player.try_debit_mana(-delta).accepted {
            player.combat.set_mana(0.0);
        }
        Ok(())
    }

    fn apply_crow_blindness(&mut self, damage: &EnemyPlayerDamage) {
        let Some(chance) = damage.crow_blind_chance_percent else {
            return;
        };
        let duration = native_crow_blindness(
            self.participant.player.combat.current_health,
            self.shield_remaining(),
            chance,
            &mut self.secondary.rng,
        );
        if duration == 0.0 {
            return;
        }
        self.participant.lighting.blindness_ticks_remaining = duration;
        let mut event = player_sound_event(
            &mut self.world.enemies,
            damage.actor_id,
            self.participant.player_id,
            self.participant.wizard.position,
            self.tick,
            EnemyEventKind::PlayerStatusSound,
        );
        event.gain_scale = Some(1.0);
        event.pitch = Some(1.0);
        event.sound = Some(EnemySound::Blind);
        self.result.player_damage_sound_events.push(event);
    }

    fn apply_web_contact(&mut self, damage: &EnemyPlayerDamage) {
        let Some(strength) = damage.webbed_strength else {
            return;
        };
        let prior = self.world.enemies.webbed_players.get(self.participant.player_id).copied();
        let web = apply_native_webbed(prior, strength);
        self.world.enemies.webbed_players.insert(self.participant.player_id.into(), web);
        if web.severity == 3.0 && prior.map_or(0.0, |prior| prior.severity) < 3.0 {
            let position = self.participant.wizard.position;
            let target = EnemyTarget {
                id: self.participant.player_id.into(),
                summoned: false,
                alive: true,
                connected: true,
                eligible: true,
                collision_radius: PLAYER_CHARACTER_RADIUS,
                heading_deg: Some(self.participant.wizard.heading_degrees),
                position,
                velocity_per_tick: Vec2::ZERO,
            };
            // The source keeps the birth/counter changes but discards spawn-event publication.
            drop(self.world.enemies.add_native_cocoon(
                self.participant.player_id,
                position,
                &[target],
                self.tick,
                Some(self.painter_order),
            ));
        }
    }

    fn play_hurt_response(
        &mut self,
        damage: &EnemyPlayerDamage,
        before: PlayerCombatState,
        capped_poison_damage: f64,
        filtered_health_damage: f64,
    ) {
        let world = &mut self.world;
        let dialogue_idle = world.enemies.boss_narration.current.is_none()
            && world.enemies.boss_narration.pending.is_empty()
            && world
                .encounter
                .as_ref()
                .is_none_or(|encounter| encounter.phase != SolomonPhase::Speaking)
            && world.tutorial.as_ref().is_none_or(|tutorial| {
                tutorial.narration.current.is_none() && tutorial.narration.pending.is_empty()
            });
        let after = &self.participant.player.combat;
        if after.current_health < before.current_health
            && after.life_state == PlayerLifeState::Alive
            && capped_poison_damage == 0.0
            && filtered_health_damage > 0.0
            && dialogue_idle
            && native_wizard_ouch_cooldown_ready(self.tick, world.player_ouch_deadline_tick)
        {
            let result = emit_boneyard_player_damage_sound(
                &mut world.enemies,
                PlayerDamageSoundRequest {
                    actor_id: damage.actor_id,
                    current_health: after.current_health,
                    player_id: self.participant.player_id,
                    position: self.participant.wizard.position,
                    tick: self.tick,
                },
                &mut self.secondary.rng,
            );
            world.player_ouch_deadline_tick = result.deadline_tick;
            self.result.player_damage_sound_events.push(result.event);
        }
    }
}

fn player_contact_source(
    world: &BoneyardWorld,
    damage: &EnemyPlayerDamage,
) -> Option<EnemyPlayerDamageSource> {
    if let Some(source) = damage.source {
        return source;
    }
    if let Some(actor) = world.enemies.actors.iter().find(|actor| actor.id == damage.actor_id) {
        return Some(EnemyPlayerDamageSource {
            position: actor.position,
            collision_radius: boneyard_enemy_collision_radius(actor),
            reflectable_actor_id: Some(actor.id),
        });
    }
    world.enemies.maggots.iter().find(|actor| actor.id == damage.actor_id).map(|actor| {
        EnemyPlayerDamageSource {
            position: actor.position,
            collision_radius: actor.collision_radius,
            reflectable_actor_id: Some(actor.id),
        }
    })
}

fn squared_distance(left: Vec2, right: Vec2) -> f64 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    dx * dx + dy * dy
}
