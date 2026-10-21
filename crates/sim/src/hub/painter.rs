//! The courtyard painter queue, ported from `region-painter-order.ts` and
//! `hub-painter-order.ts`.
//!
//! The stock engine paints world actors by two-unit rows measured from the local player's
//! feet, and within a row by the order the actors registered with the world manager. Fixed
//! courtyard actors reserve the first ordinals in a stock order; the first player registers
//! right after them.

use crate::collision::Vec2;
use std::collections::BTreeMap;

/// Ordinals the fixed hub actors reserve before any player registers.
pub const HUB_FIXED_ACTOR_PAINTER_COUNT: usize = 48;
/// `college-obstacle-0` through `college-obstacle-7` follow the six named traders.
pub const HUB_COURTYARD_OBSTACLE_PAINTER_ORDINAL_BASE: usize = 6;
pub const HUB_COLLEGE_STATUE_PAINTER_ORDINAL: usize = 14;
/// The first player's registration in a fresh world.
pub const HUB_FIRST_PLAYER_PAINTER_ORDINAL: usize = HUB_FIXED_ACTOR_PAINTER_COUNT;

/// Inside this rectangle the player sorts under the north arch when facing away from it.
pub const HUB_NORTH_ARCH_BIAS_RECT: [f64; 4] = [874.0, 34.0, 1031.0, 181.0];

/// Every fixed courtyard painter in registration order (`NATIVE_HUB_FIXED_ACTOR_PAINTER_IDS`):
/// the five standing traders, the teacher, the eight obstacles, the statue, then the private
/// rooms' painters, the librarian, Shlorio, the arch-chancellor and the polisher. The index is
/// the registration ordinal.
pub const HUB_FIXED_PAINTER_IDS: [&str; HUB_FIXED_ACTOR_PAINTER_COUNT] = [
    "hagatha",
    "fomentius",
    "annalist",
    "luthacus",
    "skorcha",
    "teacher",
    "college-obstacle-0",
    "college-obstacle-1",
    "college-obstacle-2",
    "college-obstacle-3",
    "college-obstacle-4",
    "college-obstacle-5",
    "college-obstacle-6",
    "college-obstacle-7",
    "college-statue",
    "memorator",
    "mortuary-painting-0",
    "mortuary-painting-1",
    "mortuary-painting-100",
    "mortuary-painting-3",
    "mortuary-painting-4",
    "mortuary-painting-5",
    "mortuary-painting-6",
    "mortuary-painting-7",
    "mortuary-painting-8",
    "mortuary-painting-9",
    "mortuary-custom-0",
    "mortuary-custom-1",
    "mortuary-custom-2",
    "mortuary-custom-3",
    "mortuary-custom-4",
    "mortuary-custom-5",
    "mortuary-custom-6",
    "mortuary-custom-7",
    "mortuary-custom-8",
    "mortuary-custom-9",
    "library-custom-0",
    "library-custom-1",
    "library-custom-2",
    "library-custom-100",
    "librarian",
    "shlorio",
    "storeroom-custom-0",
    "storeroom-custom-1",
    "storeroom-custom-2",
    "office-custom-0",
    "arch-chancellor",
    "polisher",
];

/// A fixed presence on the courtyard that sorts among the actors: where it stands, how far it
/// keeps others away, and how its painter row is biased.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubFixedActor {
    pub position: Vec2,
    pub radius: f64,
    pub sort_bias: f64,
}

impl HubFixedActor {
    const fn standing(x: f64, y: f64, radius: f64) -> Self {
        Self { position: Vec2::new(x, y), radius, sort_bias: 0.0 }
    }
}

/// The eight courtyard obstacles in painter order, `college-obstacle-0` first
/// (`NATIVE_HUB_COURTYARD_OBSTACLES`).
pub const HUB_COURTYARD_OBSTACLES: [HubFixedActor; 8] = [
    HubFixedActor::standing(1458.5, 320.5, 40.0),
    HubFixedActor::standing(955.5, 239.5, 40.0),
    HubFixedActor::standing(749.5, 162.5, 40.0),
    HubFixedActor::standing(1893.0, 490.0, 40.0),
    HubFixedActor::standing(1746.0, 534.0, 40.0),
    HubFixedActor::standing(1840.0, 715.0, 40.0),
    HubFixedActor::standing(628.0, 215.0, 40.0),
    HubFixedActor::standing(956.0, 169.0, 40.0),
];

/// The College statue, sorted by its root rather than its art (`NATIVE_HUB_COLLEGE_STATUE`,
/// `HUB_STATUE_ROOT`).
pub const HUB_COLLEGE_STATUE: HubFixedActor = HubFixedActor::standing(961.0, 834.0, 50.0);

/// The registration ordinal of a fixed painter, by its Website id.
#[must_use]
pub fn hub_fixed_painter_ordinal(id: &str) -> Option<usize> {
    HUB_FIXED_PAINTER_IDS.iter().position(|candidate| *candidate == id)
}

/// World-manager lanes are visited in this order before entries enter their painter row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PainterLane {
    Actor,
    Scenery,
    Transient,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PainterLayer {
    pub visible: bool,
    /// An authored child of the root with the same manager registration. It is
    /// queued when that root's row drains, after roots already waiting in its row.
    pub is_insertion: bool,
    pub manager_lane: PainterLane,
    pub registration_ordinal: usize,
    pub world_y: f64,
    pub sort_bias: f64,
}

/// `nativeCourtyardPlayerSortBias`: ±20 inside the north arch, 0 elsewhere.
#[must_use]
pub fn courtyard_player_sort_bias(position: Vec2, heading_index: i64) -> f64 {
    let [left, top, right, bottom] = HUB_NORTH_ARCH_BIAS_RECT;
    if position.x <= left || position.x >= right || position.y <= top || position.y >= bottom {
        return 0.0;
    }
    match heading_index.rem_euclid(24) {
        0 | 12 | 23 => -20.0,
        _ => 20.0,
    }
}

/// `nativeRegionPainterRow`: the two-unit row of an actor relative to the reference actor,
/// truncated toward zero.
#[must_use]
pub fn painter_row(world_y: f64, sort_bias: f64, reference_y: f64) -> i64 {
    let relative = world_y.trunc() + sort_bias.trunc() - reference_y.trunc();
    (relative / 2.0).trunc() as i64
}

/// Native Region queue order as indices into `layers`. Roots enroll by manager
/// lane and registration; inserted bands enter only when their owner row drains.
/// Invisible roots still admit their children. Child input order is authored order.
#[must_use]
pub fn hub_painter_order(layers: &[PainterLayer], reference_y: f64) -> Vec<usize> {
    let mut roots = Vec::new();
    let mut insertions: BTreeMap<(PainterLane, usize), Vec<usize>> = BTreeMap::new();
    for (index, layer) in layers.iter().enumerate() {
        if layer.is_insertion {
            insertions
                .entry((layer.manager_lane, layer.registration_ordinal))
                .or_default()
                .push(index);
        } else {
            roots.push(index);
        }
    }
    roots.sort_by_key(|&index| (layers[index].manager_lane, layers[index].registration_ordinal));
    for pair in roots.windows(2) {
        assert_ne!(
            (layers[pair[0]].manager_lane, layers[pair[0]].registration_ordinal),
            (layers[pair[1]].manager_lane, layers[pair[1]].registration_ordinal),
            "duplicate painter registration ordinal in one manager lane"
        );
    }
    let mut rows: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    for index in roots {
        let layer = layers[index];
        rows.entry(painter_row(layer.world_y, layer.sort_bias, reference_y))
            .or_default()
            .push(index);
    }
    let mut order = Vec::new();
    while let Some((row, mut bucket)) = rows.pop_first() {
        let mut next = 0;
        while next < bucket.len() {
            let index = bucket[next];
            next += 1;
            let layer = layers[index];
            if layer.visible {
                order.push(index);
            }
            if layer.is_insertion {
                continue;
            }
            let Some(children) =
                insertions.remove(&(layer.manager_lane, layer.registration_ordinal))
            else {
                continue;
            };
            for child in children {
                let layer = layers[child];
                let child_row = painter_row(layer.world_y, layer.sort_bias, reference_y);
                assert!(
                    child_row >= row,
                    "a native painter cannot insert into a row that already painted"
                );
                if child_row == row {
                    bucket.push(child);
                } else {
                    rows.entry(child_row).or_default().push(child);
                }
            }
        }
    }
    assert!(insertions.is_empty(), "an inserted painter must have a registered owner");
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixed_painter_table_places_the_obstacles_statue_and_first_player() {
        assert_eq!(
            hub_fixed_painter_ordinal("college-obstacle-0"),
            Some(HUB_COURTYARD_OBSTACLE_PAINTER_ORDINAL_BASE)
        );
        assert_eq!(
            hub_fixed_painter_ordinal("college-statue"),
            Some(HUB_COLLEGE_STATUE_PAINTER_ORDINAL)
        );
        assert_eq!(
            hub_fixed_painter_ordinal("polisher"),
            Some(HUB_FIRST_PLAYER_PAINTER_ORDINAL - 1)
        );
        assert_eq!(hub_fixed_painter_ordinal("wizard"), None);
        let mut sorted = HUB_FIXED_PAINTER_IDS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), HUB_FIXED_ACTOR_PAINTER_COUNT, "ids are unique");
    }

    #[test]
    fn rows_truncate_toward_zero_around_the_reference() {
        assert_eq!(painter_row(100.0, 0.0, 100.0), 0);
        assert_eq!(painter_row(101.9, 0.0, 100.0), 0);
        assert_eq!(painter_row(102.0, 0.0, 100.0), 1);
        assert_eq!(painter_row(99.0, 0.0, 100.0), 0);
        assert_eq!(painter_row(97.0, 0.0, 100.0), -1);
        assert_eq!(painter_row(100.0, 20.0, 100.0), 10);
        assert_eq!(painter_row(100.0, -20.5, 100.0), -10);
    }

    #[test]
    fn same_row_paints_in_registration_order_and_lower_rows_first() {
        let player = PainterLayer {
            visible: true,
            is_insertion: false,
            manager_lane: PainterLane::Actor,
            registration_ordinal: 48,
            world_y: 101.0,
            sort_bias: 0.0,
        };
        let obstacle_just_below = PainterLayer {
            visible: true,
            is_insertion: false,
            manager_lane: PainterLane::Actor,
            registration_ordinal: 7,
            world_y: 102.0,
            sort_bias: 0.0,
        };
        let obstacle_two_below = PainterLayer {
            visible: true,
            is_insertion: false,
            manager_lane: PainterLane::Actor,
            registration_ordinal: 6,
            world_y: 103.0,
            sort_bias: 0.0,
        };
        let obstacle_above = PainterLayer {
            visible: true,
            is_insertion: false,
            manager_lane: PainterLane::Actor,
            registration_ordinal: 9,
            world_y: 60.0,
            sort_bias: 0.0,
        };
        let order = hub_painter_order(
            &[player, obstacle_just_below, obstacle_two_below, obstacle_above],
            player.world_y,
        );
        assert_eq!(order, vec![3, 1, 0, 2]);
    }

    #[test]
    fn the_arch_bias_depends_on_position_and_facing() {
        let inside = Vec2::new(950.0, 100.0);
        assert_eq!(courtyard_player_sort_bias(inside, 12), -20.0);
        assert_eq!(courtyard_player_sort_bias(inside, 0), -20.0);
        assert_eq!(courtyard_player_sort_bias(inside, -1), -20.0);
        assert_eq!(courtyard_player_sort_bias(inside, 6), 20.0);
        assert_eq!(courtyard_player_sort_bias(Vec2::new(874.0, 100.0), 6), 0.0);
        assert_eq!(courtyard_player_sort_bias(Vec2::new(950.0, 181.0), 6), 0.0);
        assert_eq!(courtyard_player_sort_bias(Vec2::new(950.0, 300.0), 12), 0.0);
    }
}
