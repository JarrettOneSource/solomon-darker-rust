use std::cell::Cell;
use std::time::{Duration, Instant};

use super::{FLUSH_INTERVAL_MS, flush};

pub struct PlaytimeRecorder {
    marked_at: Cell<f64>,
    next_flush: Instant,
}

impl PlaytimeRecorder {
    pub fn start() -> Self {
        Self {
            marked_at: Cell::new(crate::platform::epoch_ms()),
            next_flush: Instant::now() + Duration::from_millis(FLUSH_INTERVAL_MS),
        }
    }

    pub fn tick(&mut self) -> Result<(), String> {
        let now = Instant::now();
        if now >= self.next_flush {
            self.next_flush = now + Duration::from_millis(FLUSH_INTERVAL_MS);
            flush(&self.marked_at)?;
        }
        Ok(())
    }
}

impl Drop for PlaytimeRecorder {
    fn drop(&mut self) {
        if let Err(error) = flush(&self.marked_at) {
            crate::platform::error(&error);
        }
    }
}
