//! Born no-item Cast1 action (`0x0044B400`) and its retained terminal frame.

use crate::profile::primary::PrimarySkillProfile;
use crate::{Rng, float::fround};

/// Admission `0x0052DA80` modifies the constructed rate, before the live
/// Puppet time scale is sampled by the action on each later update.
#[must_use]
pub fn hand_cast1_rate(profile: &PrimarySkillProfile, cast_factor: f64) -> f64 {
    let rate = fround(0.095_f32 as f64 * fround(cast_factor));
    match profile {
        PrimarySkillProfile::Ether(_) => rate,
        PrimarySkillProfile::Fire(_) => fround(rate * 0.75),
        PrimarySkillProfile::Weld(profile) => match profile.build_id {
            1000 => fround(rate * 0.87_f32 as f64),
            1001 => rate,
            1002 => fround(rate * 1.3_f32 as f64),
            1009 => fround(rate * profile.vector.values[5]),
            _ => panic!("only one-shot Welds construct HandCast1"),
        },
        _ => panic!("channel primaries construct Constant instead of HandCast1"),
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(try_from = "saved::HandCast1Document"))]
pub struct HandCast1 {
    progress: f64,
    rate: f64,
    first_pose: usize,
    pose: usize,
    done: bool,
}

impl HandCast1 {
    pub fn new(rate: f64, pose: usize, rng: &mut Rng) -> Self {
        assert!(rate.is_finite() && rate >= 0.0);
        assert!(pose < 4);
        Self {
            progress: 0.0,
            rate,
            first_pose: if rng.draw_integer(4) == 1 { 3 } else { 0 },
            pose,
            done: false,
        }
    }

    pub fn step(&mut self, progress_factor: f64) {
        assert!(!self.done, "the action manager removes a done HandCast1 before updating it");
        self.progress = fround(self.progress + fround(progress_factor) * self.rate);
        // The subclass still writes K and delivers the marker on this terminal tick.
        self.done = self.progress >= 4.0;
        self.pose = match self.progress.trunc() as usize {
            0 => self.first_pose,
            1 => 1,
            2..=4 => 2,
            _ => 0,
        };
    }

    #[must_use]
    pub fn progress(&self) -> f64 {
        self.progress
    }

    #[must_use]
    pub fn done(&self) -> bool {
        self.done
    }

    #[must_use]
    pub fn pose(&self) -> usize {
        self.pose
    }
}

#[cfg(feature = "persistence")]
mod saved {
    use super::HandCast1;

    #[derive(serde::Deserialize)]
    pub(super) struct HandCast1Document {
        progress: f64,
        rate: f64,
        first_pose: usize,
        pose: usize,
        done: bool,
    }

    impl TryFrom<HandCast1Document> for HandCast1 {
        type Error = &'static str;

        fn try_from(value: HandCast1Document) -> Result<Self, Self::Error> {
            if !value.progress.is_finite()
                || value.progress < 0.0
                || !value.rate.is_finite()
                || value.rate < 0.0
                || value.done != (value.progress >= 4.0)
                || value.pose >= 4
                || !matches!(value.first_pose, 0 | 3)
            {
                return Err("HandCast1 progress, rate or terminal state is invalid");
            }
            Ok(Self {
                progress: value.progress,
                rate: value.rate,
                first_pose: value.first_pose,
                pose: value.pose,
                done: value.done,
            })
        }
    }
}
