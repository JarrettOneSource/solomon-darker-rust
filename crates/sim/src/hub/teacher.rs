//! The teacher's cast cycle and the spell burst it releases.
//!
//! The teacher casts for 268 ticks, releases a burst that fades over the next 79, and idles
//! until the 847-tick cycle wraps. The burst's members decay the way the native actors did,
//! one single-precision subtraction per tick. Reference: `Website/frontend/src/game/hub-teacher.ts`.

use super::painter::{PainterLane, PainterLayer};
use crate::collision::Vec2;
use crate::float::fround;
use crate::painter::PainterRegistration;
use crate::random_domain::{native_random_float_from_word, native_random_int_from_word};

pub const HUB_TEACHER_TICKS_PER_SECOND: u64 = 100;
pub const HUB_TEACHER_CAST_TICKS: u64 = 268;
pub const HUB_TEACHER_IDLE_START_TICK: u64 = 347;
pub const HUB_TEACHER_CYCLE_TICKS: u64 = 847;
/// `HubWorldScene` constructs the College teacher at this fixed actor root.
pub const HUB_TEACHER_POSITION: Vec2 = Vec2::new(576.5, 710.5);
pub const HUB_TEACHER_PAINTER_ORDINAL: usize = 5;
/// `Teacher::Cast` spawns its children at actor + (-38, 40 - 25).
pub const HUB_TEACHER_CAST_ORIGIN: Vec2 = Vec2::new(-38.0, 15.0);
/// The auxiliary painter `0x00505480` centres College[13] here, then restores opaque colour
/// before drawing the ground shadow.
pub const HUB_TEACHER_RUNE_CENTER: Vec2 = Vec2::new(-40.0, 30.0);
pub const HUB_TEACHER_RUNE_ALPHA: f64 = 0.25;
/// The burst's frame strip has eleven frames.
pub const HUB_TEACHER_BURST_FRAME_COUNT: u32 = 11;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubTeacherPhase {
    Cast,
    Release,
    Idle,
}

/// Which part of the cycle `absolute_tick` falls in.
#[must_use]
pub fn hub_teacher_phase_at(absolute_tick: u64) -> HubTeacherPhase {
    let tick = absolute_tick % HUB_TEACHER_CYCLE_TICKS;
    if tick < HUB_TEACHER_CAST_TICKS {
        HubTeacherPhase::Cast
    } else if tick < HUB_TEACHER_IDLE_START_TICK {
        HubTeacherPhase::Release
    } else {
        HubTeacherPhase::Idle
    }
}

/// The teacher's own frame: a per-tick coin flip between the two cast frames, then the
/// release and idle frames.
#[must_use]
pub fn hub_teacher_frame_at(absolute_tick: u64, seed: u32) -> u32 {
    let tick = absolute_tick % HUB_TEACHER_CYCLE_TICKS;
    if tick == 0 {
        0
    } else if tick < HUB_TEACHER_CAST_TICKS {
        native_random_int_from_word(teacher_word(seed, absolute_tick, 0), 2)
    } else if tick < HUB_TEACHER_IDLE_START_TICK {
        2
    } else {
        3
    }
}

/// One drawn part of the burst.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubTeacherBurstMember {
    pub alpha: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub visible: bool,
}

impl HubTeacherBurstMember {
    const HIDDEN: Self = Self { alpha: 0.0, scale_x: 1.0, scale_y: 1.0, visible: false };

    fn decaying(raw_alpha: f64, scale_x: f64, scale_y: f64) -> Self {
        Self { alpha: raw_alpha.clamp(0.0, 1.0), scale_x, scale_y, visible: raw_alpha > 0.0 }
    }
}

/// The burst's frame strip: a member plus the frame it shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubTeacherBurstFrames {
    pub member: HubTeacherBurstMember,
    pub frame: u32,
}

/// Everything the burst draws at one tick (`hubTeacherBurstAt`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubTeacherBurst {
    /// Ticks since this release began; `None` while the teacher is still casting.
    pub age_ticks: Option<u64>,
    pub column: HubTeacherBurstMember,
    pub core: HubTeacherBurstMember,
    pub flare: HubTeacherBurstMember,
    pub frames: HubTeacherBurstFrames,
    /// Which cycle's release this is, counting from the world's first tick.
    pub release_index: u64,
    pub visible: bool,
}

/// The `HubTeacherView` presentation at one world tick. The rune and body use the teacher's
/// actor painter, the flare uses the pre-world band, and the core uses the post-world band.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubTeacherDrawPlan {
    pub actor_frame: u32,
    pub release_origin: Vec2,
    pub burst: HubTeacherBurst,
    pub world_painters: Option<[PainterLayer; 2]>,
}

/// The ambient owner retains these registrations for one release, exactly as
/// `HubTeacherWorldReleaseState` in `core-server/hub-ambient.ts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubTeacherWorldRelease {
    pub painter_registrations: [PainterRegistration; 2],
    pub release_index: u64,
}

#[must_use]
pub fn hub_teacher_draw_plan(
    absolute_tick: u64,
    seed: u32,
    world_release: Option<HubTeacherWorldRelease>,
) -> HubTeacherDrawPlan {
    let burst = hub_teacher_burst_at(absolute_tick, seed);
    let world_painters = match world_release {
        Some(release) => Some(release.painter_registrations.map(|registration| {
            registration.layer(HUB_TEACHER_POSITION.y + HUB_TEACHER_CAST_ORIGIN.y, 0.0)
        })),
        None => hub_teacher_world_painters(&burst),
    };
    HubTeacherDrawPlan {
        actor_frame: hub_teacher_frame_at(absolute_tick, seed),
        release_origin: Vec2::new(
            HUB_TEACHER_POSITION.x + HUB_TEACHER_CAST_ORIGIN.x,
            HUB_TEACHER_POSITION.y + HUB_TEACHER_CAST_ORIGIN.y,
        ),
        burst,
        world_painters,
    }
}

/// `stepHubAmbient` reserves both transient registrations when a release starts and retains
/// them until both world members are hidden. With no manager attached, ordinals are derived
/// from release order; keeping them derived avoids a second lifetime to synchronize.
#[must_use]
pub fn hub_teacher_world_painters(burst: &HubTeacherBurst) -> Option<[PainterLayer; 2]> {
    if !burst.needs_world_painters() {
        return None;
    }
    Some([0, 1].map(|index| PainterLayer {
        visible: true,
        is_insertion: false,
        manager_lane: PainterLane::Transient,
        registration_ordinal: burst.release_index as usize * 2 + index,
        world_y: HUB_TEACHER_POSITION.y + HUB_TEACHER_CAST_ORIGIN.y,
        sort_bias: 0.0,
    }))
}

/// The burst at `absolute_tick`.
///
/// The reference takes elapsed seconds and recovers the tick with a floor and a small
/// epsilon; the port takes the tick itself, which is what the ambient clock holds.
#[must_use]
pub fn hub_teacher_burst_at(absolute_tick: u64, seed: u32) -> HubTeacherBurst {
    let cycle_tick = absolute_tick % HUB_TEACHER_CYCLE_TICKS;
    let release_index = absolute_tick / HUB_TEACHER_CYCLE_TICKS;
    if cycle_tick < HUB_TEACHER_CAST_TICKS {
        return HubTeacherBurst {
            age_ticks: None,
            column: HubTeacherBurstMember::HIDDEN,
            core: HubTeacherBurstMember::HIDDEN,
            flare: HubTeacherBurstMember::HIDDEN,
            frames: HubTeacherBurstFrames { member: HubTeacherBurstMember::HIDDEN, frame: 0 },
            release_index,
            visible: false,
        };
    }

    let age_ticks = cycle_tick - HUB_TEACHER_CAST_TICKS;
    let release_word = |channel: u32| teacher_word(seed, release_index, channel);
    let flare_scale = fround(1.0 + native_random_float_from_word(release_word(1), 0.1));
    let frame_scale = fround(2.0 - native_random_float_from_word(release_word(2), 0.5));
    let frame_factor = fround(1.0 + native_random_float_from_word(release_word(3), 0.2));
    let frame_step = fround(0.75 * frame_factor);
    let frame_alpha_loss = fround(0.02 * frame_factor);
    let frame = fround(frame_step * age_ticks as f64).trunc();
    let frame_alpha = native_decay(1.0, frame_alpha_loss, age_ticks);
    let mirrored = native_random_int_from_word(release_word(4), 2) == 1;

    let core = HubTeacherBurstMember::decaying(native_decay(1.0, 0.1, age_ticks), 6.0, 4.0);
    let flare = HubTeacherBurstMember::decaying(
        native_decay(1.0, 0.0075, age_ticks),
        flare_scale,
        flare_scale,
    );
    let column = HubTeacherBurstMember::decaying(native_decay(2.0, 0.04, age_ticks), 1.0, 1.0);
    let frames = HubTeacherBurstFrames {
        member: HubTeacherBurstMember {
            alpha: frame_alpha.clamp(0.0, 1.0),
            scale_x: if mirrored { -frame_scale } else { frame_scale },
            scale_y: frame_scale,
            visible: frame >= 0.0 && frame < f64::from(HUB_TEACHER_BURST_FRAME_COUNT),
        },
        frame: frame.clamp(0.0, f64::from(HUB_TEACHER_BURST_FRAME_COUNT - 1)) as u32,
    };
    let visible = core.visible || flare.visible || column.visible || frames.member.visible;
    HubTeacherBurst {
        age_ticks: Some(age_ticks),
        column,
        core,
        flare,
        frames,
        release_index,
        visible,
    }
}

impl HubTeacherBurst {
    /// Whether the burst still needs its two world painters. The column and the frame strip
    /// sort among the actors (`stepHubAmbient` registers them in the transient lane while
    /// either is visible). The flare belongs to the pre-world band and the core to the
    /// post-world band (`renderer/hub-world-scene.ts`, `HubTeacherView`).
    #[must_use]
    pub fn needs_world_painters(&self) -> bool {
        self.column.visible || self.frames.member.visible
    }
}

/// Subtracts `loss` from `initial` once per tick in single precision, stopping at zero.
fn native_decay(initial: f64, loss: f64, age_ticks: u64) -> f64 {
    let native_loss = fround(loss);
    let mut value = fround(initial);
    for _ in 0..age_ticks {
        if value <= 0.0 {
            break;
        }
        value = fround(value - native_loss);
    }
    value
}

/// A stable 32-bit word for one (seed, tick, channel) triple: the reference's integer hash.
fn teacher_word(seed: u32, tick: u64, channel: u32) -> u32 {
    let tick = tick as u32;
    let mut value = seed
        ^ (tick.wrapping_add(1)).wrapping_mul(0x9e37_79b1)
        ^ (channel.wrapping_add(1)).wrapping_mul(0x85eb_ca6b);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cycle_casts_then_releases_then_idles() {
        assert_eq!(hub_teacher_phase_at(0), HubTeacherPhase::Cast);
        assert_eq!(hub_teacher_phase_at(267), HubTeacherPhase::Cast);
        assert_eq!(hub_teacher_phase_at(268), HubTeacherPhase::Release);
        assert_eq!(hub_teacher_phase_at(346), HubTeacherPhase::Release);
        assert_eq!(hub_teacher_phase_at(347), HubTeacherPhase::Idle);
        assert_eq!(hub_teacher_phase_at(847), HubTeacherPhase::Cast);
    }

    #[test]
    fn the_burst_is_hidden_while_casting_and_fades_after_release() {
        let hidden = hub_teacher_burst_at(100, 0);
        assert_eq!(hidden.age_ticks, None);
        assert!(!hidden.visible);
        assert_eq!(hidden.release_index, 0);

        let fresh = hub_teacher_burst_at(HUB_TEACHER_CAST_TICKS, 0);
        assert_eq!(fresh.age_ticks, Some(0));
        assert!(fresh.visible);
        assert_eq!(fresh.core.alpha, 1.0);
        assert_eq!(fresh.column.alpha, 1.0, "the column starts at 2 and is clamped to 1");
        assert_eq!(fresh.frames.frame, 0);

        let late = hub_teacher_burst_at(HUB_TEACHER_CAST_TICKS + 60, 0);
        assert!(!late.core.visible, "the core loses a tenth per tick");
        assert!(late.flare.visible, "the flare loses 0.0075 per tick");
        assert!(late.frames.frame < HUB_TEACHER_BURST_FRAME_COUNT);

        let second = hub_teacher_burst_at(HUB_TEACHER_CYCLE_TICKS + HUB_TEACHER_CAST_TICKS, 0);
        assert_eq!(second.release_index, 1);
    }

    #[test]
    fn decay_stops_at_zero_in_single_precision() {
        assert_eq!(native_decay(1.0, 0.1, 0), 1.0);
        assert_eq!(native_decay(1.0, 0.1, 1), fround(1.0 - fround(0.1)));
        assert!(native_decay(1.0, 0.1, 20) <= 0.0);
        assert_eq!(native_decay(1.0, 0.1, 20), native_decay(1.0, 0.1, 200));
    }
}
