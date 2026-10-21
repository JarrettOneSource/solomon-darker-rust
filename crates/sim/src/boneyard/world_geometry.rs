//! Enemy-world geometry callbacks from `core-server/boneyard-world.ts`.
//!
//! Prepared routes use immutable scenery. Direct obstruction, final movement and native
//! spawn acceptance use current gate collision. These are deliberately different worlds.

use std::cell::{Cell, RefCell};

use crate::actor_physics::{ActorPhysicsBody, ActorPhysicsWorld, resolve_unpushed_mover_motion};
use crate::enemies::model::EnemyTerminalOutput;
use crate::enemies::path::EnemyNavigation;
pub use crate::enemies::world::NativeEnemyVisibility;
use crate::enemies::world::{
    EnemyGeometry, EnemyMovementPurpose, EnemyMovementRequest, EnemyProjectileWorldQuery,
    EnemySpawnPlacementRequest,
};
use crate::float::fround;
use crate::{NativeRng, Vec2};

use super::BoneyardBounds;
use super::collision::{
    BoneyardCollisionWorld, can_place_boneyard_body, first_boneyard_line_obstruction,
    first_boneyard_path_block_progress, resolve_boneyard_movement,
};
use super::feedback::{NATIVE_EXPLOSION_FEEDBACK_INTENSITY, NativeEnemyWorldFeedback};
use super::lighting::{
    NATIVE_LIGHT_OUTER_DISTANCE, NativeBoneyardLightSource, native_boneyard_light_scalar,
};
use super::navigation::{NavigationSet, RouteRequest};
use super::projectile_world::{BoneyardProjectileViewport, BoneyardProjectileWorld};
use super::spawn::{SpawnPlacement, SpawnPolicyContext, boneyard_spawn_position_is_offscreen};
use super::world_light::{BoneyardWorldLightQuery, native_primary_view_bounds};

pub struct BoneyardPhysicsWorld<'a> {
    pub bounds: BoneyardBounds,
    pub collision: &'a BoneyardCollisionWorld,
}

impl ActorPhysicsWorld for BoneyardPhysicsWorld<'_> {
    fn can_place(&self, _body_id: &str, position: Vec2, radius: f64) -> bool {
        can_place_boneyard_body(position, self.bounds, self.collision, radius)
    }

    fn move_body(&mut self, _body_id: &str, position: Vec2, delta: Vec2, radius: f64) -> Vec2 {
        resolve_boneyard_movement(
            position,
            Vec2::new(position.x + delta.x, position.y + delta.y),
            self.bounds,
            self.collision,
            radius,
        )
    }
}

pub struct BoneyardGeometryInput<'a> {
    pub active_bounds: BoneyardBounds,
    pub spawn_bounds: BoneyardBounds,
    pub spawn_camera_bounds: BoneyardBounds,
    pub static_collision: &'a BoneyardCollisionWorld,
    pub collision: &'a BoneyardCollisionWorld,
    pub navigation: &'a NavigationSet,
    pub world_light: &'a BoneyardWorldLightQuery,
    pub spawn_policy_focuses: &'a [Vec2],
    /// The tutorial's actual arena/camera domain, when that world owns one.
    pub spawn_domain: Option<&'a dyn Fn(Vec2, f64) -> bool>,
    pub spawn_retry_bounds: Option<BoneyardBounds>,
    pub projectile_viewports: &'a [BoneyardProjectileViewport],
    /// The `&self` enemy callbacks interleave impulses within this one world step. A Cell
    /// holds the copied pair of numbers only until the owner commits it after stepping;
    /// saved world state never contains this adapter or any hidden retained feedback.
    pub feedback: &'a Cell<NativeEnemyWorldFeedback>,
}

/// The source callbacks advance one ordered crowd as actors move. Interior mutability is
/// confined to this per-tick geometry adapter; no handles or caches enter saved world state.
pub struct BoneyardEnemyGeometry<'a> {
    input: BoneyardGeometryInput<'a>,
    dynamic_bodies: RefCell<Vec<ActorPhysicsBody>>,
    projectile_world: BoneyardProjectileWorld<'a>,
    native_views: Vec<BoneyardBounds>,
    native_sources: Vec<NativeBoneyardLightSource>,
}

impl<'a> BoneyardEnemyGeometry<'a> {
    #[must_use]
    pub fn new(input: BoneyardGeometryInput<'a>, dynamic_bodies: Vec<ActorPhysicsBody>) -> Self {
        assert!(!input.spawn_policy_focuses.is_empty(), "world spawn policy has a camera focus");
        let projectile_world = BoneyardProjectileWorld::new(
            input.active_bounds,
            input.collision,
            input.projectile_viewports,
        );
        let mut native_views = Vec::with_capacity(input.projectile_viewports.len());
        for viewport in input.projectile_viewports {
            native_views.push(native_primary_view_bounds(
                input.spawn_camera_bounds,
                viewport.position,
                viewport.viewport,
                1.35,
                0.0,
            ));
        }
        let native_sources = input.world_light.accepted_sources().copied().collect();
        Self {
            input,
            dynamic_bodies: RefCell::new(dynamic_bodies),
            projectile_world,
            native_views,
            native_sources,
        }
    }

    #[must_use]
    pub fn into_bodies(self) -> Vec<ActorPhysicsBody> {
        self.dynamic_bodies.into_inner()
    }

    /// `boneyard-enemy-visibility.ts`: admission uses the actor's native body rectangle
    /// and a circular light reach, followed by the radial scalar for its intensity.
    #[must_use]
    pub fn native_visibility(&self, position: Vec2) -> NativeEnemyVisibility {
        let onscreen = self.native_views.iter().any(|view| {
            position.x - 32.0 < view.x + view.width
                && position.x + 32.0 > view.x
                && position.y - 44.0 < view.y + view.height
                && position.y + 12.0 > view.y
        });
        let admitted = onscreen
            && self.native_sources.iter().any(|light| {
                let dx = position.x - light.position.x;
                let dy = position.y - light.position.y;
                dx * dx + dy * dy < (light.radius * NATIVE_LIGHT_OUTER_DISTANCE).powi(2)
            });
        let intensity = if admitted {
            native_boneyard_light_scalar(position, &self.native_sources)
        } else {
            0.0
        };
        NativeEnemyVisibility { admitted, intensity }
    }

    #[must_use]
    pub fn projected_point_visible(&self, position: Vec2) -> bool {
        self.native_views.iter().any(|view| {
            position.x >= view.x
                && position.x <= view.x + view.width
                && position.y >= view.y
                && position.y <= view.y + view.height
        })
    }

    #[must_use]
    pub fn native_view_bounds(&self) -> Option<BoneyardBounds> {
        let first = self.native_views.first()?;
        let mut left = first.x;
        let mut top = first.y;
        let mut right = first.x + first.width;
        let mut bottom = first.y + first.height;
        for view in &self.native_views[1..] {
            left = left.min(view.x);
            top = top.min(view.y);
            right = right.max(view.x + view.width);
            bottom = bottom.max(view.y + view.height);
        }
        Some(BoneyardBounds::new(left, top, right - left, bottom - top))
    }
}

impl EnemyNavigation for BoneyardEnemyGeometry<'_> {
    fn direct_path_clear(
        &self,
        start: Vec2,
        end: Vec2,
        _clearance: f64,
        _body_radius: f64,
    ) -> bool {
        // enemies/movement.ts passes radius zero here. Navigation clearance belongs to
        // route attachment, and must not inflate this source point-obstruction query.
        first_boneyard_path_block_progress(
            start,
            end,
            self.input.active_bounds,
            self.input.collision,
            0.0,
            &[],
        )
        .is_none()
    }

    fn find_route(
        &self,
        start: Vec2,
        end: Vec2,
        clearance: f64,
        body_radius: f64,
    ) -> Option<Vec<Vec2>> {
        self.input.navigation.find_route(
            RouteRequest {
                body_radius,
                bounds: self.input.active_bounds,
                clearance,
                end,
                end_body_radius: None,
                ignored_source_ids: &[],
                start,
            },
            self.input.static_collision,
        )
    }
}

impl EnemyGeometry for BoneyardEnemyGeometry<'_> {
    fn on_collision_recipient_cleared(&self, actor_id: usize) {
        let id = format!("enemy-{actor_id}");
        if let Some(body) = self.dynamic_bodies.borrow_mut().iter_mut().find(|body| body.id == id) {
            body.collision_recipient = false;
        }
    }

    fn projectile_world_blocked(&self, query: EnemyProjectileWorldQuery) -> bool {
        self.projectile_world.blocked(query)
    }

    fn on_projectile_explosion(&self, position: Vec2) {
        if let Some(gain) = self.projectile_world.explosion_point_gain(position) {
            let mut feedback = self.input.feedback.get();
            feedback.apply(fround(NATIVE_EXPLOSION_FEEDBACK_INTENSITY * gain));
            self.input.feedback.set(feedback);
        }
    }

    fn on_terminal_output(&self, output: EnemyTerminalOutput, output_count: Option<usize>) {
        let mut feedback = self.input.feedback.get();
        feedback.apply_terminal(output, output_count);
        self.input.feedback.set(feedback);
    }

    fn clip_spell_segment(&self, start: Vec2, end: Vec2) -> Vec2 {
        self.line_obstruction(start, end, 0).unwrap_or(end)
    }

    fn resolve_movement(&self, request: EnemyMovementRequest) -> Vec2 {
        if request.purpose == EnemyMovementPurpose::SpawnPlacement {
            return SpawnPlacement {
                position: request.position,
                bounds: self.input.spawn_bounds,
                collision: self.input.collision,
                radius: request.radius,
            }
            .resolve(None)
            .expect("Boneyard has a collision-safe enemy placement");
        }
        let id = format!("enemy-{}", request.actor_id);
        let mut bodies = self.dynamic_bodies.borrow_mut();
        let index = if let Some(index) = bodies.iter().position(|body| body.id == id) {
            index
        } else {
            let index = bodies.len();
            bodies.push(enemy_collision_body(id, request.position, request.radius));
            index
        };
        let mut physics = BoneyardPhysicsWorld {
            bounds: self.input.active_bounds,
            collision: self.input.collision,
        };
        let position = resolve_unpushed_mover_motion(&bodies, index, request.delta, &mut physics);
        bodies[index].position = position;
        position
    }

    fn line_obstruction(&self, start: Vec2, end: Vec2, native_exclusion_mask: u32) -> Option<Vec2> {
        first_boneyard_line_obstruction(
            start,
            end,
            self.input.active_bounds,
            self.input.collision,
            None,
            native_exclusion_mask,
            &[],
        )
    }

    fn resolve_spawn_placement(
        &self,
        request: EnemySpawnPlacementRequest,
        rng: &mut NativeRng,
    ) -> Vec2 {
        let policy = EnemySpawnPolicy {
            input: &self.input,
            navigation_clearance: request.navigation_clearance,
            reachability_radius: request.reachability_radius,
        };
        SpawnPlacement {
            position: request.position,
            bounds: self.input.spawn_bounds,
            collision: self.input.collision,
            radius: request.radius,
        }
        .resolve_native(request.position_policy, rng, &policy)
        .expect("Boneyard has a reachable native enemy spawn placement")
    }
}

struct EnemySpawnPolicy<'a, 'b> {
    input: &'a BoneyardGeometryInput<'b>,
    navigation_clearance: f64,
    reachability_radius: f64,
}

impl SpawnPolicyContext for EnemySpawnPolicy<'_, '_> {
    fn light_at(&self, position: Vec2) -> f64 {
        self.input.world_light.scalar_at(position)
    }

    fn accepts_domain(&self, position: Vec2, radius: f64) -> bool {
        if self.input.spawn_domain.is_some_and(|domain| !domain(position, radius)) {
            return false;
        }
        for &focus in self.input.spawn_policy_focuses {
            let route = self.input.navigation.find_route(
                RouteRequest {
                    body_radius: self.reachability_radius,
                    bounds: self.input.spawn_bounds,
                    clearance: self.navigation_clearance,
                    end: focus,
                    end_body_radius: Some(25.0),
                    ignored_source_ids: &[],
                    start: position,
                },
                self.input.collision,
            );
            if route.is_some() {
                return true;
            }
        }
        false
    }

    fn is_offscreen(&self, position: Vec2) -> bool {
        boneyard_spawn_position_is_offscreen(
            position,
            self.input.spawn_camera_bounds,
            self.input.spawn_policy_focuses,
        )
    }

    fn retry_bounds(&self) -> Option<BoneyardBounds> {
        self.input.spawn_retry_bounds
    }
}

#[must_use]
pub fn enemy_collision_body(id: String, position: Vec2, radius: f64) -> ActorPhysicsBody {
    ActorPhysicsBody {
        collision_recipient: true,
        id,
        position,
        radius,
        delta: Vec2::ZERO,
        driven: false,
        push_enabled: false,
        push_resistance: 0.0,
        push_strength: 0.0,
    }
}

#[must_use]
pub fn boneyard_lantern_body(position: Vec2) -> ActorPhysicsBody {
    // Stock Lantern ctor: radius +0x30 = 8, resistance +0x28 = 1; passive pushing enabled.
    ActorPhysicsBody {
        collision_recipient: true,
        id: "lantern".into(),
        position,
        radius: 8.0,
        delta: Vec2::ZERO,
        driven: false,
        push_enabled: true,
        push_resistance: 1.0,
        push_strength: 0.0,
    }
}
