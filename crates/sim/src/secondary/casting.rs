//! Every stock secondary constructor and admission/cooldown path, in source RNG order.

use super::context::*;
use super::dampened::create_native_dampened_spell;
use super::golem::{NativeGolemBirthStats, NativeGolemWorld, NativeSecondaryGolemState};
use super::leviathan::*;
use super::program::*;
use super::{
    NativeSecondaryAbilityId as Ability, NativeSecondaryActorKind as Kind,
    NativeSecondaryAudioCue as Cue, NativeSecondaryEventKind as Event,
    NativeSecondaryManaCostBonuses, NativeSecondaryPlayerState, NativeSecondarySimulationState,
    NativeSecondaryTargetEffectPatch, apply_native_secondary_target_effect,
    native_secondary_ability_mana_cost, native_secondary_cooldown_capacity_ticks,
    resolved_secondary_ability_rank_stats,
};
use crate::float;
use crate::float::{atan2, fround, hypot};
use crate::heading::heading_vector;
use crate::profile::{NATIVE_SKILL_CATALOG, native_skill_stat, native_weld_build};
use crate::secondary::SecondaryActorRead;
use crate::{NativeRng, Vec2};

pub(crate) struct SecondaryCastResult {
    pub target_heading_changes: Vec<super::NativeSecondaryTargetHeadingChange>,
    pub dispelled_shield_target_ids: Vec<usize>,
    pub dampened_caster_target_ids: Vec<usize>,
    pub mana_recovered: f64,
    pub mana_underflow: bool,
    pub mana_spent: f64,
    pub player: NativeSecondaryPlayerState,
    pub facing_heading_degrees: Option<f64>,
    pub relocated: Option<Vec2>,
    pub removed_projectile_ids: Vec<usize>,
}
impl SecondaryCastResult {
    fn none(player: NativeSecondaryPlayerState, mana_underflow: bool) -> Self {
        Self {
            target_heading_changes: Vec::new(),
            dispelled_shield_target_ids: Vec::new(),
            dampened_caster_target_ids: Vec::new(),
            mana_recovered: 0.0,
            mana_underflow,
            mana_spent: 0.0,
            player,
            facing_heading_degrees: None,
            relocated: None,
            removed_projectile_ids: Vec::new(),
        }
    }
}

pub(crate) struct GolemWorld<'a, W> {
    pub world: &'a W,
    pub owner: &'a str,
    pub world_key: &'a str,
    pub origin: Vec2,
}
impl<W: NativeSecondaryWorld> NativeGolemWorld for GolemWorld<'_, W> {
    fn resolve_movement(&mut self, requested: Vec2) -> Vec2 {
        self.world.golem_movement(self.owner, self.world_key, self.origin, requested, 30.0)
    }
    fn resolve_foot_target(&mut self, current: Vec2, requested: Vec2) -> Vec2 {
        self.world.golem_foot_placement(self.owner, self.world_key, current, requested)
    }
}

pub(crate) fn cast_ability(
    state: &mut NativeSecondarySimulationState,
    mut player: NativeSecondaryPlayerState,
    owner: &str,
    ability: Ability,
    context: &NativeSecondaryTickContext<'_>,
    world: &impl NativeSecondaryWorld,
) -> SecondaryCastResult {
    let authority = &context.players[owner];
    let tick = context.tick;
    let skill_id = ability.skill_id();
    if !authority.eligible {
        state.emit_event(cast_event(
            owner,
            skill_id,
            authority,
            tick,
            Event::Fizzle,
            Some(Cue::Fizzle),
        ));
        player.fizzle_sequence += 1;
        player.last_skill_id = Some(ability);
        return SecondaryCastResult::none(player, false);
    }
    if player.cast_action.is_some()
        || player.cast_spin_ticks_remaining > 0
        || player.global_cooldown_ticks > 0.0
    {
        return SecondaryCastResult::none(player, false);
    }
    if player.cooldown_ticks_by_skill[skill_id] > 0.0 {
        state.emit_event(cast_event(
            owner,
            skill_id,
            authority,
            tick,
            Event::Fizzle,
            Some(Cue::Fizzle),
        ));
        player.fizzle_sequence += 1;
        player.last_skill_id = Some(ability);
        return SecondaryCastResult::none(player, false);
    }
    let ranked = resolved_secondary_ability_rank_stats(
        authority.skill_book,
        ability,
        &authority.offensive_factors,
    );
    let value = |name| ranked.value(name).expect("secondary constructor has its authored property");
    let cost = native_secondary_ability_mana_cost(
        authority.skill_book,
        ability,
        &authority.offensive_factors,
        NativeSecondaryManaCostBonuses {
            explosive_shield: authority.explosive_shield_raw_mana_cost,
            golem: authority.golem_raw_mana_cost,
            magic_storm: authority.magic_storm_raw_mana_cost,
        },
    );
    let capacity = native_secondary_cooldown_capacity_ticks(authority.skill_book, ability);
    let toggling_off = ability == Ability::Planewalker && player.planewalker_ticks_remaining > 0
        || ability == Ability::Firewalker && player.firewalker
        || ability == Ability::Mindstar && player.mindstar
        || ability == Ability::Regenerate && player.regenerate;
    if !toggling_off && cost > authority.current_mana {
        state.emit_event(cast_event(
            owner,
            skill_id,
            authority,
            tick,
            Event::Fizzle,
            Some(Cue::Fizzle),
        ));
        player.fizzle_sequence += 1;
        player.last_skill_id = Some(ability);
        return SecondaryCastResult::none(player, true);
    }
    let origin = authority.character.position;
    let aim = authority.input.aim.unwrap_or(origin);
    let direction = unit(origin, aim);
    let make = |kind, position| {
        actor(kind, owner, authority.world_key, Some(skill_id), ranked.rank, position)
    };
    player.cast_sequence += 1;
    player.last_skill_id = Some(ability);
    let mut result = SecondaryCastResult::none(player, false);
    result.mana_spent = cost;
    let mut post_cast_cue = None;
    match ability {
        Ability::CallLeviathan => {
            let birth = create_native_leviathan_birth(
                &mut state.rng,
                value("mQuantity"),
                authority.maximum_leviathan,
            );
            let damage = value("mDamage");
            let parent_id = state.next_actor_id;
            let mut parent = make(Kind::Leviathan, aim);
            parent.damage = damage;
            parent.enhanced = authority.enhanced_effects;
            parent.lifetime_ticks = NATIVE_LEVIATHAN_LIFETIME_TICKS;
            parent.quantity = birth.quantity as f64;
            parent.radius = fround(birth.maximum_scale * 30.0);
            parent.rotation_radians = birth.rotation_radians;
            parent.scale = 0.0;
            parent.slow_factor = birth.maximum_scale;
            state.spawn_actor(parent);
            for (index, appendage) in birth.appendages.into_iter().enumerate() {
                let mut child = make(Kind::LeviathanAppendage, aim);
                child.damage = damage;
                child.endpoint = appendage.base_offset;
                child.frame =
                    (native_leviathan_heading_vector(appendage.heading_degrees).y * 100.0 + 0.5)
                        .floor()
                        + 100.0;
                child.hit_target_ids = vec![parent_id];
                child.lifetime_ticks = NATIVE_LEVIATHAN_LIFETIME_TICKS;
                child.phase = f64::from(appendage.bank);
                child.quantity = f64::from(appendage.countdown);
                child.radius = appendage.sprite_scale;
                child.rotation_radians = appendage.heading_degrees * std::f64::consts::PI / 180.0;
                child.scale = 0.0;
                child.slow_factor = 1.0;
                child.variant = index as i32;
                child.velocity = Vec2::new(appendage.spin_degrees, appendage.spin_step_degrees);
                state.spawn_actor(child);
            }
        }
        Ability::Planewalker => {
            if result.player.planewalker_ticks_remaining > 0 {
                result.player.planewalker_ticks_remaining = 0;
                result.player.plane_orb_held = false;
                state.emit_event(cast_event(
                    owner,
                    skill_id,
                    authority,
                    tick,
                    Event::ToggleOff,
                    Some(Cue::PlanewalkerOff),
                ));
                result.player.start_cast(authority.weapon_kind);
                result.player.begin_cooldown(
                    ability,
                    capacity,
                    authority.focus_instant_recharge_chance_percent,
                    &mut state.rng,
                );
                return SecondaryCastResult::none(result.player, false);
            }
            result.player.planewalker_ticks_remaining = result
                .player
                .planewalker_ticks_remaining
                .max((value("mDuration") * 100.0 + 0.5).floor() as u64);
            let mut event = cast_event(owner, skill_id, authority, tick, Event::Pulse, None);
            event.screen_flash = Some(screen_flash(1.0, 0.0, 1.0, 0.1, false));
            state.emit_event(event);
        }
        Ability::Phasing => {
            let direction = heading_vector(authority.character.heading_index);
            result.relocated = world.phasing_destination(owner, origin, direction);
            if result.relocated.is_none() {
                result.player.start_cast(authority.weapon_kind);
                result.player.begin_cooldown(
                    ability,
                    capacity,
                    authority.focus_instant_recharge_chance_percent,
                    &mut state.rng,
                );
                return result;
            }
            let marker = Vec2::new(
                fround(origin.x + direction.x * 10.0),
                fround(origin.y + direction.y * 10.0),
            );
            let mut phase = make(Kind::PhaseBurst, marker);
            phase.alpha = 2.0;
            phase.lifetime_ticks = 20;
            phase.rotation_radians = atan2(direction.y, direction.x);
            phase.scale = 2.0;
            state.spawn_actor(phase);
            let mut event = cast_event(owner, skill_id, authority, tick, Event::Pulse, None);
            event.position = marker;
            state.emit_event(event);
        }
        Ability::RingOfFire => {
            for index in 0..30 {
                let phase = state.rng.draw_scaled_float(32.0, false);
                let mirror = state.rng.draw_sign(1.0);
                let jitter = state.rng.draw_scaled_float(2.0, true);
                let radial = state.rng.draw_scaled_float(30.0, false);
                let radial_direction = native_unit_vector(&mut state.rng);
                let speed = state.rng.draw_scaled_float(0.025, false);
                let radians = (f64::from(index) * 12.0 + jitter) * std::f64::consts::PI / 180.0;
                let heading = Vec2::new(float::sin(radians), -float::cos(radians));
                let position = Vec2::new(
                    fround(origin.x + heading.x * 25.0 + radial_direction.x * radial),
                    fround(origin.y + heading.y * 25.0 + radial_direction.y * radial * 0.8),
                );
                let mut fire = make(Kind::MovingFire, position);
                fire.lifetime_ticks = fire_lifetime_ticks(RING_FIRE_INITIAL_LIFE);
                fire.phase = phase;
                fire.quantity = mirror;
                fire.rotation_radians = radians;
                fire.scale = 2.75;
                fire.slow_factor = RING_FIRE_INITIAL_LIFE;
                fire.variant = index;
                fire.velocity = Vec2::new(
                    fround(heading.x * 2.5 * (1.0 - speed)),
                    fround(heading.y * 2.5 * (1.0 - speed)),
                );
                state.spawn_actor(fire);
            }
            let mut wave = make(Kind::Shockwave, origin);
            wave.damage = value("mDamage");
            wave.lifetime_ticks = 116;
            wave.phase = 1.155_f32 as f64;
            wave.quantity = 6.0;
            wave.radius = 75.0;
            wave.slow_factor = 0.12375_f32 as f64;
            wave.variant = i32::from(authority.maximum_ring_of_fire);
            state.spawn_actor(wave);
            let mut event =
                cast_event(owner, skill_id, authority, tick, Event::Cast, Some(Cue::BigFire));
            event.camera_magnitude = 0.25;
            event.screen_flash = Some(screen_flash(1.0, 0.5, 0.0, 0.01, true));
            state.emit_event(event);
            state.emit_event(cast_event(
                owner,
                skill_id,
                authority,
                tick,
                Event::Pulse,
                Some(Cue::Nuke),
            ));
        }
        Ability::Firewalker => {
            result.player.firewalker = !result.player.firewalker;
            result.player.reserved_mana =
                result.player.mana_reserve(authority.skill_book, authority.maximum_mana);
            if result.player.firewalker {
                firewalker_patch(state, owner, authority, true);
                result.player.start_cast(authority.weapon_kind);
                result.player.begin_cooldown(
                    ability,
                    capacity,
                    authority.focus_instant_recharge_chance_percent,
                    &mut state.rng,
                );
            }
            let mut event = cast_event(
                owner,
                skill_id,
                authority,
                tick,
                if result.player.firewalker { Event::ToggleOn } else { Event::ToggleOff },
                if result.player.firewalker { Some(Cue::Ignite) } else { None },
            );
            event.screen_flash = Some(screen_flash(1.0, 0.5, 0.0, 0.1, true));
            state.emit_event(event);
            return SecondaryCastResult::none(result.player, false);
        }
        Ability::MagicStorm => {
            let ticks = if authority.maximum_magic_storm { 2000 } else { 1000 }
                + authority.magic_storm_duration_bonus_ticks;
            let moving = authority.skill_book.effective_ranks[28] > 0;
            let presentation_rng = state.rng.clone();
            state.rng.draw_scaled_float(1.0, true);
            for _ in 0..15 {
                state.rng.draw_scaled_float(360.0, false);
                state.rng.draw_scaled_float(2.0, false);
            }
            let heading = if moving { state.rng.draw_scaled_float(360.0, false) } else { 0.0 };
            let mut storm = make(Kind::StormCloud, aim);
            storm.alpha = 0.0;
            storm.damage = value("mDamage1");
            storm.enhanced = authority.enhanced_effects;
            storm.freeze_ticks = ticks;
            storm.lifetime_ticks = ticks + 101;
            storm.phase = value("mDamage2");
            storm.presentation_rng = Some(presentation_rng);
            storm.quantity = 50.0;
            storm.radius = 500.0;
            storm.rotation_radians = heading * std::f64::consts::PI / 180.0;
            storm.slow_factor = authority.magic_storm_frequency_factor;
            storm.scale = 0.01_f32 as f64;
            storm.variant = i32::from(moving);
            state.spawn_actor(storm);
        }
        Ability::PrismaticShock => {
            let sign = state.rng.draw_sign(1.0);
            let color = state.rng.draw_integer(5) as usize;
            let colors = [
                [1.0, 0.0, 0.0],
                [1.0, 0.5, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
            ];
            for target in stable_targets(world.targets(authority.world_key, origin, 350.0)) {
                apply_native_secondary_target_effect(
                    &mut state.target_effects,
                    authority.world_key,
                    target.id,
                    NativeSecondaryTargetEffectPatch {
                        prismatic_ticks: Some((value("mDuration") * 100.0 + 0.5).floor() as u64),
                        ..NativeSecondaryTargetEffectPatch::default()
                    },
                );
            }
            let mut wave = make(Kind::PrismaticWave, Vec2::new(origin.x, origin.y - 25.0));
            wave.alpha = 0.0;
            wave.freeze_ticks = (value("mDuration") * 100.0 + 0.5).floor() as u64;
            wave.lifetime_ticks = 167;
            wave.phase = authority.character.heading_degrees;
            wave.presentation_rng = Some(state.rng.clone());
            wave.radius = 350.0;
            wave.scale = 2.0;
            wave.slow_factor = sign;
            state.spawn_actor(wave);
            let color = colors[color];
            let mut event = cast_event(
                owner,
                skill_id,
                authority,
                tick,
                Event::Cast,
                Some(Cue::PrismaticShock),
            );
            event.screen_flash = Some(screen_flash(color[0], color[1], color[2], 0.05, true));
            state.emit_event(event);
            let mut event = cast_event(
                owner,
                skill_id,
                authority,
                tick,
                Event::Pulse,
                Some(Cue::LightningStart),
            );
            event.pitch = 0.8;
            state.emit_event(event);
        }
        Ability::RingOfIce => {
            let mut wave = make(Kind::FreezeWave, origin);
            wave.freeze_ticks =
                (value("mDamage") * authority.freeze_duration_multiplier * 100.0).trunc() as u64;
            freeze_wave(
                state,
                wave,
                // Ring of Ice's constructor explicitly retains the owner flag.
                // Comet's later wave and Stoneskin use the current global setting.
                authority.enhanced_effects,
                authority.maximum_ring_of_ice,
            );
            let mut event = cast_event(owner, skill_id, authority, tick, Event::Pulse, None);
            event.screen_flash = Some(screen_flash(0.9, 1.0, 1.0, 0.01, true));
            state.emit_event(event);
        }
        Ability::Earthquake => {
            let rotation = state.rng.draw_scaled_float(360.0, false);
            let mut scenery: Vec<_> = world
                .scenery_targets(authority.world_key, origin, 512.0)
                .into_iter()
                .filter(|target| squared_distance(origin, target.position) < 512.0 * 512.0)
                .collect();
            shuffle_fixed_bound(&mut scenery, &mut state.rng);
            let quake_id = state.next_actor_id;
            let mut quake = make(Kind::Earthquake, origin);
            quake.enhanced = authority.enhanced_effects;
            quake.lifetime_ticks = (value("mDuration") * 100.0 + 0.5).floor() as u64;
            quake.phase = -5.0;
            quake.quantity = 2.0;
            quake.radius = 512.0;
            quake.rotation_radians = rotation * std::f64::consts::PI / 180.0;
            state.spawn_actor(quake);
            let mut scenery_ids = Vec::new();
            for target in scenery {
                let existing = state
                    .actors
                    .iter()
                    .find(|actor| {
                        actor.kind == Kind::EarthquakeSceneryWobble
                            && actor.world_key == authority.world_key
                            && actor.target_id == Some(target.id)
                    })
                    .map(|actor| actor.id);
                let id = match existing {
                    Some(id) => id,
                    None => {
                        let mut wobble = make(Kind::EarthquakeSceneryWobble, target.position);
                        wobble.lifetime_ticks = MAX_NATIVE_TICKS;
                        wobble.target_id = Some(target.id);
                        wobble.variant = target.type_id as i32;
                        state.spawn_actor(wobble)
                    }
                };
                scenery_ids.push(id);
            }
            state
                .actors
                .iter_mut()
                .find(|actor| actor.id == quake_id)
                .expect("Earthquake retained its root")
                .hit_target_ids = scenery_ids;
            let mut event = cast_event(owner, skill_id, authority, tick, Event::Pulse, None);
            event.screen_flash = Some(screen_flash(0.8, 1.0, 0.8, 0.025, false));
            state.emit_event(event);
        }
        Ability::RaiseGolem => {
            let cap = if authority.maximum_golem { 2 } else { 1 };
            let mut owned: Vec<_> = state
                .actors
                .iter()
                .filter(|actor| actor.kind == Kind::Golem && actor.owner_id == owner)
                .collect();
            if owned.len() >= cap {
                owned.sort_by(|left, right| {
                    left.golem()
                        .map_or(0.0, |golem| golem.current_health)
                        .total_cmp(&right.golem().map_or(0.0, |golem| golem.current_health))
                        .then(left.id.cmp(&right.id))
                });
                let id = owned[0].id;
                state.actors.retain(|actor| actor.id != id);
            }
            let sign = state.rng.draw_sign(45.0);
            let heading = fround(authority.character.heading_degrees + sign);
            let direction = native_leviathan_heading_vector(heading);
            let requested = Vec2::new(
                fround(origin.x + fround(direction.x * 100.0)),
                fround(origin.y + fround(direction.y * 100.0)),
            );
            let position =
                world.golem_placement(owner, authority.world_key, requested, &mut state.rng);
            result.facing_heading_degrees = Some(heading);
            let pose = state.rng.draw_integer(2) as u32;
            let rotation = normalize_radians((heading + 180.0) * std::f64::consts::PI / 180.0);
            let mut geometry =
                GolemWorld { world, owner, world_key: authority.world_key, origin: position };
            let golem = NativeSecondaryGolemState::new(
                position,
                rotation,
                NativeGolemBirthStats {
                    health: value("mHP"),
                    damage_maximum: value("mDamage2"),
                    iron: authority.golem_iron,
                    reflect_factor: authority.golem_reflect_factor,
                    pose_variant: pose,
                },
                &mut geometry,
            );
            let mut summon = make(Kind::Golem, position);
            summon.damage = value("mDamage1");
            summon.golem = Some(golem);
            summon.lifetime_ticks = MAX_NATIVE_TICKS;
            summon.quantity = value("mHP");
            summon.radius = 30.0;
            summon.rotation_radians = rotation;
            summon.variant = i32::from(authority.golem_iron);
            state.spawn_actor(summon);
        }
        Ability::Stoneskin => {
            state.stoneskin_warp =
                Some(super::motion::create_native_stoneskin_warp(&mut state.rng));
            state.spawn_stoneskin_burst(
                owner,
                origin,
                authority.world_key,
                context.enhanced_effects_for(owner),
            );
            result.player.stoneskin_ticks_remaining = result
                .player
                .stoneskin_ticks_remaining
                .max((value("mDuration") * 100.0 + 0.5).floor() as u64);
            let mut event =
                cast_event(owner, skill_id, authority, tick, Event::Cast, Some(Cue::StoneskinOn));
            event.screen_flash = Some(screen_flash(1.0, 1.0, 1.0, 0.1, false));
            state.emit_event(event);
            state.emit_event(cast_event(
                owner,
                skill_id,
                authority,
                tick,
                Event::Pulse,
                Some(Cue::Stoneskin),
            ));
        }
        Ability::Teleport => {
            let rotation = state.rng.draw_scaled_float(360.0, false);
            let mut burst = make(Kind::TeleportBurst, Vec2::new(origin.x, origin.y - 15.0));
            burst.alpha = 2.0;
            burst.lifetime_ticks = 20;
            burst.rotation_radians = rotation * std::f64::consts::PI / 180.0;
            state.spawn_actor(burst);
            let mut event =
                cast_event(owner, skill_id, authority, tick, Event::Pulse, Some(Cue::Teleport));
            event.screen_flash = Some(screen_flash(1.0, 1.0, 1.0, 0.025, false));
            state.emit_event(event);
            let destination = world.teleport_destination(owner, &mut state.rng);
            result.relocated = Some(destination);
            let rotation = state.rng.draw_scaled_float(360.0, false);
            let mut burst =
                make(Kind::TeleportBurst, Vec2::new(destination.x, destination.y - 15.0));
            burst.alpha = 2.0;
            burst.lifetime_ticks = 20;
            burst.rotation_radians = rotation * std::f64::consts::PI / 180.0;
            burst.scale = 8.0;
            burst.variant = 1;
            state.spawn_actor(burst);
            let mut event =
                cast_event(owner, skill_id, authority, tick, Event::Pulse, Some(Cue::Teleport));
            event.position = destination;
            event.screen_flash = Some(screen_flash(1.0, 1.0, 1.0, 0.025, true));
            state.emit_event(event);
        }
        Ability::MagicCircle => {
            let presentation_rng = state.rng.clone();
            let intensity = magic_circle_visual_tick(&mut state.rng, tick);
            let mut circle = make(Kind::MagicCircle, aim);
            circle.alpha = intensity;
            circle.lifetime_ticks = 1519;
            circle.phase = tick as f64;
            circle.presentation_rng = Some(presentation_rng);
            circle.quantity = authority.mana_recovery_per_tick;
            circle.radius = hypot(210.0, 168.0);
            circle.scale = 4.0;
            circle.slow_factor = value("mSlow") / 100.0;
            state.spawn_actor(circle);
            if inside_rectangle(aim, origin, 210.0, 168.0) {
                result.mana_recovered += authority.mana_recovery_per_tick * 2.0;
                magic_circle_player_flash(
                    state,
                    owner,
                    authority.world_key,
                    Vec2::new(origin.x, origin.y - 15.0),
                    ranked.rank,
                );
            }
            for target in
                stable_targets(world.targets(authority.world_key, aim, hypot(210.0, 168.0)))
            {
                if inside_rectangle(aim, target.position, 210.0, 168.0) {
                    apply_native_secondary_target_effect(
                        &mut state.target_effects,
                        authority.world_key,
                        target.id,
                        NativeSecondaryTargetEffectPatch {
                            circle_slow_factor: Some((1.0 - value("mSlow") / 100.0).max(0.0)),
                            circle_slow_ticks: Some(20),
                            ..NativeSecondaryTargetEffectPatch::default()
                        },
                    );
                }
            }
        }
        Ability::MagicTrap => {
            let selector = magic_trap_selector(authority, &mut state.rng);
            let base = magic_trap_base_damage(authority.skill_book, selector, &mut state.rng);
            let mut trap = make(Kind::MagicTrap, aim);
            trap.damage = fround(base * fround(value("mDamage")));
            trap.lifetime_ticks = MAX_NATIVE_TICKS;
            trap.phase = 3.0;
            trap.quantity = authority.fire_burn_damage;
            trap.radius = 65.0;
            trap.scale = 0.0;
            trap.slow_factor = authority.cold_slow_factor;
            trap.variant = selector as i32;
            state.spawn_actor(trap);
            post_cast_cue = Some(magic_trap_primary_cue(selector));
            let mut event = cast_event(owner, skill_id, authority, tick, Event::Pulse, None);
            event.position = aim;
            event.screen_flash = Some(magic_trap_screen_flash(selector, 0.1, false));
            state.emit_event(event);
        }
        Ability::Dampen => {
            state.rng.draw_integer(100000);
            let candidates = world.dampen_candidates(authority.world_key, origin);
            result.removed_projectile_ids =
                candidates.projectiles.iter().map(|projectile| projectile.id).collect();
            result.dampened_caster_target_ids = candidates.caster_target_ids;
            let mut shields = candidates.shield_target_ids;
            shields.sort_unstable();
            for id in shields {
                if state.rng.draw_integer(100) < 0x33 {
                    result.dispelled_shield_target_ids.push(id);
                }
            }
            result.player.cast_spin_ticks_remaining = 73;
            for projectile in candidates.projectiles {
                let flyout =
                    create_native_dampened_spell(origin, projectile.position, &mut state.rng);
                let mut effect = make(Kind::DampenedProjectile, projectile.position);
                effect.lifetime_ticks = MAX_NATIVE_TICKS;
                effect.phase = flyout.phase_degrees;
                effect.rotation_radians = -std::f64::consts::PI / 180.0;
                effect.target_id = Some(projectile.id);
                effect.variant = dampened_variant(projectile.kind, projectile.payload);
                effect.velocity = flyout.velocity;
                state.spawn_actor(effect);
            }
            let presentation_rng = state.rng.clone();
            advance_rng(&mut state.rng, 2970);
            let mut wave = make(Kind::DampenWave, origin);
            wave.lifetime_ticks = 100;
            wave.presentation_rng = Some(presentation_rng);
            state.spawn_actor(wave);
            state.emit_event(cast_event(
                owner,
                skill_id,
                authority,
                tick,
                Event::Cast,
                Some(Cue::Flash),
            ));
            state.emit_event(cast_event(
                owner,
                skill_id,
                authority,
                tick,
                Event::Pulse,
                Some(Cue::Dampen),
            ));
        }
        Ability::MagicShield => {
            result.player.magic_shield_absorb = value("mAbsorb");
            result.player.magic_shield_explosion_damage = authority.explosive_shield_damage;
            result.player.magic_shield_maximum = value("mAbsorb");
            result.player.magic_shield_pulse_ticks = 0;
            let mut event = cast_event(owner, skill_id, authority, tick, Event::Pulse, None);
            event.screen_flash = Some(screen_flash(0.5, 1.0, 1.0, 0.1, true));
            state.emit_event(event);
        }
        Ability::AcidRain => {
            let phase = state.rng.draw_scaled_float(1.0, false);
            let mut rain = make(Kind::AcidRain, aim);
            rain.alpha = 0.0;
            rain.damage = fround(value("mDamage") / 6.0);
            rain.enhanced = authority.enhanced_effects;
            rain.lifetime_ticks = 3600;
            rain.quantity = 50.0;
            rain.radius = 400.0;
            rain.rotation_radians = phase;
            rain.scale = 0.01_f32 as f64;
            state.spawn_actor(rain);
        }
        Ability::FireWall => {
            let normal = Vec2::new(-direction.y, direction.x);
            for index in 0..11 {
                let offset = -150.0 + f64::from(index) * 30.0;
                let phase = state.rng.draw_scaled_float(32.0, false);
                let mirror = state.rng.draw_sign(1.0);
                let radius = state.rng.draw_scaled_float(10.0, false);
                let jitter = native_unit_vector(&mut state.rng);
                let position = Vec2::new(
                    fround(aim.x + normal.x * offset + jitter.x * radius),
                    fround(aim.y + normal.y * offset + jitter.y * radius),
                );
                let mut fire = make(Kind::FirePatch, position);
                fire.damage = value("mDamage");
                fire.enhanced = true;
                fire.lifetime_ticks = fire_lifetime_ticks(7.0);
                fire.phase = phase;
                fire.quantity = mirror;
                fire.scale =
                    fround(0.8 + 0.6 * float::sin(std::f64::consts::PI * f64::from(index) / 10.0));
                fire.slow_factor = 7.0;
                fire.variant = index;
                state.spawn_actor(fire);
            }
            let mut event =
                cast_event(owner, skill_id, authority, tick, Event::Cast, Some(Cue::Ignite));
            event.position = aim;
            event.screen_flash = Some(screen_flash(1.0, 0.5, 0.0, 0.1, true));
            state.emit_event(event);
            state.emit_event(cast_event(
                owner,
                skill_id,
                authority,
                tick,
                Event::Pulse,
                Some(Cue::FireballHit),
            ));
        }
        Ability::EtherDrain => {
            let rotation = state.rng.draw_scaled_float(360.0, false);
            let mut drain = make(Kind::EtherDrain, aim);
            drain.alpha = 0.0;
            drain.damage = value("mDamage");
            drain.enhanced = authority.enhanced_effects;
            drain.freeze_ticks = 1000;
            drain.lifetime_ticks = 1061;
            drain.radius = 512.0;
            drain.rotation_radians = rotation * std::f64::consts::PI / 180.0;
            drain.scale = 0.0;
            state.spawn_actor(drain);
            state.emit_event(cast_event(
                owner,
                skill_id,
                authority,
                tick,
                Event::Cast,
                Some(Cue::DistortReality),
            ));
            state.emit_event(cast_event(
                owner,
                skill_id,
                authority,
                tick,
                Event::Pulse,
                Some(Cue::LightningStart),
            ));
        }
        Ability::CallComet => {
            let heading = state.rng.draw_scaled_float(1.0, false);
            let mut comet = make(Kind::Comet, aim);
            comet.damage = value("mDamage");
            comet.enhanced = authority.enhanced_effects;
            comet.freeze_ticks =
                (value("mFreeze") * authority.freeze_duration_multiplier * 100.0).trunc() as u64;
            comet.lifetime_ticks = 400;
            comet.quantity = if authority.maximum_ring_of_ice { 1.0 } else { 0.0 };
            comet.radius = 400.0;
            comet.rotation_radians = atan2(3.0, heading);
            comet.scale = 2.0;
            state.spawn_actor(comet);
        }
        Ability::TurnUndead => {
            for target in stable_targets(world.targets(authority.world_key, origin, 250.0)) {
                if !["SKELETON", "SKELETONARCHER", "SKELETONMAGE", "ZOMBIE"]
                    .contains(&target.family.as_str())
                {
                    continue;
                }
                result.target_heading_changes.push(super::NativeSecondaryTargetHeadingChange {
                    degrees: crate::heading::heading_degrees_from_vector(
                        target.position.x - origin.x,
                        target.position.y - origin.y,
                    ),
                    mode: super::NativeSecondaryHeadingMode::Absolute,
                    target_id: target.id,
                });
                apply_native_secondary_target_effect(
                    &mut state.target_effects,
                    authority.world_key,
                    target.id,
                    NativeSecondaryTargetEffectPatch {
                        flee_ticks: Some((value("mFlee") * 100.0 + 0.5).floor() as u64),
                        weaken_factor: Some((1.0 - value("mWeaken") / 100.0).max(0.0)),
                        ..NativeSecondaryTargetEffectPatch::default()
                    },
                );
            }
            let mut heading = state.rng.draw_scaled_float(360.0, false);
            for index in 0..35 {
                let scale = state.rng.draw_scaled_float(0.5, false);
                let mut effect = make(Kind::TurnUndead, origin);
                effect.lifetime_ticks = 20;
                effect.radius = 250.0;
                effect.rotation_radians = heading * std::f64::consts::PI / 180.0;
                effect.scale = 1.0 + scale;
                effect.variant = index;
                state.spawn_actor(effect);
                let increment = state.rng.draw_scaled_float(40.0, false);
                heading = fround(heading + 20.0 + increment);
            }
            for pitch in [2.0, 3.0] {
                let mut event =
                    cast_event(owner, skill_id, authority, tick, Event::Pulse, Some(Cue::LevelUp));
                event.pitch = pitch;
                state.emit_event(event);
            }
        }
        Ability::Mindstar | Ability::Regenerate => {
            let active = if ability == Ability::Mindstar {
                result.player.mindstar = !result.player.mindstar;
                result.player.mindstar
            } else {
                result.player.regenerate = !result.player.regenerate;
                result.player.regenerate
            };
            result.player.reserved_mana =
                result.player.mana_reserve(authority.skill_book, authority.maximum_mana);
            let mut event = cast_event(
                owner,
                skill_id,
                authority,
                tick,
                if active { Event::ToggleOn } else { Event::ToggleOff },
                Some(Cue::Mindstar),
            );
            event.screen_flash = Some(if ability == Ability::Mindstar {
                screen_flash(0.0, 0.5, 1.0, 0.1, true)
            } else {
                screen_flash(1.0, 0.5, 0.0, 0.1, true)
            });
            state.emit_event(event);
            return SecondaryCastResult::none(result.player, false);
        }
    }
    result.player.begin_cooldown(
        ability,
        capacity,
        authority.focus_instant_recharge_chance_percent,
        &mut state.rng,
    );
    result.player.start_cast(authority.weapon_kind);
    state.emit_event(cast_event(owner, skill_id, authority, tick, Event::Cast, cast_cue(ability)));
    if let Some(cue) = post_cast_cue {
        state.emit_event(cast_event(owner, skill_id, authority, tick, Event::Pulse, Some(cue)));
    }
    result
}

fn cast_cue(ability: Ability) -> Option<Cue> {
    match ability {
        Ability::CallLeviathan => Some(Cue::LeviathanRoar),
        Ability::Planewalker => Some(Cue::PlanewalkerOn),
        Ability::Phasing => Some(Cue::Phase),
        Ability::Firewalker => Some(Cue::Ignite),
        Ability::MagicStorm | Ability::AcidRain => Some(Cue::MagicStorm),
        Ability::RingOfIce => Some(Cue::RingOfIce),
        Ability::MagicTrap => Some(Cue::SetTrap),
        Ability::MagicShield => Some(Cue::MagicShieldUp),
        Ability::Mindstar | Ability::Regenerate => Some(Cue::Mindstar),
        _ => None,
    }
}
pub(crate) fn magic_trap_screen_flash(
    selector: usize,
    decay: f64,
    point: bool,
) -> super::NativeSecondaryScreenFlashState {
    let colors = [
        [1.0, 0.1, 1.0],
        [1.0, 0.35, 0.1],
        [0.1, 1.0, 1.0],
        [0.1, 0.5, 1.0],
        [0.1, 1.0, 0.1],
        [1.0, 0.5, 0.1],
        [0.1, 0.5, 0.5],
        [0.75, 0.75, 0.75],
        [1.0, 1.0, 1.0],
    ];
    let color = colors[selector];
    screen_flash(color[0], color[1], color[2], decay, point)
}
fn magic_trap_primary_cue(selector: usize) -> Cue {
    [Cue::MagicMissile, Cue::ThrowFire, Cue::LightningStart, Cue::IceStart, Cue::StartBoulder]
        [selector]
}
fn magic_trap_selector(
    authority: &NativeSecondaryPlayerAuthority<'_>,
    rng: &mut NativeRng,
) -> usize {
    let ids = [8, 16, 24, 32, 40];
    let primary = if authority.skill_book.primary_skill_id == 52 {
        let build = native_weld_build(
            authority.skill_book.weld_build_id.expect("selected Weld has its build"),
        )
        .expect("selected Weld build exists");
        build.primary_skill_ids[rng.draw_integer(2) as usize]
    } else {
        authority.skill_book.primary_skill_id
    };
    ids.iter()
        .position(|id| *id == primary)
        .expect("selected primary has a native Magic Trap selector")
}
fn magic_trap_base_damage(
    book: &crate::profile::NativeSkillBookState,
    selector: usize,
    rng: &mut NativeRng,
) -> f64 {
    let id = [8, 16, 24, 32, 40][selector];
    let rank = book.effective_ranks[id];
    assert!(
        rank > 0 && rank <= NATIVE_SKILL_CATALOG[id].maximum_level,
        "trap component has a valid primary rank"
    );
    let shared = native_skill_stat(id, "mDamage", rank);
    let minimum =
        shared.or_else(|| native_skill_stat(id, "mDamage1", rank)).expect("primary minimum exists");
    let maximum =
        shared.or_else(|| native_skill_stat(id, "mDamage2", rank)).expect("primary maximum exists");
    assert!(minimum > 0.0 && maximum >= minimum, "trap primary damage range is valid");
    if selector == 0 { rng.draw_float_range(minimum, maximum) } else { fround(maximum) }
}
fn dampened_variant(
    kind: NativeSecondaryDampenProjectileKind,
    payload: NativeSecondaryDampenPayload,
) -> i32 {
    if matches!(
        kind,
        NativeSecondaryDampenProjectileKind::SkullMissile
            | NativeSecondaryDampenProjectileKind::DarkFireball
    ) {
        3
    } else if kind == NativeSecondaryDampenProjectileKind::Firebolt {
        0
    } else if payload == NativeSecondaryDampenPayload::Poison {
        1
    } else {
        2
    }
}
