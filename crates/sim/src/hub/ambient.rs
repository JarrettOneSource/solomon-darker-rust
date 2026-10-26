//! The courtyard's ambient life: fountain spray, the seals' pulse, the statue's bob, and the
//! teacher's clock.
//!
//! The ambient state advances once per 10 ms tick from its own xorshift sampler, so it never
//! disturbs the player's collision sampler and the two replay independently. Reference:
//! `Website/frontend/src/game/core-server/hub-ambient.ts`; the colour helpers come from
//! `hub-presentation.ts`.

use crate::collision::Vec2;
use crate::float;
use crate::hub::teacher::{HubTeacherBurst, HubTeacherWorldRelease, hub_teacher_burst_at};
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};

/// Seed of the ambient sampler at world creation (`createHubAmbientState`).
pub const HUB_AMBIENT_RNG_SEED: u32 = 0x2f6e_2b1d;
/// World position the spray is drawn from (`HUB_FOUNTAIN_ORIGIN`).
pub const HUB_FOUNTAIN_ORIGIN: Vec2 = Vec2::new(957.0, 333.0);
/// Spray brighter than this is clamped (`FOUNTAIN_ALPHA_LIMIT`).
pub const FOUNTAIN_ALPHA_LIMIT: f64 = 0.25;

/// A zero xorshift state would stay zero forever; the reference substitutes this word.
const RNG_ZERO_ESCAPE: u32 = 0x6d2b_79f5;
const FOUNTAIN_ROLL_RANGE: u32 = 80;
const FOUNTAIN_SPAWN_ROLL: u32 = 3;
const FOUNTAIN_INITIAL_SCALE: f64 = 0.02;
const FOUNTAIN_LIFETIME_DECREMENT: f64 = 0.00625;
/// The native per-tick growth factor, carried as the single it was stored in.
const FOUNTAIN_SCALE_MULTIPLIER: f64 = 1.002_500_057_220_459;
const SEAL_TRACK_LENGTH: f64 = 3.0;
const STATUE_DEGREES_PER_TICK: f64 = 0.5;

/// One xorshift32 draw: the next state and a unit float in `[0, 1)`.
fn next_random(state: u32) -> (u32, f64) {
    let mut value = state;
    value ^= value << 13;
    value ^= value >> 17;
    value ^= value << 5;
    let next_state = if value == 0 { RNG_ZERO_ESCAPE } else { value };
    (next_state, f64::from(value) / 4_294_967_296.0)
}

fn random_unsigned(state: u32, maximum: f64) -> (u32, f64) {
    let (next_state, unit) = next_random(state);
    (next_state, unit * maximum)
}

fn random_integer(state: u32, maximum_exclusive: u32) -> (u32, u32) {
    let (next_state, unit) = next_random(state);
    let scaled = (unit * f64::from(maximum_exclusive)).floor() as u32;
    (next_state, scaled.min(maximum_exclusive - 1))
}

/// One drop of spray above the fountain.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[derive(bevy_ecs::component::Component)]
pub struct HubFountainParticle {
    pub id: u32,
    /// Seconds of life left; it is also the drop's brightness before the alpha limit.
    pub remaining: f64,
    pub scale: f64,
}

/// Everything in the courtyard that moves on its own.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubAmbientState {
    pub fountain_particles: crate::ComponentEntities<HubFountainParticle>,
    pub next_fountain_particle_id: u32,
    pub rng_state: u32,
    /// Position along the seal core's three-colour track, in `[0, 3)`.
    pub seal_core_phase: f64,
    /// Position along the seal glyphs' three-colour track, in `[0, 3)`.
    pub seal_glyph_phase: f64,
    pub statue_phase_degrees: f64,
    /// Ticks since the world was created; the teacher's cycle counts from here.
    pub teacher_tick: u64,
    pub teacher_world_release: Option<HubTeacherWorldRelease>,
}

impl Default for HubAmbientState {
    fn default() -> Self {
        Self::new()
    }
}

impl HubAmbientState {
    /// The state of a freshly created world.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fountain_particles: crate::ComponentEntities::default(),
            next_fountain_particle_id: 0,
            rng_state: HUB_AMBIENT_RNG_SEED,
            seal_core_phase: 0.0,
            seal_glyph_phase: 0.0,
            statue_phase_degrees: 0.0,
            teacher_tick: 0,
            teacher_world_release: None,
        }
    }

    /// Advances one tick (`stepHubAmbient`): ages and spawns spray, drifts both seal phases,
    /// bobs the statue, and moves the teacher's clock.
    ///
    /// The sampler is drawn in the reference's order: the spawn roll, the new drop's lifetime
    /// when the roll hits, then the two seal increments.
    pub fn step(&mut self, painter_order: Option<&mut WorldPainterOrder>) {
        let (state, roll) = random_integer(self.rng_state, FOUNTAIN_ROLL_RANGE);
        self.rng_state = state;
        for particle in self.fountain_particles.iter_mut() {
            particle.remaining -= FOUNTAIN_LIFETIME_DECREMENT;
            particle.scale *= FOUNTAIN_SCALE_MULTIPLIER;
        }
        self.fountain_particles.retain(|particle| particle.remaining > 0.0);
        if roll == FOUNTAIN_SPAWN_ROLL {
            let (state, lifetime) = random_unsigned(self.rng_state, 3.0);
            self.rng_state = state;
            self.fountain_particles.push(HubFountainParticle {
                id: self.next_fountain_particle_id,
                remaining: (lifetime + 6.0) * 0.25,
                scale: FOUNTAIN_INITIAL_SCALE,
            });
            self.next_fountain_particle_id += 1;
        }
        let (state, core_increment) = random_unsigned(self.rng_state, 0.15);
        let (state, glyph_increment) = random_unsigned(state, 0.019);
        self.rng_state = state;
        self.seal_core_phase =
            (self.seal_core_phase + (core_increment + 0.01) * 0.5) % SEAL_TRACK_LENGTH;
        self.seal_glyph_phase =
            (self.seal_glyph_phase + (glyph_increment + 0.001) * 0.5) % SEAL_TRACK_LENGTH;
        self.statue_phase_degrees = (self.statue_phase_degrees + STATUE_DEGREES_PER_TICK) % 360.0;
        self.teacher_tick += 1;
        let burst = self.teacher_burst();
        if !burst.needs_world_painters() {
            self.teacher_world_release = None;
            return;
        }
        if self
            .teacher_world_release
            .is_some_and(|release| release.release_index == burst.release_index)
        {
            return;
        }
        let painter_registrations = match painter_order {
            Some(order) => [
                order.register(WorldManagerLane::Transient),
                order.register(WorldManagerLane::Transient),
            ],
            None => [0, 1].map(|index| PainterRegistration {
                manager_lane: WorldManagerLane::Transient,
                registration_ordinal: burst.release_index as usize * 2 + index,
            }),
        };
        self.teacher_world_release = Some(HubTeacherWorldRelease {
            painter_registrations,
            release_index: burst.release_index,
        });
    }

    /// The teacher's burst at this tick.
    #[must_use]
    pub fn teacher_burst(&self) -> HubTeacherBurst {
        hub_teacher_burst_at(self.teacher_tick, 0)
    }
}

/// How bright one drop of spray draws (`hubFountainParticleAlpha`).
#[must_use]
pub fn hub_fountain_particle_alpha(particle: &HubFountainParticle) -> f64 {
    particle.remaining.min(FOUNTAIN_ALPHA_LIMIT)
}

/// An RGBA colour with unit channels (`HubColor`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubColor {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl HubColor {
    #[must_use]
    pub const fn new(red: f64, green: f64, blue: f64, alpha: f64) -> Self {
        Self { red, green, blue, alpha }
    }
}

const SEAL_CORE_TRACK: [HubColor; 3] = [
    HubColor::new(1.0, 1.0, 1.0, 1.0),
    HubColor::new(0.0, 1.0, 1.0, 1.0),
    HubColor::new(1.0, 1.0, 1.0, 1.0),
];
const SEAL_GLYPH_TRACK: [HubColor; 3] = [
    HubColor::new(0.5, 0.5, 1.0, 1.0),
    HubColor::new(0.75, 1.0, 1.0, 1.0),
    HubColor::new(1.0, 1.0, 1.0, 1.0),
];
/// The native luminance weights, as the singles stock stored them.
const LUMINANCE_WEIGHTS: [f64; 3] = [0.3086_f32 as f64, 0.6094_f32 as f64, 0.082_f32 as f64];

/// Blends linearly between the track's colours; the track wraps, so phase 2.5 is halfway
/// from the last colour back to the first.
fn interpolate_color(track: &[HubColor; 3], phase: f64) -> HubColor {
    let length = track.len() as f64;
    let wrapped = ((phase % length) + length) % length;
    let first = wrapped.floor() as usize;
    let second = (first + 1) % track.len();
    let blend = wrapped - first as f64;
    let mix = |from: f64, to: f64| from + (to - from) * blend;
    HubColor {
        red: mix(track[first].red, track[second].red),
        green: mix(track[first].green, track[second].green),
        blue: mix(track[first].blue, track[second].blue),
        alpha: mix(track[first].alpha, track[second].alpha),
    }
}

/// Pulls each channel toward the colour's luminance by `factor`.
fn saturate(color: HubColor, factor: f64) -> HubColor {
    let luminance = color.red * LUMINANCE_WEIGHTS[0]
        + color.green * LUMINANCE_WEIGHTS[1]
        + color.blue * LUMINANCE_WEIGHTS[2];
    let retained = 1.0 - factor;
    HubColor {
        red: luminance * factor + color.red * retained,
        green: luminance * factor + color.green * retained,
        blue: luminance * factor + color.blue * retained,
        alpha: color.alpha,
    }
}

/// The additive tints of the seal's two layers this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubSealColors {
    pub core: HubColor,
    pub glyphs: HubColor,
}

/// `hubSealColors`: the core cycles white to cyan; the glyphs cycle blue to white, half
/// desaturated.
#[must_use]
pub fn hub_seal_colors(state: &HubAmbientState) -> HubSealColors {
    HubSealColors {
        core: interpolate_color(&SEAL_CORE_TRACK, state.seal_core_phase),
        glyphs: saturate(interpolate_color(&SEAL_GLYPH_TRACK, state.seal_glyph_phase), 0.5),
    }
}

/// Where the statue's art sits relative to its registered origins (`hubStatueOffsets`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HubStatueOffsets {
    pub aura: Vec2,
    pub body: Vec2,
}

/// The statue floats fifteen units up and bobs two units either way; its aura swings along a
/// sixty-degree line, squashed vertically.
#[must_use]
pub fn hub_statue_offsets(state: &HubAmbientState) -> HubStatueOffsets {
    let wave = -2.0 * float::sin(state.statue_phase_degrees * std::f64::consts::PI / 180.0);
    let third_turn = std::f64::consts::PI / 3.0;
    HubStatueOffsets {
        aura: Vec2::new(float::cos(third_turn) * wave, -float::sin(third_turn) * wave * 0.8),
        body: Vec2::new(0.0, wave - 15.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stepped(ticks: u32) -> HubAmbientState {
        let mut state = HubAmbientState::new();
        for _ in 0..ticks {
            state.step(None);
        }
        state
    }

    #[test]
    fn the_sampler_never_sticks_at_zero() {
        let (state, unit) = next_random(0);
        assert_eq!(state, RNG_ZERO_ESCAPE);
        assert_eq!(unit, 0.0);
        let (state, unit) = next_random(HUB_AMBIENT_RNG_SEED);
        assert_ne!(state, HUB_AMBIENT_RNG_SEED);
        assert!((0.0..1.0).contains(&unit));
    }

    #[test]
    fn spray_is_born_small_lives_up_to_two_and_a_quarter_seconds_and_grows() {
        let state = stepped(2000);
        assert!(!state.fountain_particles.is_empty(), "a roll of 3 in 80 lands within 2000 ticks");
        for particle in &state.fountain_particles {
            assert!(particle.remaining > 0.0 && particle.remaining <= 2.25, "{particle:?}");
            assert!(particle.scale >= FOUNTAIN_INITIAL_SCALE, "{particle:?}");
            assert!(hub_fountain_particle_alpha(particle) <= FOUNTAIN_ALPHA_LIMIT);
        }
        let ids: Vec<u32> = state.fountain_particles.iter().map(|particle| particle.id).collect();
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]), "oldest drops first: {ids:?}");
        assert!(state.next_fountain_particle_id > ids[ids.len() - 1]);
    }

    #[test]
    fn phases_stay_on_their_tracks_and_the_clock_counts_ticks() {
        let state = stepped(3000);
        assert!((0.0..3.0).contains(&state.seal_core_phase));
        assert!((0.0..3.0).contains(&state.seal_glyph_phase));
        assert!((0.0..360.0).contains(&state.statue_phase_degrees));
        assert_eq!(state.statue_phase_degrees, (3000.0 * 0.5) % 360.0);
        assert_eq!(state.teacher_tick, 3000);
        assert_eq!(state.teacher_burst().release_index, 3);
    }

    #[test]
    fn seal_colours_follow_their_tracks() {
        let mut state = HubAmbientState::new();
        let rest = hub_seal_colors(&state);
        assert_eq!(rest.core, HubColor::new(1.0, 1.0, 1.0, 1.0));
        let expected_glyph = saturate(SEAL_GLYPH_TRACK[0], 0.5);
        assert_eq!(rest.glyphs, expected_glyph);
        assert!(expected_glyph.red > 0.5 && expected_glyph.blue < 1.0, "half desaturated");

        state.seal_core_phase = 1.0;
        assert_eq!(hub_seal_colors(&state).core, HubColor::new(0.0, 1.0, 1.0, 1.0));
        state.seal_core_phase = 2.5;
        assert_eq!(hub_seal_colors(&state).core.red, 1.0, "the last colour wraps to the first");
        state.seal_core_phase = -0.5;
        assert_eq!(hub_seal_colors(&state).core.red, 1.0, "negative phases wrap forward");
    }

    #[test]
    fn the_statue_rests_fifteen_units_up_and_wraps_its_phase() {
        let mut state = HubAmbientState::new();
        let rest = hub_statue_offsets(&state);
        assert_eq!(rest, HubStatueOffsets { aura: Vec2::ZERO, body: Vec2::new(0.0, -15.0) });
        state.statue_phase_degrees = 90.0;
        let peak = hub_statue_offsets(&state);
        assert!((peak.body.y + 17.0).abs() < 1e-12, "{peak:?}");
        assert!((peak.aura.x + 1.0).abs() < 1e-12, "{peak:?}");
        let aura_y = 2.0 * (std::f64::consts::PI / 3.0).sin() * 0.8;
        assert!((peak.aura.y - aura_y).abs() < 1e-12, "{peak:?}");
        state.statue_phase_degrees = 359.75;
        state.step(None);
        assert_eq!(state.statue_phase_degrees, 0.25);
    }
}
