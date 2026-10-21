//! Shared authority-order screen flashes from `core-kernels/native-screen-flash.ts`.
//!
//! Family-local actor/event IDs cannot order these writes. One writer spans an
//! accepted Session operation; the browser applies point gain and conditional admission.

use crate::Vec2;
use crate::secondary::NativeSecondaryScreenFlashState;
use std::collections::BTreeMap;

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeScreenFlashWrite {
    pub order: u64,
    pub tick: u64,
    pub world_key: String,
    pub position: Vec2,
    pub flash: NativeSecondaryScreenFlashState,
    pub only_if_clear: bool,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeScreenFlashState {
    pub epoch: u64,
    pub next_order: u64,
    pub writes: Vec<NativeScreenFlashWrite>,
}

impl Default for NativeScreenFlashState {
    fn default() -> Self {
        Self { epoch: 0, next_order: 1, writes: Vec::new() }
    }
}

impl NativeScreenFlashState {
    pub fn reset(&mut self) {
        assert!(self.epoch < MAX_SAFE_INTEGER, "screen flash epoch exhausted");
        self.epoch += 1;
        self.writes.clear();
    }

    /// Prepare the source's private operation writer. An unaccepted operation can
    /// discard it without pruning or otherwise mutating the retained authority.
    #[must_use]
    pub fn writer(&self, tick: u64) -> NativeScreenFlashWriter {
        let mut anchors = BTreeMap::new();
        for write in &self.writes {
            if !write.only_if_clear && (write.tick as f64) < tick as f64 - 200.0 {
                anchors.insert(write.world_key.as_str(), write.order);
            }
        }
        let mut writes = Vec::new();
        for write in &self.writes {
            if write.order >= anchors.get(write.world_key.as_str()).copied().unwrap_or(0) {
                writes.push(write.clone());
            }
        }
        NativeScreenFlashWriter {
            state: Self { epoch: self.epoch, next_order: self.next_order, writes },
        }
    }
}

/// The producer supplies no registration order; only the shared writer assigns it.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeScreenFlashInput {
    pub tick: u64,
    pub world_key: String,
    pub position: Vec2,
    pub flash: NativeSecondaryScreenFlashState,
    pub only_if_clear: bool,
}

pub struct NativeScreenFlashWriter {
    state: NativeScreenFlashState,
}

impl NativeScreenFlashWriter {
    pub fn write(&mut self, input: NativeScreenFlashInput) {
        assert!(self.state.next_order < MAX_SAFE_INTEGER, "screen flash order exhausted");
        self.state.writes.push(NativeScreenFlashWrite {
            order: self.state.next_order,
            tick: input.tick,
            world_key: input.world_key,
            position: input.position,
            flash: input.flash,
            only_if_clear: input.only_if_clear,
        });
        self.state.next_order += 1;
    }

    #[must_use]
    pub fn finish(self) -> NativeScreenFlashState {
        self.state
    }
}
