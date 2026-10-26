//! The field's mixed Puppet/loose-object force transaction from `boneyard-world-placement.ts`.

use super::*;
use crate::actor_physics::ActorPhysicsWorld;
use crate::boneyard::collision::resolve_boneyard_movement;
use crate::float::fround;
use crate::secondary::ether_drain::{NativeEtherDrainContact, NativeEtherDrainTargetRef};
use crate::secondary::{NativeSecondaryActorKind, NativeSecondarySimulationState};

pub struct BoneyardEtherDrainForces {
    pub consumed_field_actor_ids: Vec<u64>,
    pub player_positions: BTreeMap<String, Vec2>,
}

impl BoneyardWorld {
    pub fn apply_ether_drain_forces(
        &mut self,
        contacts: &[NativeEtherDrainContact],
        players: &[BoneyardPlayerBody<'_>],
        secondary: &mut NativeSecondarySimulationState,
    ) -> BoneyardEtherDrainForces {
        let mut consumed_field_actor_ids = Vec::new();
        let mut bodies = self.combat_bodies(players);
        let mut physics = EtherDrainPhysics {
            bounds: self.loaded.scene.bounds,
            collision: &self.dynamic_collision,
        };
        for contact in contacts {
            match &contact.target {
                NativeEtherDrainTargetRef::Loot { id, registration_ordinal } => {
                    self.loot.actors.retain_mut(|actor| {
                        if actor.id as u64 != *id
                            || actor.painter_registration.registration_ordinal
                                != *registration_ordinal
                        {
                            return true;
                        }
                        if contact.consume {
                            consumed_field_actor_ids.push(contact.source_actor_id);
                            return false;
                        }
                        // Loose flag400 bypasses body pairs but keeps the constructor radius15.
                        actor.position = physics.move_body("", actor.position, contact.delta, 15.0);
                        true
                    });
                }
                NativeEtherDrainTargetRef::Golem { id } => {
                    let Some(mut actor) = secondary.actors.iter_mut().find(|actor| {
                        actor.id == *id && actor.kind == NativeSecondaryActorKind::Golem
                    }) else {
                        continue;
                    };
                    let Some(golem) = actor.golem.as_deref_mut() else { continue };
                    let position = physics.move_body(
                        "",
                        actor.common.position,
                        contact.delta,
                        actor.common.radius,
                    );
                    let delta = Vec2::new(
                        fround(position.x - actor.common.position.x),
                        fround(position.y - actor.common.position.y),
                    );
                    let shift = |point: &mut Vec2| {
                        *point = Vec2::new(fround(point.x + delta.x), fround(point.y + delta.y));
                    };
                    actor.common.position = position;
                    shift(&mut golem.left_foot);
                    shift(&mut golem.left_foot_next);
                    shift(&mut golem.left_foot_previous);
                    shift(&mut golem.right_foot);
                    shift(&mut golem.right_foot_next);
                    shift(&mut golem.right_foot_previous);
                }
                reference => {
                    let mover = match reference {
                        NativeEtherDrainTargetRef::Enemy { id, .. } => format!("enemy-{id}"),
                        NativeEtherDrainTargetRef::Player { id } => format!("player-{id}"),
                        _ => unreachable!("loose objects and Golems have their own force paths"),
                    };
                    if !bodies.iter().any(|body| body.id == mover) {
                        continue;
                    }
                    if let NativeEtherDrainTargetRef::Enemy { id, .. } = reference {
                        self.enemies.release_skeleton_pike(*id as usize);
                    }
                    for body in &mut bodies {
                        body.driven = body.id == mover;
                        body.delta = if body.driven { contact.delta } else { Vec2::ZERO };
                    }
                    resolve_actor_motion(&mut bodies, &mut physics, |_, _| true, None);
                }
            }
        }
        let demon_positions: Vec<_> = self
            .enemies
            .actors
            .iter()
            .filter(|actor| matches!(actor.brain, EnemyBrain::Demon(_)))
            .map(|actor| (actor.id, actor.position))
            .collect();
        self.commit_enemy_positions(&bodies);
        for actor in &mut self.enemies.actors {
            let EnemyBrain::Demon(brain) = &mut actor.brain else { continue };
            let before = demon_positions.iter().find(|(id, _)| *id == actor.id).unwrap().1;
            let delta =
                Vec2::new(fround(actor.position.x - before.x), fround(actor.position.y - before.y));
            if delta == Vec2::ZERO {
                continue;
            }
            for limb in [&mut brain.articulation.front, &mut brain.articulation.rear] {
                for point in [&mut limb.current, &mut limb.start, &mut limb.target] {
                    *point = Vec2::new(fround(point.x + delta.x), fround(point.y + delta.y));
                }
            }
        }
        if let Some(body) = bodies.iter().find(|body| body.id == "lantern") {
            self.lantern_position = Some(body.position);
        }
        let mut player_positions = BTreeMap::new();
        for player in players {
            if let Some(body) =
                bodies.iter().find(|body| body.id == format!("player-{}", player.status.id))
            {
                player_positions.insert(player.status.id.to_owned(), body.position);
            }
        }
        BoneyardEtherDrainForces { consumed_field_actor_ids, player_positions }
    }
}

/// This source force path rounds the requested point before the common sweep.
/// Ordinary movement keeps its existing unrounded requested-point contract.
struct EtherDrainPhysics<'a> {
    bounds: BoneyardBounds,
    collision: &'a BoneyardCollisionWorld,
}
impl ActorPhysicsWorld for EtherDrainPhysics<'_> {
    fn can_place(&self, _: &str, position: Vec2, radius: f64) -> bool {
        can_place_boneyard_body(position, self.bounds, self.collision, radius)
    }
    fn move_body(&mut self, _: &str, position: Vec2, delta: Vec2, radius: f64) -> Vec2 {
        let requested = Vec2::new(fround(position.x + delta.x), fround(position.y + delta.y));
        resolve_boneyard_movement(position, requested, self.bounds, self.collision, radius)
    }
}
