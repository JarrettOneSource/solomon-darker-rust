//! The Source host's `boneyard-entrance-navigation.ts` controller.
//!
//! This grid walk precedes policy inference. It borrows the real world's static
//! collision; moving gates are crossed by the explicit approach/crossing phase.

use std::collections::BTreeMap;

use crate::Vec2;
use crate::boneyard::BoneyardBounds;
use crate::boneyard::arena::EntrySide;
use crate::boneyard::collision::{BoneyardCollisionWorld, resolve_boneyard_movement};
use crate::boneyard::encounter::{SolomonPhase, solomon_contact_contains};
use crate::boneyard::gate::INITIAL_SWAY;
use crate::boneyard::world::BoneyardWorld;
use crate::float::hypot;
use crate::hub::PLAYER_CHARACTER_RADIUS;
use crate::session::{GameSession, GameSessionInput};

const GRID_STEP: f64 = 40.0;
const MAXIMUM_GRID_NODES: usize = 25_000;
const REPLAN_INTERVAL_TICKS: u64 = 100;
const WAYPOINT_TOLERANCE: f64 = 20.0;
const GATE_APPROACH_MARGIN: f64 = PLAYER_CHARACTER_RADIUS + INITIAL_SWAY + 15.0;
const GATE_CROSSED_MARGIN: f64 = PLAYER_CHARACTER_RADIUS + 35.0;
const DIRECTIONS: [(i32, i32); 8] =
    [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

#[derive(Clone, Debug, PartialEq)]
pub enum MlBotEntranceNavigationError {
    NoPlayer(String),
    /// The host formats points with its canonical ECMAScript JSON writer. `None`
    /// names Solomon, whose finish condition is his contact ellipse, not a point.
    NoRoute {
        start: Vec2,
        target: Option<Vec2>,
    },
}

#[derive(Clone, Debug, Default)]
pub struct BoneyardEntranceNavigator {
    planned_at_tick: Option<u64>,
    route: Vec<Vec2>,
    route_target_key: Option<String>,
}

impl BoneyardEntranceNavigator {
    pub fn input(
        &mut self,
        session: &GameSession,
        player_id: &str,
    ) -> Result<Option<GameSessionInput>, MlBotEntranceNavigationError> {
        let Some(world) = &session.boneyard else {
            self.reset();
            return Ok(None);
        };
        let Some(encounter) = &world.encounter else {
            self.reset();
            return Ok(None);
        };
        if encounter.run_event_id > 0 {
            self.reset();
            return Ok(None);
        }
        if encounter.phase != SolomonPhase::Digging {
            return Ok(Some(GameSessionInput::default()));
        }
        let position = session
            .wizard(player_id)
            .ok_or_else(|| MlBotEntranceNavigationError::NoPlayer(player_id.to_owned()))?
            .position;
        let bounds = world
            .arena_transition
            .map_or_else(|| world.active_bounds(), |transition| transition.full_bounds);
        if let Some((fence_id, gate)) = entry_gate(world) {
            let direction = if world
                .arena_transition
                .is_some_and(|transition| transition.entry_side == EntrySide::South)
            {
                -1.0
            } else {
                1.0
            };
            let crossed = (position.y - gate.y) * direction > GATE_CROSSED_MARGIN;
            if !crossed {
                let remaining = (gate.y - position.y) * direction;
                if remaining <= GATE_APPROACH_MARGIN + 10.0 && (gate.x - position.x).abs() <= 40.0 {
                    return Ok(Some(movement_input(
                        position,
                        Vec2::new(gate.x, gate.y + direction * GATE_CROSSED_MARGIN),
                    )));
                }
                return self
                    .route_input(
                        session.ticks,
                        position,
                        Vec2::new(gate.x, gate.y - direction * GATE_APPROACH_MARGIN),
                        bounds,
                        &world.collision,
                        format!("gate:{fence_id}"),
                        false,
                    )
                    .map(Some);
            }
        }
        self.route_input(
            session.ticks,
            position,
            encounter.position,
            bounds,
            &world.collision,
            format!("solomon:{}", world.loaded.run_id),
            true,
        )
        .map(Some)
    }

    #[allow(clippy::too_many_arguments)]
    fn route_input(
        &mut self,
        tick: u64,
        position: Vec2,
        target: Vec2,
        bounds: BoneyardBounds,
        collision: &BoneyardCollisionWorld,
        target_key: String,
        solomon: bool,
    ) -> Result<GameSessionInput, MlBotEntranceNavigationError> {
        if self.route_target_key.as_ref() != Some(&target_key)
            || self.route.len() < 2
            || self
                .planned_at_tick
                .is_none_or(|planned| tick.saturating_sub(planned) >= REPLAN_INTERVAL_TICKS)
        {
            self.route_target_key = Some(target_key);
            self.planned_at_tick = Some(tick);
            self.route = plan_route(position, target, bounds, collision, solomon)?;
        }
        let mut waypoint_index = 1;
        while waypoint_index < self.route.len() - 1
            && distance(position, self.route[waypoint_index]) <= WAYPOINT_TOLERANCE
        {
            waypoint_index += 1;
        }
        Ok(self
            .route
            .get(waypoint_index)
            .map_or_else(GameSessionInput::default, |waypoint| movement_input(position, *waypoint)))
    }

    pub fn reset(&mut self) {
        self.planned_at_tick = None;
        self.route.clear();
        self.route_target_key = None;
    }
}

struct GridPoint {
    grid: (i32, i32),
    point: Vec2,
    parent: Option<usize>,
}

fn plan_route(
    start: Vec2,
    target: Vec2,
    bounds: BoneyardBounds,
    collision: &BoneyardCollisionWorld,
    solomon: bool,
) -> Result<Vec<Vec2>, MlBotEntranceNavigationError> {
    let mut visited = BTreeMap::from([((0, 0), 0)]);
    let mut queue = vec![GridPoint { grid: (0, 0), point: start, parent: None }];
    let mut cursor = 0;
    while cursor < queue.len() && cursor < MAXIMUM_GRID_NODES {
        let point = queue[cursor].point;
        let finished = if solomon {
            solomon_contact_contains(target, point)
        } else {
            distance(point, target) <= GRID_STEP && traverses(point, target, bounds, collision)
        };
        if finished {
            let mut route = Vec::new();
            let mut parent = Some(cursor);
            while let Some(index) = parent {
                route.push(queue[index].point);
                parent = queue[index].parent;
            }
            route.reverse();
            if !solomon {
                route.push(target);
            }
            return Ok(simplify_route(&route, bounds, collision));
        }
        let (grid_x, grid_y) = queue[cursor].grid;
        let mut directions = DIRECTIONS;
        // Slice sort is stable, preserving the Source direction order on ties.
        directions.sort_by(|left, right| {
            let left = Vec2::new(
                start.x + f64::from(grid_x + left.0) * GRID_STEP,
                start.y + f64::from(grid_y + left.1) * GRID_STEP,
            );
            let right = Vec2::new(
                start.x + f64::from(grid_x + right.0) * GRID_STEP,
                start.y + f64::from(grid_y + right.1) * GRID_STEP,
            );
            distance(left, target).total_cmp(&distance(right, target))
        });
        for direction in directions {
            let grid = (grid_x + direction.0, grid_y + direction.1);
            if visited.contains_key(&grid) {
                continue;
            }
            let next = Vec2::new(
                start.x + f64::from(grid.0) * GRID_STEP,
                start.y + f64::from(grid.1) * GRID_STEP,
            );
            if !traverses(point, next, bounds, collision) {
                continue;
            }
            visited.insert(grid, queue.len());
            queue.push(GridPoint { grid, point: next, parent: Some(cursor) });
        }
        cursor += 1;
    }
    Err(MlBotEntranceNavigationError::NoRoute { start, target: (!solomon).then_some(target) })
}

fn entry_gate(world: &BoneyardWorld) -> Option<(&str, Vec2)> {
    // A Vec preserves the first encounter of each fence ID, as Source's Map does.
    let mut groups: Vec<(&str, Vec<Vec2>)> = Vec::new();
    for leaf in &world.gate_leaves {
        if let Some((_, hinges)) = groups.iter_mut().find(|(id, _)| *id == leaf.fence_eid) {
            hinges.push(leaf.hinge);
        } else {
            groups.push((&leaf.fence_eid, vec![leaf.hinge]));
        }
    }
    let mut centers = Vec::new();
    for (fence_id, hinges) in groups {
        if hinges.len() == 2 {
            centers.push((
                fence_id,
                Vec2::new((hinges[0].x + hinges[1].x) / 2.0, (hinges[0].y + hinges[1].y) / 2.0),
            ));
        }
    }
    let spawn = world.loaded.scene.spawn.position;
    centers.sort_by(|left, right| distance(left.1, spawn).total_cmp(&distance(right.1, spawn)));
    centers.first().copied()
}

fn simplify_route(
    route: &[Vec2],
    bounds: BoneyardBounds,
    collision: &BoneyardCollisionWorld,
) -> Vec<Vec2> {
    let mut simplified = vec![route[0]];
    let mut current = 0;
    while current < route.len() - 1 {
        let mut next = route.len() - 1;
        while next > current + 1 && !traverses(route[current], route[next], bounds, collision) {
            next -= 1;
        }
        simplified.push(route[next]);
        current = next;
    }
    simplified
}

fn traverses(
    start: Vec2,
    target: Vec2,
    bounds: BoneyardBounds,
    collision: &BoneyardCollisionWorld,
) -> bool {
    let steps = (distance(start, target) / 8.0).ceil() as usize;
    let mut current = start;
    for step in 1..=steps {
        let requested = Vec2::new(
            start.x + (target.x - start.x) * step as f64 / steps as f64,
            start.y + (target.y - start.y) * step as f64 / steps as f64,
        );
        let resolved = resolve_boneyard_movement(
            current,
            requested,
            bounds,
            collision,
            PLAYER_CHARACTER_RADIUS,
        );
        if distance(resolved, requested) > 0.25 {
            return false;
        }
        current = resolved;
    }
    true
}

fn movement_input(position: Vec2, target: Vec2) -> GameSessionInput {
    let dx = target.x - position.x;
    let dy = target.y - position.y;
    let length = hypot(dx, dy);
    GameSessionInput {
        movement: if length > 0.0 { Vec2::new(dx / length, dy / length) } else { Vec2::ZERO },
        ..GameSessionInput::default()
    }
}

fn distance(left: Vec2, right: Vec2) -> f64 {
    hypot(right.x - left.x, right.y - left.y)
}
