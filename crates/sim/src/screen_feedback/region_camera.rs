//! Current Source `renderer/native-region-camera-presentation.ts`.
//! Late Flame births replay one short shared history; independently aging each
//! family's displacement would reverse stored-float ties and cutoff decisions.

use std::collections::{BTreeMap, BTreeSet};

use crate::Vec2;
use crate::float::fround;
use crate::secondary::presentation::draws::repeated_float_multiply;
use crate::spells::weld::{
    NATIVE_FLAME_CAMERA_MAX_PRESENTATION_TICKS, decay_native_region_camera_displacement,
    native_camera_magnitude_squared,
};

#[derive(Clone, Debug)]
pub struct NativeRegionCameraPresentation {
    checkpoint_tick: i64,
    checkpoint: Vec2,
    tick: u64,
    value: Vec2,
    before_decay: BTreeMap<u64, (Vec2, u64)>,
    after_decay: BTreeMap<u64, Vec2>,
}

impl NativeRegionCameraPresentation {
    #[must_use]
    pub fn new(initial_tick: u64) -> Self {
        Self {
            checkpoint_tick: initial_tick as i64
                - NATIVE_FLAME_CAMERA_MAX_PRESENTATION_TICKS as i64,
            checkpoint: Vec2::ZERO,
            tick: initial_tick,
            value: Vec2::ZERO,
            before_decay: BTreeMap::new(),
            after_decay: BTreeMap::new(),
        }
    }

    pub fn reset(&mut self, tick: u64) {
        *self = Self::new(tick);
    }

    #[must_use]
    pub const fn sample(&self) -> Vec2 {
        self.value
    }

    pub fn advance_to(&mut self, tick: u64) {
        if tick <= self.tick {
            return;
        }
        self.tick = tick;
        let floor = tick as i64 - NATIVE_FLAME_CAMERA_MAX_PRESENTATION_TICKS as i64;
        if floor > self.checkpoint_tick {
            self.checkpoint = self.replay(floor);
            self.checkpoint_tick = floor;
            self.before_decay.retain(|at, _| *at as i64 > floor);
            self.after_decay.retain(|at, _| *at as i64 > floor);
        }
        self.value = self.replay(tick as i64);
    }

    /// Existing families enter after the Region update represented by the event.
    pub fn propose_current(&mut self, displacement: Vec2, event_tick: u64) {
        let elapsed = self.tick.saturating_sub(event_tick);
        let proposal = Vec2::new(
            repeated_float_multiply(fround(displacement.x), 0.75_f32 as f64, elapsed),
            repeated_float_multiply(fround(displacement.y), 0.75_f32 as f64, elapsed),
        );
        if self.after_decay.get(&self.tick).is_none_or(|previous| {
            native_camera_magnitude_squared(proposal) > native_camera_magnitude_squared(*previous)
        }) {
            self.after_decay.insert(self.tick, proposal);
        }
        self.value = self.replay(self.tick as i64);
    }

    /// Flame enters before its birth pass's shared decay; IDs preserve birth order.
    pub fn propose_flame(&mut self, displacement: Vec2, birth_tick: u64, order: u64) {
        if birth_tick as i64 <= self.checkpoint_tick {
            return;
        }
        let proposal = Vec2::new(fround(displacement.x), fround(displacement.y));
        let squared = native_camera_magnitude_squared(proposal);
        if self.before_decay.get(&birth_tick).is_none_or(|(previous, previous_order)| {
            let previous_squared = native_camera_magnitude_squared(*previous);
            squared > previous_squared || (squared == previous_squared && order < *previous_order)
        }) {
            self.before_decay.insert(birth_tick, (proposal, order));
        }
        self.value = self.replay(self.tick as i64);
    }

    fn replay(&self, through_tick: i64) -> Vec2 {
        let mut value = self.checkpoint;
        let mut tick = self.checkpoint_tick;
        let changes: BTreeSet<_> = self
            .before_decay
            .keys()
            .chain(self.after_decay.keys())
            .copied()
            .filter(|at| tick < *at as i64 && *at as i64 <= through_tick)
            .collect();
        for at in changes {
            value = decay_native_region_camera_displacement(value, (at as i64 - tick - 1) as u64);
            if let Some((before, _)) = self.before_decay.get(&at) {
                value = stronger(value, *before);
            }
            value = decay_native_region_camera_displacement(value, 1);
            if let Some(after) = self.after_decay.get(&at) {
                value = stronger(value, *after);
            }
            tick = at as i64;
        }
        decay_native_region_camera_displacement(value, (through_tick - tick) as u64)
    }
}

fn stronger(current: Vec2, proposal: Vec2) -> Vec2 {
    if native_camera_magnitude_squared(proposal) > native_camera_magnitude_squared(current) {
        proposal
    } else {
        current
    }
}
