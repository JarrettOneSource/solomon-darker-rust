//! Field membership, binary32 pressure and private animations from `native-ether-drain.ts`.

use super::{
    NativeEtherDrainAnimation, NativeEtherDrainState, NativeEtherDrainTargetRef,
    NativeEtherDrainWorldAnimationIds, NativeEtherDrainWorldAnimationRef,
};
use crate::float::{fround, sin};
use crate::{NativeRng, Vec2};

pub const NATIVE_ETHER_DRAIN_BROAD_RADIUS: f64 = 1024.0;
pub const NATIVE_ETHER_DRAIN_CORPSE_TIMER_TICKS: u64 = 130;
pub const NATIVE_ETHER_DRAIN_CONSUMPTION_DISTANCE_SQUARED: f64 = 100.0;
const PRESSURE_DISTANCE_SQUARED: f64 = 262_144.0;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeEtherDrainTarget {
    pub activation_delay_ticks: i64,
    pub force_factor: f64,
    pub native_flags: u32,
    pub position: Vec2,
    #[cfg_attr(feature = "persistence", serde(rename = "ref"))]
    pub reference: NativeEtherDrainTargetRef,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeEtherDrainWorldAnimationTarget {
    pub bounce_velocity: f64,
    pub position: Vec2,
    #[cfg_attr(feature = "persistence", serde(rename = "ref"))]
    pub reference: NativeEtherDrainWorldAnimationRef,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeEtherDrainField<'a> {
    pub alpha: f64,
    pub damage: f64,
    pub id: u64,
    pub owner_id: &'a str,
    pub position: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeEtherDrainContact {
    pub amount: f64,
    pub damage_flags: u32,
    pub magic_contact: bool,
    pub consume: bool,
    pub delta: Vec2,
    pub owner_id: String,
    pub source_actor_id: u64,
    pub target: NativeEtherDrainTargetRef,
    /// The field's shared RNG supplies this after accepting a magic contact.
    pub hit_strength: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeEtherDrainWorldAnimationContact {
    pub consume: bool,
    pub delta: Vec2,
    #[cfg_attr(feature = "persistence", serde(rename = "ref"))]
    pub reference: NativeEtherDrainWorldAnimationRef,
    pub source_actor_id: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub struct NativeEtherDrainAnimationStep {
    pub animations: Vec<NativeEtherDrainAnimation>,
    pub pulses: Vec<f64>,
}

#[must_use]
pub fn create_native_ether_drain_debris(
    center: Vec2,
    position: Vec2,
    oscillation_degrees: f64,
    rotation_degrees: f64,
    variant: u32,
) -> NativeEtherDrainAnimation {
    let dx = fround(position.x - center.x);
    let dy = fround(position.y - center.y);
    let distance = fround(fround(dx * dx + dy * dy).sqrt());
    let inverse = if distance == 0.0 { 0.0 } else { fround(1.0 / distance) };
    NativeEtherDrainAnimation::Debris {
        position,
        direction: Vec2::new(fround(dx * inverse), fround(dy * inverse)),
        remaining_distance: distance,
        speed: 1.0,
        oscillation_degrees,
        rotation_degrees,
        variant,
    }
}

/// Native signed area and first moments accumulate at binary32 stores.
#[must_use]
pub fn native_ether_drain_leaf_center(polygon: &[Vec2]) -> Vec2 {
    let mut area = 0.0;
    let mut x = 0.0;
    let mut y = 0.0;
    for (index, current) in polygon.iter().enumerate() {
        let previous = polygon[(index + polygon.len() - 1) % polygon.len()];
        let cross = fround(current.x * previous.y - current.y * previous.x);
        area = fround(area + cross);
        x = fround(x + fround((current.x + previous.x) * cross));
        y = fround(y + fround((current.y + previous.y) * cross));
    }
    let denominator = fround(area * 3.0);
    Vec2::new(fround(x / denominator), fround(y / denominator))
}

/// The loose-object prefix survives ordinary spatial refresh. Notification order
/// is significant: a newly registered drop can force a query between timer polls.
#[must_use]
pub fn refresh_native_ether_drain_targets(
    source: Option<&NativeEtherDrainState>,
    targets: &[NativeEtherDrainTarget],
    center: Vec2,
    refresh: bool,
) -> (bool, NativeEtherDrainState) {
    let mut state = source.cloned().unwrap_or_default();
    state.loose.retain(|reference| targets.iter().any(|target| &target.reference == reference));
    let previous_ordinal = source.map_or(-1, |source| source.last_loot_registration_ordinal);
    let mut notified = false;
    for target in targets {
        let NativeEtherDrainTargetRef::Loot { registration_ordinal, .. } = target.reference else {
            continue;
        };
        if registration_ordinal as i64 > previous_ordinal
            && (source.is_none_or(|source| !source.targets_initialized)
                || inside_broad_ellipse(center, target.position, 0.85_f32 as f64))
        {
            state.loose.push(target.reference.clone());
            notified = true;
        }
        state.last_loot_registration_ordinal =
            state.last_loot_registration_ordinal.max(registration_ordinal as i64);
    }
    let refreshed = refresh || source.is_none() || notified;
    if refreshed {
        state.queried.clear();
        for target in targets {
            if !matches!(target.reference, NativeEtherDrainTargetRef::Loot { .. })
                && target.native_flags & 0xc02 != 0
                && inside_broad_ellipse(center, target.position, 0.8_f32 as f64)
            {
                state.queried.push(target.reference.clone());
            }
        }
    } else {
        state
            .queried
            .retain(|reference| targets.iter().any(|target| &target.reference == reference));
    }
    state.targets_initialized = true;
    (refreshed, state)
}

pub fn refresh_native_ether_drain_world_animations(
    state: &mut NativeEtherDrainState,
    targets: &[NativeEtherDrainWorldAnimationTarget],
    center: Vec2,
    initialized: bool,
) {
    state
        .world_animation_refs
        .retain(|reference| targets.iter().any(|target| target.reference == *reference));
    let previous_ids = state.last_world_animation_ids;
    for target in targets {
        let (id, previous_id) = animation_id(target.reference, &previous_ids);
        let dx = fround(target.position.x - center.x);
        let dy = fround(target.position.y - center.y);
        if (!initialized || id > previous_id)
            && fround(dx * dx + dy * dy) < NATIVE_ETHER_DRAIN_BROAD_RADIUS.powi(2)
            && !state.world_animation_refs.contains(&target.reference)
        {
            state.world_animation_refs.push(target.reference);
        }
        let last_id = animation_id_mut(target.reference, &mut state.last_world_animation_ids);
        *last_id = (*last_id).max(id);
    }
}

#[must_use]
pub fn native_ether_drain_contact(
    field: NativeEtherDrainField<'_>,
    target: &NativeEtherDrainTarget,
) -> Option<NativeEtherDrainContact> {
    let dx = fround(field.position.x - target.position.x);
    let dy = fround(field.position.y - target.position.y);
    let squared = fround(dx * dx + dy * dy);
    if squared > PRESSURE_DISTANCE_SQUARED {
        return None;
    }
    let loose = target.native_flags & 0x400 != 0;
    if loose && target.activation_delay_ticks > 0 {
        return None;
    }
    let falloff = fround((0.1_f32 as f64).max(1.0 - squared / PRESSURE_DISTANCE_SQUARED));
    let strength = fround(falloff * 1.100000023841858);
    let inverse = if squared == 0.0 { 0.0 } else { fround(1.0 / fround(squared.sqrt())) };
    let component = |delta| {
        let mut value = fround(fround(fround(delta * inverse) * strength) * field.alpha);
        if loose {
            value = fround(value * falloff);
        }
        fround(value * target.force_factor)
    };
    let mut amount = if squared < 400.0 && !loose { fround(field.damage / 100.0) } else { 0.0 };
    if squared < 225.0 {
        amount = fround(amount * 2.0);
    }
    if squared < 100.0 {
        amount = fround(amount * 2.0);
    }
    if target.native_flags & 1 != 0 {
        amount = fround(amount * 2.0);
    }
    Some(NativeEtherDrainContact {
        amount,
        damage_flags: 0x10a,
        magic_contact: !loose && squared < 400.0,
        consume: loose && squared < NATIVE_ETHER_DRAIN_CONSUMPTION_DISTANCE_SQUARED,
        delta: Vec2::new(component(dx), component(dy)),
        owner_id: field.owner_id.into(),
        source_actor_id: field.id,
        target: target.reference.clone(),
        hit_strength: 0.0,
    })
}

/// Grounded animations use the Hurricane fast normalization and squared strength.
/// They have no 512-unit pressure cutoff, unlike Puppet targets.
#[must_use]
pub fn native_ether_drain_world_animation_contact(
    field: NativeEtherDrainField<'_>,
    target: NativeEtherDrainWorldAnimationTarget,
) -> Option<NativeEtherDrainWorldAnimationContact> {
    if target.bounce_velocity != 0.0 {
        return None;
    }
    let dx = fround(field.position.x - target.position.x);
    let dy = fround(field.position.y - target.position.y);
    let squared = fround(dx * dx + dy * dy);
    let inverse = fround(1.0 / crate::hurricane::native_hurricane_fast_distance(squared));
    let strength = fround(
        fround((0.1_f32 as f64).max(1.0 - squared / PRESSURE_DISTANCE_SQUARED)) * 1.100000023841858,
    );
    let component =
        |delta| fround(fround(fround(fround(delta * inverse) * strength) * strength) * field.alpha);
    Some(NativeEtherDrainWorldAnimationContact {
        consume: squared < NATIVE_ETHER_DRAIN_CONSUMPTION_DISTANCE_SQUARED,
        delta: Vec2::new(component(dx), component(dy)),
        reference: target.reference,
        source_actor_id: field.id,
    })
}

#[must_use]
pub fn native_ether_drain_enemy_force_factor(family: &str) -> f64 {
    match family {
        "DEMON" => 0.5,
        "DIREFACULTY" => 0.35_f32 as f64,
        "DEMONSKULL" => 0.1_f32 as f64,
        "HEARTMONGER" => 0.25,
        _ => 1.0,
    }
}

#[must_use]
pub fn native_ether_drain_captures_family(family: &str) -> bool {
    matches!(
        family,
        "SKELETON"
            | "SKELETONARCHER"
            | "SKELETONMAGE"
            | "ZOMBIE"
            | "DEMON"
            | "SPIDER"
            | "IMP"
            | "WRAITH"
            | "MAGGOT"
    )
}

/// The private Anim_Manager runs after the field, including children born in this
/// tick. Even a debris fragment expiring now consumes its three source draws.
#[must_use]
pub fn step_native_ether_drain_animations(
    source: &[NativeEtherDrainAnimation],
    center: Vec2,
    rng: &mut NativeRng,
) -> NativeEtherDrainAnimationStep {
    let mut animations = Vec::new();
    let mut pulses = Vec::new();
    for animation in source {
        match *animation {
            NativeEtherDrainAnimation::Captured { alpha, atlas, entry, tint, body_y_offset } => {
                let alpha = fround(alpha - (0.2_f32 as f64));
                if alpha <= 0.0 {
                    pulses.push(1.5);
                } else {
                    animations.push(NativeEtherDrainAnimation::Captured {
                        alpha,
                        atlas,
                        entry,
                        tint,
                        body_y_offset,
                    });
                }
            }
            NativeEtherDrainAnimation::Debris {
                direction,
                remaining_distance,
                speed,
                oscillation_degrees,
                rotation_degrees,
                variant,
                ..
            } => {
                let oscillation = rng.draw_scaled_float(17.0, false);
                let speed_gate = rng.draw_integer(100);
                let rotation = rng.draw_scaled_float(5.0, false);
                let remaining_distance = fround(remaining_distance - speed);
                let mut speed = fround(speed + (0.05_f32 as f64));
                if speed_gate == 3 {
                    speed = fround(speed * (0.5_f32 as f64));
                }
                let oscillation_degrees = fround(oscillation_degrees + 3.0 + oscillation);
                let perpendicular = fround(
                    sin(oscillation_degrees * std::f64::consts::PI / 180.0) * remaining_distance
                        / 7.0,
                );
                if remaining_distance <= 0.0 {
                    pulses.push(0.5);
                    continue;
                }
                animations.push(NativeEtherDrainAnimation::Debris {
                    position: Vec2::new(
                        fround(
                            center.x
                                + remaining_distance * direction.x
                                + perpendicular * direction.y,
                        ),
                        fround(
                            center.y + remaining_distance * direction.y
                                - perpendicular * direction.x,
                        ),
                    ),
                    direction,
                    remaining_distance,
                    speed,
                    oscillation_degrees,
                    rotation_degrees: fround(rotation_degrees + 3.0 + rotation),
                    variant,
                });
            }
        }
    }
    NativeEtherDrainAnimationStep { animations, pulses }
}

pub(super) fn inside_broad_ellipse(center: Vec2, point: Vec2, vertical: f64) -> bool {
    let dx = fround(point.x - center.x);
    let dy = fround(fround(point.y - center.y) / vertical);
    fround(dx * dx + dy * dy) < NATIVE_ETHER_DRAIN_BROAD_RADIUS.powi(2)
}

fn animation_id(
    reference: NativeEtherDrainWorldAnimationRef,
    ids: &NativeEtherDrainWorldAnimationIds,
) -> (u64, u64) {
    let id = match reference {
        NativeEtherDrainWorldAnimationRef::EnemyDeathEffect { id }
        | NativeEtherDrainWorldAnimationRef::PrimaryTransient { id }
        | NativeEtherDrainWorldAnimationRef::SecondaryActor { id }
        | NativeEtherDrainWorldAnimationRef::DeathWeapon { id }
        | NativeEtherDrainWorldAnimationRef::GolemFragment { id, .. } => id,
    };
    (id, *animation_id_ref(reference, ids))
}
fn animation_id_ref(
    reference: NativeEtherDrainWorldAnimationRef,
    ids: &NativeEtherDrainWorldAnimationIds,
) -> &u64 {
    match reference {
        NativeEtherDrainWorldAnimationRef::EnemyDeathEffect { .. } => &ids.enemy_death_effect,
        NativeEtherDrainWorldAnimationRef::PrimaryTransient { .. } => &ids.primary_transient,
        NativeEtherDrainWorldAnimationRef::SecondaryActor { .. } => &ids.secondary_actor,
        NativeEtherDrainWorldAnimationRef::DeathWeapon { .. } => &ids.death_weapon,
        NativeEtherDrainWorldAnimationRef::GolemFragment { .. } => &ids.golem_fragment,
    }
}
fn animation_id_mut(
    reference: NativeEtherDrainWorldAnimationRef,
    ids: &mut NativeEtherDrainWorldAnimationIds,
) -> &mut u64 {
    match reference {
        NativeEtherDrainWorldAnimationRef::EnemyDeathEffect { .. } => &mut ids.enemy_death_effect,
        NativeEtherDrainWorldAnimationRef::PrimaryTransient { .. } => &mut ids.primary_transient,
        NativeEtherDrainWorldAnimationRef::SecondaryActor { .. } => &mut ids.secondary_actor,
        NativeEtherDrainWorldAnimationRef::DeathWeapon { .. } => &mut ids.death_weapon,
        NativeEtherDrainWorldAnimationRef::GolemFragment { .. } => &mut ids.golem_fragment,
    }
}
