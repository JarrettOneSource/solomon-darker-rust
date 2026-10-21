//! Arena shake authority from `core-kernels/native-enemy-world-feedback.ts`.
//!
//! Impulses use the preceding accumulator, then increase that accumulator. Projectile and
//! terminal callbacks must therefore preserve their observed order within the world tick.

use crate::enemies::model::EnemyTerminalOutput;
use crate::float::fround;

pub const NATIVE_EXPLOSION_FEEDBACK_INTENSITY: f64 = 4.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeEnemyWorldFeedback {
    pub accumulator: f64,
    pub magnitude: f64,
}

impl NativeEnemyWorldFeedback {
    pub fn step(&mut self, elapsed_ticks: u64) {
        if elapsed_ticks == 0 {
            return;
        }
        self.accumulator = fround((self.accumulator - 0.0025 * elapsed_ticks as f64).max(0.1));
        let retained = fround(self.magnitude * 0.94_f64.powf(elapsed_ticks as f64));
        self.magnitude = if retained < 0.001 { 0.0 } else { retained };
    }

    pub fn apply(&mut self, intensity: f64) {
        assert!(
            intensity.is_finite() && intensity >= 0.0,
            "enemy world-feedback intensity must be nonnegative and finite"
        );
        let prior_accumulator = self.accumulator;
        self.accumulator = fround((prior_accumulator + 0.2_f32 as f64).min(3.5));
        self.magnitude = fround(prior_accumulator.min(1.0) * intensity);
    }

    pub fn apply_terminal(&mut self, output: EnemyTerminalOutput, output_count: Option<usize>) {
        for &intensity in native_enemy_world_feedback_impulses(output, output_count) {
            self.apply(intensity);
        }
    }
}

#[must_use]
pub fn native_enemy_world_feedback_impulses(
    output: EnemyTerminalOutput,
    output_count: Option<usize>,
) -> &'static [f64] {
    match output {
        EnemyTerminalOutput::SpiderCollapse | EnemyTerminalOutput::DiscorporealBanish => &[],
        EnemyTerminalOutput::FacultyBreak => &[0.2_f32 as f64],
        EnemyTerminalOutput::ArcherShatter
        | EnemyTerminalOutput::MageShatter
        | EnemyTerminalOutput::SkeletonShatter
        | EnemyTerminalOutput::ZombieCollapse => &[0.1],
        EnemyTerminalOutput::ImpSplit => {
            if output_count.is_some_and(|count| count > 0) {
                &[0.05]
            } else {
                &[0.1]
            }
        }
        EnemyTerminalOutput::WraithFragments => &[0.1, 0.1],
        EnemyTerminalOutput::HeartmongerShatter
        | EnemyTerminalOutput::CoffinBreak
        | EnemyTerminalOutput::DemonSplit
        | EnemyTerminalOutput::PortalBreak => &[0.2],
    }
}
