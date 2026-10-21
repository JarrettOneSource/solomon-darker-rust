//! One ordered secondary tick. Births wait until the next tick, and all families
//! consume the same RNG stream in actor registration order, as in the Website.

use std::collections::{BTreeMap, BTreeSet};

use super::program::*;
use super::*;
use crate::Vec2;
use crate::belt::NativeBeltEntry;
use crate::float::{atan2, fround};
use crate::painter::WorldPainterOrder;
use crate::profile::offensive::resolve_native_skill_damage_value;
use crate::spells::targeting::{
    PrimarySpellQueryLane, PrimarySpellTarget, PrimarySpellTargetKind,
    native_primary_circle_targets,
};

pub(super) struct BurnRequest {
    pub actor: NativeSecondaryActorState,
    pub damage: f64,
    pub target: NativeSecondaryTarget,
}

pub(super) struct StepEngine<'state, 'context, 'player, W> {
    pub state: &'state mut NativeSecondarySimulationState,
    pub context: &'context NativeSecondaryTickContext<'player>,
    pub world: &'context mut W,
    pub output: NativeSecondaryTickResult,
    pub sources: BTreeMap<u64, NativeSecondaryActorState>,
    pub last_appendages: BTreeMap<u64, u64>,
    pub wobble_phases: BTreeMap<u64, f64>,
    pub scenery_phases: BTreeMap<(String, usize), f64>,
    pub drain_pulses: BTreeSet<u64>,
    pub fire_burns: Vec<BurnRequest>,
    pub electric_burns: Vec<BurnRequest>,
}

impl<W: NativeSecondaryWorld> StepEngine<'_, '_, '_, W> {
    pub fn candidates(
        &self,
        actor: &NativeSecondaryActorState,
        radius: f64,
    ) -> Vec<NativeSecondaryTarget> {
        let targets = self.world.targets(&actor.world_key, actor.position, radius);
        if !matches!(
            actor.kind,
            NativeSecondaryActorKind::Shockwave
                | NativeSecondaryActorKind::RescueShockwave
                | NativeSecondaryActorKind::MindblastShockwave
                | NativeSecondaryActorKind::FreezeWave
        ) {
            return stable_targets(targets);
        }
        let mut query_targets = Vec::with_capacity(targets.len());
        let mut by_id = BTreeMap::new();
        for (index, target) in targets.iter().enumerate() {
            let order = target.registration_order.unwrap_or(index);
            let id = target.id.to_string();
            by_id.insert(id.clone(), target);
            query_targets.push(PrimarySpellTarget {
                active: true,
                actor_flags: target.native_flags.unwrap_or(2),
                attachment: target.position,
                body_radius: target.radius,
                cell_binding_order: order,
                heading_degrees: None,
                grid_position: None,
                id,
                kind: PrimarySpellTargetKind::Enemy,
                native_priority: 0,
                pending_remove: false,
                position: target.position,
                query_lane: PrimarySpellQueryLane::Grid,
                registration_order: order,
            });
        }
        let mut selected = Vec::new();
        for target in native_primary_circle_targets(actor.position, radius, 2, &query_targets) {
            let original =
                by_id.get(&target.id).expect("circle query retains the submitted target identity");
            selected.push((*original).clone());
        }
        selected
    }

    pub fn effect(
        &self,
        world_key: &str,
        target_id: usize,
    ) -> Option<&NativeSecondaryTargetEffectState> {
        native_secondary_target_effect(&self.state.target_effects, world_key, target_id)
    }

    pub fn merge(
        &mut self,
        actor: &NativeSecondaryActorState,
        target_id: usize,
        patch: NativeSecondaryTargetEffectPatch,
    ) {
        apply_native_secondary_target_effect(
            &mut self.state.target_effects,
            &actor.world_key,
            target_id,
            patch,
        );
    }

    pub fn damage(
        &mut self,
        actor: &NativeSecondaryActorState,
        target: &NativeSecondaryTarget,
        amount: f64,
        kind: NativeSecondaryDamageKind,
    ) {
        if (amount <= 0.0 || amount.is_nan())
            && !matches!(
                actor.kind,
                NativeSecondaryActorKind::RescueShockwave
                    | NativeSecondaryActorKind::GolemKnockback
            )
        {
            return;
        }
        let prismatic = self
            .effect(&actor.world_key, target.id)
            .is_some_and(|effect| effect.prismatic_ticks > 0);
        self.output.damage.push(NativeSecondaryDamageContact {
            amount: if kind == NativeSecondaryDamageKind::Lightning && prismatic {
                amount * 2.0
            } else {
                amount
            },
            ether_drain: actor.kind == NativeSecondaryActorKind::EtherDrain,
            hit_strength: match actor.kind {
                NativeSecondaryActorKind::FireBurn => Some(0.0),
                NativeSecondaryActorKind::ElectricBurn => Some(actor.phase),
                _ => None,
            },
            kind,
            owner_id: actor.owner_id.clone(),
            source_actor_id: actor.id,
            special_death_payload: None,
            suppress_hit_reaction: matches!(
                actor.kind,
                NativeSecondaryActorKind::FirePatch
                    | NativeSecondaryActorKind::MovingFire
                    | NativeSecondaryActorKind::FireBurn
                    | NativeSecondaryActorKind::ElectricBurn
                    | NativeSecondaryActorKind::AcidRain
                    | NativeSecondaryActorKind::EtherDrain
            ),
            suppress_hurt_sound: actor.kind == NativeSecondaryActorKind::FireBurn,
            target_id: target.id,
        });
    }

    pub fn event(
        &mut self,
        actor: &NativeSecondaryActorState,
        cue: Option<NativeSecondaryAudioCue>,
        kind: NativeSecondaryEventKind,
    ) {
        self.state.emit_event(actor_event(actor, self.context.tick, kind, cue));
    }

    fn advance(&mut self, source: &NativeSecondaryActorState) -> Option<NativeSecondaryActorState> {
        if source.kind == NativeSecondaryActorKind::GolemDeath {
            let mut actor = source.clone();
            let animation =
                actor.golem_death.as_mut().expect("Golem death retains its mutable fragments");
            crate::death_animations::step_native_golem_death_animation(
                animation,
                self.context.tick,
                &mut self.state.rng,
            );
            let retained_fragments = animation.fragments.iter().any(Option::is_some);
            advance_actor(&mut actor);
            return (actor.age_ticks < crate::death_animations::NATIVE_GOLEM_DEATH_STAR_TICKS
                || retained_fragments)
                .then_some(actor);
        }
        let owner = self.context.players.get(&source.owner_id)?;
        if owner.world_key != source.world_key {
            return None;
        }
        let mut actor = source.clone();
        advance_actor(&mut actor);
        let mut retain = actor.age_ticks < actor.lifetime_ticks;
        use NativeSecondaryActorKind::*;
        match actor.kind {
            Leviathan
            | LeviathanAppendage
            | LeviathanMote
            | EtherBolt
            | PlaneOrbShot
            | PlaneOrbParticle
            | EtherDrain
            | EtherDrainCloud
            | EtherDrainDebris
            | EtherDrainCaptureFlare => {
                super::cosmic::step(self, source, &mut actor, &mut retain);
            }
            MovingFire | FirePatch | Shockwave | MindblastShockwave | RescueShockwave
            | FreezeWave | FrostBurnFlare | RingFireFragment | FireBurn | FireBurnFlame
            | EtherBurn | EtherBurnFlare | StormCloud | AcidRain | StormDrop | AcidDrop
            | AcidSplash | Comet | CometTrail | CometDebris | StoneskinChip => {
                super::elemental::step(self, source, &mut actor, &mut retain);
            }
            Earthquake
            | EarthquakeSceneryWobble
            | EarthquakeQuake
            | EarthquakeDust
            | EarthquakeDebris
            | GolemAssemblyDebris
            | Golem
            | GolemKnockback => {
                super::earth::step(self, source, &mut actor, &mut retain);
            }
            MagicCircle | MagicCirclePlayerFlash | MagicTrap | MagicTrapShimmer | ElectricBurn => {
                super::fields::step(self, source, &mut actor, &mut retain);
            }
            ElectricBurnFlare => super::electric_burn::step_flare(source, &mut actor, &mut retain),
            PrismaticWave | PhaseBurst | IceBlast | StormStrike | TurnUndead
            | DampenedProjectile | DampenedSmoke | FlashResponseGrow | FlashResponseFade
            | ShieldBreak | TeleportBurst | ShieldExplosion | PlanewalkerMote => {
                super::local_tick::step(self, source, &mut actor, &mut retain);
            }
            EtherFade | FreezeWaveVisual | RingFireExplosion | GolemDeath | MagicTrapBurst
            | ElectricBurnArc | DampenWave | MindblastBurst | CometImpact => {}
        }
        retain.then_some(actor)
    }
}

impl NativeSecondarySimulationState {
    /// Advances statuses, existing actors and player input in native order, then
    /// enrolls the births in the world's shared painter/light allocator. The host
    /// applies the returned contacts and resource changes after this snapshot tick.
    pub fn step(
        &mut self,
        context: &NativeSecondaryTickContext<'_>,
        world: &mut impl NativeSecondaryWorld,
        order: Option<&mut WorldPainterOrder>,
    ) -> NativeSecondaryTickResult {
        assert!(context.tick <= MAX_NATIVE_TICKS, "secondary tick is a non-negative safe integer");
        self.camera_displacements.clear();
        let old_events = self.events.clone();
        // Source retains identity before both decay and final-tick pulses. A health
        // query cannot replace this: dying and emerging actors still own modifiers.
        self.target_effects
            .retain(|effect| world.target_exists(&effect.world_key, effect.target_id));
        let old_effects = self.target_effects.clone();
        self.events.retain(|event| event.tick >= context.tick.saturating_sub(200));
        for effect in &mut self.target_effects {
            effect.step();
        }
        self.target_effects.retain(NativeSecondaryTargetEffectState::is_active);
        let sources = std::mem::take(&mut self.actors);
        let mut engine = StepEngine {
            state: self,
            context,
            world,
            output: NativeSecondaryTickResult::default(),
            sources: BTreeMap::new(),
            last_appendages: BTreeMap::new(),
            wobble_phases: BTreeMap::new(),
            scenery_phases: BTreeMap::new(),
            drain_pulses: BTreeSet::new(),
            fire_burns: Vec::new(),
            electric_burns: Vec::new(),
        };
        for source in &sources {
            engine.sources.insert(source.id, source.clone());
            if source.kind == NativeSecondaryActorKind::LeviathanAppendage
                && let Some(parent) = source.hit_target_ids.first()
            {
                engine.last_appendages.insert(*parent, source.id);
            }
            if source.kind == NativeSecondaryActorKind::EarthquakeSceneryWobble
                && let Some(target) = source.target_id
            {
                engine.scenery_phases.insert((source.world_key.clone(), target), source.phase);
            }
        }
        super::elemental::step_status_pulses(&mut engine, &old_effects);
        let mut retained = Vec::with_capacity(sources.len());
        for source in &sources {
            if let Some(actor) = engine.advance(source) {
                retained.push(actor);
            }
        }
        // New effects are born during the actor loop; none advances on its birth tick.
        retained.append(&mut engine.state.actors);
        engine.state.actors = retained;
        for actor in &mut engine.state.actors {
            if let Some(phase) = engine.wobble_phases.get(&actor.id) {
                actor.phase = *phase;
            }
            if actor.kind == NativeSecondaryActorKind::EtherDrain
                && engine.drain_pulses.contains(&actor.id)
            {
                actor.slow_factor = 2.0;
            }
        }
        for request in std::mem::take(&mut engine.fire_burns) {
            engine.state.apply_fire_burn(NativeSecondaryFireBurnInput {
                damage: request.damage,
                owner_id: request.actor.owner_id,
                rank: request.actor.rank,
                skill_id: request.actor.skill_id.expect("fire contact retains its source skill"),
                target: request.target,
                world_key: request.actor.world_key,
            });
        }
        for request in std::mem::take(&mut engine.electric_burns) {
            apply_electric_burn(engine.state, request);
        }
        step_players(&mut engine, &old_events);
        if engine.state.events.len() > 512 {
            engine.state.events.drain(..engine.state.events.len() - 512);
        }
        enroll_native_secondary_light_owners(&mut engine.state.actors, order, |key, id| {
            engine.world.target(key, id).map(|target| target.light_registration)
        });
        normalize_output(&mut engine.output);
        engine.output
    }
}

fn apply_electric_burn(state: &mut NativeSecondarySimulationState, request: BurnRequest) {
    if request.damage <= 0.0 || request.damage.is_nan() {
        return;
    }
    let damage = fround(request.damage / 100.0);
    if let Some(actor) = state.actors.iter_mut().find(|actor| {
        actor.kind == NativeSecondaryActorKind::ElectricBurn
            && actor.world_key == request.actor.world_key
            && actor.target_id == Some(request.target.id)
    }) {
        actor.lifetime_ticks = (actor.lifetime_ticks.saturating_sub(actor.age_ticks)).max(100);
        actor.age_ticks = 0;
        actor.alpha = 0.0;
        actor.damage = damage;
        actor.phase = 0.0;
        actor.light_registration = Some(request.target.light_registration);
        actor.owner_id = request.actor.owner_id;
        actor.position = request.target.position;
        actor.rank = request.actor.rank;
        actor.skill_id = request.actor.skill_id;
        return;
    }
    let mut actor = actor(
        NativeSecondaryActorKind::ElectricBurn,
        &request.actor.owner_id,
        &request.actor.world_key,
        request.actor.skill_id,
        request.actor.rank,
        request.target.position,
    );
    actor.alpha = 0.0;
    actor.damage = damage;
    actor.lifetime_ticks = 100;
    actor.light_registration = Some(request.target.light_registration);
    actor.misc_light_append_ordinal = Some(0);
    actor.radius = 0.5;
    actor.target_id = Some(request.target.id);
    actor.variant = 2;
    state.spawn_actor(actor);
}

fn step_players<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    old_events: &[NativeSecondaryEventState],
) {
    let mut players = BTreeMap::new();
    for (id, authority) in engine.context.players {
        let mut player = engine.state.players.get(id).cloned().unwrap_or_default();
        if authority.eligible
            && player.cast_action.is_some_and(|action| action.progress == 0.0)
            && old_events.iter().any(|event| {
                &event.owner_id == id && Some(event.tick) == engine.context.tick.checked_sub(1)
            })
        {
            engine.output.staff_cast_pulse_player_ids.push(id.clone());
        }
        let was_planewalker = player.planewalker_ticks_remaining > 0;
        let was_stoneskin = player.stoneskin_ticks_remaining > 0;
        player.step(
            authority.skill_book,
            authority.secondary_recharge_factor,
            &authority.offensive_factors,
        );
        player.reserved_mana = player.mana_reserve(authority.skill_book, authority.maximum_mana);
        for (expired, ability, kind, cue) in [
            (
                was_planewalker && player.planewalker_ticks_remaining == 0,
                NativeSecondaryAbilityId::Planewalker,
                NativeSecondaryEventKind::ToggleOff,
                NativeSecondaryAudioCue::PlanewalkerOff,
            ),
            (
                was_stoneskin && player.stoneskin_ticks_remaining == 0,
                NativeSecondaryAbilityId::Stoneskin,
                NativeSecondaryEventKind::Pulse,
                NativeSecondaryAudioCue::Stoneskin,
            ),
        ] {
            if expired {
                if ability == NativeSecondaryAbilityId::Stoneskin {
                    engine.state.spawn_stoneskin_burst(
                        id,
                        authority.character.position,
                        authority.world_key,
                        engine.context.enhanced_effects_for(id),
                    );
                }
                engine.state.emit_event(cast_event(
                    id,
                    ability.skill_id(),
                    authority,
                    engine.context.tick,
                    kind,
                    Some(cue),
                ));
            }
        }
        if !authority.alive || player.reserved_mana > authority.maximum_mana {
            let overloaded = player.firewalker || player.mindstar || player.regenerate;
            player.clear_toggles();
            if overloaded && authority.alive {
                emit_overload(engine, id, authority, &player);
            }
        }
        if player.regenerate && authority.eligible {
            engine.output.health_recovered.insert(id.clone(), 1.5 / 100.0);
        }
        if player.planewalker_ticks_remaining > 0 && authority.eligible {
            engine.output.primary_override_player_ids.push(id.clone());
            super::spawns::planewalker_mote(
                engine.state,
                id,
                authority.character.position,
                authority.world_key,
                engine.context.enhanced_effects_for(id),
            );
            let held = authority.input.primary && authority.input.aim.is_some();
            if held && !player.plane_orb_held {
                let origin = authority.character.position;
                let direction = unit(origin, authority.input.aim.unwrap_or(origin));
                let maximum_scale = engine.state.rng.draw_scaled_float(1.5, false);
                let mut orb = actor(
                    NativeSecondaryActorKind::PlaneOrbShot,
                    id,
                    authority.world_key,
                    Some(12),
                    1,
                    origin,
                );
                orb.damage = resolve_native_skill_damage_value(
                    80,
                    native_plane_orb_damage(authority.skill_book),
                    &authority.offensive_factors,
                );
                orb.enhanced = authority.enhanced_effects;
                orb.lifetime_ticks = 1125;
                orb.phase = 1.0;
                orb.radius = 1.0;
                orb.rotation_radians = atan2(direction.y, direction.x);
                orb.scale = 0.5;
                orb.slow_factor = 1.0 + maximum_scale;
                orb.velocity = Vec2::new(direction.x * 1.75, direction.y * 1.75);
                engine.state.spawn_actor(orb);
                super::spawns::plane_orb_birth(engine.state, id, authority.world_key, origin);
                engine.state.emit_event(cast_event(
                    id,
                    12,
                    authority,
                    engine.context.tick,
                    NativeSecondaryEventKind::Cast,
                    Some(NativeSecondaryAudioCue::DistortReality),
                ));
                let mut event = cast_event(
                    id,
                    12,
                    authority,
                    engine.context.tick,
                    NativeSecondaryEventKind::Pulse,
                    Some(NativeSecondaryAudioCue::LightningStart),
                );
                event.pitch = 2.0;
                event.screen_flash =
                    Some(NativeSecondaryScreenFlashState::new(1.0, 0.0, 1.0, 0.1, false, 0.1));
                engine.state.emit_event(event);
            }
            player.plane_orb_held = held;
        } else {
            player.plane_orb_held = false;
        }
        if player.firewalker && authority.eligible && engine.context.tick.is_multiple_of(10) {
            firewalker_patch(
                engine.state,
                id,
                authority,
                engine.state.firewalker_geometry_sequence == 0,
            );
            engine.state.firewalker_geometry_sequence =
                (engine.state.firewalker_geometry_sequence + 1) % 3;
        }
        let slot = authority.input.quickbar;
        if let Some(slot) = slot.filter(|slot| Some(*slot) != player.held_slot)
            && let Some(Some(NativeBeltEntry::Skill { skill_id })) =
                authority.belt.entries.get(slot)
            && let Some(ability) = NativeSecondaryAbilityId::from_skill_id(*skill_id)
        {
            let result = super::casting::cast_ability(
                engine.state,
                player,
                id,
                ability,
                engine.context,
                engine.world,
            );
            player = result.player;
            if result.mana_recovered > 0.0 {
                *engine.output.mana_recovered.entry(id.clone()).or_default() +=
                    result.mana_recovered;
            }
            if result.mana_spent > 0.0 {
                engine.output.mana_spent.insert(id.clone(), result.mana_spent);
            }
            if result.mana_underflow {
                engine.output.mana_underflow_player_ids.push(id.clone());
            }
            if let Some(position) = result.relocated {
                engine.output.relocated_players.insert(id.clone(), position);
            }
            if let Some(heading) = result.facing_heading_degrees {
                engine.output.facing_heading_degrees_by_player.insert(id.clone(), heading);
            }
            engine.output.removed_projectile_ids.extend(result.removed_projectile_ids);
            engine.output.target_heading_changes.extend(result.target_heading_changes);
            engine.output.dampened_caster_target_ids.extend(result.dampened_caster_target_ids);
            engine.output.dispelled_shield_target_ids.extend(result.dispelled_shield_target_ids);
        }
        if player.reserved_mana > authority.maximum_mana {
            let overloaded = player.firewalker || player.mindstar || player.regenerate;
            player.clear_toggles();
            if overloaded {
                emit_overload(engine, id, authority, &player);
            }
        }
        if player.planewalker_ticks_remaining > 0 && authority.eligible {
            engine.output.primary_override_player_ids.push(id.clone());
        }
        player.held_slot = slot;
        players.insert(id.clone(), player);
    }
    engine.state.players = players;
}

fn emit_overload<W: NativeSecondaryWorld>(
    engine: &mut StepEngine<'_, '_, '_, W>,
    id: &str,
    authority: &NativeSecondaryPlayerAuthority<'_>,
    player: &NativeSecondaryPlayerState,
) {
    engine.output.overloaded_player_ids.push(id.to_owned());
    let mut event = cast_event(
        id,
        player.last_skill_id.unwrap_or(NativeSecondaryAbilityId::Mindstar).skill_id(),
        authority,
        engine.context.tick,
        NativeSecondaryEventKind::Overload,
        Some(NativeSecondaryAudioCue::Fizzle),
    );
    event.pitch = 1.0;
    engine.state.emit_event(event);
}

fn normalize_output(output: &mut NativeSecondaryTickResult) {
    for ids in [
        &mut output.dampened_caster_target_ids,
        &mut output.dispelled_shield_target_ids,
        &mut output.disrupted_target_ids,
        &mut output.removed_projectile_ids,
    ] {
        ids.sort_unstable();
        ids.dedup();
    }
    for ids in [
        &mut output.mana_underflow_player_ids,
        &mut output.overloaded_player_ids,
        &mut output.primary_override_player_ids,
        &mut output.staff_cast_pulse_player_ids,
    ] {
        ids.sort();
        ids.dedup();
    }
}
