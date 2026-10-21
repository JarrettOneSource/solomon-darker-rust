//! Source EtherDrain world projections and accepted forces from
//! `core-server/native-ether-drain-world.ts` and `boneyard-world-placement.ts`.

use super::*;
use crate::Vec2;
use crate::boneyard::BoneyardWorld;
use crate::combat::PlayerLifeState;
use crate::enemies::model::{EnemyDeathEffectKind, EnemyLifeState};
use crate::float::fround;
use crate::loot::NativeLootDropKind;
use crate::secondary::ether_drain::*;
use crate::secondary::{NativeSecondaryActorKind as Kind, NativeSecondarySimulationState};
use crate::spells::boneyard::boneyard_native_secondary_targets;
use crate::spells::model::PrimarySpellTransient;
use crate::spells::weld::{NativeWeldHailChildActorState, NativeWeldWorldActor};

/// Source callbacks see the accepted pre-secondary store. This copied query
/// projection lets the single mutable secondary owner advance without aliasing it.
pub(super) struct SessionEtherDrainSnapshot {
    pub targets: Vec<NativeEtherDrainTarget>,
    pub animations: Vec<NativeEtherDrainWorldAnimationTarget>,
}
impl SessionEtherDrainSnapshot {
    pub fn capture(session: &GameSession) -> Option<Self> {
        let world = session.boneyard.as_ref()?;
        let key = format!("boneyard:{}", world.loaded.run_id);
        let mut targets = Vec::new();
        for target in boneyard_native_secondary_targets(&world.enemies, Vec2::ZERO, f64::MAX) {
            targets.push(NativeEtherDrainTarget {
                activation_delay_ticks: 0,
                force_factor: native_ether_drain_enemy_force_factor(&target.family),
                native_flags: target.native_flags.unwrap_or(2),
                position: target.position,
                reference: NativeEtherDrainTargetRef::Enemy {
                    id: target.id as u64,
                    registration_ordinal: target.light_registration.registration_ordinal,
                },
            });
        }
        for maggot in &world.enemies.maggots {
            if maggot.life_state == EnemyLifeState::Alive && !maggot.combat_active {
                targets.push(NativeEtherDrainTarget {
                    activation_delay_ticks: 0,
                    force_factor: 1.0,
                    native_flags: 2,
                    position: maggot.position,
                    reference: NativeEtherDrainTargetRef::Enemy {
                        id: maggot.id as u64,
                        registration_ordinal: maggot.registration_ordinal,
                    },
                });
            }
        }
        for id in &session.participant_order {
            let participant = &session.participants[id];
            if participant.player.combat.life_state == PlayerLifeState::Spectating
                || participant.player.combat.corpse_consumed
            {
                continue;
            }
            targets.push(NativeEtherDrainTarget {
                activation_delay_ticks: 0,
                force_factor: 1.0,
                native_flags: 0x801,
                position: participant.wizard.position,
                reference: NativeEtherDrainTargetRef::Player { id: id.clone() },
            });
        }
        for actor in &world.loot.actors {
            if !matches!(actor.kind, NativeLootDropKind::Gold | NativeLootDropKind::Sack) {
                continue;
            }
            targets.push(NativeEtherDrainTarget {
                activation_delay_ticks: actor.activation_delay_ticks,
                force_factor: 1.0,
                native_flags: 0x400,
                position: actor.position,
                reference: NativeEtherDrainTargetRef::Loot {
                    id: actor.id as u64,
                    registration_ordinal: actor.painter_registration.registration_ordinal,
                },
            });
        }
        for actor in &session.secondary.actors {
            if actor.kind == Kind::Golem && actor.world_key == key && actor.golem.is_some() {
                targets.push(NativeEtherDrainTarget {
                    activation_delay_ticks: 0,
                    force_factor: 0.25,
                    native_flags: 0x800,
                    position: actor.position,
                    reference: NativeEtherDrainTargetRef::Golem { id: actor.id },
                });
            }
        }
        Some(Self {
            targets,
            animations: capture_world_animations(
                world,
                &session.spells.transients,
                &session.secondary,
            ),
        })
    }
}

fn capture_world_animations(
    world: &BoneyardWorld,
    primary: &[PrimarySpellTransient],
    secondary: &NativeSecondarySimulationState,
) -> Vec<NativeEtherDrainWorldAnimationTarget> {
    let mut targets = Vec::new();
    let mut push = |bounce_velocity, position, reference| {
        targets.push(NativeEtherDrainWorldAnimationTarget { bounce_velocity, position, reference });
    };
    for actor in &world.death_weapons {
        push(
            actor.motion.bounce_velocity,
            actor.motion.position,
            NativeEtherDrainWorldAnimationRef::DeathWeapon { id: actor.id },
        );
    }
    for effect in &world.enemies.death_effects {
        if matches!(
            effect.kind,
            EnemyDeathEffectKind::BoulderBit
                | EnemyDeathEffectKind::Bouncer
                | EnemyDeathEffectKind::SmokyBouncer
                | EnemyDeathEffectKind::BlackSmokyBouncer
        ) {
            push(
                effect.bounce_velocity,
                effect.position,
                NativeEtherDrainWorldAnimationRef::EnemyDeathEffect { id: effect.id as u64 },
            );
        }
    }
    let key = format!("boneyard:{}", world.loaded.run_id);
    for effect in primary {
        if effect.common().world_key != key {
            continue;
        }
        let reference =
            NativeEtherDrainWorldAnimationRef::PrimaryTransient { id: effect.common().id };
        match effect {
            PrimarySpellTransient::WaterHail(actor) => {
                push(actor.saved_bounce_velocity, actor.position, reference)
            }
            PrimarySpellTransient::Weld(NativeWeldWorldActor::HailChild(
                NativeWeldHailChildActorState::TerrainBouncer(actor),
            )) => push(actor.bounce_velocity, actor.position, reference),
            PrimarySpellTransient::Weld(NativeWeldWorldActor::BoulderDebris(actor)) => {
                push(actor.debris.bounce_velocity, actor.position, reference)
            }
            _ => {}
        }
    }
    for actor in &secondary.actors {
        if actor.world_key != key {
            continue;
        }
        if actor.kind == Kind::GolemDeath
            && let Some(death) = &actor.golem_death
        {
            for (index, fragment) in death.fragments.iter().enumerate() {
                if let Some(fragment) = fragment {
                    push(
                        fragment.motion.bounce_velocity,
                        fragment.motion.position,
                        NativeEtherDrainWorldAnimationRef::GolemFragment { id: actor.id, index },
                    );
                }
            }
        }
        let bounce = match actor.kind {
            Kind::EarthquakeDebris => actor.quantity,
            Kind::GolemAssemblyDebris => actor.endpoint.x,
            Kind::CometDebris | Kind::StoneskinChip => actor.endpoint.y,
            _ => continue,
        };
        push(
            bounce,
            actor.position,
            NativeEtherDrainWorldAnimationRef::SecondaryActor { id: actor.id },
        );
    }
    targets
}

impl GameSession {
    pub(super) fn apply_ether_drain_world_animation_forces(
        &mut self,
        contacts: &[NativeEtherDrainWorldAnimationContact],
    ) {
        let Some(world) = &mut self.boneyard else { return };
        for contact in contacts {
            let shift = |point: Vec2| {
                Vec2::new(fround(point.x + contact.delta.x), fround(point.y + contact.delta.y))
            };
            match contact.reference {
                NativeEtherDrainWorldAnimationRef::DeathWeapon { id } => {
                    world.death_weapons.retain_mut(|actor| {
                        if actor.id != id {
                            return true;
                        }
                        if contact.consume {
                            return false;
                        }
                        actor.motion.position = shift(actor.motion.position);
                        true
                    })
                }
                NativeEtherDrainWorldAnimationRef::EnemyDeathEffect { id } => {
                    world.enemies.death_effects.retain_mut(|effect| {
                        if effect.id as u64 != id {
                            return true;
                        }
                        if contact.consume {
                            return false;
                        }
                        effect.position = shift(effect.position);
                        true
                    })
                }
                NativeEtherDrainWorldAnimationRef::GolemFragment { id, index } => {
                    if let Some(death) = self
                        .secondary
                        .actors
                        .iter_mut()
                        .find(|actor| actor.id == id && actor.kind == Kind::GolemDeath)
                        .and_then(|actor| actor.golem_death.as_mut())
                        && let Some(fragment) = death.fragments.get_mut(index)
                    {
                        if contact.consume {
                            *fragment = None
                        } else if let Some(fragment) = fragment {
                            fragment.motion.position = shift(fragment.motion.position);
                        }
                    }
                }
                NativeEtherDrainWorldAnimationRef::PrimaryTransient { id } => {
                    self.spells.transients.retain_mut(|effect| {
                        if effect.common().id != id {
                            return true;
                        }
                        match effect {
                            PrimarySpellTransient::WaterHail(actor) => {
                                if contact.consume {
                                    return false;
                                }
                                actor.position = shift(actor.position);
                            }
                            PrimarySpellTransient::Weld(NativeWeldWorldActor::HailChild(
                                NativeWeldHailChildActorState::TerrainBouncer(actor),
                            )) => {
                                if contact.consume {
                                    return false;
                                }
                                actor.position = shift(actor.position);
                            }
                            PrimarySpellTransient::Weld(NativeWeldWorldActor::BoulderDebris(
                                actor,
                            )) => {
                                if contact.consume {
                                    return false;
                                }
                                actor.position = shift(actor.position);
                                actor.debris.common.position = actor.position;
                            }
                            _ => {}
                        }
                        true
                    })
                }
                NativeEtherDrainWorldAnimationRef::SecondaryActor { id } => {
                    self.secondary.actors.retain_mut(|actor| {
                        if actor.id != id {
                            return true;
                        }
                        if contact.consume {
                            return false;
                        }
                        actor.position = shift(actor.position);
                        true
                    })
                }
            }
        }
    }

    pub(super) fn consume_ether_drain_player_corpses(&mut self, tick: u64) {
        let Some(world) = &self.boneyard else { return };
        let key = format!("boneyard:{}", world.loaded.run_id);
        let fields: Vec<_> = self
            .secondary
            .actors
            .iter()
            .filter(|actor| actor.kind == Kind::EtherDrain && actor.world_key == key)
            .map(|actor| (actor.id, actor.position))
            .collect();
        for id in &self.participant_order {
            let participant = self.participants.get_mut(id).unwrap();
            let combat = &mut participant.player.combat;
            if combat.life_state != PlayerLifeState::Dying
                || combat.corpse_consumed
                || combat.death_tick < NATIVE_ETHER_DRAIN_CORPSE_TIMER_TICKS
            {
                continue;
            }
            for (field_id, position) in &fields {
                let dx = fround(position.x - participant.wizard.position.x);
                let dy = fround(position.y - participant.wizard.position.y);
                if fround(dx * dx + dy * dy) < NATIVE_ETHER_DRAIN_CONSUMPTION_DISTANCE_SQUARED {
                    self.secondary.pulse_ether_drain(*field_id, 2.0, tick, false);
                    combat.corpse_consumed = true;
                }
            }
        }
    }
}

impl GameSession {
    pub(super) fn apply_ether_drain_forces_and_player_contacts(
        &mut self,
        contacts: &[NativeEtherDrainContact],
        tick: u64,
        extensions: Option<&dyn GameSimulationExtensions>,
    ) -> Result<(), String> {
        if self.boneyard.is_none() || contacts.is_empty() {
            return Ok(());
        }
        let bodies = super::secondary::boneyard_player_bodies(
            &self.participants,
            &self.participant_order,
            &self.run,
        );
        let moved = self.boneyard.as_mut().unwrap().apply_ether_drain_forces(
            contacts,
            &bodies,
            &mut self.secondary,
        );
        for (id, position) in moved.player_positions {
            self.participants.get_mut(&id).unwrap().wizard.position = position;
        }
        for id in moved.consumed_field_actor_ids {
            self.secondary.pulse_ether_drain(id, 1.0, tick, false);
        }
        let mut damage = Vec::new();
        for contact in contacts {
            if contact.amount <= 0.0 {
                continue;
            }
            let player_id = match &contact.target {
                NativeEtherDrainTargetRef::Player { id } => id.clone(),
                NativeEtherDrainTargetRef::Golem { id } => format!("golem:{id}"),
                _ => continue,
            };
            let Some(field) =
                self.secondary.actors.iter().find(|actor| actor.id == contact.source_actor_id)
            else {
                continue;
            };
            let mut request = crate::enemies::model::EnemyPlayerDamage::new(0, 0, player_id);
            request.hit_strength = Some(contact.hit_strength);
            request.magic_damage = contact.amount;
            request.suppress_hit_response = contact.damage_flags & 8 != 0;
            request.source = Some(Some(crate::enemies::model::EnemyPlayerDamageSource {
                position: field.position,
                collision_radius: field.radius,
                reflectable_actor_id: None,
            }));
            damage.push(request);
        }
        let mut players: Vec<_> =
            ordered_participants_mut(&mut self.participants, &self.participant_order)
                .into_iter()
                .map(|participant| super::player_contacts::PlayerContactPlayer {
                    player_id: &participant.id,
                    player: &mut participant.player,
                    wizard: &mut participant.wizard,
                    lighting: &mut participant.lighting,
                })
                .collect();
        // This early Source phase accepts the mutated stores, but does not
        // forward the ordinary enemy-contact feedback outboxes.
        super::player_contacts::apply_player_contacts_with_extensions(
            super::player_contacts::PlayerContactContext {
                enhanced_effects: self.enhanced_effects,
                players: &mut players,
                world: self.boneyard.as_mut().unwrap(),
                secondary: &mut self.secondary,
                tick,
                painter_order: &mut self.painter_order,
                screen_flashes: &mut self.screen_flashes,
            },
            &damage,
            extensions,
        )?;
        Ok(())
    }
}

impl GameSession {
    pub(super) fn apply_ether_drain_enemy_contacts(
        &mut self,
        contacts: &[NativeEtherDrainContact],
        tick: u64,
    ) {
        let Some(world) = &self.boneyard else { return };
        let key = format!("boneyard:{}", world.loaded.run_id);
        let fields: Vec<_> = self
            .secondary
            .actors
            .iter()
            .filter(|actor| actor.kind == Kind::EtherDrain && actor.world_key == key)
            .map(|actor| (actor.id, actor.position))
            .collect();
        let mut captures = Vec::new();
        for contact in contacts {
            let NativeEtherDrainTargetRef::Enemy { id, .. } = contact.target else { continue };
            if contact.amount <= 0.0 {
                continue;
            }
            let enemies = &self.boneyard.as_ref().unwrap().enemies;
            let victim = enemies.actors.iter().find(|actor| actor.id as u64 == id).cloned();
            let position = victim.as_ref().map(|actor| actor.position).or_else(|| {
                enemies
                    .maggots
                    .iter()
                    .find(|actor| actor.id as u64 == id)
                    .map(|actor| actor.position)
            });
            let capturable = native_ether_drain_captures_family(
                victim.as_ref().map_or("MAGGOT", |actor| actor.config.enemy_token.id()),
            );
            let field = position
                .filter(|_| capturable)
                .and_then(|position| {
                    fields.iter().find(|(_, field)| {
                        let dx = field.x - position.x;
                        let dy = field.y - position.y;
                        dx * dx + dy * dy < 1600.0
                    })
                })
                .map(|(id, _)| *id);
            let factor = victim.as_ref().map_or(1.0, |actor| {
                self.participants.get(&contact.owner_id).map_or(1.0, |owner| {
                    crate::profile::native_hagatha_boss_damage_factor(
                        &owner.player.economy.owned_perk_selectors,
                        u32::from(actor.config.native_type_id),
                    )
                })
            });
            let result = self.damage_enemy(crate::enemies::model::EnemyDamageRequest {
                enhanced_effects: Some(self.enhanced_effects),
                actor_id: id as usize,
                amount: contact.amount * factor,
                ether_drain_capture: field.is_some(),
                has_magic_damage: true,
                hit_strength: contact.hit_strength,
                magic: true,
                source_player_id: Some(contact.owner_id.clone()),
                suppress_hit_reaction: contact.damage_flags & 8 != 0,
                suppress_hurt_sound: contact.damage_flags & 8 != 0,
                tick,
            });
            if result.killed
                && let Some(field) = field
            {
                let image = victim.as_ref().and_then(|actor| {
                    crate::enemies::actor_presentation::native_ether_drain_captured_image(
                        actor, tick,
                    )
                });
                captures.push((field, image));
            }
        }
        // Source resolves the complete ordered damage lane before captures
        // flush their private managers and consume secondary random draws.
        for (field, image) in captures {
            self.secondary.capture_ether_drain_image(field, image, tick);
        }
    }
}
