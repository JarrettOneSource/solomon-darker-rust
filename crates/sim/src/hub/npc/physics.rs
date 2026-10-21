//! Courtyard geometry adapter for the shared source actor-response solver.

use crate::Vec2;
use crate::actor_physics::{ActorPhysicsBody, ActorPhysicsWorld};
use crate::hub::{is_hub_traversable, move_with_hub_collision};

use super::HubStudentState;

#[derive(Clone, Copy, Debug)]
pub(super) struct ActorBody {
    pub position: Vec2,
    delta: Vec2,
    radius: f64,
    push_resistance: f64,
    push_strength: f64,
    driven: bool,
    static_collision_enabled: bool,
    pub player_collision_enabled: bool,
}

impl ActorBody {
    pub fn wizard(position: Vec2, delta: Vec2, static_collision_enabled: bool) -> Self {
        Self {
            position,
            delta,
            radius: 25.0,
            push_resistance: 10.0,
            push_strength: 12.0,
            driven: true,
            static_collision_enabled,
            player_collision_enabled: true,
        }
    }

    pub fn student(student: &HubStudentState, delta: Vec2) -> Self {
        Self {
            position: student.position,
            delta,
            radius: student.profile.radius,
            push_resistance: student.profile.push_resistance,
            push_strength: student.profile.push_strength,
            driven: true,
            static_collision_enabled: student.static_collision_enabled,
            player_collision_enabled: true,
        }
    }

    pub fn fixed(position: Vec2, radius: f64) -> Self {
        Self {
            position,
            delta: Vec2::ZERO,
            radius,
            push_resistance: 90.0,
            push_strength: 0.0,
            driven: false,
            static_collision_enabled: true,
            player_collision_enabled: true,
        }
    }
}

pub(super) fn resolve_actor_motion(bodies: &mut [ActorBody], rng: &mut u32) {
    let static_collision: Vec<_> =
        bodies.iter().map(|body| body.static_collision_enabled).collect();
    let mut shared_bodies: Vec<_> = bodies
        .iter()
        .enumerate()
        .map(|(index, body)| ActorPhysicsBody {
            collision_recipient: true,
            id: index.to_string(),
            position: body.position,
            delta: body.delta,
            radius: body.radius,
            push_resistance: body.push_resistance,
            push_strength: body.push_strength,
            driven: body.driven,
            push_enabled: true,
        })
        .collect();
    let mut world = HubPhysicsWorld { static_collision: &static_collision, rng };
    // The College walker occupies index0 before every student. The source bypasses
    // student/player pairs during onboarding while preserving its fixed trader contacts.
    crate::actor_physics::resolve_actor_motion(
        &mut shared_bodies,
        &mut world,
        |mover, other| {
            let mover: usize = mover.id.parse().expect("Hub body uses its ordered index");
            let other: usize = other.id.parse().expect("Hub body uses its ordered index");
            (mover != 0 || bodies[other].player_collision_enabled)
                && (other != 0 || bodies[mover].player_collision_enabled)
        },
        None,
    );
    for (body, resolved) in bodies.iter_mut().zip(shared_bodies) {
        body.position = resolved.position;
    }
}

struct HubPhysicsWorld<'a> {
    static_collision: &'a [bool],
    rng: &'a mut u32,
}

impl HubPhysicsWorld<'_> {
    fn static_collision_enabled(&self, body_id: &str) -> bool {
        let index: usize = body_id.parse().expect("Hub physics body id is its ordered index");
        self.static_collision[index]
    }
}

impl ActorPhysicsWorld for HubPhysicsWorld<'_> {
    fn can_place(&self, body_id: &str, position: Vec2, radius: f64) -> bool {
        !self.static_collision_enabled(body_id) || is_hub_traversable(position, radius)
    }

    fn move_body(&mut self, body_id: &str, position: Vec2, delta: Vec2, radius: f64) -> Vec2 {
        if self.static_collision_enabled(body_id) {
            let moved = move_with_hub_collision(position, delta, radius, *self.rng);
            *self.rng = moved.rng_state;
            moved.position
        } else {
            Vec2::new(position.x + delta.x, position.y + delta.y)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_trader_separates_the_player_without_moving() {
        let trader = Vec2::new(1000.0, 600.0);
        let mut bodies = [
            ActorBody::wizard(Vec2::new(1030.0, 600.0), Vec2::ZERO, false),
            ActorBody::fixed(trader, 15.0),
        ];
        resolve_actor_motion(&mut bodies, &mut 1);
        assert_eq!(bodies[0].position, Vec2::new(1040.1, 600.0));
        assert_eq!(bodies[1].position, trader);
    }
}
