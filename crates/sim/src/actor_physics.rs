//! Ordered actor circle response from Website `core-kernels/actor-physics.ts`.
//!
//! This extracts the verified Courtyard solver for use by Boneyard players, enemies and the
//! lantern. Root moves use swept world collision; pair corrections require their entire
//! candidate to be placeable. Each recursive recipient is visited once per root epoch.

use crate::Vec2;

pub const NATIVE_ACTOR_SEPARATION_EPSILON: f64 = 0.1;

#[derive(Clone, Debug, PartialEq)]
pub struct ActorPhysicsBody {
    /// Native Actor +0x36 admits this body as another mover's contact candidate.
    pub collision_recipient: bool,
    pub id: String,
    pub position: Vec2,
    pub delta: Vec2,
    pub radius: f64,
    pub push_resistance: f64,
    pub push_strength: f64,
    pub driven: bool,
    pub push_enabled: bool,
}

pub trait ActorPhysicsWorld {
    fn can_place(&self, body_id: &str, position: Vec2, radius: f64) -> bool;
    fn move_body(&mut self, body_id: &str, position: Vec2, delta: Vec2, radius: f64) -> Vec2;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActorRootContact {
    pub mover_id: String,
    pub other_id: String,
}

pub fn resolve_actor_motion(
    bodies: &mut [ActorPhysicsBody],
    world: &mut impl ActorPhysicsWorld,
    should_collide: impl Fn(&ActorPhysicsBody, &ActorPhysicsBody) -> bool,
    root_contacts: Option<&mut Vec<ActorRootContact>>,
) {
    let strengths = bodies.iter().map(|body| body.push_strength).collect();
    let mut solver = ActorMotionSolver { bodies, world, should_collide, strengths, root_contacts };
    for index in 0..solver.bodies.len() {
        if !solver.bodies[index].driven {
            continue;
        }
        let mut recipients = vec![false; solver.bodies.len()];
        let delta = solver.bodies[index].delta;
        solver.move_body(index, delta, &mut recipients, false);
    }
}

/// Enemy root movement is the source's non-pushing specialization. Pair corrections keep
/// ascending body order and never mutate the crowd, including when a correction is blocked.
#[must_use]
pub fn resolve_unpushed_mover_motion(
    bodies: &[ActorPhysicsBody],
    mover_index: usize,
    delta: Vec2,
    world: &mut impl ActorPhysicsWorld,
) -> Vec2 {
    let mover = &bodies[mover_index];
    assert!(!mover.push_enabled, "unpushed mover motion requires push_enabled == false");
    let mut position = world.move_body(&mover.id, mover.position, delta, mover.radius);
    for (other_index, other) in bodies.iter().enumerate() {
        if other_index == mover_index || !other.collision_recipient {
            continue;
        }
        let correction =
            actor_separation(position, mover.radius, other.position, other.radius, false);
        if let Some(candidate) =
            placed_correction(&mover.id, position, mover.radius, correction, world)
        {
            position = candidate;
        }
    }
    position
}

struct ActorMotionSolver<'a, W, F> {
    bodies: &'a mut [ActorPhysicsBody],
    world: &'a mut W,
    should_collide: F,
    strengths: Vec<f64>,
    root_contacts: Option<&'a mut Vec<ActorRootContact>>,
}

impl<W: ActorPhysicsWorld, F: Fn(&ActorPhysicsBody, &ActorPhysicsBody) -> bool>
    ActorMotionSolver<'_, W, F>
{
    fn move_body(&mut self, index: usize, delta: Vec2, recipients: &mut [bool], recursive: bool) {
        if recursive {
            if recipients[index] {
                return;
            }
            recipients[index] = true;
        } else {
            self.strengths[index] = self.bodies[index].push_strength;
        }
        let mover = &self.bodies[index];
        self.bodies[index].position =
            self.world.move_body(&mover.id, mover.position, delta, mover.radius);
        for other in 0..self.bodies.len() {
            if index == other
                || !self.bodies[other].collision_recipient
                || !(self.should_collide)(&self.bodies[index], &self.bodies[other])
            {
                continue;
            }
            if (!recursive && !self.bodies[index].push_enabled)
                || self.strengths[index] < self.bodies[other].push_resistance
            {
                let correction = separation(&self.bodies[index], &self.bodies[other], false);
                if self.apply_correction(index, correction) && !recursive {
                    self.record_contact(index, other);
                }
                continue;
            }

            let correction = separation(&self.bodies[other], &self.bodies[index], true);
            if correction != Vec2::ZERO {
                if !recursive {
                    self.record_contact(index, other);
                }
                let resistance = self.bodies[other].push_resistance;
                let factor =
                    if resistance > 0.0 { self.strengths[index] / (resistance * 2.0) } else { 1.0 }
                        .clamp(0.0, 1.0);
                self.strengths[other] = self.strengths[index] * factor;
                self.move_body(
                    other,
                    Vec2::new(correction.x * factor, correction.y * factor),
                    recipients,
                    true,
                );
            }
            // A recursive push may have changed either body. Re-read the positions here.
            let correction = separation(&self.bodies[index], &self.bodies[other], true);
            self.apply_correction(index, correction);
        }
    }

    fn apply_correction(&mut self, index: usize, correction: Vec2) -> bool {
        let body = &self.bodies[index];
        let Some(candidate) =
            placed_correction(&body.id, body.position, body.radius, correction, self.world)
        else {
            return false;
        };
        self.bodies[index].position = candidate;
        true
    }

    fn record_contact(&mut self, mover_index: usize, other_index: usize) {
        if let Some(contacts) = &mut self.root_contacts {
            contacts.push(ActorRootContact {
                mover_id: self.bodies[mover_index].id.clone(),
                other_id: self.bodies[other_index].id.clone(),
            });
        }
    }
}

fn separation(body: &ActorPhysicsBody, other: &ActorPhysicsBody, weighted: bool) -> Vec2 {
    actor_separation(body.position, body.radius, other.position, other.radius, weighted)
}

fn actor_separation(
    position: Vec2,
    radius: f64,
    other_position: Vec2,
    other_radius: f64,
    weighted: bool,
) -> Vec2 {
    let dx = position.x - other_position.x;
    let dy = position.y - other_position.y;
    let squared = dx * dx + dy * dy;
    let radii = radius + other_radius;
    let radii_squared = radii * radii;
    if squared >= radii_squared || squared == 0.0 {
        return Vec2::ZERO;
    }
    let distance = squared.sqrt();
    let overlap = radii + NATIVE_ACTOR_SEPARATION_EPSILON - distance;
    // JavaScript's `ratio ** 4` uses its floating exponent operation. Integer-power
    // multiplication changes the last bit of weighted corrections in dense crowds.
    let weight = if weighted { (squared / radii_squared).powf(4.0) * 0.99 + 0.01 } else { 1.0 };
    Vec2::new(dx / distance * overlap * weight, dy / distance * overlap * weight)
}

fn placed_correction(
    body_id: &str,
    position: Vec2,
    radius: f64,
    correction: Vec2,
    world: &impl ActorPhysicsWorld,
) -> Option<Vec2> {
    if correction == Vec2::ZERO {
        return None;
    }
    let candidate = Vec2::new(position.x + correction.x, position.y + correction.y);
    if world.can_place(body_id, candidate, radius) { Some(candidate) } else { None }
}
