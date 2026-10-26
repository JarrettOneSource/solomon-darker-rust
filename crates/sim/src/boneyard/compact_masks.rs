//! Compact ground-mask admission from `renderer/native-compact-mask-view.ts`.
//! The masks use whole MagicGrid cells and unscaled record bounds, even when
//! their actual stamp is rotated or stretched. Their RNG is presentation-only.

use crate::enemies::spider_remains::SpiderRemains;
use crate::float;
use crate::float::fround;
use crate::{NativeRng, Vec2};

use super::terrain_surface::{
    NativeBridgeSurface, NativeSurfaceQuad, build_native_bridge_surfaces,
    build_native_terrain_surface, native_bridge_surface_contains, native_surface_quad_bounds,
    native_surface_quad_contains,
};
use super::{BoneyardBounds, BoneyardScene};
use std::collections::BTreeMap;

pub const NATIVE_COMPACT_MASK_TARGET_SIDE: u32 = 256;
pub const NATIVE_COMPACT_MASK_RADIAL_ENTRY: usize = 9;
pub const NATIVE_COMPACT_MASK_RADIAL_SCALE: f64 = 2.009999990463257;
pub const NATIVE_COMPACT_MASK_COMPOSITE_SCALE: f64 = 2.0250000953674316;
pub const NATIVE_COMPACT_MASK_COMPOSITE_ANCHOR: f64 = 128.5;
pub const NATIVE_DIRECT_ENVIRONMENT_LIGHT_ENTRY: usize = 18;

#[derive(Clone, Debug, PartialEq)]
pub struct NativeCompactMask {
    pub id: String,
    pub entry: usize,
    pub position: Vec2,
    pub rotation_degrees: f64,
    pub scale: Vec2,
    pub alpha: f64,
    pub dynamic: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeCompactMaskPlayerPlan {
    /// Actual participant insertion order, including players with no candidates.
    pub slot: usize,
    pub position: Vec2,
    pub alpha: f64,
    pub mask_indices: Vec<usize>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeCompactMaskPlan {
    pub masks: Vec<NativeCompactMask>,
    pub players: Vec<NativeCompactMaskPlayerPlan>,
}

pub struct NativeCompactMasks {
    authored: Vec<NativeCompactMask>,
    maximum_column: i64,
    maximum_row: i64,
}

/// Arena's separate surface query. Terrain wins before compact/Spider glyphs;
/// a bridge subtracts even an overlapping authored or dynamic stain.
pub struct NativeCompactGroundSurface {
    terrain: BTreeMap<(i64, i64), Vec<NativeSurfaceQuad>>,
    bridges: Vec<NativeBridgeSurface>,
    authored: Vec<(usize, Vec2)>,
    maximum_column: i64,
    maximum_row: i64,
}

impl NativeCompactGroundSurface {
    #[must_use]
    pub fn new(scene: &BoneyardScene) -> Self {
        let surfaces: Vec<_> = scene.terrain.iter().map(build_native_terrain_surface).collect();
        let mut result = Self {
            bridges: build_native_bridge_surfaces(&surfaces, &scene.roads),
            terrain: BTreeMap::new(),
            authored: scene
                .sprites
                .iter()
                .filter(|sprite| (25..=29).contains(&sprite.atlas_entry))
                .map(|sprite| (sprite.atlas_entry, stored_point(sprite.position)))
                .collect(),
            maximum_column: (scene.bounds.width.trunc() / 50.0 + 0.5).trunc() as i64,
            maximum_row: (scene.bounds.height.trunc() / 50.0 + 0.5).trunc() as i64,
        };
        for surface in surfaces {
            for quad in surface.quads {
                let cells = result.grid_cells(native_surface_quad_bounds(&quad));
                for row in cells.top..=cells.bottom {
                    for column in cells.left..=cells.right {
                        result.terrain.entry((column, row)).or_default().push(quad);
                    }
                }
            }
        }
        result
    }

    #[must_use]
    pub fn contains<'a>(
        &self,
        point: Vec2,
        remains: impl IntoIterator<Item = &'a SpiderRemains>,
        record_size: &dyn Fn(usize) -> Vec2,
    ) -> bool {
        let point = stored_point(point);
        let cell = (cell(point.x, self.maximum_column), cell(point.y, self.maximum_row));
        for quad in self.terrain.get(&(cell.0, cell.1)).into_iter().flatten() {
            if !native_surface_quad_contains(quad, point) {
                continue;
            }
            if self.bridges.iter().any(|bridge| native_bridge_surface_contains(bridge, point)) {
                return false;
            }
            return true;
        }
        for &(selector, position) in &self.authored {
            if self.contains_record(selector, position, point, cell, record_size) {
                return true;
            }
        }
        for remains in remains {
            let Some(decal) = &remains.state.decal else {
                continue;
            };
            if decal.entry < 114 {
                continue;
            }
            if self.contains_record(
                decal.entry - 114,
                stored_point(decal.position),
                point,
                cell,
                record_size,
            ) {
                return true;
            }
        }
        false
    }

    fn contains_record(
        &self,
        selector: usize,
        position: Vec2,
        point: Vec2,
        cell: (i64, i64),
        record_size: &dyn Fn(usize) -> Vec2,
    ) -> bool {
        let size = record_size(selector + 114);
        let bounds = BoneyardBounds::new(
            fround(position.x - size.x / 2.0),
            fround(position.y - size.y / 2.0),
            size.x,
            size.y,
        );
        let cells = self.grid_cells(bounds);
        if cell.0 < cells.left
            || cell.0 > cells.right
            || cell.1 < cells.top
            || cell.1 > cells.bottom
            || point.x < bounds.x
            || point.y < bounds.y
            || point.x > fround(bounds.x + bounds.width)
            || point.y > fround(bounds.y + bounds.height)
        {
            return false;
        }
        native_compact_surface_contains(
            selector,
            Vec2::new(fround(point.x - position.x), fround(point.y - position.y)),
        )
    }

    fn grid_cells(&self, bounds: BoneyardBounds) -> GridCells {
        GridCells {
            left: cell(bounds.x, self.maximum_column),
            right: cell(fround(bounds.x) + fround(bounds.width), self.maximum_column),
            top: cell(bounds.y, self.maximum_row),
            bottom: cell(fround(bounds.y) + fround(bounds.height), self.maximum_row),
        }
    }
}

fn stored_point(point: Vec2) -> Vec2 {
    Vec2::new(fround(point.x), fround(point.y))
}

/// The five compact glyph contours from `native-ground-auxiliary.ts`.
#[must_use]
pub fn native_compact_surface_contains(selector: usize, point: Vec2) -> bool {
    const CONTOURS: [&[[f64; 2]]; 5] = [
        &[
            [-24., -25.],
            [2., -37.],
            [41., -6.],
            [32., 12.],
            [9., 21.],
            [-5., 37.],
            [-24., 42.],
            [-45., 26.],
            [-37., 6.],
            [-14., -3.],
        ],
        &[[13.5, -8.5], [23.5, 14.5], [4.5, 31.5], [-22.5, -0.5], [-10.5, -29.5], [5.5, -31.5]],
        &[[-3., -38.], [38., -19.], [47., 0.], [41., 19.], [5., 38.], [-41., 24.], [-42., -7.]],
        &[
            [7.5, 20.],
            [-29.5, 3.],
            [-28.5, -14.],
            [-12.5, -15.],
            [5.5, -21.],
            [30.5, -4.],
            [28.5, 14.],
        ],
        &[
            [-3.5, -107.],
            [35.5, -98.],
            [77.5, -64.],
            [106.5, -3.],
            [100.5, 34.],
            [39.5, 97.],
            [-1.5, 108.],
            [-64.5, 91.],
            [-91.5, 61.],
            [-107.5, 1.],
            [-73.5, -70.],
        ],
    ];
    let Some(points) = selector.checked_sub(25).and_then(|index| CONTOURS.get(index)) else {
        return false;
    };
    let point = stored_point(point);
    let mut inside = false;
    let mut previous = points[points.len() - 1];
    for &current in *points {
        if (current[1] > point.y) != (previous[1] > point.y)
            && point.x
                < current[0]
                    + (point.y - current[1]) * (previous[0] - current[0])
                        / (previous[1] - current[1])
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

impl NativeCompactMasks {
    #[must_use]
    pub fn new(scene: &BoneyardScene) -> Self {
        let mut authored = Vec::new();
        for sprite in &scene.sprites {
            if !(25..=29).contains(&sprite.atlas_entry) {
                continue;
            }
            authored.push(NativeCompactMask {
                id: sprite.eid.clone(),
                entry: 114 + sprite.atlas_entry,
                position: sprite.position,
                rotation_degrees: sprite.s0,
                scale: Vec2::new(
                    sprite.s1 * if sprite.flags & 1 != 0 { 0.8 } else { 1.0 },
                    sprite.s1,
                ),
                alpha: if sprite.atlas_entry == 29 { 1.0 } else { 0.75 },
                dynamic: false,
            });
        }
        Self {
            authored,
            maximum_column: (scene.bounds.width.trunc() / 50.0 + 0.5).trunc() as i64,
            maximum_row: (scene.bounds.height.trunc() / 50.0 + 0.5).trunc() as i64,
        }
    }

    /// Record dimensions come from the already-loaded DeadHawg stock atlas.
    #[must_use]
    pub fn plan<'a>(
        &self,
        players: &[Vec2],
        remains: impl IntoIterator<Item = &'a SpiderRemains>,
        world_bounds: BoneyardBounds,
        presentation_frame: f64,
        record_size: &dyn Fn(usize) -> Vec2,
    ) -> NativeCompactMaskPlan {
        let mut masks = self.authored.clone();
        for remains in remains {
            let Some(decal) = &remains.state.decal else {
                continue;
            };
            masks.push(NativeCompactMask {
                id: format!("spider-decal:{}", remains.id),
                entry: decal.entry,
                position: decal.position,
                rotation_degrees: decal.rotation_deg,
                scale: Vec2::new(decal.scale, decal.scale),
                alpha: decal.alpha,
                dynamic: true,
            });
        }
        masks.retain(|mask| intersects(mask, record_size(mask.entry), world_bounds));
        let mut mask_cells = Vec::with_capacity(masks.len());
        for mask in &masks {
            let size = record_size(mask.entry);
            mask_cells.push(self.grid_cells(BoneyardBounds::new(
                mask.position.x - size.x / 2.0,
                mask.position.y - size.y / 2.0,
                size.x,
                size.y,
            )));
        }
        let mut player_plans = Vec::with_capacity(players.len());
        for (slot, &position) in players.iter().enumerate() {
            let query = self.grid_cells(BoneyardBounds::new(
                position.x - 256.0,
                position.y - 256.0,
                512.0,
                512.0,
            ));
            let mut mask_indices = Vec::new();
            for (index, cells) in mask_cells.iter().enumerate() {
                if cells.left <= query.right
                    && cells.right >= query.left
                    && cells.top <= query.bottom
                    && cells.bottom >= query.top
                {
                    mask_indices.push(index);
                }
            }
            let alpha = if mask_indices.is_empty() {
                0.0
            } else {
                native_compact_mask_alpha(presentation_frame, slot)
            };
            player_plans.push(NativeCompactMaskPlayerPlan { slot, position, alpha, mask_indices });
        }
        NativeCompactMaskPlan { masks, players: player_plans }
    }

    fn grid_cells(&self, bounds: BoneyardBounds) -> GridCells {
        GridCells {
            left: cell(bounds.x, self.maximum_column),
            right: cell(fround(bounds.x) + fround(bounds.width), self.maximum_column),
            top: cell(bounds.y, self.maximum_row),
            bottom: cell(fround(bounds.y) + fround(bounds.height), self.maximum_row),
        }
    }
}

/// The source's 0.95 base and remaining range are already single-precision.
#[must_use]
pub fn native_compact_mask_alpha(presentation_frame: f64, slot: usize) -> f64 {
    let seed = (presentation_frame.trunc() as i64 as u32) ^ slot as u32;
    let mut rng = NativeRng::new(i64::from(seed));
    fround(fround(0.95) + rng.draw_scaled_float(0.050000011920928955, false))
}

/// `boneyard-environment-light-plan.ts`: this direct aperture is separate from
/// the logical light provider and the multiplied region-light render target.
#[must_use]
pub fn native_direct_environment_light_alpha(now_ms: f64, player_index: usize) -> f64 {
    let flicker = (float::sin(now_ms * 0.017 + player_index as f64 * 2.399) + 1.0) / 2.0;
    (0.2375 + flicker * 0.0125) * 0.14
}

struct GridCells {
    left: i64,
    right: i64,
    top: i64,
    bottom: i64,
}

fn cell(value: f64, maximum: i64) -> i64 {
    let integer = fround(value).trunc();
    if integer < 0.0 {
        return -1;
    }
    let index = (integer / 50.0).trunc() as i64;
    index.min(maximum)
}

fn intersects(mask: &NativeCompactMask, size: Vec2, bounds: BoneyardBounds) -> bool {
    mask.position.x + size.x / 2.0 >= bounds.x
        && mask.position.y + size.y / 2.0 >= bounds.y
        && mask.position.x - size.x / 2.0 <= bounds.x + bounds.width
        && mask.position.y - size.y / 2.0 <= bounds.y + bounds.height
}
