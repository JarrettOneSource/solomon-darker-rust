//! Scalar/cycle and stable-member projection from `client/presentation-math.ts`
//! and `client/hub-presentation-timeline.ts::lerpCycle`.

use std::collections::{BTreeMap, BTreeSet};

use solomon_sim::Vec2;

#[must_use]
pub(super) fn lerp(first: f64, second: f64, blend: f64) -> f64 {
    first + (second - first) * blend
}

#[must_use]
pub(super) fn lerp_vec(first: Vec2, second: Vec2, blend: f64) -> Vec2 {
    Vec2::new(lerp(first.x, second.x, blend), lerp(first.y, second.y, blend))
}

#[must_use]
pub(super) fn lerp_cycle(first: f64, second: f64, blend: f64, period: f64) -> f64 {
    let modulo = |value: f64| ((value % period) + period) % period;
    let first = modulo(first);
    let second = modulo(second);
    let mut delta = second - first;
    if delta > period / 2.0 {
        delta -= period;
    } else if delta < -period / 2.0 {
        delta += period;
    }
    modulo(first + delta * blend)
}

#[must_use]
pub(super) fn lerp_degrees(first: f64, second: f64, blend: f64) -> f64 {
    let delta = ((second - first + 540.0) % 360.0) - 180.0;
    first + delta * blend
}

#[must_use]
pub(super) fn lerp_forward_cycle(first: f64, second: f64, blend: f64, period: f64) -> f64 {
    let delta = ((second - first) % period + period) % period;
    ((first + delta * blend) % period + period) % period
}

#[must_use]
pub(super) fn lerp_option(first: Option<f64>, second: Option<f64>, blend: f64) -> Option<f64> {
    match (first, second) {
        (Some(first), Some(second)) => Some(lerp(first, second, blend)),
        _ if blend < 1.0 => first,
        _ => second,
    }
}

/// Existing members keep their source order through a bracket. New members are
/// admitted, and retired members removed, only at the newer endpoint.
#[must_use]
pub(super) fn interpolate_members<T: Clone, K: Clone + Ord>(
    older: &[T],
    newer: &[T],
    blend: f64,
    id: impl Fn(&T) -> K,
    mut project: impl FnMut(&T, &T, f64) -> T,
) -> Vec<T> {
    let newer_by_id: BTreeMap<_, _> = newer.iter().map(|member| (id(member), member)).collect();
    let mut result = Vec::with_capacity(older.len());
    for member in older {
        result.push(match newer_by_id.get(&id(member)) {
            Some(next) => project(member, next, blend),
            None => member.clone(),
        });
    }
    if blend >= 1.0 {
        let known: BTreeSet<_> = result.iter().map(&id).collect();
        for member in newer {
            if !known.contains(&id(member)) {
                result.push(member.clone());
            }
        }
        result.retain(|member| newer_by_id.contains_key(&id(member)));
    }
    result
}
