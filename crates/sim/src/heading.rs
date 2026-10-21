//! Actor heading quantization and the wizard walk cycle.
//!
//! Headings are 24 sprite facings 15° apart, index 0 pointing up the screen (north) and
//! increasing clockwise. Ported from the Website `core-kernels/actor-heading.ts` and
//! `player-character.ts`.

use crate::collision::Vec2;
use crate::float;
use crate::float::{atan2, fround};

pub const HEADING_COUNT: usize = 24;
pub const HEADING_STEP_DEGREES: f64 = 15.0;
/// Walk-cycle distance per pose, in world units.
pub const WALK_CYCLE_DISTANCE_PER_FRAME: f64 = 10.0;
/// Number of walk poses; the cycle wraps back to zero past this value.
pub const WALK_CYCLE_WRAP: f64 = 5.0;
/// Gait phase advance per world unit travelled, in degrees.
pub const GAIT_DEGREES_PER_UNIT: f64 = 5.0;

/// Heading in degrees `[0, 360)` for a travel vector, clockwise from north.
#[must_use]
pub fn heading_degrees_from_vector(x: f64, y: f64) -> f64 {
    let degrees = atan2(x, -y) * 180.0 / std::f64::consts::PI;
    (degrees + 360.0) % 360.0
}

/// The sprite facing nearest to a heading in degrees.
#[must_use]
pub fn heading_index(heading_degrees: f64) -> usize {
    let normalized = ((heading_degrees % 360.0) + 360.0) % 360.0;
    (((normalized + HEADING_STEP_DEGREES / 2.0) / HEADING_STEP_DEGREES).floor() as usize)
        % HEADING_COUNT
}

/// The unit travel vector at the centre of a sprite facing.
#[must_use]
pub fn heading_vector(heading_index: usize) -> Vec2 {
    let radians = heading_index as f64 * HEADING_STEP_DEGREES * std::f64::consts::PI / 180.0;
    Vec2::new(float::sin(radians), -float::cos(radians))
}

/// Advances the walk cycle by the distance travelled this tick.
#[must_use]
pub fn advance_walk_cycle(walk_cycle: f64, distance: f64) -> f64 {
    let advanced = fround(walk_cycle + distance / WALK_CYCLE_DISTANCE_PER_FRAME);
    if advanced > WALK_CYCLE_WRAP { fround(advanced - WALK_CYCLE_WRAP) } else { advanced }
}

/// Advances the gait phase by the distance travelled this tick.
#[must_use]
pub fn advance_gait_degrees(gait_degrees: f64, distance: f64) -> f64 {
    (gait_degrees + distance * GAIT_DEGREES_PER_UNIT) % 360.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinal_directions_map_to_the_expected_facings() {
        assert_eq!(heading_index(heading_degrees_from_vector(0.0, -1.0)), 0);
        assert_eq!(heading_index(heading_degrees_from_vector(1.0, 0.0)), 6);
        assert_eq!(heading_index(heading_degrees_from_vector(0.0, 1.0)), 12);
        assert_eq!(heading_index(heading_degrees_from_vector(-1.0, 0.0)), 18);
        assert_eq!(heading_index(180.0), 12);
        assert_eq!(heading_index(-7.5), 0);
        assert_eq!(heading_index(352.5), 0);
        assert_eq!(heading_index(352.49), 23);
    }

    #[test]
    fn heading_vectors_round_trip_through_their_index() {
        for index in 0..HEADING_COUNT {
            let vector = heading_vector(index);
            assert_eq!(heading_index(heading_degrees_from_vector(vector.x, vector.y)), index);
        }
    }

    #[test]
    fn walk_cycle_wraps_past_five_poses() {
        let mut cycle = 0.0;
        for _ in 0..49 {
            cycle = advance_walk_cycle(cycle, 1.0);
        }
        assert!(cycle < 5.0);
        cycle = advance_walk_cycle(cycle, 20.0);
        assert!(cycle < 5.0, "cycle {cycle} did not wrap");
    }
}
