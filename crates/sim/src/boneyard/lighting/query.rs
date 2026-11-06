//! PC24 light queries: the radial and elevated light a point receives from the field.
//! Camera-relative stores are observable at large/fractional world positions.
//! Puppet callbacks keep radial/elevated lanes separate; surface callbacks multiply.

use super::{BoneyardLightSource, LIGHT_VERTICAL_SCALE};
use crate::Vec2;
use crate::float::fround as f;

/// The camera origin that light positions are measured from, and the ambient light level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneyardLightQuery {
    pub camera_origin: Vec2,
    pub ambient: f64,
}

impl Default for BoneyardLightQuery {
    fn default() -> Self {
        Self { camera_origin: Vec2::ZERO, ambient: 0.0 }
    }
}

/// The light at a point: the radial factor, and the elevated factor, which also fades for
/// points below a source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneyardLightFactors {
    pub radial: f64,
    pub elevated: f64,
}

#[must_use]
pub fn boneyard_source_light_factors(
    position: Vec2,
    source: BoneyardLightSource,
    query: BoneyardLightQuery,
) -> BoneyardLightFactors {
    let camera_x = f(query.camera_origin.x);
    let camera_y = f(query.camera_origin.y);
    let root_x = f(f(position.x) - camera_x);
    let root_y = f(f(position.y) - camera_y);
    let source_x = f(f(source.position.x) - camera_x);
    let source_y = f(f(source.position.y) - camera_y);
    let radius = f(source.radius);
    let dx = f(f(root_x - source_x) / radius);
    let gap = f(root_y - source_y);
    let dy = f(f(gap / LIGHT_VERTICAL_SCALE) / radius);
    let distance_squared = f(f(dx * dx) + f(dy * dy));
    if distance_squared >= 21_025.0 {
        return BoneyardLightFactors { radial: 0.0, elevated: 0.0 };
    }
    let intensity = f(source.intensity);
    let radial = if distance_squared < 5_625.0 {
        intensity
    } else {
        f(f(1.0 - f(f(distance_squared - 5_625.0) / 15_400.0)) * intensity)
    };
    // Height uses the unnormalized camera-space gap, separately from radial Y.
    let height =
        if root_y <= source_y { 1.0 } else { maximum(0.0, f(1.0 - f(f(gap * 1.5) / 145.0))) };
    BoneyardLightFactors { radial, elevated: f(radial * height) }
}

pub(super) fn factors(
    position: Vec2,
    sources: impl Iterator<Item = BoneyardLightSource>,
    query: BoneyardLightQuery,
) -> BoneyardLightFactors {
    let mut output = BoneyardLightFactors { radial: f(query.ambient), elevated: 0.0 };
    for source in sources {
        let value = boneyard_source_light_factors(position, source, query);
        output.radial = maximum(output.radial, value.radial);
        output.elevated = maximum(output.elevated, value.elevated);
    }
    output
}

pub(super) fn surface(
    position: Vec2,
    sources: impl Iterator<Item = BoneyardLightSource>,
    query: BoneyardLightQuery,
) -> f64 {
    let output = factors(position, sources, query);
    f(output.radial * maximum(f(query.ambient), output.elevated))
}

fn maximum(left: f64, right: f64) -> f64 {
    // NaN from a degenerate division, such as by a zero light radius, propagates here;
    // `f64::max` would discard it.
    if left.is_nan() || right.is_nan() { f64::NAN } else { left.max(right) }
}
