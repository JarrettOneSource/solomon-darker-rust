//! Interpolation helpers for display sampling: scalars, vectors, wrapping cycles,
//! angles, and member lists matched by ID.

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

/// Existing members keep the older snapshot's order through a bracket. New members are
/// admitted, and retired members removed, only at the newer endpoint.
#[must_use]
pub(super) fn interpolate_members<'a, T: Clone + 'a, K: Clone + Ord>(
    older: impl IntoIterator<Item = &'a T> + Clone,
    newer: impl IntoIterator<Item = &'a T> + Clone,
    blend: f64,
    id: impl Fn(&T) -> K,
    mut project: impl FnMut(&T, &T, f64) -> T,
) -> Vec<T> {
    interpolate_projected_members(
        older,
        newer,
        blend,
        |member| id(member),
        |member| (*member).clone(),
        |first, next, blend| project(first, next, blend),
    )
}

/// Blends members that appear in both snapshots, matched by `id`, into owned
/// display state. Before the newer endpoint the output keeps the older snapshot's
/// members, copying unmatched ones with `snapshot`; at the endpoint it takes the
/// newer snapshot's members instead. Members are usually borrowed records, so the
/// output type can differ from the member type.
pub(super) fn interpolate_projected_members<V, T, K: Clone + Ord>(
    older: impl IntoIterator<Item = V> + Clone,
    newer: impl IntoIterator<Item = V> + Clone,
    blend: f64,
    id: impl Fn(&V) -> K,
    snapshot: impl Fn(&V) -> T,
    mut project: impl FnMut(&V, &V, f64) -> T,
) -> Vec<T> {
    // Most adjacent snapshots retain the same ordered population. Match those
    // directly before handling births, retirements and reordering.
    if older
        .clone()
        .into_iter()
        .map(|member| id(&member))
        .eq(newer.clone().into_iter().map(|member| id(&member)))
    {
        return older
            .into_iter()
            .zip(newer)
            .map(|(first, next)| project(&first, &next, blend))
            .collect();
    }
    let at_newer_endpoint = blend >= 1.0;
    // Effect IDs increase in admission order. Even as those populations change,
    // merge their ordered IDs directly without building a per-frame lookup tree.
    if older.clone().into_iter().map(|member| id(&member)).is_sorted_by(|a, b| a < b)
        && newer.clone().into_iter().map(|member| id(&member)).is_sorted_by(|a, b| a < b)
    {
        let older = older.into_iter();
        let mut newer = newer.into_iter().peekable();
        let mut result = Vec::with_capacity(older.size_hint().1.unwrap_or(0));
        let mut births = Vec::new();
        for member in older {
            let key = id(&member);
            while newer.peek().is_some_and(|next| id(next) < key) {
                let born = newer.next().unwrap();
                if at_newer_endpoint {
                    births.push(snapshot(&born));
                }
            }
            match newer.peek() {
                Some(next) if id(next) == key => {
                    result.push(project(&member, next, blend));
                    newer.next();
                }
                _ if !at_newer_endpoint => result.push(snapshot(&member)),
                _ => {}
            }
        }
        if at_newer_endpoint {
            result.extend(births);
            result.extend(newer.map(|member| snapshot(&member)));
        }
        return result;
    }
    let newer_by_id: BTreeMap<_, _> =
        newer.clone().into_iter().map(|member| (id(&member), member)).collect();
    let older = older.into_iter();
    let mut result = Vec::with_capacity(older.size_hint().1.unwrap_or(0));
    let mut known = BTreeSet::new();
    for member in older {
        let key = id(&member);
        let projected = match newer_by_id.get(&key) {
            Some(next) => project(&member, next, blend),
            None => snapshot(&member),
        };
        if !at_newer_endpoint || newer_by_id.contains_key(&key) {
            result.push(projected);
        }
        if at_newer_endpoint {
            known.insert(key);
        }
    }
    if at_newer_endpoint {
        for member in newer {
            let key = id(&member);
            if !known.contains(&key) {
                result.push(snapshot(&member));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn members(older: &[(u32, f64)], newer: &[(u32, f64)], blend: f64) -> Vec<(u32, f64)> {
        interpolate_members(
            older,
            newer,
            blend,
            |member| member.0,
            |first, next, blend| (first.0, lerp(first.1, next.1, blend)),
        )
    }

    #[test]
    fn matching_members_interpolate_values_at_both_endpoints_and_between_them() {
        let older = [(40, 2.0), (3, 10.0)];
        let newer = [(40, 6.0), (3, 18.0)];
        assert_eq!(members(&older, &newer, 0.0), older);
        assert_eq!(members(&older, &newer, 0.5), [(40, 4.0), (3, 14.0)]);
        assert_eq!(members(&older, &newer, 1.0), newer);
        assert!(members(&[], &[], 0.5).is_empty());
    }

    #[test]
    fn births_retirements_and_reordering_keep_residents_in_older_order() {
        let older = [(40, 2.0), (3, 10.0), (9, 20.0)];
        let newer = [(3, 18.0), (7, 30.0), (40, 6.0)];
        assert_eq!(members(&older, &newer, 0.5), [(40, 4.0), (3, 14.0), (9, 20.0)]);
        assert_eq!(members(&older, &newer, 1.0), [(40, 6.0), (3, 18.0), (7, 30.0)]);
        assert_eq!(members(&older, &[], 0.5), older);
        assert!(members(&older, &[], 1.0).is_empty());
        assert!(members(&[], &newer, 0.5).is_empty());
        assert_eq!(members(&[], &newer, 1.0), newer);
    }

    #[test]
    fn ordered_populations_keep_births_after_residents_at_the_newer_endpoint() {
        let older = [(3, 10.0), (7, 20.0), (9, 30.0)];
        let newer = [(2, 2.0), (3, 14.0), (8, 8.0), (9, 34.0), (10, 10.0)];
        assert_eq!(members(&older, &newer, 0.5), [(3, 12.0), (7, 20.0), (9, 32.0)]);
        assert_eq!(
            members(&older, &newer, 1.0),
            [(3, 14.0), (9, 34.0), (2, 2.0), (8, 8.0), (10, 10.0)]
        );
    }
}
