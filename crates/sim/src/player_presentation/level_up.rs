//! The bounded sparkle program in `renderer/level-up-presentation.ts`.

use crate::float;
use std::f64::consts::PI;

use crate::float::fround;
use crate::player::PlayerState;

pub const NATIVE_LEVEL_UP_PRESENTATION_DURATION_MS: f64 = 2390.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeLevelUpParticle {
    pub alpha: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub rotation_radians: f64,
    pub scale: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeLevelUpPresentationFrame {
    pub emitting: bool,
    pub light_radius: f64,
    pub particles: Vec<NativeLevelUpParticle>,
    pub tick: u64,
}

/// MainMenuScene's pending-offer/barrier admission plus the retained renderer's
/// setLevelUpPresentation/frame clock. A null offer stops admission, not a burst
/// that already started. The wall-clock sample keeps running through the barrier.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeLevelUpPresentationTracker {
    world_key: Option<String>,
    armed_id: Option<u64>,
    last_id: Option<u64>,
    started_at_ms: Option<f64>,
}

impl NativeLevelUpPresentationTracker {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    #[must_use]
    pub fn presentation_id(&self) -> Option<u64> {
        self.armed_id
    }

    /// Keep the Hub key stable across its private rooms; replace it when the
    /// source renderer would be replaced (a new run/world or local participant).
    pub fn sample(
        &mut self,
        world_key: &str,
        player: &PlayerState,
        barrier_id: Option<u64>,
        frame_at_ms: f64,
        player_screen_y: f64,
    ) -> Option<NativeLevelUpPresentationFrame> {
        if self.world_key.as_deref() != Some(world_key) {
            self.reset();
            self.world_key = Some(world_key.to_owned());
        }
        let requested_id = barrier_id.filter(|_| player.progression.pending_offer.is_some());
        if requested_id.is_some() && requested_id != self.last_id {
            self.armed_id = requested_id;
            self.last_id = requested_id;
            self.started_at_ms = None;
        }
        let id = self.armed_id?;
        let started = *self.started_at_ms.get_or_insert(frame_at_ms);
        let elapsed = frame_at_ms - started;
        if elapsed >= NATIVE_LEVEL_UP_PRESENTATION_DURATION_MS {
            self.armed_id = None;
            self.started_at_ms = None;
            return None;
        }
        Some(native_level_up_presentation_frame(id, elapsed, player_screen_y))
    }
}

/// BoneyardSceneLights only widens an admitted local-player light while the
/// emitter is active. The additive offset uses the source's double 2.6 literal.
#[must_use]
pub fn native_level_up_player_light_radius(
    overlay_effect_phase: f64,
    frame: &NativeLevelUpPresentationFrame,
) -> Option<f64> {
    if !frame.emitting {
        return None;
    }
    Some(
        (1.0 + overlay_effect_phase) * crate::boneyard::lighting::NATIVE_PLAYER_LIGHT_RADIUS
            + (frame.light_radius - 2.6),
    )
}

#[must_use]
pub fn native_level_up_presentation_frame(
    presentation_id: u64,
    elapsed_ms: f64,
    player_screen_y: f64,
) -> NativeLevelUpPresentationFrame {
    let tick = if elapsed_ms.is_finite() && elapsed_ms > 0.0 {
        (elapsed_ms / 10.0).floor() as u64
    } else {
        0
    };
    let latest_birth_tick = tick.min(179);
    let vertical_span = if player_screen_y.is_finite() { player_screen_y.max(0.0) } else { 0.0 };
    let mut particles = Vec::new();
    for birth_tick in 0..=latest_birth_tick {
        let age = tick - birth_tick;
        if age >= 60 {
            continue;
        }
        let spawn_y = fround(-20.0 - random_float(presentation_id, birth_tick, 0, vertical_span));
        let magnitude = random_float(presentation_id, birth_tick, 1, 30.0);
        let spawn_x = if (random_word(presentation_id, birth_tick, 2) >> 6) & 1 == 1 {
            -magnitude
        } else {
            magnitude
        };
        let rotation_degrees = random_float(presentation_id, birth_tick, 3, 360.0);
        let decay = fround(3.0 + random_float(presentation_id, birth_tick, 4, 2.0));
        let mut remaining_timer = 180.0;
        let mut offset_y = spawn_y;
        for _ in 0..age {
            remaining_timer = fround(remaining_timer - decay);
            offset_y = fround(offset_y - f64::from(0.1_f32));
        }
        if remaining_timer <= 0.0 {
            continue;
        }
        let lateral_fade = 1.0 - spawn_x.abs() / 30.0;
        let emitter_timer = 180 - birth_tick - 1;
        let birth_alpha = float::sin(emitter_timer as f64 * PI / 180.0);
        particles.push(NativeLevelUpParticle {
            alpha: birth_alpha.max(0.0) * lateral_fade * 0.75,
            offset_x: spawn_x,
            offset_y,
            rotation_radians: rotation_degrees * PI / 180.0,
            scale: float::sin(remaining_timer * PI / 180.0),
        });
    }
    let emitter_timer = 179_u64.saturating_sub(tick);
    NativeLevelUpPresentationFrame {
        emitting: tick < 180,
        light_radius: 2.6 + float::sin(emitter_timer as f64 * PI / 180.0),
        particles,
        tick,
    }
}

fn random_float(id: u64, tick: u64, lane: u32, magnitude: f64) -> f64 {
    let word = random_word(id, tick, lane);
    let integer = ((word >> 6) & 0x1ffff) % 100_001;
    let quotient = fround(fround(f64::from(integer)) / 100_000.0);
    fround(quotient * fround(magnitude))
}

fn random_word(id: u64, tick: u64, lane: u32) -> u32 {
    // JavaScript imul/bitwise operands intentionally wrap to their low 32 bits.
    let mut value = (id as u32).wrapping_mul(0x9e37_79b1)
        ^ (tick as u32).wrapping_mul(0x85eb_ca77)
        ^ lane.wrapping_mul(0xc2b2_ae3d);
    value = (value ^ (value >> 16)).wrapping_mul(0x21f0_aaad);
    value = (value ^ (value >> 15)).wrapping_mul(0x735a_2d97);
    value ^ (value >> 15)
}
