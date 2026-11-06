//! Screen flash writes in authority order, shared by every effect that flashes the screen.
//!
//! Family-local actor/event IDs cannot order these writes. One writer spans an
//! accepted Session operation; the client applies point gain and conditional admission.

use crate::Vec2;
use crate::secondary::SecondaryScreenFlashState;
use std::collections::BTreeMap;

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// One accepted screen flash write: its authority order, tick, world, position and flash.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ScreenFlashWrite {
    pub order: u64,
    pub tick: u64,
    pub world_key: String,
    pub position: Vec2,
    pub flash: SecondaryScreenFlashState,
    pub only_if_clear: bool,
}

/// The current epoch's retained screen flash writes and the next write order.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct ScreenFlashState {
    pub epoch: u64,
    pub next_order: u64,
    pub writes: Vec<ScreenFlashWrite>,
}

impl Default for ScreenFlashState {
    fn default() -> Self {
        Self { epoch: 0, next_order: 1, writes: Vec::new() }
    }
}

impl ScreenFlashState {
    pub fn reset(&mut self) {
        assert!(self.epoch < MAX_SAFE_INTEGER, "screen flash epoch exhausted");
        self.epoch += 1;
        self.writes.clear();
    }

    /// Prepares the private writer for one operation. An unaccepted operation can
    /// discard it without pruning or otherwise mutating the retained authority.
    #[must_use]
    pub fn writer(&self, tick: u64) -> ScreenFlashWriter {
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
        ScreenFlashWriter { state: Self { epoch: self.epoch, next_order: self.next_order, writes } }
    }
}

/// The producer supplies no registration order; only the shared writer assigns it.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenFlashInput {
    pub tick: u64,
    pub world_key: String,
    pub position: Vec2,
    pub flash: SecondaryScreenFlashState,
    pub only_if_clear: bool,
}

/// Assigns authority order to one operation's screen flash writes.
pub struct ScreenFlashWriter {
    state: ScreenFlashState,
}

impl ScreenFlashWriter {
    pub fn write(&mut self, input: ScreenFlashInput) {
        assert!(self.state.next_order < MAX_SAFE_INTEGER, "screen flash order exhausted");
        self.state.writes.push(ScreenFlashWrite {
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
    pub fn finish(self) -> ScreenFlashState {
        self.state
    }
}
