//! Solomon's collision and navigation phase from `core-server/boneyard-world-placement.ts`.
//!
//! The encounter kernel requests motion. This world phase keeps the fixed escape target,
//! routes around static scenery, resolves against current gates and releases only the scenery
//! overlaps that existed when escape began.

use crate::heading::heading_degrees_from_vector;

use super::BoneyardBounds;
use super::collision::{
    BoneyardCollisionWorld, boneyard_body_collision_source_ids, first_boneyard_line_obstruction,
    first_boneyard_path_block_progress, resolve_boneyard_movement_ignoring,
};
use super::encounter::{
    SOLOMON_COLLISION_RADIUS, SOLOMON_ESCAPE_PATH_MARGIN,
    SOLOMON_ESCAPE_ROUTE_ARRIVAL_DISTANCE_SQUARED, SOLOMON_NAVIGATION_CLEARANCE, SolomonEncounter,
    SolomonPhase, native_solomon_escape_path_target, native_solomon_escape_target,
};
use super::navigation::{NavigationSet, RouteRequest};

pub fn prepare_solomon_escape_navigation(
    source: &mut SolomonEncounter,
    bounds: BoneyardBounds,
    collision: &BoneyardCollisionWorld,
    navigation_collision: &BoneyardCollisionWorld,
    navigation: &NavigationSet,
) {
    let target = source.escape_target.unwrap_or_else(|| {
        native_solomon_escape_target(source.position, source.heading_degrees, bounds)
    });
    if source.escape_target.is_none() {
        source.escape_collision_source_ids = boneyard_body_collision_source_ids(
            source.position,
            collision,
            SOLOMON_COLLISION_RADIUS,
        );
    }
    let path_target = native_solomon_escape_path_target(source.position, target, bounds);
    let traversal_bounds = solomon_escape_traversal_bounds(bounds);
    let ignored: Vec<&str> =
        source.escape_collision_source_ids.iter().map(String::as_str).collect();
    let blocked = first_boneyard_path_block_progress(
        source.position,
        path_target,
        traversal_bounds,
        collision,
        SOLOMON_COLLISION_RADIUS,
        &ignored,
    )
    .is_some();
    let route = if blocked {
        navigation.find_route(
            RouteRequest {
                body_radius: SOLOMON_COLLISION_RADIUS,
                bounds: traversal_bounds,
                clearance: SOLOMON_NAVIGATION_CLEARANCE,
                end: path_target,
                end_body_radius: None,
                ignored_source_ids: &ignored,
                start: source.position,
            },
            navigation_collision,
        )
    } else {
        None
    };
    let mut goal = path_target;
    if let Some(route) = route.filter(|route| route.len() >= 2) {
        goal = route[1];
        for candidate in &route[2..] {
            if first_boneyard_line_obstruction(
                source.position,
                *candidate,
                traversal_bounds,
                collision,
                None,
                0,
                &ignored,
            )
            .is_some()
            {
                break;
            }
            goal = *candidate;
        }
    }
    source.escape_target = Some(target);
    source.heading_degrees =
        heading_degrees_from_vector(goal.x - source.position.x, goal.y - source.position.y);
}

pub fn resolve_solomon_escape_movement(
    source: &SolomonEncounter,
    next: &mut SolomonEncounter,
    bounds: BoneyardBounds,
    collision: &BoneyardCollisionWorld,
) {
    let path_target = source
        .escape_target
        .map(|target| native_solomon_escape_path_target(source.position, target, bounds));
    let target_reached = path_target.is_some_and(|target| {
        let dx = source.position.x - target.x;
        let dy = source.position.y - target.y;
        dx * dx + dy * dy <= SOLOMON_ESCAPE_ROUTE_ARRIVAL_DISTANCE_SQUARED
    });
    let ignored: Vec<&str> =
        source.escape_collision_source_ids.iter().map(String::as_str).collect();
    next.position = if target_reached {
        source.position
    } else {
        resolve_boneyard_movement_ignoring(
            source.position,
            next.position,
            solomon_escape_traversal_bounds(bounds),
            collision,
            SOLOMON_COLLISION_RADIUS,
            &ignored,
        )
    };
    if next.phase == SolomonPhase::Gone {
        return;
    }
    let active_source_ids =
        boneyard_body_collision_source_ids(next.position, collision, SOLOMON_COLLISION_RADIUS);
    next.escape_collision_source_ids = source
        .escape_collision_source_ids
        .iter()
        .filter(|source_id| active_source_ids.contains(source_id))
        .cloned()
        .collect();
}

#[must_use]
pub fn solomon_escape_traversal_bounds(bounds: BoneyardBounds) -> BoneyardBounds {
    // The resolver insets by body radius. Compensate so the center reaches the source
    // state-4 rectangle expanded by its separate 50-unit path margin.
    let padding = SOLOMON_ESCAPE_PATH_MARGIN + SOLOMON_COLLISION_RADIUS;
    BoneyardBounds::new(
        bounds.x - padding,
        bounds.y - padding,
        bounds.width + padding * 2.0,
        bounds.height + padding * 2.0,
    )
}
