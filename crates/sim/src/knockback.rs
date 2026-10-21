//! Shared Staff/Golem area recurrence from current `native-knockback-area.ts`.
//! The world movement callback commits immediately: new contacts and later position
//! queries belong to this same update, rather than a deferred displacement batch.

use std::collections::BTreeMap;

use crate::float::fround;
use crate::{NativeRng, Vec2};

pub trait NativeKnockbackAreaWorld<T> {
    fn position(&self, id: &T) -> Option<Vec2>;
    /// Source permits the pure no-movement query. Live world owners override it.
    fn move_actor(&mut self, _id: &T, _delta: Vec2) -> Vec<T> {
        Vec::new()
    }
}
impl<T: Ord> NativeKnockbackAreaWorld<T> for BTreeMap<T, Vec2> {
    fn position(&self, id: &T) -> Option<Vec2> {
        self.get(id).copied()
    }
}

pub struct NativeKnockbackArea<'a, T> {
    pub origin: Vec2,
    pub remaining_distance: f64,
    pub target_ids: &'a [T],
}
#[derive(Clone, Debug, PartialEq)]
pub struct NativeKnockbackAreaDisplacement<T> {
    pub target_id: T,
    pub delta: Vec2,
}
#[derive(Clone, Debug, PartialEq)]
pub struct NativeKnockbackAreaHeading<T> {
    pub target_id: T,
    pub heading_degrees: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct NativeKnockbackAreaStep<T> {
    pub camera_displacement: Vec2,
    pub displacements: Vec<NativeKnockbackAreaDisplacement<T>>,
    pub heading_perturbations: Vec<NativeKnockbackAreaHeading<T>>,
    pub remaining_distance: f64,
    pub target_ids: Vec<T>,
    pub terminal: bool,
}

pub fn step_native_knockback_area<T: Clone + Eq>(
    source: NativeKnockbackArea<'_, T>,
    rng: &mut NativeRng,
    world: &mut impl NativeKnockbackAreaWorld<T>,
) -> NativeKnockbackAreaStep<T> {
    let vector = rng.draw_unit_vector();
    let camera_displacement = Vec2::new(fround(vector.x * 10.0), fround(vector.y * 10.0));
    let mut target_ids = source.target_ids.to_vec();
    let mut seen = target_ids.clone();
    let mut displacements = Vec::new();
    let distance = source.remaining_distance.min(10.0);
    let mut index = 0;
    while index < target_ids.len() {
        let id = target_ids[index].clone();
        let Some(position) = world.position(&id) else {
            target_ids.remove(index);
            continue;
        };
        let dx = fround(position.x - source.origin.x);
        let dy = fround(position.y - source.origin.y);
        let magnitude = fround(fround(dx * dx + dy * dy).sqrt());
        let reciprocal = if magnitude > 0.0 { fround(1.0 / magnitude) } else { 0.0 };
        let delta =
            Vec2::new(fround(distance * reciprocal * dx), fround(distance * reciprocal * dy));
        displacements.push(NativeKnockbackAreaDisplacement { target_id: id.clone(), delta });
        for collided in world.move_actor(&id, delta) {
            if !seen.contains(&collided) {
                seen.push(collided.clone());
                target_ids.push(collided);
            }
        }
        index += 1;
    }
    let remaining_distance = fround(source.remaining_distance - 10.0);
    let terminal = remaining_distance <= 0.0;
    let mut heading_perturbations = Vec::new();
    if terminal {
        for id in &target_ids {
            if world.position(id).is_some() {
                heading_perturbations.push(NativeKnockbackAreaHeading {
                    target_id: id.clone(),
                    heading_degrees: rng.draw_scaled_float(45.0, true),
                });
            }
        }
    }
    NativeKnockbackAreaStep {
        camera_displacement,
        displacements,
        heading_perturbations,
        remaining_distance,
        target_ids,
        terminal,
    }
}
