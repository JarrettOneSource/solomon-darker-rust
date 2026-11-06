//! The two recovered Social `ease` transitions, with interrupted reversal timing.
//! CSS Transitions1 §3 supplies reversal shortening; no styles or DOM are interpreted.

pub(super) struct EaseTransition {
    from: f64,
    target: f64,
    started: f64,
    duration: f64,
    full_duration: f64,
    reversing_start: f64,
    shortening: f64,
}
impl EaseTransition {
    pub(super) fn new(value: f64, duration_ms: f64) -> Self {
        Self {
            from: value,
            target: value,
            started: 0.0,
            duration: 0.0,
            full_duration: duration_ms,
            reversing_start: value,
            shortening: 1.0,
        }
    }
    fn progress(&self, now: f64) -> f64 {
        if self.duration == 0.0 || now >= self.started + self.duration {
            return 1.0;
        }
        if now <= self.started {
            return 0.0;
        }
        f64::from(crate::gpu_ui::css_bezier(
            ((now - self.started) / self.duration) as f32,
            0.25,
            0.1,
            0.25,
            1.0,
        ))
    }
    pub(super) fn value(&self, now: f64) -> f64 {
        let progress = self.progress(now);
        if progress == 0.0 {
            self.from
        } else if progress == 1.0 {
            self.target
        } else {
            self.from + (self.target - self.from) * progress
        }
    }
    pub(super) fn target(&self) -> f64 {
        self.target
    }
    pub(super) fn snap(&mut self, value: f64) {
        *self = Self::new(value, self.full_duration);
    }
    pub(super) fn set(&mut self, target: f64, now: f64) {
        if target == self.target {
            return;
        }
        let current = self.value(now);
        if current == target {
            self.snap(target);
            return;
        }
        let reversing = now < self.started + self.duration && target == self.reversing_start;
        let shortening = if reversing {
            (self.progress(now) * self.shortening + 1.0 - self.shortening).abs().clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.reversing_start = if reversing { self.target } else { current };
        self.shortening = shortening;
        self.from = current;
        self.target = target;
        self.started = now;
        self.duration = self.full_duration * shortening;
    }
}

#[cfg(test)]
mod tests {
    use super::EaseTransition;
    #[test]
    fn new_values_begin_settled_and_retarget_from_current_presentation() {
        let mut value = EaseTransition::new(0.8, 220.0);
        assert_eq!(value.value(0.0), 0.8);
        value.set(0.2, 100.0);
        let interrupted = value.value(210.0);
        assert!((0.31..0.33).contains(&interrupted));
        value.set(0.5, 210.0);
        assert_eq!(value.value(210.0), interrupted);
        assert_eq!(value.value(430.0), 0.5);
    }
    #[test]
    fn interrupted_reverse_reaches_its_target_before_another_full_duration() {
        let mut value = EaseTransition::new(1.0, 650.0);
        value.set(0.0, 0.0);
        let interrupted = value.value(325.0);
        assert!((0.19..0.21).contains(&interrupted));
        value.set(1.0, 325.0);
        assert_eq!(value.value(325.0), interrupted);
        assert!(value.value(800.0) < 1.0);
        assert_eq!(value.value(850.0), 1.0);
    }
    #[test]
    fn unchanged_targets_do_not_restart_and_removed_motion_snaps() {
        let mut value = EaseTransition::new(0.0, 650.0);
        value.set(1.0, 100.0);
        value.set(1.0, 500.0);
        assert_eq!(value.value(750.0), 1.0);
        value.set(0.0, 800.0);
        value.snap(0.0);
        assert_eq!(value.value(800.0), 0.0);
    }
}
