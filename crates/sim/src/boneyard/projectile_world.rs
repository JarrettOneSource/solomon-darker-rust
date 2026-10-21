//! Projectile world queries from `core-server/boneyard-projectile-world.ts`.

use crate::Vec2;
use crate::enemies::world::EnemyProjectileWorldQuery;

use super::BoneyardBounds;
use super::collision::{
    BoneyardCollisionWorld, boneyard_body_collides, first_boneyard_line_obstruction,
};
use super::world_light::{native_primary_view_bounds, native_region_point_gain};

#[derive(Clone, Copy, Debug)]
pub struct BoneyardProjectileViewport {
    pub position: Vec2,
    pub viewport: Vec2,
    pub alternate_player: bool,
}

pub struct BoneyardProjectileWorld<'a> {
    bounds: BoneyardBounds,
    collision: &'a BoneyardCollisionWorld,
    views: Vec<BoneyardBounds>,
    first_viewport: Option<BoneyardProjectileViewport>,
}

impl<'a> BoneyardProjectileWorld<'a> {
    #[must_use]
    pub fn new(
        bounds: BoneyardBounds,
        collision: &'a BoneyardCollisionWorld,
        viewports: &[BoneyardProjectileViewport],
    ) -> Self {
        let mut views = Vec::with_capacity(viewports.len());
        for viewport in viewports {
            // Projectile retention uses the stock height-adaptive scale. Logical lights
            // and native enemy visibility instead use their separate fixed 1.35 scale.
            views.push(native_primary_view_bounds(
                bounds,
                viewport.position,
                viewport.viewport,
                (viewport.viewport.y / 800.0).max(1.0),
                0.0,
            ));
        }
        Self { bounds, collision, views, first_viewport: viewports.first().copied() }
    }

    #[must_use]
    pub fn blocked(&self, query: EnemyProjectileWorldQuery) -> bool {
        match query {
            EnemyProjectileWorldQuery::Line { start, end, native_exclusion_mask, .. } => {
                first_boneyard_line_obstruction(
                    start,
                    end,
                    self.bounds,
                    self.collision,
                    None,
                    native_exclusion_mask,
                    &[],
                )
                .is_some()
            }
            EnemyProjectileWorldQuery::Point { position, radius, .. } => {
                boneyard_body_collides(position, self.collision, radius)
            }
            EnemyProjectileWorldQuery::Bounds { position, margin, .. } => {
                !contains_point(self.bounds, position, margin)
            }
            EnemyProjectileWorldQuery::View { position, margin, .. } => {
                !self.views.iter().any(|view| contains_point(*view, position, margin))
            }
        }
    }

    #[must_use]
    pub fn explosion_point_gain(&self, position: Vec2) -> Option<f64> {
        self.first_viewport
            .map(|viewport| boneyard_projectile_point_gain(self.bounds, viewport, position))
    }
}

#[must_use]
pub fn boneyard_projectile_point_gain(
    bounds: BoneyardBounds,
    viewport: BoneyardProjectileViewport,
    position: Vec2,
) -> f64 {
    let view = native_primary_view_bounds(
        bounds,
        viewport.position,
        viewport.viewport,
        (viewport.viewport.y / 800.0).max(1.0),
        0.0,
    );
    native_region_point_gain(
        position,
        Vec2::new(view.x + view.width / 2.0, view.y + view.height / 2.0),
        view.width,
        viewport.alternate_player,
    )
}

fn contains_point(bounds: BoneyardBounds, position: Vec2, margin: f64) -> bool {
    position.x >= bounds.x - margin
        && position.y >= bounds.y - margin
        && position.x < bounds.x + bounds.width + margin
        && position.y < bounds.y + bounds.height + margin
}
