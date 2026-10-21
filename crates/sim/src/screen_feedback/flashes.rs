//! Current Source `renderer/native-screen-flash.ts`: one accepted overwrite lane,
//! sampled at display time with point gain captured before each write is eligible.

use std::collections::BTreeMap;

use super::{NativeSecondaryScreenFeedbackContext, NativeSecondaryScreenOverlay};
use crate::float::fround;
use crate::screen_flash::{NativeScreenFlashState, NativeScreenFlashWrite};
use crate::secondary::presentation::draws::{pack_normalized_rgb, repeated_float_decay};

#[derive(Clone, Debug)]
struct ObservedWrite {
    write: NativeScreenFlashWrite,
    gain: f64,
    eligible: bool,
}

#[derive(Clone, Debug)]
pub struct NativeScreenFlashPresentation {
    writes: BTreeMap<u64, ObservedWrite>,
    epoch: Option<u64>,
    retired_order: u64,
    tick: u64,
    world_key: String,
}

impl NativeScreenFlashPresentation {
    #[must_use]
    pub fn new(initial_tick: u64, world_key: String) -> Self {
        Self {
            writes: BTreeMap::new(),
            epoch: None,
            retired_order: 0,
            tick: initial_tick,
            world_key,
        }
    }

    pub fn consume(
        &mut self,
        state: &NativeScreenFlashState,
        context: NativeSecondaryScreenFeedbackContext,
    ) {
        if self.epoch.is_some_and(|epoch| state.epoch < epoch) {
            return;
        }
        if self.epoch != Some(state.epoch) {
            self.writes.clear();
            self.retired_order = 0;
            self.epoch = Some(state.epoch);
        }
        for write in &state.writes {
            if write.world_key != self.world_key || write.order <= self.retired_order {
                continue;
            }
            if self.writes.get(&write.order).is_some_and(|existing| existing.eligible) {
                continue;
            }
            let gain =
                if write.flash.point_attenuated { context.point_gain(write.position) } else { 1.0 };
            self.writes
                .insert(write.order, ObservedWrite { write: write.clone(), gain, eligible: false });
        }
    }

    pub fn sample(&mut self, tick: f64) -> Option<NativeSecondaryScreenOverlay> {
        self.tick = self.tick.max(tick.trunc().max(0.0) as u64);
        let mut winner: Option<crate::secondary::NativeSecondaryScreenFlashState> = None;
        let mut alpha = 0.0;
        let mut last_tick = 0;
        let mut anchor = None;
        for observed in self.writes.values_mut() {
            let write = &observed.write;
            if write.tick > self.tick {
                continue;
            }
            observed.eligible = true;
            if let Some(flash) = winner {
                alpha = repeated_float_decay(
                    alpha,
                    flash.decay_per_tick,
                    write.tick.saturating_sub(last_tick),
                );
            }
            last_tick = write.tick;
            if !write.only_if_clear {
                anchor = Some(write.order);
            }
            if write.only_if_clear && alpha > 0.0 {
                continue;
            }
            winner = Some(write.flash);
            alpha = fround(write.flash.alpha * observed.gain);
        }
        let flash = winner?;
        alpha = repeated_float_decay(alpha, flash.decay_per_tick, self.tick - last_tick);
        if let Some(order) = anchor {
            self.retired_order = self.retired_order.max(order - 1);
        }
        self.writes.retain(|order, _| *order > self.retired_order);
        if alpha <= 0.0 {
            return None;
        }
        Some(NativeSecondaryScreenOverlay {
            alpha,
            color: pack_normalized_rgb(flash.red, flash.green, flash.blue),
        })
    }
}
