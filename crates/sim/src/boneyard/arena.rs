//! A generated Boneyard's entrance bounds: when combat starts, the camera closes from the full
//! entrance area onto the smaller combat bounds and the arena seals.

use crate::Vec2;
use crate::float::fround;

use super::BoneyardBounds;

/// The arena's stage: open before combat, locking while the camera closes in, then sealed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum ArenaPhase {
    Open,
    Locking,
    Sealed,
}

/// The side of the Boneyard that players enter from, chosen by the spawn position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EntrySide {
    North,
    South,
}

/// The full entrance area and the smaller bounds used once combat starts.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ArenaTransition {
    pub blend_factor: f64,
    pub camera_bounds: BoneyardBounds,
    pub combat_bounds: BoneyardBounds,
    pub entry_side: EntrySide,
    pub full_bounds: BoneyardBounds,
    pub phase: ArenaPhase,
    pub seal_ticks_remaining: u64,
}

impl ArenaTransition {
    #[must_use]
    pub fn new(bounds: BoneyardBounds, spawn: Vec2) -> Self {
        assert!(bounds.height > 400.0, "generated Boneyard needs its 400-unit entrance extension");
        let entry_side = if spawn.y < bounds.y + bounds.height / 2.0 {
            EntrySide::North
        } else {
            EntrySide::South
        };
        let full_bounds = copy_bounds(bounds);
        let north_inset = if entry_side == EntrySide::North { 375.0 } else { 0.0 };
        let combat_bounds = BoneyardBounds::new(
            fround(bounds.x),
            fround(bounds.y + north_inset),
            fround(bounds.width),
            fround(bounds.height - 400.0),
        );
        Self {
            blend_factor: 0.0,
            camera_bounds: full_bounds,
            combat_bounds,
            entry_side,
            full_bounds,
            phase: ArenaPhase::Open,
            seal_ticks_remaining: 0,
        }
    }

    /// The wave director calls this only once every living body is inside combat bounds.
    pub fn start(&mut self) {
        if self.phase != ArenaPhase::Open {
            return;
        }
        self.blend_factor = 0.01_f32 as f64;
        self.phase = ArenaPhase::Locking;
        self.seal_ticks_remaining = 400;
    }

    /// Each tick narrows both the interpolated bounds and the growing blend to single precision.
    pub fn step(&mut self) {
        if self.phase == ArenaPhase::Open {
            return;
        }
        let source = self.camera_bounds;
        let target = self.combat_bounds;
        let interpolate = |start, end| fround(start + (end - start) * self.blend_factor);
        self.camera_bounds = BoneyardBounds::new(
            interpolate(source.x, target.x),
            interpolate(source.y, target.y),
            interpolate(source.width, target.width),
            interpolate(source.height, target.height),
        );
        self.blend_factor = fround((self.blend_factor * 1.01).min(1.0));
        self.seal_ticks_remaining = self.seal_ticks_remaining.saturating_sub(1);
        if self.seal_ticks_remaining == 0 {
            self.phase = ArenaPhase::Sealed;
        }
    }

    /// Where a body of `radius` enters once the arena has begun to lock: the spawn
    /// point moved just inside the combat bounds. The spawn point lies in the entrance
    /// strip, which the locked arena excludes, and its column is the path every wizard
    /// walked in on.
    #[must_use]
    pub fn entry_inside(&self, spawn: Vec2, radius: f64) -> Vec2 {
        let bounds = self.combat_bounds;
        let clamp = |value: f64, start: f64, extent: f64| {
            fround(value.clamp(start + radius, start + extent - radius))
        };
        Vec2::new(clamp(spawn.x, bounds.x, bounds.width), clamp(spawn.y, bounds.y, bounds.height))
    }

    #[must_use]
    pub fn active_bounds(&self) -> BoneyardBounds {
        if self.phase == ArenaPhase::Open { self.full_bounds } else { self.combat_bounds }
    }

    #[must_use]
    pub fn safety_clear(&self, bodies: &[(Vec2, f64)]) -> bool {
        bodies.iter().all(|(position, radius)| {
            radius.is_finite()
                && *radius > 0.0
                && position.x - radius >= self.combat_bounds.x
                && position.y - radius >= self.combat_bounds.y
                && position.x + radius <= self.combat_bounds.x + self.combat_bounds.width
                && position.y + radius <= self.combat_bounds.y + self.combat_bounds.height
        })
    }
}

fn copy_bounds(bounds: BoneyardBounds) -> BoneyardBounds {
    BoneyardBounds::new(
        fround(bounds.x),
        fround(bounds.y),
        fround(bounds.width),
        fround(bounds.height),
    )
}
