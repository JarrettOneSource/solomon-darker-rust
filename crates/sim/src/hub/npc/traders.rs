//! Native trader clocks from Website `hub-presentation.ts` and `core-server/hub-skorcha.ts`.

use crate::float;
use crate::float::fround;
use crate::{NativeRng, Vec2};

pub const DEFAULT_HUB_TRADER_ANIMATION_SEED: u32 = 0x5eed_c0de;
const HAGATHA_ANCHORS: [Vec2; 8] = [
    Vec2::new(79.5, 80.5),
    Vec2::new(81.5, 81.5),
    Vec2::new(83.5, 82.5),
    Vec2::new(86.5, 83.5),
    Vec2::new(90.5, 83.5),
    Vec2::new(103.5, 77.5),
    Vec2::new(104.5, 75.5),
    Vec2::new(84.5, 84.5),
];
/// Authored `NATIVE_HUB_NPC_CATALOG.skorcha.placements`, also checked on save admission.
pub const HUB_SKORCHA_PLACEMENTS: [Vec2; 3] =
    [Vec2::new(1437.5, 732.5), Vec2::new(1637.0, 403.5), Vec2::new(669.0, 705.5)];

/// Luthacus, Shlorio and PotionGuy share the stock four-frame stochastic idle gesture.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubCommonTraderClock {
    pub active: bool,
    pub phase_degrees: f64,
    pub speed_degrees: f64,
    pub rng: NativeRng,
}

impl HubCommonTraderClock {
    #[must_use]
    pub fn new(seed: u32) -> Self {
        Self {
            active: false,
            phase_degrees: 0.0,
            speed_degrees: 0.0,
            rng: NativeRng::new(i64::from(seed)),
        }
    }

    pub fn step(&mut self) {
        if !self.active {
            if self.rng.draw_integer(200) != 2 {
                return;
            }
            self.active = true;
            self.phase_degrees = 0.0;
            self.speed_degrees =
                fround((self.rng.draw_scaled_float(3.0, false) + 1.0) * (0.45_f32 as f64));
            return;
        }
        self.phase_degrees = fround(self.phase_degrees + self.speed_degrees);
        if self.phase_degrees >= 180.0 {
            self.active = false;
            self.phase_degrees = 0.0;
            self.speed_degrees = 0.0;
        }
    }

    #[must_use]
    pub fn frame(&self) -> usize {
        if !self.active {
            return 0;
        }
        ((4.0 - 0.01) * float::sin(self.phase_degrees * std::f64::consts::PI / 180.0)).trunc()
            as usize
    }
}

/// PotionGuy's native balloon bank, with endpoint holds and half-degree presentation drift.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubPotionTraderClock {
    pub actor: HubCommonTraderClock,
    pub balloon_phase: f64,
    pub direction: i32,
    pub hold_ticks: u64,
    pub tick: u64,
}

impl Default for HubPotionTraderClock {
    fn default() -> Self {
        Self::new()
    }
}

impl HubPotionTraderClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            actor: HubCommonTraderClock::new(0x50b110),
            balloon_phase: 0.0,
            direction: 1,
            hold_ticks: 0,
            tick: 0,
        }
    }

    pub fn step(&mut self) {
        self.actor.step();
        self.tick += 1;
        if self.hold_ticks > 0 {
            self.hold_ticks -= 1;
            return;
        }
        let step = 0.05_f32 as f64;
        self.balloon_phase = fround(self.balloon_phase + fround(f64::from(self.direction) * step));
        if self.balloon_phase >= 5.0 {
            self.direction = -1;
            self.hold_ticks = 100;
            self.balloon_phase = fround(self.balloon_phase - step - step);
        }
        if self.balloon_phase < 0.0 {
            self.direction = 1;
            self.hold_ticks = 100;
            self.balloon_phase = fround(step + self.balloon_phase + step);
        }
    }

    #[must_use]
    pub fn balloon_frame(&self) -> usize {
        self.balloon_phase.trunc().clamp(0.0, 4.0) as usize
    }

    #[must_use]
    pub fn balloon_offset_y(&self) -> f64 {
        float::sin(self.tick as f64 * 0.5 * std::f64::consts::PI / 180.0) * 2.0
    }
}

/// One native Hagatha crossfade sprite, with its independently sampled fade and lifetime.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubHagathaParticle {
    pub frame: usize,
    pub id: u64,
    pub offset: Vec2,
    pub opacity: f64,
    pub progress: f64,
    pub progress_step: f64,
    pub scale: f64,
}

impl HubHagathaParticle {
    #[must_use]
    pub fn alpha(&self) -> f64 {
        (float::cos(self.progress * std::f64::consts::PI) * self.opacity).clamp(0.0, 1.0)
    }
}

/// Hagatha's eight-frame loop and the native crossfade particle field.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubHagathaClock {
    pub next_particle_id: u64,
    pub particles: Vec<HubHagathaParticle>,
    pub phase: f64,
    pub rng: NativeRng,
    pub velocity: f64,
}

impl HubHagathaClock {
    #[must_use]
    pub fn new(seed: u32) -> Self {
        Self {
            next_particle_id: 0,
            particles: Vec::new(),
            phase: 0.0,
            rng: NativeRng::new(i64::from(seed)),
            velocity: 0.05_f32 as f64,
        }
    }

    pub fn step(&mut self) {
        for particle in &mut self.particles {
            particle.progress = fround(particle.progress + particle.progress_step);
        }
        self.particles.retain(|particle| particle.progress <= 1.0);
        let phase_draw = self.rng.draw_scaled_float(0.25, false);
        self.phase = fround(self.phase + fround((phase_draw + 1.0) * self.velocity));
        if self.phase >= 8.0 {
            self.phase = fround(self.phase - 8.0);
        }
        if self.phase < 0.0 {
            self.phase = fround(self.phase + 8.0);
        }
        let frame = self.rng.draw_integer(4) as usize;
        let opacity = self.rng.draw_scaled_float(1.0, false);
        let lifetime = self.rng.draw_scaled_float(0.25, false);
        let scale = self.rng.draw_scaled_float(0.1, true);
        let anchor = HAGATHA_ANCHORS[self.rng.draw_integer(8) as usize];
        let radius = self.rng.draw_scaled_float(2.0, false);
        let angle = self.rng.draw_scaled_float(360.0, false);
        let radians = fround(angle * fround(std::f64::consts::PI) / 180.0);
        self.particles.push(HubHagathaParticle {
            frame,
            id: self.next_particle_id,
            offset: Vec2::new(
                fround(anchor.x - 75.0 + fround(radius * float::cos(radians))),
                fround(anchor.y + 14.0 - 75.0 - fround(radius * float::sin(radians))),
            ),
            opacity,
            progress: 0.0,
            progress_step: fround(1.0 / (fround(lifetime + 1.25) * 100.0)),
            scale: fround(scale + 0.15),
        });
        self.next_particle_id += 1;
        if self.rng.draw_integer(1500) == 3 {
            self.velocity = fround(-self.velocity);
        }
    }

    #[must_use]
    pub fn frame(&self) -> usize {
        self.phase.floor() as usize % 8
    }
}

/// A present Skorcha, with native variant, gesture and independently pulsing hat.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubSkorcha {
    pub dismissal_index: usize,
    pub gesture: usize,
    pub gesture_ticks_remaining: u64,
    pub hat_active: bool,
    pub hat_phase_degrees: f64,
    pub hat_rate_degrees_per_tick: f64,
    pub position: Vec2,
    pub rng: NativeRng,
    pub variant: usize,
}

impl HubSkorcha {
    fn at_variant(mut rng: NativeRng, variant: usize) -> Self {
        let dismissal_index = rng.draw_integer(3) as usize;
        Self {
            dismissal_index,
            gesture: 0,
            gesture_ticks_remaining: 0,
            hat_active: false,
            hat_phase_degrees: 0.0,
            hat_rate_degrees_per_tick: 0.0,
            position: HUB_SKORCHA_PLACEMENTS[variant],
            rng,
            variant,
        }
    }

    fn step(&mut self) {
        if !self.hat_active {
            if self.rng.draw_integer(200) == 2 {
                let rate = self.rng.draw_scaled_float(3.0, false);
                self.hat_active = true;
                self.hat_phase_degrees = 0.0;
                self.hat_rate_degrees_per_tick = fround(fround(rate + 1.0) * fround(0.45));
            }
        } else {
            self.hat_phase_degrees =
                fround(self.hat_phase_degrees + self.hat_rate_degrees_per_tick);
            if self.hat_phase_degrees >= 180.0 {
                self.hat_active = false;
                self.hat_phase_degrees = 0.0;
            }
        }
        if self.gesture_ticks_remaining > 1 {
            self.gesture_ticks_remaining -= 1;
            return;
        }
        let delay = self.rng.draw_integer(10) as u64 + 20;
        loop {
            let gesture = self.rng.draw_integer(3) as usize;
            if gesture == self.gesture {
                continue;
            }
            self.gesture = gesture;
            break;
        }
        self.gesture_ticks_remaining = delay;
    }

    #[must_use]
    pub fn hat_frame(&self) -> usize {
        if !self.hat_active {
            return 0;
        }
        let radians = fround(self.hat_phase_degrees * fround(std::f64::consts::PI) / 180.0);
        let wave = fround(float::sin(radians));
        fround(wave * (4.0 - fround(0.01))).trunc() as usize
    }
}

/// Skorcha's independent appearance windows, each sampled between twenty and forty minutes.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubSkorchaSchedule {
    pub rng: NativeRng,
    pub skorcha: Option<HubSkorcha>,
    pub transition_ticks_remaining: u64,
    hidden_ticks: Option<u64>,
    visible_ticks: Option<u64>,
}

impl HubSkorchaSchedule {
    /// Fixed host/test windows survive the source's empty shared-College reset.
    #[must_use]
    pub const fn fixed_windows(&self) -> (Option<u64>, Option<u64>) {
        (self.hidden_ticks, self.visible_ticks)
    }

    #[must_use]
    pub fn new(seed: u32) -> Self {
        Self::with_windows(seed, None, None)
    }

    #[must_use]
    pub fn with_windows(seed: u32, hidden_ticks: Option<u64>, visible_ticks: Option<u64>) -> Self {
        assert!(
            hidden_ticks.is_none_or(|ticks| ticks > 0)
                && visible_ticks.is_none_or(|ticks| ticks > 0),
            "Skorcha windows must contain a tick"
        );
        let mut rng = NativeRng::new(i64::from(seed ^ 5007));
        let skorcha = if rng.draw_integer(3) == 1 {
            let variant = rng.draw_integer(3) as usize;
            let skorcha = HubSkorcha::at_variant(rng.clone(), variant);
            rng = skorcha.rng.clone();
            Some(skorcha)
        } else {
            None
        };
        let mut schedule =
            Self { rng, skorcha, transition_ticks_remaining: 0, hidden_ticks, visible_ticks };
        schedule.schedule_window();
        schedule
    }

    pub fn step(&mut self) {
        if self.transition_ticks_remaining > 1 {
            self.transition_ticks_remaining -= 1;
            if let Some(skorcha) = &mut self.skorcha {
                skorcha.step();
            }
            return;
        }
        if self.skorcha.is_some() {
            self.skorcha = None;
        } else {
            let variant = self.rng.draw_integer(3) as usize;
            let skorcha = HubSkorcha::at_variant(self.rng.clone(), variant);
            self.rng = skorcha.rng.clone();
            self.skorcha = Some(skorcha);
        }
        self.schedule_window();
    }

    fn schedule_window(&mut self) {
        let fixed = if self.skorcha.is_some() { self.visible_ticks } else { self.hidden_ticks };
        self.transition_ticks_remaining =
            fixed.unwrap_or_else(|| 120_000 + self.rng.draw_integer(120_001) as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn potion_balloons_hold_their_endpoints_for_one_second() {
        let mut clock = HubPotionTraderClock::new();
        for _ in 0..100 {
            clock.step();
        }
        assert_eq!(clock.balloon_frame(), 4);
        assert_eq!(clock.hold_ticks, 100);
        for _ in 0..100 {
            clock.step();
        }
        assert_eq!(clock.balloon_frame(), 4);
        assert_eq!(clock.hold_ticks, 0);
    }
}
