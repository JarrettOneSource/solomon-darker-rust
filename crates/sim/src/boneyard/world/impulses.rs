//! Collision-resolved enemy impulses from `boneyard-world-placement.ts`.

use super::*;
use crate::actor_physics::{ActorPhysicsWorld, ActorRootContact};
use crate::float::fround;

impl BoneyardWorld {
    /// Body positions carry through successive ordinary impulses, including
    /// temporary player pushes. Source commits only enemy/lantern roots.
    pub fn apply_secondary_enemy_knockbacks(
        &mut self,
        knockbacks: &[(usize, Vec2)],
        players: &[BoneyardPlayerBody<'_>],
    ) {
        let _ = self.apply_enemy_knockbacks(knockbacks, players, false);
    }

    /// `moveBoneyardKnockbackAreaTarget` owns one target's two physics passes.
    /// Each call rebuilds player bodies from the resolved characters, while
    /// previously committed enemy/lantern roots remain visible to the next call.
    pub fn move_knockback_area_target(
        &mut self,
        target_id: usize,
        delta: Vec2,
        players: &[BoneyardPlayerBody<'_>],
    ) -> Vec<usize> {
        self.apply_enemy_knockbacks(&[(target_id, delta)], players, true)
    }

    fn apply_enemy_knockbacks(
        &mut self,
        knockbacks: &[(usize, Vec2)],
        players: &[BoneyardPlayerBody<'_>],
        area: bool,
    ) -> Vec<usize> {
        let mut contact_ids = Vec::new();
        if knockbacks.is_empty() {
            return contact_ids;
        }
        let mut bodies = self.combat_bodies(players);
        for (actor_id, delta) in knockbacks {
            let mover_id = format!("enemy-{actor_id}");
            let Some(mover_index) = bodies.iter().position(|body| body.id == mover_id) else {
                continue;
            };
            self.enemies.release_skeleton_pike(*actor_id);
            for body in &mut bodies {
                body.driven = body.id == mover_id;
                body.delta = if body.driven { *delta } else { Vec2::ZERO };
            }
            let mut physics = BoneyardPhysicsWorld {
                bounds: self.loaded.scene.bounds,
                collision: &self.dynamic_collision,
            };
            resolve_actor_motion(&mut bodies, &mut physics, |_, _| !area, None);
            if area {
                let original_radius = bodies[mover_index].radius;
                for body in &mut bodies {
                    body.delta = Vec2::ZERO;
                }
                bodies[mover_index].radius = fround(original_radius * (0.6_f32 as f64));
                let mut contacts = Vec::<ActorRootContact>::new();
                let mut physics = KnockbackAreaPhysics {
                    bounds: self.loaded.scene.bounds,
                    collision: &self.dynamic_collision,
                };
                resolve_actor_motion(&mut bodies, &mut physics, |_, _| true, Some(&mut contacts));
                bodies[mover_index].radius = original_radius;
                for contact in contacts {
                    if contact.mover_id != mover_id {
                        continue;
                    }
                    let Some(id) = contact.other_id.strip_prefix("enemy-") else {
                        continue;
                    };
                    let id = id.parse::<usize>().expect("enemy bodies retain their numeric ID");
                    let hostile = self.enemies.actors.iter().any(|actor| {
                        actor.id == id
                            && crate::enemies::targeting::boneyard_enemy_actor_flags(actor) & 2 != 0
                    }) || self
                        .enemies
                        .maggots
                        .iter()
                        .any(|actor| actor.id == id && actor.combat_active);
                    if hostile {
                        contact_ids.push(id);
                    }
                }
            }
        }
        self.commit_enemy_positions(&bodies);
        if let Some(body) = bodies.iter().find(|body| body.id == "lantern") {
            self.lantern_position = Some(body.position);
        }
        contact_ids
    }
}

/// The Source area separation pass stores its zero-delta request as float32.
/// Ordinary impulses deliberately retain their existing double request above.
struct KnockbackAreaPhysics<'a> {
    bounds: BoneyardBounds,
    collision: &'a super::super::collision::BoneyardCollisionWorld,
}
impl ActorPhysicsWorld for KnockbackAreaPhysics<'_> {
    fn can_place(&self, _: &str, position: Vec2, radius: f64) -> bool {
        super::super::collision::can_place_boneyard_body(
            position,
            self.bounds,
            self.collision,
            radius,
        )
    }
    fn move_body(&mut self, _: &str, position: Vec2, delta: Vec2, radius: f64) -> Vec2 {
        super::super::collision::resolve_boneyard_movement(
            position,
            Vec2::new(fround(position.x + delta.x), fround(position.y + delta.y)),
            self.bounds,
            self.collision,
            radius,
        )
    }
}
