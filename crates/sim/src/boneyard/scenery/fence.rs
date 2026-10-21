//! Fence materialization in `core-kernels/boneyard-fence-posts.ts` and
//! `editor/native-fence-geometry.ts`. Broken halves own a private seeded RNG;
//! generating a scene never consumes the game's accepted random stream.

use crate::float::{fround as f, hypot};
use crate::{NativeRng, Vec2};

use super::super::BoneyardFence;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSceneryFencePost {
    pub fence_index: usize,
    pub position: Vec2,
    pub variant: u32,
    pub style: u8,
}

#[must_use]
pub fn native_boneyard_fence_posts(fences: &[BoneyardFence]) -> Vec<NativeSceneryFencePost> {
    let mut posts: Vec<NativeSceneryFencePost> = Vec::new();
    for (fence_index, fence) in fences.iter().enumerate() {
        if fence.code() == 3 {
            continue;
        }
        for &position in fence.points.iter().take(2) {
            let index = match posts.iter().position(|post| post.position == position) {
                Some(index) => index,
                None => {
                    posts.push(NativeSceneryFencePost {
                        fence_index,
                        position,
                        variant: 0,
                        style: 0,
                    });
                    posts.len() - 1
                }
            };
            if fence.code() == 4 {
                posts[index].style = 1;
            }
        }
    }
    for fence in fences {
        if fence.code() == 3 {
            continue;
        }
        let variants = [fence.start_post_variant, fence.end_post_variant];
        for (endpoint, position) in fence.points.iter().take(2).enumerate() {
            if let Some(variant) = variants[endpoint].filter(|value| *value != u32::MAX) {
                let post = posts.iter_mut().find(|post| post.position == *position).unwrap();
                post.variant = variant;
            }
        }
    }
    posts
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeBrokenFenceHalf {
    /// Upper start/end, lower start/end: indices 0,1,2 / 2,1,3.
    pub corners: [Vec2; 4],
    pub root: Vec2,
    pub seed: u32,
    pub shadow_start: Vec2,
    pub shadow_end: Vec2,
    pub shadow_step: Vec2,
    pub shadow_count: usize,
}

#[must_use]
pub fn native_broken_fence_halves(
    points: &[Vec2],
    start_variant: u32,
    end_variant: u32,
) -> Vec<NativeBrokenFenceHalf> {
    if points.len() < 2 {
        return Vec::new();
    }
    let start = Vec2::new(f(points[0].x), f(points[0].y));
    let end = Vec2::new(f(points[1].x), f(points[1].y));
    let inward = stored_unit(Vec2::new(f(end.x - start.x), f(f(end.y + 5.0) - f(start.y + 5.0))));
    let mut halves = Vec::with_capacity(2);
    // Retail constructs the end half before the start half.
    for is_start in [false, true] {
        let anchor = if is_start { start } else { end };
        let direction = if is_start { inward } else { Vec2::new(f(-inward.x), f(-inward.y)) };
        let selector = if is_start { start_variant } else { end_variant };
        let seed_input =
            f(f(f(f64::from(selector) + 1.0 + anchor.x) + anchor.y) * f(anchor.x * anchor.y));
        // JavaScript's bitwise conversion keeps the low word, including when
        // coordinate products exceed a signed native integer.
        let mut word = if seed_input.is_finite() {
            seed_input.trunc().rem_euclid(4_294_967_296.0) as u32
        } else {
            0
        };
        word ^= word << 21;
        word ^= word >> 11;
        let seed = ((word ^ (word << 4)).wrapping_mul(0x0a67_cfcf) as i32).unsigned_abs();
        let mut rng = NativeRng::new(i64::from(seed));
        let first = rng.draw_scaled_float(20.0, true);
        let second = rng.draw_scaled_float(20.0, true);
        let third = rng.draw_scaled_float(18.0, false);
        let tip = Vec2::new(
            f(f(anchor.x + f(52.0 * direction.x)) + f(first * direction.y)),
            f(f(anchor.y + f(52.0 * direction.y)) + f(-direction.x * first)),
        );
        let [p2, p3] = stored_inset(tip, anchor, 12.0);
        let height = f(third + 32.0);
        let p0 = Vec2::new(
            f(p2.x + f(second * direction.x)),
            f(f(f(p2.y - height) + f(second * direction.y)) - 20.0),
        );
        let p1 = Vec2::new(p3.x, f(f(p3.y - 32.0) - 20.0));
        let [shadow_start, shadow_end] = stored_inset(p2, p3, 6.0);
        let delta = Vec2::new(f(shadow_end.x - shadow_start.x), f(shadow_end.y - shadow_start.y));
        let unit = stored_unit(delta);
        let shadow_step = Vec2::new(f(8.0 * unit.x), f(8.0 * unit.y));
        let step_length = stored_length(shadow_step);
        let shadow_count = if step_length > 0.0 {
            f(stored_length(delta) / step_length).trunc() as usize + 1
        } else {
            0
        };
        halves.push(NativeBrokenFenceHalf {
            corners: [p0, p1, p2, p3],
            root: if shadow_end.y > shadow_start.y { shadow_end } else { shadow_start },
            seed,
            shadow_start,
            shadow_end,
            shadow_step,
            shadow_count,
        });
    }
    halves
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeFenceGrate {
    pub corners: [Vec2; 4],
    pub length: f64,
    pub u_span: f64,
}

#[must_use]
pub fn native_fence_grate(points: &[Vec2]) -> Option<NativeFenceGrate> {
    let (&start, &end) = (points.first()?, points.get(1)?);
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = hypot(dx, dy);
    if length == 0.0 {
        return None;
    }
    let bottom_start = Vec2::new(start.x + dx / length * 12.0, start.y + dy / length * 12.0);
    let bottom_end = Vec2::new(end.x - dx / length * 12.0, end.y - dy / length * 12.0);
    let length = hypot(bottom_end.x - bottom_start.x, bottom_end.y - bottom_start.y);
    Some(NativeFenceGrate {
        corners: [
            Vec2::new(bottom_start.x, bottom_start.y - 52.0),
            Vec2::new(bottom_end.x, bottom_end.y - 52.0),
            bottom_start,
            bottom_end,
        ],
        length,
        u_span: length / 53.33333121405716,
    })
}

pub(super) fn stored_length(point: Vec2) -> f64 {
    f(f(f(point.x * point.x) + f(point.y * point.y)).sqrt())
}

pub(super) fn stored_unit(point: Vec2) -> Vec2 {
    let length = stored_length(point);
    let inverse = if length > 0.0 { f(1.0 / length) } else { 0.0 };
    Vec2::new(f(point.x * inverse), f(point.y * inverse))
}

pub(super) fn stored_inset(start: Vec2, end: Vec2, inset: f64) -> [Vec2; 2] {
    let unit = stored_unit(Vec2::new(f(end.x - start.x), f(end.y - start.y)));
    let offset = Vec2::new(f(inset * unit.x), f(inset * unit.y));
    [
        Vec2::new(f(start.x + offset.x), f(start.y + offset.y)),
        Vec2::new(f(end.x - offset.x), f(end.y - offset.y)),
    ]
}
