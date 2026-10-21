//! Secondary actor and articulated Golem projection from
//! `client/native-secondary-presentation.ts`.

use solomon_sim::secondary::NativeSecondarySimulationState;

use super::PresentationClocks;
use super::math::{interpolate_members, lerp, lerp_vec};

pub(super) fn project(
    output: &mut NativeSecondarySimulationState,
    older: &NativeSecondarySimulationState,
    newer: &NativeSecondarySimulationState,
    blend: f64,
    clocks: &mut PresentationClocks,
) {
    clocks.secondary.clear();
    output.actors = interpolate_members(
        &older.actors,
        &newer.actors,
        blend,
        |actor| actor.id,
        |first, next, blend| {
            // Source keeps the older row when an ID changes kind; no invented
            // halfway actor may cross that discrete presentation boundary.
            if first.kind != next.kind {
                return first.clone();
            }
            let mut actor = if blend < 1.0 { first.clone() } else { next.clone() };
            let age = lerp(first.age_ticks as f64, next.age_ticks as f64, blend);
            clocks.secondary.insert(actor.id, age);
            actor.age_ticks = age.floor() as u64;
            actor.alpha = lerp(first.alpha, next.alpha, blend);
            actor.endpoint = lerp_vec(first.endpoint, next.endpoint, blend);
            actor.midpoint = lerp_vec(first.midpoint, next.midpoint, blend);
            actor.phase = lerp(first.phase, next.phase, blend);
            actor.position = lerp_vec(first.position, next.position, blend);
            actor.radius = lerp(first.radius, next.radius, blend);
            let full = std::f64::consts::PI * 2.0;
            let delta = ((next.rotation_radians - first.rotation_radians + std::f64::consts::PI)
                % full
                + full)
                % full
                - std::f64::consts::PI;
            actor.rotation_radians = first.rotation_radians + delta * blend;
            actor.scale = lerp(first.scale, next.scale, blend);
            actor.velocity = lerp_vec(first.velocity, next.velocity, blend);
            if let (Some(first), Some(next), Some(golem)) =
                (&first.golem, &next.golem, &mut actor.golem)
            {
                golem.left_connector_offset =
                    lerp_vec(first.left_connector_offset, next.left_connector_offset, blend);
                golem.left_foot = lerp_vec(first.left_foot, next.left_foot, blend);
                golem.left_foot_bob = lerp_vec(first.left_foot_bob, next.left_foot_bob, blend);
                golem.left_foot_next = lerp_vec(first.left_foot_next, next.left_foot_next, blend);
                golem.left_foot_previous =
                    lerp_vec(first.left_foot_previous, next.left_foot_previous, blend);
                golem.left_foot_progress =
                    lerp(first.left_foot_progress, next.left_foot_progress, blend);
                golem.left_foot_rotation_degrees =
                    lerp(first.left_foot_rotation_degrees, next.left_foot_rotation_degrees, blend);
                golem.right_connector_offset =
                    lerp_vec(first.right_connector_offset, next.right_connector_offset, blend);
                golem.right_foot = lerp_vec(first.right_foot, next.right_foot, blend);
                golem.right_foot_bob = lerp_vec(first.right_foot_bob, next.right_foot_bob, blend);
                golem.right_foot_next =
                    lerp_vec(first.right_foot_next, next.right_foot_next, blend);
                golem.right_foot_previous =
                    lerp_vec(first.right_foot_previous, next.right_foot_previous, blend);
                golem.right_foot_progress =
                    lerp(first.right_foot_progress, next.right_foot_progress, blend);
                golem.right_foot_rotation_degrees = lerp(
                    first.right_foot_rotation_degrees,
                    next.right_foot_rotation_degrees,
                    blend,
                );
            }
            actor
        },
    );
    clocks.secondary.retain(|id, _| output.actors.iter().any(|actor| actor.id == *id));
    for actor in &output.actors {
        clocks.secondary.entry(actor.id).or_insert(actor.age_ticks as f64);
    }
}
