//! Shared source geometry and constructor programs for the secondary actor families.

use super::context::NativeSecondaryPlayerAuthority;
use super::{
    NativeSecondaryActorKind, NativeSecondaryActorState, NativeSecondaryAudioCue,
    NativeSecondaryEventKind, NativeSecondaryEventState, NativeSecondaryScreenFlashState,
    NativeSecondarySimulationState, NativeSecondaryTarget,
};
use crate::float::{fround, hypot};
use crate::secondary::{SecondaryActorRead, SecondaryActorWrite};
use crate::{NativeRng, Vec2};

pub(crate) const MAX_NATIVE_TICKS: u64 = 9_007_199_254_740_991;
pub(crate) const FIRE_LIFE_STEP: f64 = 0.01_f32 as f64;
pub(crate) const RING_FIRE_INITIAL_LIFE: f64 = ((0.7_f32 as f64) * (1.5_f32 as f64)) as f32 as f64;

pub(crate) fn actor(
    kind: NativeSecondaryActorKind,
    owner: &str,
    world: &str,
    skill: Option<usize>,
    rank: u32,
    position: Vec2,
) -> NativeSecondaryActorState {
    let mut actor =
        NativeSecondaryActorState::new(0, kind, owner.to_owned(), world.to_owned(), skill);
    actor.rank = rank;
    actor.position = position;
    actor
}
pub(crate) fn advance_rng(rng: &mut NativeRng, words: usize) {
    for _ in 0..words {
        rng.draw_word();
    }
}
pub(crate) fn unit(from: Vec2, to: Vec2) -> Vec2 {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let length = hypot(dx, dy);
    if length > 0.0001 { Vec2::new(dx / length, dy / length) } else { Vec2::new(0.0, -1.0) }
}
pub(crate) fn squared_distance(from: Vec2, to: Vec2) -> f64 {
    let dx = from.x - to.x;
    let dy = from.y - to.y;
    dx * dx + dy * dy
}
pub(crate) fn normalize_radians(value: f64) -> f64 {
    (value % std::f64::consts::TAU + std::f64::consts::TAU) % std::f64::consts::TAU
}
pub(crate) fn native_unit_vector(rng: &mut NativeRng) -> Vec2 {
    rng.draw_unit_vector()
}
pub(crate) fn stable_targets(
    mut targets: Vec<NativeSecondaryTarget>,
) -> Vec<NativeSecondaryTarget> {
    targets.sort_by_key(|target| target.id);
    targets
}
pub(crate) fn shuffle_fixed_bound<T>(values: &mut [T], rng: &mut NativeRng) {
    for index in 0..values.len() {
        let selected = rng.draw_integer(values.len() as i32) as usize;
        values.swap(index, selected);
    }
}
pub(crate) fn inside_rectangle(
    center: Vec2,
    point: Vec2,
    half_width: f64,
    half_height: f64,
) -> bool {
    (point.x - center.x).abs() <= half_width && (point.y - center.y).abs() <= half_height
}
pub(crate) fn fire_lifetime_ticks(initial_life: f64) -> u64 {
    let mut remaining = fround(initial_life);
    let mut ticks = 0;
    while remaining > 0.0 {
        remaining = fround(remaining - FIRE_LIFE_STEP);
        ticks += 1;
    }
    ticks.max(1)
}
pub(crate) fn screen_flash(
    red: f64,
    green: f64,
    blue: f64,
    decay: f64,
    point: bool,
) -> NativeSecondaryScreenFlashState {
    NativeSecondaryScreenFlashState::new(red, green, blue, decay, point, 1.0)
}
pub(crate) fn cast_event(
    owner: &str,
    skill: usize,
    authority: &NativeSecondaryPlayerAuthority<'_>,
    tick: u64,
    kind: NativeSecondaryEventKind,
    cue: Option<NativeSecondaryAudioCue>,
) -> NativeSecondaryEventState {
    let mut event = NativeSecondaryEventState::new(
        kind,
        owner.to_owned(),
        authority.world_key.to_owned(),
        tick,
    );
    event.cue = cue;
    event.position = authority.character.position;
    event.skill_id = Some(skill);
    event.pitch = if skill == 77 { 2.0 } else { 1.0 };
    event
}
pub(crate) fn actor_event(
    actor: &impl SecondaryActorRead,
    tick: u64,
    kind: NativeSecondaryEventKind,
    cue: Option<NativeSecondaryAudioCue>,
) -> NativeSecondaryEventState {
    assert_ne!(actor.skill_id, Some(14), "Ether Burn has no native semantic audio event");
    let mut event =
        NativeSecondaryEventState::new(kind, actor.owner_id.clone(), actor.world_key.clone(), tick);
    event.actor_id = Some(actor.id);
    event.cue = cue;
    event.position = actor.position;
    event.skill_id = actor.skill_id;
    event
}

pub(crate) fn freeze_wave(
    state: &mut NativeSecondarySimulationState,
    mut seed: NativeSecondaryActorState,
    enhanced: bool,
    maximum: bool,
) {
    let presentation_rng = state.rng.clone();
    for _ in 0..3 {
        state.rng.draw_scaled_float(360.0, false);
    }
    for _ in 0..if enhanced { 200 } else { 100 } {
        for magnitude in [360.0, 10.0, 40.0, 4.0, 250.0, 0.5, 360.0, 1.5] {
            state.rng.draw_scaled_float(magnitude, false);
        }
    }
    seed.kind = NativeSecondaryActorKind::FreezeWave;
    seed.lifetime_ticks = 93;
    seed.phase = 0.924_f32 as f64;
    seed.radius = 75.0;
    seed.variant = i32::from(maximum);
    let mut visual = actor(
        NativeSecondaryActorKind::FreezeWaveVisual,
        &seed.owner_id,
        &seed.world_key,
        seed.skill_id,
        seed.rank,
        seed.position,
    );
    visual.enhanced = enhanced;
    visual.lifetime_ticks = 176;
    visual.presentation_rng = Some(presentation_rng);
    state.spawn_actor(seed);
    state.spawn_actor(visual);
}

pub(crate) fn magic_circle_visual_tick(rng: &mut NativeRng, tick: u64) -> f64 {
    let light = rng.draw_scaled_float(0.25, true);
    advance_rng(rng, (1 + (tick & 1)) as usize * 5);
    fround(0.75 + light)
}
pub(crate) fn magic_circle_player_flash(
    state: &mut NativeSecondarySimulationState,
    owner: &str,
    world: &str,
    position: Vec2,
    rank: u32,
) {
    let rotation = state.rng.draw_scaled_float(360.0, false);
    let scale = state.rng.draw_scaled_float(1.0, false);
    let life = state.rng.draw_scaled_float(0.25, false);
    let mut flash = actor(
        NativeSecondaryActorKind::MagicCirclePlayerFlash,
        owner,
        world,
        Some(49),
        rank,
        position,
    );
    flash.alpha = fround(0.5 + life);
    flash.rotation_radians = rotation * std::f64::consts::PI / 180.0;
    flash.scale = fround(1.0 + scale * 0.65);
    flash.lifetime_ticks = 100;
    state.spawn_actor(flash);
}

pub(crate) fn firewalker_patch(
    state: &mut NativeSecondarySimulationState,
    owner: &str,
    authority: &NativeSecondaryPlayerAuthority<'_>,
    contact_geometry: bool,
) {
    let phase = state.rng.draw_scaled_float(32.0, false);
    let mirror = state.rng.draw_sign(1.0);
    let perpendicular = state.rng.draw_scaled_float(10.0, true);
    let forward = state.rng.draw_scaled_float(8.0, false);
    let scale = state.rng.draw_scaled_float(0.5, false);
    let life = state.rng.draw_scaled_float(0.25, false);
    let skill = super::resolved_secondary_ability_rank_stats(
        authority.skill_book,
        super::NativeSecondaryAbilityId::Firewalker,
        &authority.offensive_factors,
    );
    let velocity =
        Vec2::new(authority.character.velocity.x * 0.01, authority.character.velocity.y * 0.01);
    let remaining = fround(skill.value("mDuration").unwrap_or(2.0) * fround(1.1 - life));
    let position = Vec2::new(
        authority.character.position.x + velocity.y * perpendicular + velocity.x * forward,
        authority.character.position.y - velocity.x * perpendicular + velocity.y * forward,
    );
    let mut patch = actor(
        NativeSecondaryActorKind::FirePatch,
        owner,
        authority.world_key,
        Some(23),
        skill.rank,
        position,
    );
    patch.damage = skill.value("mDamage").unwrap_or(0.0);
    patch.enhanced = contact_geometry;
    patch.lifetime_ticks = fire_lifetime_ticks(remaining);
    patch.phase = phase;
    patch.quantity = mirror;
    patch.slow_factor = remaining;
    patch.scale = 1.0 - scale;
    state.spawn_actor(patch);
}

pub(crate) fn advance_actor(actor: &mut impl SecondaryActorWrite) {
    use NativeSecondaryActorKind::*;
    let moves = !matches!(
        actor.kind,
        PlaneOrbShot
            | PlaneOrbParticle
            | LeviathanAppendage
            | Earthquake
            | EarthquakeSceneryWobble
            | EarthquakeDust
            | EarthquakeDebris
            | Golem
            | AcidDrop
            | StormDrop
            | CometDebris
            | StormStrike
            | RingFireFragment
            | Shockwave
            | MindblastShockwave
            | RescueShockwave
            | FreezeWave
    );
    let ages = actor.kind != Golem;
    let frames = ages
        && !matches!(
            actor.kind,
            GolemAssemblyDebris
                | StoneskinChip
                | PlanewalkerMote
                | LeviathanAppendage
                | Earthquake
                | EarthquakeSceneryWobble
                | EarthquakeDebris
                | RingFireFragment
        );
    if moves {
        actor.position.x += actor.velocity.x;
        actor.position.y += actor.velocity.y;
    }
    if ages {
        actor.age_ticks += 1;
    }
    if frames {
        actor.frame += 1.0;
    }
}
