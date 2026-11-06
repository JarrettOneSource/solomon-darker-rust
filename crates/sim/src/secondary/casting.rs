//! Every stock secondary spell's constructor, admission and cooldown path, in RNG draw
//! order.

use super::context::*;
use super::dampened::create_dampened_spell;
use super::golem::{GolemBirthStats, GolemTerrain, SecondaryGolemState};
use super::leviathan::*;
use super::program::*;
use super::{
    SecondaryAbilityId as Ability, SecondaryAbilityRankStats, SecondaryActorKind as Kind,
    SecondaryActorState, SecondaryAudioCue as Cue, SecondaryEventKind as Event,
    SecondaryEventState, SecondaryManaCostBonuses, SecondaryPlayerState, SecondarySimulationState,
    SecondaryTargetEffectPatch, apply_secondary_target_effect,
    resolved_secondary_ability_rank_stats, secondary_ability_mana_cost,
    secondary_cooldown_capacity_ticks,
};
use crate::float;
use crate::float::{atan2, fround, hypot};
use crate::heading::heading_vector;
use crate::profile::{SKILL_CATALOG, skill_stat, weld_build};
use crate::{Rng, Vec2};

/// The outcome of one secondary cast: mana spent or recovered, the caster's updated
/// state, and its effects on targets, projectiles and the caster's position.
pub(crate) struct SecondaryCastResult {
    pub target_heading_changes: Vec<super::SecondaryTargetHeadingChange>,
    pub dispelled_shield_target_ids: Vec<usize>,
    pub dampened_caster_target_ids: Vec<usize>,
    pub mana_recovered: f64,
    pub mana_underflow: bool,
    pub mana_spent: f64,
    pub player: SecondaryPlayerState,
    pub facing_heading_degrees: Option<f64>,
    pub relocated: Option<Vec2>,
    pub removed_projectile_ids: Vec<usize>,
}
impl SecondaryCastResult {
    fn none(player: SecondaryPlayerState, mana_underflow: bool) -> Self {
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

/// A Golem's view of the secondary world: movement and foot placement for one owner.
pub(crate) struct GolemWorld<'a, W> {
    pub world: &'a W,
    pub owner: &'a str,
    pub world_key: &'a str,
    pub origin: Vec2,
}
impl<W: SecondaryWorld> GolemTerrain for GolemWorld<'_, W> {
    fn resolve_movement(&mut self, requested: Vec2) -> Vec2 {
        self.world.golem_movement(self.owner, self.world_key, self.origin, requested, 30.0)
    }
    fn resolve_foot_target(&mut self, current: Vec2, requested: Vec2) -> Vec2 {
        self.world.golem_foot_placement(self.owner, self.world_key, current, requested)
    }
}

pub(crate) fn cast_ability(
    state: &mut SecondarySimulationState,
    mut player: SecondaryPlayerState,
    owner: &str,
    ability: Ability,
    context: &SecondaryTickContext<'_>,
    world: &impl SecondaryWorld,
) -> SecondaryCastResult {
    let authority = &context.players[owner];
    let skill_id = ability.skill_id();
    if !authority.eligible {
        return fizzle(state, player, owner, ability, context, false);
    }
    if player.cast_action.is_some()
        || player.cast_spin_ticks_remaining > 0
        || player.global_cooldown_ticks > 0.0
    {
        return SecondaryCastResult::none(player, false);
    }
    if player.cooldown_ticks_by_skill[skill_id] > 0.0 {
        return fizzle(state, player, owner, ability, context, false);
    }
    let cast = Cast::new(owner, authority, ability, context.tick);
    let toggling_off = ability == Ability::Planewalker && player.planewalker_ticks_remaining > 0
        || ability == Ability::Firewalker && player.firewalker
        || ability == Ability::Mindstar && player.mindstar
        || ability == Ability::Regenerate && player.regenerate;
    if !toggling_off && cast.cost > authority.current_mana {
        return fizzle(state, player, owner, ability, context, true);
    }
    player.cast_sequence += 1;
    player.last_skill_id = Some(ability);
    let mut result = SecondaryCastResult::none(player, false);
    result.mana_spent = cast.cost;
    let mut post_cast_cue = None;
    // Toggles, ending Planewalker and a blocked Phasing finish their own cast here; every
    // other ability continues to the shared cooldown and cast event below.
    match ability {
        Ability::CallLeviathan => cast_call_leviathan(state, &cast),
        Ability::Planewalker if result.player.planewalker_ticks_remaining > 0 => {
            return end_planewalker(state, &cast, result.player);
        }
        Ability::Planewalker => cast_planewalker(state, &cast, &mut result.player),
        Ability::Phasing => {
            if !cast_phasing(state, &cast, world, &mut result) {
                return result;
            }
        }
        Ability::RingOfFire => cast_ring_of_fire(state, &cast),
        Ability::Firewalker => return toggle_firewalker(state, &cast, result.player),
        Ability::MagicStorm => cast_magic_storm(state, &cast),
        Ability::PrismaticShock => cast_prismatic_shock(state, &cast, world),
        Ability::RingOfIce => cast_ring_of_ice(state, &cast),
        Ability::Earthquake => cast_earthquake(state, &cast, world),
        Ability::RaiseGolem => cast_raise_golem(state, &cast, world, &mut result),
        Ability::Stoneskin => cast_stoneskin(state, &cast, context, &mut result.player),
        Ability::Teleport => cast_teleport(state, &cast, world, &mut result),
        Ability::MagicCircle => cast_magic_circle(state, &cast, world, &mut result),
        Ability::MagicTrap => post_cast_cue = Some(cast_magic_trap(state, &cast)),
        Ability::Dampen => cast_dampen(state, &cast, world, &mut result),
        Ability::MagicShield => cast_magic_shield(state, &cast, &mut result.player),
        Ability::AcidRain => cast_acid_rain(state, &cast),
        Ability::FireWall => cast_fire_wall(state, &cast),
        Ability::EtherDrain => cast_ether_drain(state, &cast),
        Ability::CallComet => cast_call_comet(state, &cast),
        Ability::TurnUndead => cast_turn_undead(state, &cast, world, &mut result),
        Ability::Mindstar | Ability::Regenerate => {
            return toggle_mindstar_or_regenerate(state, &cast, result.player);
        }
    }
    cast.begin_cooldown(&mut result.player, &mut state.rng);
    result.player.start_cast(authority.weapon_kind);
    state.emit_event(cast.event(Event::Cast, cast_cue(ability)));
    if let Some(cue) = post_cast_cue {
        state.emit_event(cast.event(Event::Pulse, Some(cue)));
    }
    result
}

/// A refused cast: the fizzle event and sound, the fizzle count and the refused skill.
fn fizzle(
    state: &mut SecondarySimulationState,
    mut player: SecondaryPlayerState,
    owner: &str,
    ability: Ability,
    context: &SecondaryTickContext<'_>,
    mana_underflow: bool,
) -> SecondaryCastResult {
    let authority = &context.players[owner];
    state.emit_event(cast_event(
        owner,
        ability.skill_id(),
        authority,
        context.tick,
        Event::Fizzle,
        Some(Cue::Fizzle),
    ));
    player.fizzle_sequence += 1;
    player.last_skill_id = Some(ability);
    SecondaryCastResult::none(player, mana_underflow)
}

/// What every constructor of one admitted cast reads: the caster, the ability's rank stats,
/// mana cost and cooldown, and where the cast starts and aims.
struct Cast<'a> {
    owner: &'a str,
    authority: &'a SecondaryPlayerAuthority<'a>,
    tick: u64,
    ability: Ability,
    skill_id: usize,
    ranked: SecondaryAbilityRankStats,
    cost: f64,
    capacity: f64,
    origin: Vec2,
    aim: Vec2,
    direction: Vec2,
}

impl<'a> Cast<'a> {
    fn new(
        owner: &'a str,
        authority: &'a SecondaryPlayerAuthority<'a>,
        ability: Ability,
        tick: u64,
    ) -> Self {
        let ranked = resolved_secondary_ability_rank_stats(
            authority.skill_book,
            ability,
            &authority.offensive_factors,
        );
        let cost = secondary_ability_mana_cost(
            authority.skill_book,
            ability,
            &authority.offensive_factors,
            SecondaryManaCostBonuses {
                explosive_shield: authority.explosive_shield_raw_mana_cost,
                golem: authority.golem_raw_mana_cost,
                magic_storm: authority.magic_storm_raw_mana_cost,
            },
        );
        let capacity = secondary_cooldown_capacity_ticks(authority.skill_book, ability);
        let origin = authority.character.position;
        let aim = authority.input.aim.unwrap_or(origin);
        let direction = unit(origin, aim);
        Self {
            owner,
            authority,
            tick,
            ability,
            skill_id: ability.skill_id(),
            ranked,
            cost,
            capacity,
            origin,
            aim,
            direction,
        }
    }

    fn value(&self, name: &str) -> f64 {
        self.ranked.value(name).expect("secondary constructor has its authored property")
    }

    /// A new actor owned by the caster, carrying this skill and rank.
    fn actor(&self, kind: Kind, position: Vec2) -> SecondaryActorState {
        actor(
            kind,
            self.owner,
            self.authority.world_key,
            Some(self.skill_id),
            self.ranked.rank,
            position,
        )
    }

    /// An event of this cast at the caster's position.
    fn event(&self, kind: Event, cue: Option<Cue>) -> SecondaryEventState {
        cast_event(self.owner, self.skill_id, self.authority, self.tick, kind, cue)
    }

    fn begin_cooldown(&self, player: &mut SecondaryPlayerState, rng: &mut Rng) {
        player.begin_cooldown(
            self.ability,
            self.capacity,
            self.authority.focus_instant_recharge_chance_percent,
            rng,
        );
    }
}

fn cast_call_leviathan(state: &mut SecondarySimulationState, cast: &Cast<'_>) {
    let birth = create_leviathan_birth(
        &mut state.rng,
        cast.value("mQuantity"),
        cast.authority.maximum_leviathan,
    );
    let damage = cast.value("mDamage");
    let parent_id = state.next_actor_id;
    let mut parent = cast.actor(Kind::Leviathan, cast.aim);
    parent.damage = damage;
    parent.enhanced = cast.authority.enhanced_effects;
    parent.lifetime_ticks = LEVIATHAN_LIFETIME_TICKS;
    parent.quantity = birth.quantity as f64;
    parent.radius = fround(birth.maximum_scale * 30.0);
    parent.rotation_radians = birth.rotation_radians;
    parent.scale = 0.0;
    parent.slow_factor = birth.maximum_scale;
    state.spawn_actor(parent);
    for (index, appendage) in birth.appendages.into_iter().enumerate() {
        let mut child = cast.actor(Kind::LeviathanAppendage, cast.aim);
        child.damage = damage;
        child.endpoint = appendage.base_offset;
        child.frame =
            (leviathan_heading_vector(appendage.heading_degrees).y * 100.0 + 0.5).floor() + 100.0;
        child.hit_target_ids = vec![parent_id];
        child.lifetime_ticks = LEVIATHAN_LIFETIME_TICKS;
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

/// Casting Planewalker while it is active ends it early: no mana is spent, and the
/// cooldown starts.
fn end_planewalker(
    state: &mut SecondarySimulationState,
    cast: &Cast<'_>,
    mut player: SecondaryPlayerState,
) -> SecondaryCastResult {
    player.planewalker_ticks_remaining = 0;
    player.plane_orb_held = false;
    state.emit_event(cast.event(Event::ToggleOff, Some(Cue::PlanewalkerOff)));
    player.start_cast(cast.authority.weapon_kind);
    cast.begin_cooldown(&mut player, &mut state.rng);
    SecondaryCastResult::none(player, false)
}

fn cast_planewalker(
    state: &mut SecondarySimulationState,
    cast: &Cast<'_>,
    player: &mut SecondaryPlayerState,
) {
    player.planewalker_ticks_remaining = player
        .planewalker_ticks_remaining
        .max((cast.value("mDuration") * 100.0 + 0.5).floor() as u64);
    let mut event = cast.event(Event::Pulse, None);
    event.screen_flash = Some(screen_flash(1.0, 0.0, 1.0, 0.1, false));
    state.emit_event(event);
}

/// Returns false when the world has no destination along the caster's heading. That cast
/// still spends its mana and cools down, but has no burst or cast event.
fn cast_phasing(
    state: &mut SecondarySimulationState,
    cast: &Cast<'_>,
    world: &impl SecondaryWorld,
    result: &mut SecondaryCastResult,
) -> bool {
    let direction = heading_vector(cast.authority.character.heading_index);
    result.relocated = world.phasing_destination(cast.owner, cast.origin, direction);
    if result.relocated.is_none() {
        result.player.start_cast(cast.authority.weapon_kind);
        cast.begin_cooldown(&mut result.player, &mut state.rng);
        return false;
    }
    let marker = Vec2::new(
        fround(cast.origin.x + direction.x * 10.0),
        fround(cast.origin.y + direction.y * 10.0),
    );
    let mut phase = cast.actor(Kind::PhaseBurst, marker);
    phase.alpha = 2.0;
    phase.lifetime_ticks = 20;
    phase.rotation_radians = atan2(direction.y, direction.x);
    phase.scale = 2.0;
    state.spawn_actor(phase);
    let mut event = cast.event(Event::Pulse, None);
    event.position = marker;
    state.emit_event(event);
    true
}

fn cast_ring_of_fire(state: &mut SecondarySimulationState, cast: &Cast<'_>) {
    for index in 0..30 {
        let phase = state.rng.draw_scaled_float(32.0, false);
        let mirror = state.rng.draw_sign(1.0);
        let jitter = state.rng.draw_scaled_float(2.0, true);
        let radial = state.rng.draw_scaled_float(30.0, false);
        let radial_direction = unit_vector(&mut state.rng);
        let speed = state.rng.draw_scaled_float(0.025, false);
        let radians = (f64::from(index) * 12.0 + jitter) * std::f64::consts::PI / 180.0;
        let heading = Vec2::new(float::sin(radians), -float::cos(radians));
        let position = Vec2::new(
            fround(cast.origin.x + heading.x * 25.0 + radial_direction.x * radial),
            fround(cast.origin.y + heading.y * 25.0 + radial_direction.y * radial * 0.8),
        );
        let mut fire = cast.actor(Kind::MovingFire, position);
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
    let mut wave = cast.actor(Kind::Shockwave, cast.origin);
    wave.damage = cast.value("mDamage");
    wave.lifetime_ticks = 116;
    wave.phase = 1.155_f32 as f64;
    wave.quantity = 6.0;
    wave.radius = 75.0;
    wave.slow_factor = 0.12375_f32 as f64;
    wave.variant = i32::from(cast.authority.maximum_ring_of_fire);
    state.spawn_actor(wave);
    let mut event = cast.event(Event::Cast, Some(Cue::BigFire));
    event.camera_magnitude = 0.25;
    event.screen_flash = Some(screen_flash(1.0, 0.5, 0.0, 0.01, true));
    state.emit_event(event);
    state.emit_event(cast.event(Event::Pulse, Some(Cue::Nuke)));
}

/// Firewalker reserves mana instead of spending it. Switching it on lays the first fire
/// patch and starts the cooldown.
fn toggle_firewalker(
    state: &mut SecondarySimulationState,
    cast: &Cast<'_>,
    mut player: SecondaryPlayerState,
) -> SecondaryCastResult {
    player.firewalker = !player.firewalker;
    player.reserved_mana =
        player.mana_reserve(cast.authority.skill_book, cast.authority.maximum_mana);
    if player.firewalker {
        firewalker_patch(state, cast.owner, cast.authority, true);
        player.start_cast(cast.authority.weapon_kind);
        cast.begin_cooldown(&mut player, &mut state.rng);
    }
    let mut event = cast.event(
        if player.firewalker { Event::ToggleOn } else { Event::ToggleOff },
        if player.firewalker { Some(Cue::Ignite) } else { None },
    );
    event.screen_flash = Some(screen_flash(1.0, 0.5, 0.0, 0.1, true));
    state.emit_event(event);
    SecondaryCastResult::none(player, false)
}

fn cast_magic_storm(state: &mut SecondarySimulationState, cast: &Cast<'_>) {
    let ticks = if cast.authority.maximum_magic_storm { 2000 } else { 1000 }
        + cast.authority.magic_storm_duration_bonus_ticks;
    let moving = cast.authority.skill_book.effective_ranks[28] > 0;
    let presentation_rng = state.rng.clone();
    state.rng.draw_scaled_float(1.0, true);
    for _ in 0..15 {
        state.rng.draw_scaled_float(360.0, false);
        state.rng.draw_scaled_float(2.0, false);
    }
    let heading = if moving { state.rng.draw_scaled_float(360.0, false) } else { 0.0 };
    let mut storm = cast.actor(Kind::StormCloud, cast.aim);
    storm.alpha = 0.0;
    storm.damage = cast.value("mDamage1");
    storm.enhanced = cast.authority.enhanced_effects;
    storm.freeze_ticks = ticks;
    storm.lifetime_ticks = ticks + 101;
    storm.phase = cast.value("mDamage2");
    storm.presentation_rng = Some(presentation_rng);
    storm.quantity = 50.0;
    storm.radius = 500.0;
    storm.rotation_radians = heading * std::f64::consts::PI / 180.0;
    storm.slow_factor = cast.authority.magic_storm_frequency_factor;
    storm.scale = 0.01_f32 as f64;
    storm.variant = i32::from(moving);
    state.spawn_actor(storm);
}

fn cast_prismatic_shock(
    state: &mut SecondarySimulationState,
    cast: &Cast<'_>,
    world: &impl SecondaryWorld,
) {
    let sign = state.rng.draw_sign(1.0);
    let color = state.rng.draw_integer(5) as usize;
    let colors =
        [[1.0, 0.0, 0.0], [1.0, 0.5, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 1.0]];
    for target in stable_targets(world.targets(cast.authority.world_key, cast.origin, 350.0)) {
        apply_secondary_target_effect(
            &mut state.target_effects,
            cast.authority.world_key,
            target.id,
            SecondaryTargetEffectPatch {
                prismatic_ticks: Some((cast.value("mDuration") * 100.0 + 0.5).floor() as u64),
                ..SecondaryTargetEffectPatch::default()
            },
        );
    }
    let mut wave = cast.actor(Kind::PrismaticWave, Vec2::new(cast.origin.x, cast.origin.y - 25.0));
    wave.alpha = 0.0;
    wave.freeze_ticks = (cast.value("mDuration") * 100.0 + 0.5).floor() as u64;
    wave.lifetime_ticks = 167;
    wave.phase = cast.authority.character.heading_degrees;
    wave.presentation_rng = Some(state.rng.clone());
    wave.radius = 350.0;
    wave.scale = 2.0;
    wave.slow_factor = sign;
    state.spawn_actor(wave);
    let color = colors[color];
    let mut event = cast.event(Event::Cast, Some(Cue::PrismaticShock));
    event.screen_flash = Some(screen_flash(color[0], color[1], color[2], 0.05, true));
    state.emit_event(event);
    let mut event = cast.event(Event::Pulse, Some(Cue::LightningStart));
    event.pitch = 0.8;
    state.emit_event(event);
}

fn cast_ring_of_ice(state: &mut SecondarySimulationState, cast: &Cast<'_>) {
    let mut wave = cast.actor(Kind::FreezeWave, cast.origin);
    wave.freeze_ticks =
        (cast.value("mDamage") * cast.authority.freeze_duration_multiplier * 100.0).trunc() as u64;
    freeze_wave(
        state,
        wave,
        // Ring of Ice's constructor explicitly retains the owner flag.
        // Comet's later wave and Stoneskin use the current global setting.
        cast.authority.enhanced_effects,
        cast.authority.maximum_ring_of_ice,
    );
    let mut event = cast.event(Event::Pulse, None);
    event.screen_flash = Some(screen_flash(0.9, 1.0, 1.0, 0.01, true));
    state.emit_event(event);
}

fn cast_earthquake(
    state: &mut SecondarySimulationState,
    cast: &Cast<'_>,
    world: &impl SecondaryWorld,
) {
    let rotation = state.rng.draw_scaled_float(360.0, false);
    let mut scenery: Vec<_> = world
        .scenery_targets(cast.authority.world_key, cast.origin, 512.0)
        .into_iter()
        .filter(|target| squared_distance(cast.origin, target.position) < 512.0 * 512.0)
        .collect();
    shuffle_fixed_bound(&mut scenery, &mut state.rng);
    let quake_id = state.next_actor_id;
    let mut quake = cast.actor(Kind::Earthquake, cast.origin);
    quake.enhanced = cast.authority.enhanced_effects;
    quake.lifetime_ticks = (cast.value("mDuration") * 100.0 + 0.5).floor() as u64;
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
                    && actor.world_key == cast.authority.world_key
                    && actor.target_id == Some(target.id)
            })
            .map(|actor| actor.id);
        let id = match existing {
            Some(id) => id,
            None => {
                let mut wobble = cast.actor(Kind::EarthquakeSceneryWobble, target.position);
                wobble.lifetime_ticks = MAX_TICKS;
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
    let mut event = cast.event(Event::Pulse, None);
    event.screen_flash = Some(screen_flash(0.8, 1.0, 0.8, 0.025, false));
    state.emit_event(event);
}

fn cast_raise_golem(
    state: &mut SecondarySimulationState,
    cast: &Cast<'_>,
    world: &impl SecondaryWorld,
    result: &mut SecondaryCastResult,
) {
    let cap = if cast.authority.maximum_golem { 2 } else { 1 };
    let mut owned: Vec<_> = state
        .actors
        .iter()
        .filter(|actor| actor.kind == Kind::Golem && actor.owner_id == cast.owner)
        .collect();
    if owned.len() >= cap {
        owned.sort_by(|left, right| {
            left.golem
                .as_ref()
                .map_or(0.0, |golem| golem.current_health)
                .total_cmp(&right.golem.as_ref().map_or(0.0, |golem| golem.current_health))
                .then(left.id.cmp(&right.id))
        });
        let id = owned[0].id;
        state.actors.retain(|actor| actor.id != id);
    }
    let sign = state.rng.draw_sign(45.0);
    let heading = fround(cast.authority.character.heading_degrees + sign);
    let direction = leviathan_heading_vector(heading);
    let requested = Vec2::new(
        fround(cast.origin.x + fround(direction.x * 100.0)),
        fround(cast.origin.y + fround(direction.y * 100.0)),
    );
    let position =
        world.golem_placement(cast.owner, cast.authority.world_key, requested, &mut state.rng);
    result.facing_heading_degrees = Some(heading);
    let pose = state.rng.draw_integer(2) as u32;
    let rotation = normalize_radians((heading + 180.0) * std::f64::consts::PI / 180.0);
    let mut geometry = GolemWorld {
        world,
        owner: cast.owner,
        world_key: cast.authority.world_key,
        origin: position,
    };
    let golem = SecondaryGolemState::new(
        position,
        rotation,
        GolemBirthStats {
            health: cast.value("mHP"),
            damage_maximum: cast.value("mDamage2"),
            iron: cast.authority.golem_iron,
            reflect_factor: cast.authority.golem_reflect_factor,
            pose_variant: pose,
        },
        &mut geometry,
    );
    let mut summon = cast.actor(Kind::Golem, position);
    summon.damage = cast.value("mDamage1");
    summon.golem = Some(golem);
    summon.lifetime_ticks = MAX_TICKS;
    summon.quantity = cast.value("mHP");
    summon.radius = 30.0;
    summon.rotation_radians = rotation;
    summon.variant = i32::from(cast.authority.golem_iron);
    state.spawn_actor(summon);
}

fn cast_stoneskin(
    state: &mut SecondarySimulationState,
    cast: &Cast<'_>,
    context: &SecondaryTickContext<'_>,
    player: &mut SecondaryPlayerState,
) {
    state.stoneskin_warp = Some(super::motion::create_stoneskin_warp(&mut state.rng));
    state.spawn_stoneskin_burst(
        cast.owner,
        cast.origin,
        cast.authority.world_key,
        context.enhanced_effects_for(cast.owner),
    );
    player.stoneskin_ticks_remaining = player
        .stoneskin_ticks_remaining
        .max((cast.value("mDuration") * 100.0 + 0.5).floor() as u64);
    let mut event = cast.event(Event::Cast, Some(Cue::StoneskinOn));
    event.screen_flash = Some(screen_flash(1.0, 1.0, 1.0, 0.1, false));
    state.emit_event(event);
    state.emit_event(cast.event(Event::Pulse, Some(Cue::Stoneskin)));
}

fn cast_teleport(
    state: &mut SecondarySimulationState,
    cast: &Cast<'_>,
    world: &impl SecondaryWorld,
    result: &mut SecondaryCastResult,
) {
    let rotation = state.rng.draw_scaled_float(360.0, false);
    let mut burst = cast.actor(Kind::TeleportBurst, Vec2::new(cast.origin.x, cast.origin.y - 15.0));
    burst.alpha = 2.0;
    burst.lifetime_ticks = 20;
    burst.rotation_radians = rotation * std::f64::consts::PI / 180.0;
    state.spawn_actor(burst);
    let mut event = cast.event(Event::Pulse, Some(Cue::Teleport));
    event.screen_flash = Some(screen_flash(1.0, 1.0, 1.0, 0.025, false));
    state.emit_event(event);
    let destination = world.teleport_destination(cast.owner, &mut state.rng);
    result.relocated = Some(destination);
    let rotation = state.rng.draw_scaled_float(360.0, false);
    let mut burst = cast.actor(Kind::TeleportBurst, Vec2::new(destination.x, destination.y - 15.0));
    burst.alpha = 2.0;
    burst.lifetime_ticks = 20;
    burst.rotation_radians = rotation * std::f64::consts::PI / 180.0;
    burst.scale = 8.0;
    burst.variant = 1;
    state.spawn_actor(burst);
    let mut event = cast.event(Event::Pulse, Some(Cue::Teleport));
    event.position = destination;
    event.screen_flash = Some(screen_flash(1.0, 1.0, 1.0, 0.025, true));
    state.emit_event(event);
}

fn cast_magic_circle(
    state: &mut SecondarySimulationState,
    cast: &Cast<'_>,
    world: &impl SecondaryWorld,
    result: &mut SecondaryCastResult,
) {
    let presentation_rng = state.rng.clone();
    let intensity = magic_circle_visual_tick(&mut state.rng, cast.tick);
    let mut circle = cast.actor(Kind::MagicCircle, cast.aim);
    circle.alpha = intensity;
    circle.lifetime_ticks = 1519;
    circle.phase = cast.tick as f64;
    circle.presentation_rng = Some(presentation_rng);
    circle.quantity = cast.authority.mana_recovery_per_tick;
    circle.radius = hypot(210.0, 168.0);
    circle.scale = 4.0;
    circle.slow_factor = cast.value("mSlow") / 100.0;
    state.spawn_actor(circle);
    if inside_rectangle(cast.aim, cast.origin, 210.0, 168.0) {
        result.mana_recovered += cast.authority.mana_recovery_per_tick * 2.0;
        magic_circle_player_flash(
            state,
            cast.owner,
            cast.authority.world_key,
            Vec2::new(cast.origin.x, cast.origin.y - 15.0),
            cast.ranked.rank,
        );
    }
    for target in
        stable_targets(world.targets(cast.authority.world_key, cast.aim, hypot(210.0, 168.0)))
    {
        if inside_rectangle(cast.aim, target.position, 210.0, 168.0) {
            apply_secondary_target_effect(
                &mut state.target_effects,
                cast.authority.world_key,
                target.id,
                SecondaryTargetEffectPatch {
                    circle_slow_factor: Some((1.0 - cast.value("mSlow") / 100.0).max(0.0)),
                    circle_slow_ticks: Some(20),
                    ..SecondaryTargetEffectPatch::default()
                },
            );
        }
    }
}

/// Returns the selected primary's cue, which sounds after the cast event.
fn cast_magic_trap(state: &mut SecondarySimulationState, cast: &Cast<'_>) -> Cue {
    let selector = magic_trap_selector(cast.authority, &mut state.rng);
    let base = magic_trap_base_damage(cast.authority.skill_book, selector, &mut state.rng);
    let mut trap = cast.actor(Kind::MagicTrap, cast.aim);
    trap.damage = fround(base * fround(cast.value("mDamage")));
    trap.lifetime_ticks = MAX_TICKS;
    trap.phase = 3.0;
    trap.quantity = cast.authority.fire_burn_damage;
    trap.radius = 65.0;
    trap.scale = 0.0;
    trap.slow_factor = cast.authority.cold_slow_factor;
    trap.variant = selector as i32;
    state.spawn_actor(trap);
    let post_cast_cue = magic_trap_primary_cue(selector);
    let mut event = cast.event(Event::Pulse, None);
    event.position = cast.aim;
    event.screen_flash = Some(magic_trap_screen_flash(selector, 0.1, false));
    state.emit_event(event);
    post_cast_cue
}

fn cast_dampen(
    state: &mut SecondarySimulationState,
    cast: &Cast<'_>,
    world: &impl SecondaryWorld,
    result: &mut SecondaryCastResult,
) {
    state.rng.draw_integer(100000);
    let candidates = world.dampen_candidates(cast.authority.world_key, cast.origin);
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
        let flyout = create_dampened_spell(cast.origin, projectile.position, &mut state.rng);
        let mut effect = cast.actor(Kind::DampenedProjectile, projectile.position);
        effect.lifetime_ticks = MAX_TICKS;
        effect.phase = flyout.phase_degrees;
        effect.rotation_radians = -std::f64::consts::PI / 180.0;
        effect.target_id = Some(projectile.id);
        effect.variant = dampened_variant(projectile.kind, projectile.payload);
        effect.velocity = flyout.velocity;
        state.spawn_actor(effect);
    }
    let presentation_rng = state.rng.clone();
    advance_rng(&mut state.rng, 2970);
    let mut wave = cast.actor(Kind::DampenWave, cast.origin);
    wave.lifetime_ticks = 100;
    wave.presentation_rng = Some(presentation_rng);
    state.spawn_actor(wave);
    state.emit_event(cast.event(Event::Cast, Some(Cue::Flash)));
    state.emit_event(cast.event(Event::Pulse, Some(Cue::Dampen)));
}

fn cast_magic_shield(
    state: &mut SecondarySimulationState,
    cast: &Cast<'_>,
    player: &mut SecondaryPlayerState,
) {
    player.magic_shield_absorb = cast.value("mAbsorb");
    player.magic_shield_explosion_damage = cast.authority.explosive_shield_damage;
    player.magic_shield_maximum = cast.value("mAbsorb");
    player.magic_shield_pulse_ticks = 0;
    let mut event = cast.event(Event::Pulse, None);
    event.screen_flash = Some(screen_flash(0.5, 1.0, 1.0, 0.1, true));
    state.emit_event(event);
}

fn cast_acid_rain(state: &mut SecondarySimulationState, cast: &Cast<'_>) {
    let phase = state.rng.draw_scaled_float(1.0, false);
    let mut rain = cast.actor(Kind::AcidRain, cast.aim);
    rain.alpha = 0.0;
    rain.damage = fround(cast.value("mDamage") / 6.0);
    rain.enhanced = cast.authority.enhanced_effects;
    rain.lifetime_ticks = 3600;
    rain.quantity = 50.0;
    rain.radius = 400.0;
    rain.rotation_radians = phase;
    rain.scale = 0.01_f32 as f64;
    state.spawn_actor(rain);
}

fn cast_fire_wall(state: &mut SecondarySimulationState, cast: &Cast<'_>) {
    let normal = Vec2::new(-cast.direction.y, cast.direction.x);
    for index in 0..11 {
        let offset = -150.0 + f64::from(index) * 30.0;
        let phase = state.rng.draw_scaled_float(32.0, false);
        let mirror = state.rng.draw_sign(1.0);
        let radius = state.rng.draw_scaled_float(10.0, false);
        let jitter = unit_vector(&mut state.rng);
        let position = Vec2::new(
            fround(cast.aim.x + normal.x * offset + jitter.x * radius),
            fround(cast.aim.y + normal.y * offset + jitter.y * radius),
        );
        let mut fire = cast.actor(Kind::FirePatch, position);
        fire.damage = cast.value("mDamage");
        fire.enhanced = true;
        fire.lifetime_ticks = fire_lifetime_ticks(7.0);
        fire.phase = phase;
        fire.quantity = mirror;
        fire.scale = fround(0.8 + 0.6 * float::sin(std::f64::consts::PI * f64::from(index) / 10.0));
        fire.slow_factor = 7.0;
        fire.variant = index;
        state.spawn_actor(fire);
    }
    let mut event = cast.event(Event::Cast, Some(Cue::Ignite));
    event.position = cast.aim;
    event.screen_flash = Some(screen_flash(1.0, 0.5, 0.0, 0.1, true));
    state.emit_event(event);
    state.emit_event(cast.event(Event::Pulse, Some(Cue::FireballHit)));
}

fn cast_ether_drain(state: &mut SecondarySimulationState, cast: &Cast<'_>) {
    let rotation = state.rng.draw_scaled_float(360.0, false);
    let mut drain = cast.actor(Kind::EtherDrain, cast.aim);
    drain.alpha = 0.0;
    drain.damage = cast.value("mDamage");
    drain.enhanced = cast.authority.enhanced_effects;
    drain.freeze_ticks = 1000;
    drain.lifetime_ticks = 1061;
    drain.radius = 512.0;
    drain.rotation_radians = rotation * std::f64::consts::PI / 180.0;
    drain.scale = 0.0;
    state.spawn_actor(drain);
    state.emit_event(cast.event(Event::Cast, Some(Cue::DistortReality)));
    state.emit_event(cast.event(Event::Pulse, Some(Cue::LightningStart)));
}

fn cast_call_comet(state: &mut SecondarySimulationState, cast: &Cast<'_>) {
    let heading = state.rng.draw_scaled_float(1.0, false);
    let mut comet = cast.actor(Kind::Comet, cast.aim);
    comet.damage = cast.value("mDamage");
    comet.enhanced = cast.authority.enhanced_effects;
    comet.freeze_ticks =
        (cast.value("mFreeze") * cast.authority.freeze_duration_multiplier * 100.0).trunc() as u64;
    comet.lifetime_ticks = 400;
    comet.quantity = if cast.authority.maximum_ring_of_ice { 1.0 } else { 0.0 };
    comet.radius = 400.0;
    comet.rotation_radians = atan2(3.0, heading);
    comet.scale = 2.0;
    state.spawn_actor(comet);
}

fn cast_turn_undead(
    state: &mut SecondarySimulationState,
    cast: &Cast<'_>,
    world: &impl SecondaryWorld,
    result: &mut SecondaryCastResult,
) {
    for target in stable_targets(world.targets(cast.authority.world_key, cast.origin, 250.0)) {
        if !["SKELETON", "SKELETONARCHER", "SKELETONMAGE", "ZOMBIE"]
            .contains(&target.family.as_str())
        {
            continue;
        }
        result.target_heading_changes.push(super::SecondaryTargetHeadingChange {
            degrees: crate::heading::heading_degrees_from_vector(
                target.position.x - cast.origin.x,
                target.position.y - cast.origin.y,
            ),
            mode: super::SecondaryHeadingMode::Absolute,
            target_id: target.id,
        });
        apply_secondary_target_effect(
            &mut state.target_effects,
            cast.authority.world_key,
            target.id,
            SecondaryTargetEffectPatch {
                flee_ticks: Some((cast.value("mFlee") * 100.0 + 0.5).floor() as u64),
                weaken_factor: Some((1.0 - cast.value("mWeaken") / 100.0).max(0.0)),
                ..SecondaryTargetEffectPatch::default()
            },
        );
    }
    let mut heading = state.rng.draw_scaled_float(360.0, false);
    for index in 0..35 {
        let scale = state.rng.draw_scaled_float(0.5, false);
        let mut effect = cast.actor(Kind::TurnUndead, cast.origin);
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
        let mut event = cast.event(Event::Pulse, Some(Cue::LevelUp));
        event.pitch = pitch;
        state.emit_event(event);
    }
}

/// Mindstar and Regenerate reserve mana instead of spending it.
fn toggle_mindstar_or_regenerate(
    state: &mut SecondarySimulationState,
    cast: &Cast<'_>,
    mut player: SecondaryPlayerState,
) -> SecondaryCastResult {
    let active = if cast.ability == Ability::Mindstar {
        player.mindstar = !player.mindstar;
        player.mindstar
    } else {
        player.regenerate = !player.regenerate;
        player.regenerate
    };
    player.reserved_mana =
        player.mana_reserve(cast.authority.skill_book, cast.authority.maximum_mana);
    let mut event =
        cast.event(if active { Event::ToggleOn } else { Event::ToggleOff }, Some(Cue::Mindstar));
    event.screen_flash = Some(if cast.ability == Ability::Mindstar {
        screen_flash(0.0, 0.5, 1.0, 0.1, true)
    } else {
        screen_flash(1.0, 0.5, 0.0, 0.1, true)
    });
    state.emit_event(event);
    SecondaryCastResult::none(player, false)
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
) -> super::SecondaryScreenFlashState {
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
fn magic_trap_selector(authority: &SecondaryPlayerAuthority<'_>, rng: &mut Rng) -> usize {
    let ids = [8, 16, 24, 32, 40];
    let primary = if authority.skill_book.primary_skill_id == 52 {
        let build =
            weld_build(authority.skill_book.weld_build_id.expect("selected Weld has its build"));
        build.primary_skill_ids[rng.draw_integer(2) as usize]
    } else {
        authority.skill_book.primary_skill_id
    };
    ids.iter()
        .position(|id| *id == primary)
        .expect("selected primary has a native Magic Trap selector")
}
fn magic_trap_base_damage(
    book: &crate::profile::SkillBookState,
    selector: usize,
    rng: &mut Rng,
) -> f64 {
    let id = [8, 16, 24, 32, 40][selector];
    let rank = book.effective_ranks[id];
    assert!(
        rank > 0 && rank <= SKILL_CATALOG[id].maximum_level,
        "trap component has a valid primary rank"
    );
    let shared = skill_stat(id, "mDamage", rank);
    let minimum =
        shared.or_else(|| skill_stat(id, "mDamage1", rank)).expect("primary minimum exists");
    let maximum =
        shared.or_else(|| skill_stat(id, "mDamage2", rank)).expect("primary maximum exists");
    assert!(minimum > 0.0 && maximum >= minimum, "trap primary damage range is valid");
    if selector == 0 { rng.draw_float_range(minimum, maximum) } else { fround(maximum) }
}
fn dampened_variant(kind: SecondaryDampenProjectileKind, payload: SecondaryDampenPayload) -> i32 {
    if matches!(
        kind,
        SecondaryDampenProjectileKind::SkullMissile | SecondaryDampenProjectileKind::DarkFireball
    ) {
        3
    } else if kind == SecondaryDampenProjectileKind::Firebolt {
        0
    } else if payload == SecondaryDampenPayload::Poison {
        1
    } else {
        2
    }
}
