//! `protocol/flame-presentation-history.ts`, retaining Source Map insertion order.
use super::{
    FlamePresentationBaseline, FlamePresentationBirth, FlamePresentationFrame, require_tick,
};

pub struct FlamePresentationHistoryOptions {
    pub world_key: String,
    pub tick: u64,
    pub maximum_births: usize,
    pub maximum_bytes: Option<usize>,
    pub initial_epoch: u64,
}

#[derive(Clone, Debug)]
struct RetainedBirth {
    birth: FlamePresentationBirth,
    changed_tick: u64,
    bytes: usize,
}

/// The native edge passes the existing ECMAScript JSON formatter and counts
/// UTF-16 bytes. Keeping that I/O dependency outside protocol avoids a new codec.
pub type FlamePresentationBirthBytes = fn(&FlamePresentationBirth) -> Result<usize, String>;

#[derive(Clone, Debug)]
pub struct FlamePresentationHistory {
    world_key: String,
    tick: u64,
    epoch: u64,
    covered_after_tick: u64,
    maximum_births: usize,
    maximum_bytes: Option<usize>,
    retained_bytes: usize,
    encoded_bytes: FlamePresentationBirthBytes,
    // Source iteration order is admission order, including retirement rewrites.
    births: Vec<RetainedBirth>,
}

impl FlamePresentationHistory {
    pub fn new(
        options: FlamePresentationHistoryOptions,
        encoded_bytes: FlamePresentationBirthBytes,
    ) -> Result<Self, String> {
        require_tick(options.tick)?;
        require_tick(options.initial_epoch)?;
        if options.maximum_births == 0 {
            return Err("Flame presentation history needs a positive birth limit".into());
        }
        if options.initial_epoch == 0 {
            return Err("Invalid Flame history epoch".into());
        }
        if options.maximum_bytes == Some(0) {
            return Err("Invalid Flame history byte budget".into());
        }
        Ok(Self {
            world_key: options.world_key,
            tick: options.tick,
            epoch: options.initial_epoch,
            covered_after_tick: options.tick,
            maximum_births: options.maximum_births,
            maximum_bytes: options.maximum_bytes,
            retained_bytes: 0,
            encoded_bytes,
            births: Vec::new(),
        })
    }
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }
    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }
    pub fn reset(&mut self, world_key: String, tick: u64) -> Result<(), String> {
        require_tick(tick)?;
        require_tick(self.epoch + 1)?;
        self.world_key = world_key;
        self.tick = tick;
        self.covered_after_tick = tick;
        self.epoch += 1;
        self.births.clear();
        self.retained_bytes = 0;
        Ok(())
    }
    pub fn record_tick(
        &mut self,
        tick: u64,
        incoming: &[FlamePresentationBirth],
    ) -> Result<(), String> {
        require_tick(tick)?;
        if tick < self.tick || tick > self.tick + 1 {
            return Err("Flame history must observe each admitted world update".into());
        }
        let mut prepared: Vec<RetainedBirth> = Vec::new();
        for birth in incoming {
            birth.validate(&self.world_key)?;
            if birth.birth_tick != tick {
                return Err("Flame birth does not belong to this update".into());
            }
            let prior = self.births.iter().find(|item| item.birth.same_identity(birth));
            let in_batch = prepared.iter().find(|item| item.birth.same_identity(birth));
            if prior.is_some_and(|item| !item.birth.same_recipe(birth))
                || in_batch.is_some_and(|item| !item.birth.same_recipe(birth))
            {
                return Err("Flame birth recipe changed".into());
            }
            if prior.is_none() && in_batch.is_none() {
                prepared.push(RetainedBirth {
                    birth: birth.clone(),
                    changed_tick: tick,
                    bytes: (self.encoded_bytes)(birth)?,
                });
            }
        }
        self.tick = tick;
        for item in prepared {
            self.retained_bytes += item.bytes;
            self.births.push(item);
        }
        self.enforce_budget();
        Ok(())
    }
    pub fn retire_birth(
        &mut self,
        birth: &FlamePresentationBirth,
        at_tick: u64,
    ) -> Result<(), String> {
        self.require_committed_tick(at_tick)?;
        birth.validate(&self.world_key)?;
        if let Some(item) = self.births.iter_mut().find(|item| item.birth.same_identity(birth)) {
            if item.birth.end_tick_exclusive <= at_tick {
                return Ok(());
            }
            item.birth.end_tick_exclusive = at_tick;
            item.changed_tick = at_tick;
        } else {
            if birth.end_tick_exclusive <= at_tick {
                return Ok(());
            }
            let mut retired = birth.clone();
            retired.end_tick_exclusive = at_tick;
            let bytes = (self.encoded_bytes)(&retired)?;
            self.births.push(RetainedBirth { birth: retired, changed_tick: at_tick, bytes });
            self.retained_bytes += bytes;
        }
        self.enforce_budget();
        Ok(())
    }
    pub fn retire_owner(&mut self, owner_id: &str, at_tick: u64) -> Result<(), String> {
        self.require_committed_tick(at_tick)?;
        for item in &mut self.births {
            if item.birth.owner_id == owner_id && item.birth.end_tick_exclusive > at_tick {
                item.birth.end_tick_exclusive = at_tick;
                item.changed_tick = at_tick;
            }
        }
        Ok(())
    }
    pub fn discard_through_tick(&mut self, oldest_baseline: u64) -> Result<(), String> {
        require_tick(oldest_baseline)?;
        if oldest_baseline > self.tick {
            return Err("Cannot prune future Flame history".into());
        }
        self.births.retain(|item| {
            if item.birth.end_tick_exclusive <= oldest_baseline
                && item.changed_tick <= oldest_baseline
            {
                self.retained_bytes -= item.bytes;
                false
            } else {
                true
            }
        });
        self.covered_after_tick = self.covered_after_tick.max(oldest_baseline);
        Ok(())
    }
    pub fn frame(
        &self,
        baseline: Option<FlamePresentationBaseline>,
        through_tick: u64,
    ) -> Result<FlamePresentationFrame, String> {
        if through_tick != self.tick {
            return Err("Flame frame must use the committed world tick".into());
        }
        let Some(baseline) = baseline.filter(|baseline| {
            baseline.epoch == self.epoch
                && baseline.world_tick >= self.covered_after_tick
                && baseline.world_tick <= self.tick
        }) else {
            return Ok(FlamePresentationFrame::reset(
                self.epoch,
                self.world_key.clone(),
                self.tick,
            ));
        };
        let births = self
            .births
            .iter()
            .filter(|item| {
                item.birth.birth_tick > baseline.world_tick
                    || item.changed_tick > baseline.world_tick
            })
            .map(|item| item.birth.clone())
            .collect();
        Ok(FlamePresentationFrame::Continuous {
            epoch: self.epoch,
            world_key: self.world_key.clone(),
            through_tick: self.tick,
            after_tick: baseline.world_tick,
            births,
        })
    }
    /// Source host advance over compact before/after captures. A same-tick
    /// mutation cannot be acknowledged by the stopped-tick cursor, so it resets.
    pub fn advance(
        &mut self,
        world_key: &str,
        tick: u64,
        previous: &[FlamePresentationBirth],
        current: &[FlamePresentationBirth],
    ) -> Result<(), String> {
        require_tick(tick)?;
        if world_key != self.world_key || tick < self.tick || tick > self.tick + 1 {
            return self.reset(world_key.into(), tick);
        }
        if tick == self.tick && live_births(previous, tick) != live_births(current, tick) {
            return self.reset(world_key.into(), tick);
        }
        let incoming: Vec<_> =
            current.iter().filter(|birth| birth.birth_tick == tick).cloned().collect();
        self.record_tick(tick, &incoming)?;
        for birth in previous {
            // Source host removal membership is world+ID, independent of recipe.
            if !current.iter().any(|live| live.world_key == birth.world_key && live.id == birth.id)
            {
                self.retire_birth(birth, tick)?;
            }
        }
        Ok(())
    }
    fn require_committed_tick(&self, tick: u64) -> Result<(), String> {
        require_tick(tick)?;
        if tick != self.tick {
            return Err("Flame retirement must use the admitted world tick".into());
        }
        Ok(())
    }
    fn enforce_budget(&mut self) {
        while self.births.len() > self.maximum_births
            || self.maximum_bytes.is_some_and(|limit| self.retained_bytes > limit)
        {
            let oldest_tick = self
                .births
                .iter()
                .map(|item| item.birth.birth_tick)
                .min()
                .expect("over-budget history is nonempty");
            self.births.retain(|item| {
                if item.birth.birth_tick == oldest_tick {
                    self.covered_after_tick = self.covered_after_tick.max(item.changed_tick);
                    self.retained_bytes -= item.bytes;
                    false
                } else {
                    true
                }
            });
            self.covered_after_tick = self.covered_after_tick.max(oldest_tick);
        }
    }
}

pub type FlamePresentationRetainedEnd = fn(&FlamePresentationBirth) -> u64;
#[derive(Clone, Debug)]
pub struct FlamePresentationReplay {
    epoch: u64,
    world_key: String,
    through_tick: u64,
    retained_floor: u64,
    maximum_births: usize,
    retained_end_tick: Option<FlamePresentationRetainedEnd>,
    births: Vec<FlamePresentationBirth>,
}
impl FlamePresentationReplay {
    #[must_use]
    pub const fn new(
        maximum_births: usize,
        retained_end_tick: Option<FlamePresentationRetainedEnd>,
    ) -> Self {
        Self {
            epoch: 0,
            world_key: String::new(),
            through_tick: 0,
            retained_floor: 0,
            maximum_births,
            retained_end_tick,
            births: Vec::new(),
        }
    }
    #[must_use]
    pub const fn through_tick(&self) -> u64 {
        self.through_tick
    }
    #[must_use]
    pub fn retained(&self, birth: &FlamePresentationBirth) -> Option<&FlamePresentationBirth> {
        self.births.iter().find(|prior| prior.same_identity(birth))
    }
    pub fn accept(&mut self, frame: &FlamePresentationFrame) -> Result<(), String> {
        require_tick(frame.through_tick())?;
        require_tick(frame.epoch())?;
        if frame.epoch() == 0 {
            return Err("Invalid Flame history epoch".into());
        }
        if frame.epoch() < self.epoch {
            return Ok(());
        }
        let FlamePresentationFrame::Continuous {
            epoch,
            world_key,
            through_tick,
            after_tick,
            births,
        } = frame
        else {
            if frame.epoch() == self.epoch && frame.through_tick() < self.through_tick {
                return Ok(());
            }
            self.epoch = frame.epoch();
            self.world_key = frame.world_key().into();
            self.through_tick = frame.through_tick();
            self.retained_floor = self.through_tick;
            self.births.clear();
            return Ok(());
        };
        if *epoch != self.epoch || *world_key != self.world_key {
            return Err("Flame history needs the existing snapshot recovery keyframe".into());
        }
        require_tick(*after_tick)?;
        if *after_tick > self.through_tick || *after_tick > *through_tick {
            return Err("Flame history has a snapshot coverage gap".into());
        }
        if births.len() > self.maximum_births {
            return Err("Flame replay exceeds its birth budget".into());
        }
        let mut prepared: Vec<FlamePresentationBirth> = Vec::new();
        for birth in births {
            birth.validate(world_key)?;
            if birth.birth_tick > *through_tick {
                return Err("Flame history contains a future birth".into());
            }
            if self.retained(birth).is_some_and(|prior| !prior.same_recipe(birth))
                || prepared
                    .iter()
                    .any(|prior| prior.same_identity(birth) && !prior.same_recipe(birth))
            {
                return Err("Flame birth recipe changed".into());
            }
            prepared.push(birth.clone());
        }
        let mut new_births: Vec<&FlamePresentationBirth> = Vec::new();
        for birth in &prepared {
            if self.retained(birth).is_none()
                && self.retained_until(birth) > self.retained_floor
                && !new_births.iter().any(|prior| prior.same_identity(birth))
            {
                new_births.push(birth);
            }
        }
        if self.births.len() + new_births.len() > self.maximum_births {
            return Err("Flame replay needs snapshot recovery after overflow".into());
        }
        for mut birth in prepared {
            if self.retained_until(&birth) <= self.retained_floor {
                continue;
            }
            if let Some(prior) = self.births.iter_mut().find(|prior| prior.same_identity(&birth)) {
                birth.end_tick_exclusive = birth.end_tick_exclusive.min(prior.end_tick_exclusive);
                *prior = birth;
            } else {
                self.births.push(birth);
            }
        }
        self.through_tick = self.through_tick.max(*through_tick);
        Ok(())
    }
    pub fn sample(&self, target_tick: f64) -> Result<Vec<&FlamePresentationBirth>, String> {
        if !target_tick.is_finite() {
            return Err("Invalid Flame presentation tick".into());
        }
        let tick = target_tick.min(self.through_tick as f64).floor();
        if tick < self.retained_floor as f64 {
            return Ok(Vec::new());
        }
        Ok(self
            .births
            .iter()
            .filter(|birth| {
                birth.birth_tick as f64 <= tick && tick < (birth.end_tick_exclusive as f64)
            })
            .collect())
    }
    #[must_use]
    pub fn history_at(&self, target_tick: f64) -> Vec<&FlamePresentationBirth> {
        let tick = target_tick.min(self.through_tick as f64).floor();
        if tick < self.retained_floor as f64 {
            return Vec::new();
        }
        self.births
            .iter()
            .filter(|birth| {
                birth.birth_tick as f64 <= tick && tick < (self.retained_until(birth) as f64)
            })
            .collect()
    }
    pub fn discard_before_tick(&mut self, oldest_tick: u64) -> Result<(), String> {
        require_tick(oldest_tick)?;
        self.retained_floor = self.retained_floor.max(oldest_tick);
        let retained_end_tick = self.retained_end_tick;
        self.births.retain(|birth| {
            birth.maximum_native_end_tick().max(retained_end_tick.map_or(0, |end| end(birth)))
                > oldest_tick
        });
        Ok(())
    }
    fn retained_until(&self, birth: &FlamePresentationBirth) -> u64 {
        birth.maximum_native_end_tick().max(self.retained_end_tick.map_or(0, |end| end(birth)))
    }
}

fn live_births(births: &[FlamePresentationBirth], tick: u64) -> Vec<&FlamePresentationBirth> {
    let mut live: Vec<_> = births
        .iter()
        .filter(|birth| birth.birth_tick <= tick && birth.end_tick_exclusive > tick)
        .collect();
    live.sort_by_key(|birth| birth.id);
    live
}
