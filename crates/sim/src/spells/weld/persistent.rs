//! Held growth, native split/release and flight of the three persistent builds
//! from `core-kernels/native-weld-primary-runtime.ts`.

use crate::spells::transient_storage::{WeldPersistentMut, WeldPersistentRef};

use super::model::{NativeWeldActorBase, NativeWeldActorBirth};
use crate::float::{fround, hypot, hypot3};
use crate::heading::heading_degrees_from_vector;
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::spells::earth::{
    EARTH_BOULDER_IDENTITY_ORIENTATION, earth_boulder_flight_orientation_step,
    earth_boulder_held_orientation_step, native_earth_boulder_released_damage,
};
use crate::spells::earth_debris::{
    NativeWeldMeteorDebrisSeed, create_native_weld_ethereal_boulder_weak_debris_program,
};
use crate::spells::model::{EarthBoulderOrientation, PrimarySpellProjectilePhase};
use crate::spells::targeting::{
    PrimarySpellTarget, direction_from_heading, native_heading_turn_direction,
    select_ether_primary_target_at_point,
};
use crate::{NativeRng, Vec2};

pub const NATIVE_WELD_HAIL_FLIGHT_SUBSTEPS: usize = 3;
pub const NATIVE_WELD_HAIL_SUBSTEP_DISTANCE: f64 = 10.0;
pub const NATIVE_WELD_HAIL_LOOKAHEAD_DISTANCE: f64 = 30.0;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct NativeWeldEtherealBoulderState {
    pub common: NativeWeldActorBase,
    pub assembly_scale: f64,
    pub damage: f64,
    pub flight_ticks: u64,
    pub hit_target_ids: Vec<String>,
    pub flight_speed: f64,
    pub heading_degrees: f64,
    pub reacquires_target: bool,
    #[cfg_attr(feature = "persistence", serde(deserialize_with = "required_boulder_target"))]
    pub target_id: Option<String>,
    pub tracking_ticks_remaining: u64,
    pub turn_accumulator: f64,
    pub turn_input: f64,
    pub maximum_scale: f64,
    pub orientation: EarthBoulderOrientation,
    pub phase: PrimarySpellProjectilePhase,
    pub pulse_sequence: u64,
    pub quantity: u32,
    pub remaining_damage: f64,
    pub scale: f64,
    pub shell_scale: f64,
    pub toughness: f64,
    pub velocity: Vec2,
}

#[cfg(feature = "persistence")]
fn required_boulder_target<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    // Nullable is still required on the gameplay wire; only disk migration defaults it.
    <Option<String> as serde::Deserialize>::deserialize(deserializer)
}

impl NativeWeldEtherealBoulderState {
    /// Required authoritative guidance fields cross checkpoint and wire boundaries.
    pub fn validate_guidance(&self) -> Result<(), String> {
        if !self.flight_speed.is_finite()
            || self.flight_speed <= 0.0
            || !self.heading_degrees.is_finite()
            || !self.turn_input.is_finite()
            || self.turn_input <= 0.0
            || !self.turn_accumulator.is_finite()
            || !(0.01_f32 as f64..=10.0).contains(&self.turn_accumulator)
            || self.tracking_ticks_remaining > 9_007_199_254_740_991
            || self.target_id.as_ref().is_some_and(|id| id.encode_utf16().count() > 256)
        {
            return Err("saved Boulder guidance is invalid".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldHailstoneLocalPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeWeldHailstoneRockState {
    pub damage_remaining: f64,
    pub decay: f64,
    pub local_position: NativeWeldHailstoneLocalPosition,
    pub phase: f64,
    pub rock_id: usize,
    /// Collision retains this release offset even while the draw height decays.
    pub release_offset: Option<Vec2>,
    pub sprite_record: usize,
    pub visual_scale: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct NativeWeldHailstonesState {
    pub common: NativeWeldActorBase,
    pub collision_radius: f64,
    pub damage: f64,
    pub maximum_scale: f64,
    pub phase: PrimarySpellProjectilePhase,
    pub pulse_sequence: u64,
    pub pushback: f64,
    pub release_age_ticks: Option<u64>,
    pub release_fade_scale: Option<f64>,
    pub rocks: Vec<NativeWeldHailstoneRockState>,
    pub scale: f64,
    pub toughness: f64,
    pub widen: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct NativeWeldMeteorFieldState {
    pub common: NativeWeldActorBase,
    pub pulse_sequence: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeWeldPersistentActorState {
    EtherealBoulder(NativeWeldEtherealBoulderState),
    Hailstones(NativeWeldHailstonesState),
    MeteorField(NativeWeldMeteorFieldState),
}

impl NativeWeldPersistentActorState {
    #[must_use]
    pub fn common(&self) -> &NativeWeldActorBase {
        match self {
            Self::EtherealBoulder(actor) => &actor.common,
            Self::Hailstones(actor) => &actor.common,
            Self::MeteorField(actor) => &actor.common,
        }
    }

    pub fn common_mut(&mut self) -> &mut NativeWeldActorBase {
        match self {
            Self::EtherealBoulder(actor) => &mut actor.common,
            Self::Hailstones(actor) => &mut actor.common,
            Self::MeteorField(actor) => &mut actor.common,
        }
    }

    #[must_use]
    pub fn held(&self) -> bool {
        match self {
            Self::EtherealBoulder(actor) => actor.phase == PrimarySpellProjectilePhase::Held,
            Self::Hailstones(actor) => actor.phase == PrimarySpellProjectilePhase::Held,
            Self::MeteorField(_) => true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeWeldHailRockFadeSeed {
    pub position: Vec2,
    pub rotation_degrees: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeWeldPersistentUpdate {
    pub debris: Vec<NativeWeldMeteorDebrisSeed>,
    pub hail_rock_fades: Vec<NativeWeldHailRockFadeSeed>,
    pub release_requested: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeWeldPersistentUpdateOptions {
    pub cast_progress_factor: f64,
    pub enhanced_effects: bool,
    pub underpowered: bool,
}

impl Default for NativeWeldPersistentUpdateOptions {
    fn default() -> Self {
        Self { cast_progress_factor: 1.0, enhanced_effects: true, underpowered: false }
    }
}

#[must_use]
pub fn create_native_weld_persistent_actor(
    birth: NativeWeldActorBirth<'_>,
    painters: Option<&mut WorldPainterOrder>,
) -> NativeWeldPersistentActorState {
    let vector = birth.vector;
    let initial_scale = 0.18_f32 as f64;
    match birth.build_id {
        1006 => NativeWeldPersistentActorState::EtherealBoulder(NativeWeldEtherealBoulderState {
            common: birth.actor_with_light(painters),
            assembly_scale: initial_scale,
            damage: vector[0],
            flight_ticks: 0,
            hit_target_ids: Vec::new(),
            flight_speed: 2.25,
            heading_degrees: fround(heading_degrees_from_vector(
                birth.direction.x,
                birth.direction.y,
            )),
            reacquires_target: vector[3] > 1.0,
            target_id: None,
            tracking_ticks_remaining: if vector[3] > 1.0 {
                (vector[3] * 1_000.0 + 250.0).trunc() as u64
            } else {
                250
            },
            turn_accumulator: 0.01_f32 as f64,
            turn_input: fround(vector[3]),
            maximum_scale: 0.75,
            orientation: EARTH_BOULDER_IDENTITY_ORIENTATION,
            phase: PrimarySpellProjectilePhase::Held,
            pulse_sequence: 0,
            quantity: (vector[2].round() as u32).clamp(1, 4),
            remaining_damage: vector[0],
            scale: initial_scale,
            shell_scale: initial_scale,
            toughness: vector[4],
            velocity: Vec2::ZERO,
        }),
        1008 => NativeWeldPersistentActorState::Hailstones(NativeWeldHailstonesState {
            common: birth.actor_with_light(painters),
            collision_radius: 40.0,
            damage: vector[0],
            maximum_scale: 1.0,
            phase: PrimarySpellProjectilePhase::Held,
            pulse_sequence: 0,
            pushback: vector[4],
            release_age_ticks: None,
            release_fade_scale: None,
            rocks: Vec::new(),
            scale: initial_scale,
            toughness: vector[3],
            widen: vector[5],
        }),
        1007 => NativeWeldPersistentActorState::MeteorField(NativeWeldMeteorFieldState {
            common: birth.actor(),
            pulse_sequence: 0,
        }),
        _ => panic!("Weld build is not a persistent actor"),
    }
}

pub fn update_native_weld_persistent_actor(
    actor: &mut NativeWeldPersistentActorState,
    origin: Vec2,
    direction: Vec2,
    options: NativeWeldPersistentUpdateOptions,
    rng: &mut NativeRng,
) -> NativeWeldPersistentUpdate {
    assert!(
        options.cast_progress_factor.is_finite() && options.cast_progress_factor > 0.0,
        "Weld cast progress factor must be finite and positive",
    );
    let mut update = NativeWeldPersistentUpdate::default();
    match actor {
        NativeWeldPersistentActorState::EtherealBoulder(actor) => {
            if options.underpowered && actor.quantity > 1 {
                update.debris = create_native_weld_ethereal_boulder_weak_debris_program(
                    actor.common.direction,
                    actor.scale,
                    rng,
                );
            }
            let growth_input = if options.underpowered {
                1.0
            } else {
                fround(actor.common.vector[5] * options.cast_progress_factor)
            };
            let growth = fround(fround(growth_input * 0.0025) * 3.0);
            let scale = fround(actor.scale + growth).min(actor.maximum_scale);
            if (30.0 * scale).floor() != (30.0 * actor.scale).floor() {
                actor.assembly_scale = scale;
            }
            update.release_requested = options.underpowered && actor.scale > 0.3_f32 as f64;
            actor.common.owner.age_ticks += 1;
            actor.common.direction = direction;
            actor.heading_degrees = fround(heading_degrees_from_vector(direction.x, direction.y));
            actor.common.origin = origin;
            actor.reacquires_target = !options.underpowered && actor.common.vector[3] > 1.0;
            if options.underpowered {
                actor.tracking_ticks_remaining = 150;
            } else if actor.common.vector[3] > 1.0 {
                actor.tracking_ticks_remaining =
                    (actor.common.vector[3] * 1_000.0 + 250.0).trunc() as u64;
            }
            actor.orientation = earth_boulder_held_orientation_step(actor.orientation, direction);
            actor.pulse_sequence += 1;
            actor.scale = scale;
            actor.shell_scale = actor.assembly_scale;
            if options.underpowered {
                actor.damage = fround(actor.common.vector[0] * 0.5);
                actor.quantity = 1;
                actor.remaining_damage = fround(actor.remaining_damage * 0.5);
                actor.turn_input = 1.0;
            }
        }
        NativeWeldPersistentActorState::Hailstones(actor) => {
            let old_bucket = fround(30.0 * actor.scale).round_ties_even();
            let growth_input = if options.underpowered && actor.scale > 0.3_f32 as f64 {
                0.0
            } else {
                fround(fround(actor.common.vector[2] * options.cast_progress_factor) * 0.5)
            };
            let growth = fround(fround(growth_input * 0.0025) * 3.0);
            let scale = fround(actor.scale + growth).min(actor.maximum_scale);
            if fround(30.0 * scale).round_ties_even() != old_bucket {
                update.hail_rock_fades =
                    rebuild_hailstone_rocks(actor, origin, scale, options.enhanced_effects, rng);
            }
            actor.common.owner.age_ticks += 1;
            actor.common.direction = direction;
            actor.common.origin = origin;
            actor.pulse_sequence += 1;
            actor.scale = scale;
            actor.pushback = if options.underpowered { 0.0 } else { actor.common.vector[4] };
            actor.widen = if options.underpowered { 0.0 } else { actor.common.vector[5] };
            if options.underpowered {
                actor.damage = fround(actor.common.vector[0] * 0.5);
            }
        }
        NativeWeldPersistentActorState::MeteorField(actor) => {
            actor.common.owner.age_ticks += 1;
            actor.common.direction = direction;
            actor.common.origin = origin;
            actor.pulse_sequence += 1;
        }
    }
    update
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct NativeWeldFrostFadeActorState {
    pub common: NativeWeldActorBase,
    pub scale: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeWeldPersistentRelease {
    pub actors: Vec<NativeWeldPersistentActorState>,
    pub frost_fade: Option<NativeWeldFrostFadeActorState>,
    pub next_id: u64,
}

pub fn release_native_weld_persistent_actor(
    actor: NativeWeldPersistentActorState,
    first_child_id: u64,
    tick: u64,
    rng: &mut NativeRng,
    mut painters: Option<&mut WorldPainterOrder>,
    targets: &[PrimarySpellTarget],
) -> NativeWeldPersistentRelease {
    match actor {
        NativeWeldPersistentActorState::MeteorField(_) => NativeWeldPersistentRelease {
            actors: Vec::new(),
            frost_fade: None,
            next_id: first_child_id,
        },
        NativeWeldPersistentActorState::Hailstones(mut actor) => {
            let presentation = rng.draw_scaled_float(0.75, false);
            actor.common.origin = Vec2::new(
                fround(actor.common.origin.x - actor.common.direction.x * 20.0),
                fround(actor.common.origin.y - actor.common.direction.y * 20.0),
            );
            for rock in &mut actor.rocks {
                rock.damage_remaining = actor.damage;
                rock.release_offset = Some(native_weld_hailstone_draw_offset(rock));
            }
            actor.phase = PrimarySpellProjectilePhase::Flight;
            actor.release_age_ticks = Some(0);
            actor.release_fade_scale = Some(fround(presentation + 0.75));
            let fade_birth = NativeWeldActorBirth {
                build_id: 1008,
                direction: actor.common.direction,
                id: first_child_id,
                origin: Vec2::new(actor.common.origin.x, fround(actor.common.origin.y - 20.0)),
                owner_id: &actor.common.owner.owner_id,
                tick,
                vector: &actor.common.vector,
                world_key: &actor.common.owner.world_key,
            };
            let fade = NativeWeldFrostFadeActorState {
                common: fade_birth.actor(),
                scale: fround(fround(presentation + 0.75) * 5.0),
            };
            NativeWeldPersistentRelease {
                actors: vec![NativeWeldPersistentActorState::Hailstones(actor)],
                frost_fade: Some(fade),
                next_id: first_child_id + 1,
            }
        }
        NativeWeldPersistentActorState::EtherealBoulder(actor) => {
            let pieces = ethereal_boulder_split(&actor);
            let mut actors = Vec::with_capacity(pieces.len());
            for (index, piece) in pieces.iter().enumerate() {
                let mut released = actor.clone();
                released.common.direction = piece.direction;
                released.common.origin = piece.origin;
                if index > 0 {
                    released.common.owner.id = first_child_id + index as u64 - 1;
                    // Source drops the copied roots so final enrollment uses this
                    // child's newly allocated light, rather than its parent's row.
                    released.common.owner.painter_registrations.clear();
                    released.common.owner.light_registration =
                        Some(if let Some(order) = painters.as_deref_mut() {
                            order.register(WorldManagerLane::Actor)
                        } else {
                            PainterRegistration {
                                manager_lane: WorldManagerLane::Actor,
                                registration_ordinal: released.common.owner.id as usize,
                            }
                        });
                }
                released.hit_target_ids.clear();
                released.flight_speed = fround(2.25 * piece.motion_factor);
                released.heading_degrees = piece.heading_degrees;
                released.target_id =
                    select_ether_primary_target_at_point(piece.origin, None, targets)
                        .map(|target| target.id.clone());
                released.reacquires_target &= released.target_id.is_some();
                released.maximum_scale = actor.scale;
                released.phase = PrimarySpellProjectilePhase::Flight;
                released.flight_ticks = 0;
                released.pulse_sequence = actor.pulse_sequence + 1;
                released.quantity = 0;
                released.remaining_damage =
                    native_earth_boulder_released_damage(actor.damage, actor.scale, 1.0);
                released.turn_input = piece.turn_input;
                released.velocity = Vec2::new(
                    fround(piece.direction.x * released.flight_speed),
                    fround(piece.direction.y * released.flight_speed),
                );
                actors.push(NativeWeldPersistentActorState::EtherealBoulder(released));
            }
            NativeWeldPersistentRelease {
                actors,
                frost_fade: None,
                next_id: first_child_id + pieces.len() as u64 - 1,
            }
        }
    }
}

/// Preserves the source pool and scale written by actual contact acceptance.
pub fn retain_native_weld_persistent_actor_contacts(
    actor: &mut NativeWeldEtherealBoulderState,
    hit_target_ids: Vec<String>,
    remaining_damage: f64,
    scale: f64,
) -> bool {
    if remaining_damage <= 0.0 {
        return false;
    }
    actor.hit_target_ids = hit_target_ids;
    actor.remaining_damage = remaining_damage;
    actor.scale = scale;
    actor.shell_scale = scale;
    true
}

pub fn retain_native_weld_hailstone_damage(
    actor: &mut NativeWeldHailstonesState,
    damage_by_index: &[f64],
) -> bool {
    for (index, rock) in actor.rocks.iter_mut().enumerate() {
        if let Some(damage) = damage_by_index.get(index) {
            rock.damage_remaining = *damage;
        }
    }
    actor.rocks.retain(|rock| rock.damage_remaining >= 0.001);
    !actor.rocks.is_empty()
}

pub type NativeWeldPersistentTraversal<'a> = dyn Fn(WeldPersistentRef<'_>, Vec2, Vec2) -> bool + 'a;

/// Returns a terminal Boulder separately so terrain contact uses its moved pose.
pub fn step_native_weld_persistent_actor(
    actor: &mut WeldPersistentMut<'_>,
    can_advance: &NativeWeldPersistentTraversal<'_>,
) -> (bool, Option<NativeWeldEtherealBoulderState>) {
    if actor.held() {
        return (true, None);
    }
    if let WeldPersistentMut::EtherealBoulder(boulder) = actor {
        boulder.tracking_ticks_remaining = boulder.tracking_ticks_remaining.saturating_sub(1);
        boulder.common.origin = Vec2::new(
            fround(boulder.common.origin.x + boulder.velocity.x),
            fround(boulder.common.origin.y + boulder.velocity.y),
        );
        boulder.common.owner.age_ticks += 1;
        boulder.flight_ticks += 1;
        boulder.orientation = earth_boulder_flight_orientation_step(
            boulder.orientation,
            boulder.common.direction,
            boulder.velocity,
            boulder.scale,
        );
        let from = boulder.common.origin;
        let to =
            Vec2::new(fround(from.x + boulder.velocity.x), fround(from.y + boulder.velocity.y));
        if can_advance(actor.view(), from, to) {
            return (true, None);
        }
        let WeldPersistentMut::EtherealBoulder(boulder) = actor else { unreachable!() };
        return (false, Some((**boulder).clone()));
    }
    let common = actor.common();
    let lookahead = Vec2::new(
        fround(common.origin.x + common.direction.x * NATIVE_WELD_HAIL_LOOKAHEAD_DISTANCE),
        fround(common.origin.y + common.direction.y * NATIVE_WELD_HAIL_LOOKAHEAD_DISTANCE),
    );
    if !can_advance(actor.view(), common.origin, lookahead) {
        return (false, None);
    }
    let WeldPersistentMut::Hailstones(hail) = actor else { unreachable!() };
    for _ in 0..NATIVE_WELD_HAIL_FLIGHT_SUBSTEPS {
        hail.common.origin = Vec2::new(
            fround(
                hail.common.origin.x + hail.common.direction.x * NATIVE_WELD_HAIL_SUBSTEP_DISTANCE,
            ),
            fround(
                hail.common.origin.y + hail.common.direction.y * NATIVE_WELD_HAIL_SUBSTEP_DISTANCE,
            ),
        );
        if hail.widen > 0.0 {
            hail.collision_radius = fround(hail.collision_radius + hail.widen);
            for rock in &mut hail.rocks {
                widen_hailstone_rock(rock, hail.widen);
            }
        }
    }
    hail.common.owner.age_ticks += 1;
    *hail.release_age_ticks.as_mut().expect("released Hailstones own a release clock") += 1;
    for rock in &mut hail.rocks {
        rock.decay = fround(rock.decay * 0.95_f32 as f64);
        rock.phase = fround(rock.phase + 0.025_f32 as f64).min(1.0);
    }
    (true, None)
}

#[must_use]
pub fn native_weld_hailstone_draw_offset(rock: &NativeWeldHailstoneRockState) -> Vec2 {
    let position = rock.local_position;
    let falling_height = fround(50.0 - position.z * 0.8_f32 as f64);
    Vec2::new(position.x, fround(position.y + fround(falling_height - position.y) * rock.decay))
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeWeldHailstoneFlightContactSubstep {
    pub origin: Vec2,
    pub release_offsets: Vec<(usize, Vec2)>,
}

#[must_use]
pub fn native_weld_hailstone_flight_contact_substeps(
    actor: &NativeWeldHailstonesState,
) -> Vec<NativeWeldHailstoneFlightContactSubstep> {
    if actor.phase != PrimarySpellProjectilePhase::Flight {
        return Vec::new();
    }
    let mut substeps = Vec::with_capacity(NATIVE_WELD_HAIL_FLIGHT_SUBSTEPS);
    for index in 0..NATIVE_WELD_HAIL_FLIGHT_SUBSTEPS {
        let remaining = (NATIVE_WELD_HAIL_FLIGHT_SUBSTEPS - index - 1) as f64;
        let origin = Vec2::new(
            fround(
                actor.common.origin.x
                    - actor.common.direction.x * NATIVE_WELD_HAIL_SUBSTEP_DISTANCE * remaining,
            ),
            fround(
                actor.common.origin.y
                    - actor.common.direction.y * NATIVE_WELD_HAIL_SUBSTEP_DISTANCE * remaining,
            ),
        );
        let mut release_offsets = Vec::with_capacity(actor.rocks.len());
        for rock in &actor.rocks {
            let mut offset =
                rock.release_offset.expect("released Hailstone owns its collision offset");
            if actor.widen > 0.0 && remaining != 0.0 {
                let length = hypot(rock.local_position.x, rock.local_position.y);
                let x = if length > 0.0 { rock.local_position.x / length } else { 0.0 };
                let y = if length > 0.0 { rock.local_position.y / length } else { 0.0 };
                offset = Vec2::new(
                    fround(offset.x - x * actor.widen * remaining),
                    fround(offset.y - y * actor.widen * remaining),
                );
            }
            release_offsets.push((rock.rock_id, offset));
        }
        substeps.push(NativeWeldHailstoneFlightContactSubstep { origin, release_offsets });
    }
    substeps
}

fn rebuild_hailstone_rocks(
    actor: &mut NativeWeldHailstonesState,
    origin: Vec2,
    scale: f64,
    enhanced_effects: bool,
    rng: &mut NativeRng,
) -> Vec<NativeWeldHailRockFadeSeed> {
    let desired_count = fround(fround(fround(actor.widen * 3.0 + 20.0) * scale) * scale)
        .max(1.0)
        .round_ties_even() as usize;
    let mut fades = Vec::new();
    while actor.rocks.len() < desired_count {
        let sprite = rng.draw_integer(3) as usize;
        let x = rng.draw_scaled_float(50.0, true);
        let y = rng.draw_scaled_float(50.0, true);
        let z = rng.draw_scaled_float(50.0, true);
        let length = hypot3(x, y, z);
        let unit = if length > 0.0 {
            NativeWeldHailstoneLocalPosition { x: x / length, y: y / length, z: z / length }
        } else {
            NativeWeldHailstoneLocalPosition { x: 0.0, y: 0.0, z: 0.0 }
        };
        let radial_scale =
            if actor.rocks.is_empty() { rng.draw_scaled_float(10.0, false) } else { 40.0 };
        let visual = rng.draw_scaled_float(0.75, false);
        if enhanced_effects {
            let rotation = rng.draw_scaled_float(20.0, false);
            fades.push(NativeWeldHailRockFadeSeed {
                position: Vec2::new(
                    fround(origin.x + unit.x * radial_scale * 1.5),
                    fround(origin.y + unit.z * radial_scale * 0.8_f32 as f64 * 1.5),
                ),
                rotation_degrees: rotation,
            });
        }
        actor.rocks.push(NativeWeldHailstoneRockState {
            damage_remaining: 0.0,
            decay: 1.0,
            local_position: NativeWeldHailstoneLocalPosition {
                x: fround(unit.x * radial_scale),
                y: fround(unit.y * radial_scale),
                z: fround(unit.z * radial_scale),
            },
            phase: 0.0,
            rock_id: actor.rocks.len(),
            release_offset: None,
            sprite_record: 168 + sprite,
            visual_scale: fround(fround(visual + 0.5) * 0.2).min(1.0),
        });
    }
    fades
}

fn widen_hailstone_rock(rock: &mut NativeWeldHailstoneRockState, amount: f64) {
    let Some(offset) = rock.release_offset.as_mut() else {
        return;
    };
    let length = hypot(rock.local_position.x, rock.local_position.y);
    if length <= 0.0 {
        return;
    }
    let x = fround(rock.local_position.x / length * amount);
    let y = fround(rock.local_position.y / length * amount);
    rock.local_position.x = fround(rock.local_position.x + x);
    rock.local_position.y = fround(rock.local_position.y + y);
    offset.x = fround(offset.x + x);
    offset.y = fround(offset.y + y);
}

/// Retail0x621450 guidance tail: shared movement and accepted contacts run first.
pub fn finish_native_weld_ethereal_boulder_guidance(
    actor: &mut NativeWeldEtherealBoulderState,
    targets: &[PrimarySpellTarget],
) {
    if actor.phase != PrimarySpellProjectilePhase::Flight
        || actor.flight_ticks == 0
        || actor.tracking_ticks_remaining == 0
        || actor.target_id.is_none()
    {
        return;
    }
    let target = targets.iter().find(|target| Some(&target.id) == actor.target_id.as_ref());
    let Some(target) = target else {
        let selected = if actor.reacquires_target {
            select_ether_primary_target_at_point(actor.common.origin, None, targets)
        } else {
            None
        };
        actor.target_id = selected
            .filter(|target| !actor.hit_target_ids.contains(&target.id))
            .map(|target| target.id.clone());
        actor.reacquires_target &= actor.target_id.is_some();
        return;
    };
    let desired = heading_degrees_from_vector(
        target.position.x - actor.common.origin.x,
        target.position.y - actor.common.origin.y,
    );
    let turn = fround(
        f64::from(native_heading_turn_direction(actor.heading_degrees, desired)) * 0.7_f32 as f64,
    );
    actor.heading_degrees =
        fround(actor.heading_degrees + actor.turn_input * actor.turn_accumulator * turn);
    actor.common.direction = direction_from_heading(actor.heading_degrees);
    actor.turn_accumulator = fround(
        actor.turn_accumulator
            + if actor.turn_accumulator > 1.0 { 0.002_f32 as f64 } else { 0.05_f32 as f64 },
    )
    .min(10.0);
    actor.velocity = Vec2::new(
        fround(actor.common.direction.x * actor.flight_speed),
        fround(actor.common.direction.y * actor.flight_speed),
    );
    if !target.active {
        actor.target_id = None;
    }
}

struct EtherealBoulderPiece {
    direction: Vec2,
    origin: Vec2,
    heading_degrees: f64,
    motion_factor: f64,
    turn_input: f64,
}

fn ethereal_boulder_split(actor: &NativeWeldEtherealBoulderState) -> Vec<EtherealBoulderPiece> {
    let direction = actor.common.direction;
    let perpendicular = Vec2::new(direction.y, -direction.x);
    let piece = |along: f64, across: f64, offset: f64, factor: f64, original: bool| {
        let heading = fround(actor.heading_degrees + offset);
        EtherealBoulderPiece {
            direction: direction_from_heading(heading),
            heading_degrees: heading,
            origin: Vec2::new(
                fround(actor.common.origin.x + direction.x * along + perpendicular.x * across),
                fround(actor.common.origin.y + direction.y * along + perpendicular.y * across),
            ),
            motion_factor: fround(factor),
            turn_input: if original { actor.turn_input } else { fround(factor) },
        }
    };
    match actor.quantity {
        1 => vec![piece(0.0, 0.0, 0.0, 1.0, true)],
        2 => vec![piece(0.0, 30.0, 0.0, 1.0, true), piece(0.0, -30.0, 10.0, 1.0, false)],
        3 => vec![
            piece(30.0, 0.0, 0.0, 1.0, true),
            piece(0.0, 30.0, -10.0, 0.95, false),
            piece(0.0, -30.0, 10.0, 0.95, false),
        ],
        4 => vec![
            piece(30.0, 0.0, 0.0, 1.0, true),
            piece(0.0, 30.0, -10.0, 0.95, false),
            piece(0.0, -30.0, 10.0, 0.95, false),
            piece(-15.0, 0.0, 0.0, 0.9, false),
        ],
        _ => panic!("native Ethereal Boulder quantity is outside 1..4"),
    }
}
