//! Static scenery placement: the atlas entry, painter row and registration order of every
//! object, fence post and fence body, plus Tree and roof proxies. Both stock and authored
//! scenes compile through this plan; glyph pixels remain immutable catalog assets.

mod fence;
mod shadow;
mod tree;

pub use fence::{
    BrokenFenceHalf, NativeFenceGrate, SceneryFencePost, boneyard_fence_posts, broken_fence_halves,
    native_fence_grate,
};

use super::gate::closed_gate_roots;
use super::presentation::gate_art;
use super::{BoneyardBounds, BoneyardObject, BoneyardScene};
pub use shadow::*;
pub use tree::*;

use crate::Vec2;
use crate::float::{cos, sin};

/// What a scenery layer draws: an object's atlas entry, a fence post, or one piece of a
/// fence body with its broken half, if any.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScenerySource {
    Object { index: usize, entry: usize },
    FencePost { index: usize, entry: usize },
    FenceBody { index: usize, piece: usize, broken: Option<BrokenFenceHalf> },
}

/// One planned scenery drawable: what it draws, its world position, the painter row inputs
/// and its registration order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneryLayer {
    pub source: ScenerySource,
    pub position: Vec2,
    pub world_y: f64,
    pub sort_bias: f64,
    pub source_order: usize,
}

/// A scene's static scenery: underlays, compact ground sprites, main-layer drawables, Tree
/// and roof proxies, walls and the shared fence posts.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneryPlan {
    pub underlays: Vec<(usize, usize)>,
    pub compact: Vec<(usize, usize)>,
    pub main: Vec<SceneryLayer>,
    pub proxies: Vec<SceneryLayer>,
    pub walls: Vec<SceneryLayer>,
    pub posts: Vec<SceneryFencePost>,
}

/// The complete original record bank determines whether a Tree proxy is empty.
#[must_use]
pub fn scenery_plan(scene: &BoneyardScene, record_is_empty: &dyn Fn(usize) -> bool) -> SceneryPlan {
    let mut plan = SceneryPlan {
        underlays: Vec::new(),
        compact: Vec::new(),
        main: Vec::new(),
        proxies: Vec::new(),
        walls: Vec::new(),
        posts: boneyard_fence_posts(&scene.fences),
    };
    for (index, object) in scene.objects.iter().enumerate() {
        if object.type_id == 2029 {
            plan.underlays.push((
                index,
                object
                    .overlay_atlas_entry
                    .unwrap_or(88 + object.overlay_variant.unwrap_or(0) as usize),
            ));
        }
        let Some(entry) = scenery_object_entry(object) else {
            continue;
        };
        plan.main.push(SceneryLayer {
            source: ScenerySource::Object { index, entry },
            position: object.position,
            world_y: object.position.y,
            sort_bias: object.sort_bias.unwrap_or(0.0),
            source_order: index,
        });
        let proxy = match object.type_id {
            2001 if object.variant.unwrap_or(0) < 6 && object.secondary_visible != Some(false) => {
                let entry = object
                    .secondary_atlas_entry
                    .unwrap_or(243 + object.secondary_variant.unwrap_or(0) as usize);
                if record_is_empty(entry) { None } else { Some((entry, 100.0)) }
            }
            2040 => Some((
                object
                    .atlas_entries
                    .as_ref()
                    .and_then(|entries| entries.get(1))
                    .copied()
                    .unwrap_or(152 + object.variant.unwrap_or(0) as usize),
                200.0,
            )),
            _ => None,
        };
        if let Some((entry, offset)) = proxy {
            plan.proxies.push(SceneryLayer {
                source: ScenerySource::Object { index, entry },
                position: object.position,
                world_y: object.position.y + offset,
                sort_bias: 0.0,
                source_order: index,
            });
        }
    }
    for (index, sprite) in scene.sprites.iter().enumerate() {
        plan.compact.push((
            index,
            sprite.dead_hawg_entry.map_or(114 + sprite.atlas_entry, |entry| entry as usize),
        ));
    }
    for (index, post) in plan.posts.iter().enumerate() {
        plan.main.push(SceneryLayer {
            source: ScenerySource::FencePost {
                index,
                entry: if post.style == 1 { 320 } else { 36 } + post.variant as usize,
            },
            position: post.position,
            world_y: post.position.y,
            sort_bias: 0.0,
            source_order: scene.objects.len() + index,
        });
    }
    let body_start = scene.objects.len() + plan.posts.len();
    for (index, fence) in scene.fences.iter().enumerate() {
        let halves = if fence.code() == 1 {
            // The broken constructor observes only fence prefixes, before later
            // endpoint selectors overwrite the shared post's presentation record.
            let posts = boneyard_fence_posts(&scene.fences[..=index]);
            let selector = |endpoint: usize| {
                fence
                    .points
                    .get(endpoint)
                    .and_then(|position| posts.iter().find(|post| post.position == *position))
                    .map_or(0, |post| post.variant)
            };
            broken_fence_halves(&fence.points, selector(0), selector(1))
        } else {
            Vec::new()
        };
        let positions: Vec<Vec2> = match fence.code() {
            1 => halves.iter().map(|half| half.root).collect(),
            2 => closed_gate_roots(&fence.points)
                .into_iter()
                .map(|(hinge, tip)| {
                    let art = gate_art(hinge, tip);
                    Vec2::new(
                        if tip.y < (hinge.y + tip.y) / 2.0 {
                            (hinge.x + tip.x) / 2.0
                        } else {
                            tip.x
                        },
                        art.painter_y,
                    )
                })
                .collect(),
            _ => {
                let start = fence.points.first().copied().unwrap_or(Vec2::ZERO);
                let end = fence.points.get(1).copied().unwrap_or(start);
                vec![Vec2::new(
                    start.x + (end.x - start.x) * 0.5,
                    start.y + (end.y - start.y) * 0.5,
                )]
            }
        };
        for (piece, position) in positions.into_iter().enumerate() {
            let layer = SceneryLayer {
                source: ScenerySource::FenceBody {
                    index,
                    piece,
                    broken: halves.get(piece).copied(),
                },
                position,
                world_y: position.y,
                sort_bias: -15.0,
                source_order: body_start + index * 2 + piece,
            };
            if fence.code() == 3 {
                plan.walls.push(layer);
            } else {
                plan.main.push(layer);
            }
        }
    }
    plan
}

#[must_use]
pub fn scenery_object_entry(object: &BoneyardObject) -> Option<usize> {
    if let Some(entry) = object.atlas_entry {
        return Some(entry);
    }
    if let Some(entry) = object.atlas_entries.as_ref().and_then(|entries| entries.first()) {
        return Some(*entry);
    }
    let start = match object.type_id {
        2001 => 264,
        2009 => 156,
        2029 => 97,
        2040 => 148,
        2061 => 145,
        _ => return None,
    };
    Some(start + object.variant.unwrap_or(0) as usize)
}

/// Original glyph transform. The bounds of these corners also decide which scenery the
/// strict off-camera cleanup retires.
#[must_use]
pub fn scenery_art_corners(
    position: Vec2,
    size: Vec2,
    anchor: Vec2,
    rotation_degrees: f64,
    scale: Vec2,
) -> [Vec2; 4] {
    let radians = rotation_degrees * std::f64::consts::PI / 180.0;
    let sine = sin(radians);
    let cosine = cos(radians);
    let left = -anchor.x * scale.x;
    let right = (size.x - anchor.x) * scale.x;
    let top = -anchor.y * scale.y;
    let bottom = (size.y - anchor.y) * scale.y;
    [[left, top], [right, top], [right, bottom], [left, bottom]].map(|[x, y]| {
        Vec2::new(position.x + x * cosine - y * sine, position.y + x * sine + y * cosine)
    })
}

#[must_use]
pub fn scenery_points_bounds(points: &[Vec2]) -> Option<BoneyardBounds> {
    let first = points.first()?;
    let mut left = first.x;
    let mut right = first.x;
    let mut top = first.y;
    let mut bottom = first.y;
    for point in &points[1..] {
        left = left.min(point.x);
        right = right.max(point.x);
        top = top.min(point.y);
        bottom = bottom.max(point.y);
    }
    Some(BoneyardBounds::new(left, top, right - left, bottom - top))
}
