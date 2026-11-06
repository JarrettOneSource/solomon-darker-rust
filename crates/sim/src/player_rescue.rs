//! Accepted protection and retained Sparkles from `native-player-rescue.ts` and
//! `native-sparkle.ts`. Births use the shared secondary RNG and world painter owner.

use crate::float::{fround, sin};
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::{Rng, Vec2};

pub const SPARKLE_TIMER: f64 = 180.0;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SparkleState {
    pub alpha: f64,
    pub decay: f64,
    pub id: u64,
    pub position: Vec2,
    pub painter_registration: Option<PainterRegistration>,
    pub rotation_degrees: f64,
    pub timer: f64,
    pub world_key: String,
}

impl SparkleState {
    /// Returns false at the same float store that removes the native animation.
    pub fn step(&mut self) -> bool {
        let timer = fround(self.timer - self.decay);
        if timer <= 0.0 {
            return false;
        }
        self.position.y = fround(self.position.y - (0.1_f32 as f64));
        self.timer = timer;
        true
    }
}

#[must_use]
pub fn sparkle_scale(timer: f64) -> f64 {
    sin(timer * std::f64::consts::PI / 180.0)
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerRescueProtection {
    pub fraction: f64,
    pub next_particle_id: u64,
    pub particles: Vec<SparkleState>,
}

impl Default for PlayerRescueProtection {
    fn default() -> Self {
        Self { fraction: 0.0, next_particle_id: 1, particles: Vec::new() }
    }
}

impl PlayerRescueProtection {
    pub fn step(
        &mut self,
        position: Vec2,
        world_key: &str,
        rng: &mut Rng,
        order: &mut WorldPainterOrder,
    ) {
        if self.fraction <= 0.0 && self.particles.is_empty() {
            return;
        }
        self.particles.retain_mut(SparkleState::step);
        if self.fraction <= 0.0 {
            return;
        }
        // Retail subtracts this qword before the float store. The final zero
        // fraction still births a Sparkle and consumes all four random words.
        self.fraction = fround(self.fraction - 0.0024999999441206455).max(0.0);
        let radius = rng.draw_scaled_float(60.0, false);
        let vector = rng.draw_unit_vector();
        let angle = rng.draw_scaled_float(360.0, false);
        let decay = rng.draw_scaled_float(2.0, false);
        self.particles.push(SparkleState {
            alpha: self.fraction,
            decay: fround(3.0 + decay),
            id: self.next_particle_id,
            position: Vec2::new(
                fround(position.x + fround(radius * vector.x)),
                fround(fround(position.y - 35.0) + fround(radius * vector.y)),
            ),
            painter_registration: Some(order.register(WorldManagerLane::Transient)),
            rotation_degrees: angle,
            timer: SPARKLE_TIMER,
            world_key: world_key.into(),
        });
        self.next_particle_id += 1;
    }
}

#[must_use]
pub fn rescue_damage(amount: f64, protection: &PlayerRescueProtection) -> f64 {
    if protection.fraction > 0.0 { fround(amount * (1.0 - protection.fraction)) } else { amount }
}
