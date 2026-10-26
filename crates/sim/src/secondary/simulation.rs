//! Secondary actor ownership, burn replacement and shield/Golem damage contracts.

use crate::secondary::{SecondaryActorRead, SecondaryActorWrite};

use super::golem::{
    NativeGolemIncomingDamage, NativeGolemKernelActor, damage_native_secondary_golem,
};
use super::{
    NativeSecondaryActorKind, NativeSecondaryActorState, NativeSecondaryAudioCue,
    NativeSecondaryEventKind, NativeSecondaryEventState, NativeSecondaryPlayerState,
    NativeSecondaryScreenFlashState, NativeSecondaryTarget, NativeSecondaryTargetEffectState,
};
use crate::float::fround;
use crate::{NativeRng, Vec2};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeSecondarySimulationState {
    pub camera_displacements: Vec<NativeSecondaryCameraDisplacement>,
    pub stoneskin_warp: Option<Vec<f64>>,
    pub actors: super::SecondaryActors,
    /// Birth commands collected while existing entities advance. They become live
    /// together after that phase, so children cannot advance on their birth tick.
    #[cfg_attr(feature = "persistence", serde(skip))]
    pub(super) deferred_births: Option<Vec<NativeSecondaryActorState>>,
    pub events: Vec<NativeSecondaryEventState>,
    pub firewalker_geometry_sequence: u32,
    pub next_actor_id: u64,
    pub next_event_id: u64,
    pub players: crate::KeyedComponents<NativeSecondaryPlayerState>,
    pub rng: NativeRng,
    pub target_effects: crate::ComponentEntities<NativeSecondaryTargetEffectState>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeSecondaryCameraDisplacement {
    pub displacement: Vec2,
    pub tick: u64,
    pub world_key: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeSecondaryFireBurnInput {
    pub damage: f64,
    pub owner_id: String,
    pub rank: u32,
    pub skill_id: usize,
    pub target: NativeSecondaryTarget,
    pub world_key: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeSecondaryEtherBurnInput {
    pub owner_id: String,
    pub rank: u32,
    pub target: NativeSecondaryTarget,
    pub world_key: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSecondaryPlayerDamageResult {
    pub absorbed_damage: f64,
    pub health_damage: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeSecondaryPlayerDamagePresentation {
    pub physical: bool,
    pub enhanced_effects: bool,
}

impl Default for NativeSecondaryPlayerDamagePresentation {
    fn default() -> Self {
        Self { physical: false, enhanced_effects: true }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeSecondaryGolemDamageResult {
    pub ignored: bool,
    pub killed: bool,
    pub owner_id: Option<String>,
    pub reflected_damage: f64,
}

impl NativeSecondarySimulationState {
    #[must_use]
    pub fn new(seed: i64) -> Self {
        Self {
            camera_displacements: Vec::new(),
            stoneskin_warp: None,
            actors: super::SecondaryActors::default(),
            deferred_births: None,
            events: Vec::new(),
            firewalker_geometry_sequence: 0,
            next_actor_id: 1,
            next_event_id: 1,
            players: crate::KeyedComponents::default(),
            rng: NativeRng::new(seed),
            target_effects: crate::ComponentEntities::default(),
        }
    }

    /// Retain the strongest proposal per world. A replacement moves to the tail;
    /// equal magnitudes keep the earlier proposal, as in the Source Region queue.
    pub fn record_camera_displacement(&mut self, world_key: &str, displacement: Vec2, tick: u64) {
        record_camera_displacement(&mut self.camera_displacements, world_key, displacement, tick);
    }

    pub fn emit_event(&mut self, mut event: NativeSecondaryEventState) {
        event.event_id = self.next_event_id;
        self.next_event_id += 1;
        self.events.push(event);
    }

    /// `emitNativeSecondaryEvent` writes these flashes unconditionally. Call at
    /// the producer boundary, before a different family's writer can interleave.
    pub fn write_screen_flashes_since(
        &self,
        first_event_id: u64,
        writer: &mut crate::screen_flash::NativeScreenFlashWriter,
    ) {
        for event in &self.events {
            if event.event_id < first_event_id {
                continue;
            }
            if let Some(flash) = event.screen_flash {
                writer.write(crate::screen_flash::NativeScreenFlashInput {
                    tick: event.tick,
                    world_key: event.world_key.clone(),
                    position: event.position,
                    flash,
                    only_if_clear: false,
                });
            }
        }
    }

    pub fn spawn_actor(&mut self, mut actor: NativeSecondaryActorState) -> u64 {
        actor.id = self.next_actor_id;
        self.next_actor_id += 1;
        let id = actor.id;
        if let Some(births) = &mut self.deferred_births {
            births.push(actor);
        } else {
            self.actors.push(actor);
        }
        id
    }

    /// Runs the complete admitted constructor. The world tick advances clocks before
    /// calling this path and enrolls painters once all players have cast for that tick.
    pub fn cast(
        &mut self,
        owner: &str,
        ability: super::NativeSecondaryAbilityId,
        context: &super::NativeSecondaryTickContext<'_>,
        world: &impl super::NativeSecondaryWorld,
    ) -> super::NativeSecondaryTickResult {
        let player = self.players.get(owner).cloned().unwrap_or_default();
        let result = super::casting::cast_ability(self, player, owner, ability, context, world);
        self.players.insert(owner.to_owned(), result.player);
        let mut output = super::NativeSecondaryTickResult {
            target_heading_changes: result.target_heading_changes,
            dampened_caster_target_ids: result.dampened_caster_target_ids,
            dispelled_shield_target_ids: result.dispelled_shield_target_ids,
            removed_projectile_ids: result.removed_projectile_ids,
            ..super::NativeSecondaryTickResult::default()
        };
        for ids in [
            &mut output.dampened_caster_target_ids,
            &mut output.dispelled_shield_target_ids,
            &mut output.removed_projectile_ids,
        ] {
            ids.sort_unstable();
            ids.dedup();
        }
        if result.mana_recovered > 0.0 {
            output.mana_recovered.insert(owner.to_owned(), result.mana_recovered);
        }
        if result.mana_spent > 0.0 {
            output.mana_spent.insert(owner.to_owned(), result.mana_spent);
        }
        if result.mana_underflow {
            output.mana_underflow_player_ids.push(owner.to_owned());
        }
        if let Some(position) = result.relocated {
            output.relocated_players.insert(owner.to_owned(), position);
        }
        if let Some(heading) = result.facing_heading_degrees {
            output.facing_heading_degrees_by_player.insert(owner.to_owned(), heading);
        }
        output
    }

    /// `activateNativeSecondaryBeltSkill`: one immediate constructor transaction.
    /// Existing actors, target effects, cooldowns and cast actions do not advance.
    pub fn activate_belt_skill(
        &mut self,
        owner: &str,
        ability: super::NativeSecondaryAbilityId,
        slot: usize,
        context: &super::NativeSecondaryTickContext<'_>,
        world: &impl super::NativeSecondaryWorld,
        order: Option<&mut crate::painter::WorldPainterOrder>,
    ) -> super::NativeSecondaryTickResult {
        let authority = context.players.get(owner).expect("secondary authority retains its player");
        let mut player = self.players.get(owner).cloned().unwrap_or_default();
        player.reserved_mana = player.mana_reserve(authority.skill_book, authority.maximum_mana);
        let mut overloaded = false;
        if !authority.alive || player.reserved_mana > authority.maximum_mana {
            overloaded =
                authority.alive && (player.firewalker || player.mindstar || player.regenerate);
            player.clear_toggles();
            if overloaded {
                let mut event = super::program::cast_event(
                    owner,
                    player
                        .last_skill_id
                        .unwrap_or(super::NativeSecondaryAbilityId::Mindstar)
                        .skill_id(),
                    authority,
                    context.tick,
                    NativeSecondaryEventKind::Overload,
                    Some(NativeSecondaryAudioCue::Fizzle),
                );
                event.pitch = 1.0;
                self.emit_event(event);
            }
        }
        self.players.insert(owner.into(), player);
        let mut result = self.cast(owner, ability, context, world);
        let player = self.players.get_mut(owner).expect("the constructor retains its player");
        player.reserved_mana = player.mana_reserve(authority.skill_book, authority.maximum_mana);
        player.held_slot = Some(slot);
        if overloaded {
            result.overloaded_player_ids.push(owner.into());
        }
        if player.planewalker_ticks_remaining > 0 {
            result.primary_override_player_ids.push(owner.into());
        }
        if self.events.len() > 512 {
            self.events.drain(..self.events.len() - 512);
        }
        super::enroll_native_secondary_light_owners(&mut self.actors, order, |key, id| {
            world.target(key, id).map(|target| target.light_registration)
        });
        result
    }

    /// Last Word continues its common Mindblast presentation after the run is over.
    pub fn step_mindblast_presentation(&mut self, tick: u64) {
        self.camera_displacements.clear();
        let proposals = &mut self.camera_displacements;
        let rng = &mut self.rng;
        self.actors.retain_mut(|actor| {
            if !matches!(
                actor.kind,
                NativeSecondaryActorKind::MindblastBurst
                    | NativeSecondaryActorKind::MindblastShockwave
                    | NativeSecondaryActorKind::RescueShockwave
            ) {
                return true;
            }
            let before = super::motion::NativeExpandingWave {
                radius: actor.radius,
                growth: actor.quantity,
                scalar: actor.alpha,
                life: actor.phase,
                fade_threshold: actor.slow_factor,
            };
            super::program::advance_actor(actor);
            if actor.kind == NativeSecondaryActorKind::MindblastBurst {
                return actor.age_ticks < actor.lifetime_ticks;
            }
            let wave = super::motion::step_native_expanding_wave(before, rng);
            record_camera_displacement(proposals, &actor.world_key, wave.displacement, tick);
            if !wave.retain {
                return false;
            }
            actor.alpha = wave.scalar;
            actor.phase = wave.life;
            actor.radius = wave.radius;
            actor.velocity = Vec2::ZERO;
            actor.scale = fround(1.0 + actor.age_ticks as f64 * 0.08);
            true
        });
    }

    pub fn cooldown_rejuvenation(&mut self, player_id: &str) {
        if let Some(player) = self.players.get_mut(player_id) {
            player.cooldown_rejuvenation();
        }
    }

    pub fn reset_world(&mut self) {
        self.actors.clear();
        self.events.clear();
        self.target_effects.clear();
        for player in self.players.values_mut() {
            *player = NativeSecondaryPlayerState::default();
        }
    }

    pub fn remove_owner(&mut self, player_id: &str) {
        let owns_player = self.players.contains_key(player_id);
        let owns_actor = self.actors.iter().any(|actor| actor.owner_id == player_id);
        let owns_effect = self.target_effects.iter().any(|effect| {
            effect.frost_burn_owner_id.as_deref() == Some(player_id)
                || effect.electric_burn.as_ref().is_some_and(|burn| burn.owner_id == player_id)
                || effect.steamed.as_ref().is_some_and(|burn| burn.owner_id == player_id)
        });
        // The native no-op preserves unrelated entries, even an empty modifier awaiting
        // its next simulation tick. Filtering only belongs to an actual owner removal.
        if !owns_player && !owns_actor && !owns_effect {
            return;
        }
        self.players.remove(player_id);
        self.actors.retain(|actor| {
            actor.owner_id != player_id || actor.kind == NativeSecondaryActorKind::GolemDeath
        });
        for effect in self.target_effects.iter_mut() {
            if effect.electric_burn.as_ref().is_some_and(|burn| burn.owner_id == player_id) {
                effect.electric_burn = None;
            }
            if effect.steamed.as_ref().is_some_and(|burn| burn.owner_id == player_id) {
                effect.steamed = None;
            }
            if effect.frost_burn_owner_id.as_deref() == Some(player_id) {
                effect.frost_burn_damage_per_tick = 0.0;
                effect.frost_burn_owner_id = None;
                effect.frost_burn_skill_id = None;
                effect.frost_burn_source_actor_id = None;
                effect.frost_burn_ticks = 0;
            }
        }
        self.target_effects.retain(NativeSecondaryTargetEffectState::is_active);
    }

    pub fn apply_fire_burn(&mut self, input: NativeSecondaryFireBurnInput) {
        if input.damage.is_nan() || input.damage <= 0.0 {
            return;
        }
        let damage = fround(input.damage / 200.0);
        if let Some(mut existing) = self.actors.iter_mut().find(|actor| {
            actor.kind == NativeSecondaryActorKind::FireBurn
                && actor.world_key == input.world_key
                && actor.target_id == Some(input.target.id)
        }) {
            existing.age_ticks = 0;
            existing.damage = existing.damage.max(damage);
            existing.lifetime_ticks = 200;
            existing.owner_id = input.owner_id;
            existing.position = input.target.position;
            existing.rank = input.rank;
            existing.scale = input.target.scale;
            existing.skill_id = Some(input.skill_id);
            return;
        }
        let mut actor = NativeSecondaryActorState::new(
            0,
            NativeSecondaryActorKind::FireBurn,
            input.owner_id,
            input.world_key,
            Some(input.skill_id),
        );
        actor.damage = damage;
        actor.light_registration = Some(input.target.light_registration);
        actor.lifetime_ticks = 200;
        actor.misc_light_append_ordinal = Some(0);
        actor.position = input.target.position;
        actor.rank = input.rank;
        actor.scale = input.target.scale;
        actor.target_id = Some(input.target.id);
        self.spawn_actor(actor);
    }

    pub fn apply_ether_burn(&mut self, input: NativeSecondaryEtherBurnInput) {
        if let Some(mut existing) = self.actors.iter_mut().find(|actor| {
            actor.kind == NativeSecondaryActorKind::EtherBurn
                && actor.world_key == input.world_key
                && actor.target_id == Some(input.target.id)
        }) {
            existing.age_ticks = 0;
            existing.lifetime_ticks = 300;
            existing.owner_id = input.owner_id;
            existing.position = input.target.position;
            existing.rank = input.rank;
            existing.scale = input.target.scale;
            return;
        }
        let mut actor = NativeSecondaryActorState::new(
            0,
            NativeSecondaryActorKind::EtherBurn,
            input.owner_id,
            input.world_key,
            Some(14),
        );
        actor.light_registration = Some(input.target.light_registration);
        actor.lifetime_ticks = 300;
        actor.misc_light_append_ordinal = Some(0);
        actor.position = input.target.position;
        actor.rank = input.rank;
        actor.scale = input.target.scale;
        actor.target_id = Some(input.target.id);
        self.spawn_actor(actor);
    }

    pub fn apply_player_damage(
        &mut self,
        player_id: &str,
        amount: f64,
        tick: u64,
        position: Vec2,
        world_key: &str,
        presentation: NativeSecondaryPlayerDamagePresentation,
    ) -> NativeSecondaryPlayerDamageResult {
        assert!(
            amount.is_finite() && amount >= 0.0,
            "secondary player damage is finite and non-negative"
        );
        let unabsorbed =
            NativeSecondaryPlayerDamageResult { absorbed_damage: 0.0, health_damage: amount };
        let absorbed =
            NativeSecondaryPlayerDamageResult { absorbed_damage: amount, health_damage: 0.0 };
        let Some(current) = self.players.get(player_id) else {
            return unabsorbed;
        };
        if amount == 0.0 {
            return unabsorbed;
        }
        if current.magic_shield_absorb <= 0.0 {
            if current.stoneskin_ticks_remaining == 0 {
                return unabsorbed;
            }
            if presentation.physical {
                self.spawn_stoneskin_chip(
                    player_id,
                    position,
                    world_key,
                    presentation.enhanced_effects,
                    None,
                );
            }
            return absorbed;
        }
        let remaining = (current.magic_shield_absorb - amount).max(0.0);
        let broke = remaining == 0.0;
        let explosion_damage = current.magic_shield_explosion_damage;
        let hit_pitch = self.rng.draw_scaled_float(0.05, false);
        let player = self.players.get_mut(player_id).expect("damaged secondary player exists");
        player.magic_shield_absorb = remaining;
        player.magic_shield_pulse_ticks = if broke { 0 } else { 40 };
        if broke {
            player.magic_shield_explosion_damage = 0.0;
            player.magic_shield_maximum = 0.0;
        }
        let mut event = NativeSecondaryEventState::new(
            NativeSecondaryEventKind::ShieldHit,
            player_id.to_owned(),
            world_key.to_owned(),
            tick,
        );
        event.cue = Some(NativeSecondaryAudioCue::HitShield);
        event.pitch = fround(0.8 + hit_pitch);
        event.position = position;
        event.skill_id = Some(54);
        self.emit_event(event);
        if !broke {
            return absorbed;
        }
        let mut event = NativeSecondaryEventState::new(
            NativeSecondaryEventKind::ShieldBreak,
            player_id.to_owned(),
            world_key.to_owned(),
            tick,
        );
        event.cue = Some(NativeSecondaryAudioCue::PopShield);
        event.pitch = 0.8_f32 as f64;
        event.position = position;
        event.skill_id = Some(54);
        self.emit_event(event);
        for index in 0..20 {
            let rotation = self.rng.draw_scaled_float(360.0, false);
            let alpha = self.rng.draw_scaled_float(0.75, false);
            let scale = self.rng.draw_scaled_float(0.25, false);
            let mut actor = NativeSecondaryActorState::new(
                0,
                NativeSecondaryActorKind::ShieldBreak,
                player_id.to_owned(),
                world_key.to_owned(),
                Some(54),
            );
            actor.alpha = fround(0.5 + alpha);
            actor.position = Vec2::new(position.x, position.y - 35.0);
            actor.rotation_radians = rotation * std::f64::consts::PI / 180.0;
            actor.scale = fround(2.0 + scale);
            actor.lifetime_ticks = 26;
            actor.variant = index;
            self.spawn_actor(actor);
        }
        if explosion_damage > 0.0 {
            let presentation_rng = self.rng.clone();
            for _ in 0..502 {
                self.rng.draw_word();
            }
            let mut actor = NativeSecondaryActorState::new(
                0,
                NativeSecondaryActorKind::ShieldExplosion,
                player_id.to_owned(),
                world_key.to_owned(),
                Some(54),
            );
            actor.damage = explosion_damage;
            actor.lifetime_ticks = 116;
            actor.position = position;
            actor.presentation_rng = Some(presentation_rng);
            actor.radius = 110.0;
            self.spawn_actor(actor);
            let mut actor = NativeSecondaryActorState::new(
                0,
                NativeSecondaryActorKind::Shockwave,
                player_id.to_owned(),
                world_key.to_owned(),
                Some(54),
            );
            actor.lifetime_ticks = 36;
            actor.phase = 0.35_f32 as f64;
            actor.position = position;
            actor.quantity = 6.0;
            actor.radius = 75.0;
            actor.slow_factor = 0.0375_f32 as f64;
            actor.variant = 1;
            self.spawn_actor(actor);
            let mut event = NativeSecondaryEventState::new(
                NativeSecondaryEventKind::Impact,
                player_id.to_owned(),
                world_key.to_owned(),
                tick,
            );
            event.camera_magnitude = 1.25;
            event.cue = Some(NativeSecondaryAudioCue::MagicShieldExplode);
            event.position = position;
            event.screen_flash =
                Some(NativeSecondaryScreenFlashState::new(0.5, 1.0, 1.0, 0.05, true, 1.0));
            event.skill_id = Some(54);
            self.emit_event(event);
        }
        absorbed
    }

    pub fn apply_golem_damage(
        &mut self,
        actor_id: u64,
        damage: NativeGolemIncomingDamage,
        tick: u64,
        enhanced_effects: bool,
        order: Option<&mut crate::painter::WorldPainterOrder>,
    ) -> NativeSecondaryGolemDamageResult {
        let missing = NativeSecondaryGolemDamageResult {
            ignored: true,
            killed: false,
            owner_id: None,
            reflected_damage: 0.0,
        };
        let Some(index) = self.actors.iter().position(|actor| {
            actor.id == actor_id && actor.kind == NativeSecondaryActorKind::Golem
        }) else {
            return missing;
        };
        let actor = self.actors.get(index).expect("Golem index is live").snapshot();
        let Some(golem) = actor.golem else {
            return missing;
        };
        let received = damage_native_secondary_golem(
            NativeGolemKernelActor {
                age_ticks: actor.age_ticks,
                damage_minimum: actor.damage,
                golem,
                id: actor.id,
                owner_id: actor.owner_id.clone(),
                position: actor.position,
                rotation_radians: actor.rotation_radians,
                target_id: actor.target_id,
            },
            damage,
        );
        let result = NativeSecondaryGolemDamageResult {
            ignored: received.ignored,
            killed: received.killed,
            owner_id: Some(actor.owner_id.clone()),
            reflected_damage: received.reflected_damage,
        };
        if received.ignored {
            return result;
        }
        if let Some(remaining) = received.actor {
            let mut actor = self.actors.get_mut(index).expect("Golem index is live");
            *actor.golem_mut().expect("Golem retains its articulation") = remaining.golem;
            actor.quantity = remaining.golem.current_health;
            return result;
        }
        let animation = crate::death_animations::create_native_golem_death_animation(
            &mut self.rng,
            actor.position,
            enhanced_effects,
        );
        self.actors.remove(index);
        let mut death = NativeSecondaryActorState::new(
            0,
            NativeSecondaryActorKind::GolemDeath,
            actor.owner_id.clone(),
            actor.world_key.clone(),
            Some(45),
        );
        death.enhanced = enhanced_effects;
        death.lifetime_ticks = if enhanced_effects {
            crate::death_animations::NATIVE_GOLEM_DEATH_MAX_AGE_ENHANCED
        } else {
            crate::death_animations::NATIVE_GOLEM_DEATH_MAX_AGE_BASIC
        };
        death.position = actor.position;
        death.golem_death = Some(animation);
        if let Some(order) = order {
            death.painter_registrations = order.register_many(
                crate::painter::WorldManagerLane::Actor,
                crate::death_animations::NATIVE_GOLEM_DEATH_PAINTER_COUNT,
            );
        }
        death.variant = i32::from(golem.iron);
        self.spawn_actor(death);
        let death = self.actors.last().expect("Golem death presentation was spawned").snapshot();
        for cue in [
            NativeSecondaryAudioCue::StoneBreak,
            NativeSecondaryAudioCue::FlameLashStart,
            NativeSecondaryAudioCue::GolemDie,
            NativeSecondaryAudioCue::RockHit,
        ] {
            let event = super::program::actor_event(
                &death,
                tick,
                NativeSecondaryEventKind::Impact,
                Some(cue),
            );
            self.emit_event(event);
        }
        result
    }
}

fn record_camera_displacement(
    proposals: &mut Vec<NativeSecondaryCameraDisplacement>,
    world_key: &str,
    displacement: Vec2,
    tick: u64,
) {
    if let Some(current) = proposals.iter().find(|row| row.world_key == world_key)
        && displacement.x * displacement.x + displacement.y * displacement.y
            <= current.displacement.x * current.displacement.x
                + current.displacement.y * current.displacement.y
    {
        return;
    }
    proposals.retain(|row| row.world_key != world_key);
    proposals.push(NativeSecondaryCameraDisplacement {
        displacement,
        tick,
        world_key: world_key.to_owned(),
    });
}
