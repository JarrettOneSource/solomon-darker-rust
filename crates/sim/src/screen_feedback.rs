//! Shared event cursors, overlays and camera decay from
//! `renderer/native-screen-feedback.ts`. All mutable fields are retained for
//! checkpoints; the host supplies its existing tick and canonical actor events.

mod flashes;
mod region_camera;

pub use flashes::NativeScreenFlashPresentation;
pub use region_camera::NativeRegionCameraPresentation;

use crate::Vec2;
use crate::float::fround;
use crate::region_feedback::{
    NativeRegionCameraShake, NativeRegionCameraShakeAttenuation, native_region_hit_point_gain,
    native_region_point_gain,
};
use crate::secondary::presentation::draws::{
    pack_normalized_rgb, repeated_float_decay, repeated_float_multiply,
};
use crate::secondary::{
    NativeSecondaryActorKind, NativeSecondaryEventState, NativeSecondaryScreenFlashState,
};
use crate::spells::model::PrimarySpellEtherBlast;

const CAMERA_MAGNITUDE_DECAY: f64 = 0.94_f32 as f64;
const CAMERA_MAGNITUDE_CUTOFF: f64 = 0.001_f32 as f64;
const CAMERA_DISPLACEMENT_DECAY: f64 = 0.75_f32 as f64;
// `core-kernels/native-ether-blast.ts` stores both constants as singles.
const ETHER_BLAST_FLASH_DECAY: f64 = 0.025_f32 as f64;
const ETHER_BLAST_SCREEN_GREEN: f64 = 0.25_f32 as f64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSecondaryScreenOverlay {
    pub alpha: f64,
    pub color: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSecondaryScreenFeedbackContext {
    pub camera_center: Vec2,
    pub local_player_alternate: bool,
    pub visible_world_width: f64,
}

/// The presentation fields projected from the host's canonical EnemyEvent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeEnemyScreenFeedbackInput<'a> {
    pub camera_shake: Option<NativeRegionCameraShake>,
    pub event_id: u64,
    pub run_id: &'a str,
    pub screen_flash: Option<NativeSecondaryScreenFlashState>,
    pub screen_flash_only_if_clear: bool,
    pub source_position: Option<Vec2>,
    pub tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativePrimaryCameraDisplacementInput<'a> {
    pub displacement: Vec2,
    pub event_id: u64,
    pub tick: u64,
    pub world_key: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativePrimaryCameraMagnitudeInput<'a> {
    pub event_id: u64,
    pub magnitude: f64,
    pub tick: u64,
    pub world_key: &'a str,
}

#[must_use]
pub fn present_native_secondary_screen_overlay(
    overlay: Option<NativeSecondaryScreenOverlay>,
    reduced_screen_flashes: bool,
) -> Option<NativeSecondaryScreenOverlay> {
    overlay.map(|mut overlay| {
        if reduced_screen_flashes {
            overlay.alpha *= 0.2;
        }
        overlay
    })
}

#[must_use]
pub fn native_secondary_world_shake(
    actors: impl IntoIterator<Item = impl crate::secondary::SecondaryActorRead>,
    world_key: &str,
    current: Vec2,
) -> Vec2 {
    let mut selected = Vec2::new(fround(current.x), fround(current.y));
    let mut magnitude_squared = selected.x * selected.x + selected.y * selected.y;
    for actor in actors {
        if actor.world_key != world_key || actor.kind != NativeSecondaryActorKind::Earthquake {
            continue;
        }
        let candidate_squared =
            actor.velocity.x * actor.velocity.x + actor.velocity.y * actor.velocity.y;
        if candidate_squared > magnitude_squared {
            selected = actor.velocity;
            magnitude_squared = candidate_squared;
        }
    }
    selected
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeSecondaryScreenFeedbackPresentation {
    pub alpha: f64,
    pub blue: f64,
    pub camera_displacement_x: f64,
    pub camera_displacement_y: f64,
    pub camera_magnitude: f64,
    pub green: f64,
    pub last_enemy_event_id: u64,
    pub last_event_id: u64,
    pub last_primary_impact_id: u64,
    pub last_primary_feedback_id: u64,
    pub last_primary_magnitude_id: u64,
    pub last_tick: u64,
    pub red: f64,
    pub decay_per_tick: f64,
    pub world_key: String,
}

impl NativeSecondaryScreenFeedbackPresentation {
    #[must_use]
    pub fn new(initial_tick: u64, world_key: String) -> Self {
        Self {
            alpha: 0.0,
            blue: 1.0,
            camera_displacement_x: 0.0,
            camera_displacement_y: 0.0,
            camera_magnitude: 0.0,
            green: 1.0,
            last_enemy_event_id: 0,
            last_event_id: 0,
            last_primary_impact_id: 0,
            last_primary_feedback_id: 0,
            last_primary_magnitude_id: 0,
            last_tick: initial_tick,
            red: 1.0,
            decay_per_tick: 0.0,
            world_key,
        }
    }

    pub fn consume(
        &mut self,
        event: &NativeSecondaryEventState,
        context: NativeSecondaryScreenFeedbackContext,
    ) {
        if event.event_id <= self.last_event_id {
            return;
        }
        // Seen events remain seen even when they belong to another world.
        self.last_event_id = event.event_id;
        if event.world_key != self.world_key {
            return;
        }
        self.advance_to(event.tick);
        if event.camera_magnitude > 0.0 {
            self.camera_magnitude = fround(event.camera_magnitude);
            if event.tick < self.last_tick {
                self.camera_magnitude = repeated_float_multiply(
                    self.camera_magnitude,
                    CAMERA_MAGNITUDE_DECAY,
                    self.last_tick - event.tick,
                );
            }
        }
        if let Some(displacement) = event.camera_displacement {
            self.write_camera_displacement(displacement, event.tick);
        }
        if let Some(flash) = event.screen_flash {
            let point_gain =
                if flash.point_attenuated { context.point_gain(event.position) } else { 1.0 };
            self.write_flash(flash, point_gain, event.tick);
        }
    }

    pub fn consume_enemy(
        &mut self,
        event: NativeEnemyScreenFeedbackInput<'_>,
        context: NativeSecondaryScreenFeedbackContext,
    ) {
        if event.event_id <= self.last_enemy_event_id {
            return;
        }
        self.last_enemy_event_id = event.event_id;
        if self.world_key.strip_prefix("boneyard:") != Some(event.run_id) {
            return;
        }
        self.advance_to(event.tick);
        if let Some(shake) = event.camera_shake {
            let source =
                event.source_position.expect("Boss camera shake requires its world position");
            let gain = match shake.attenuation {
                NativeRegionCameraShakeAttenuation::Fixed => 1.0,
                NativeRegionCameraShakeAttenuation::Point => context.point_gain(source),
                NativeRegionCameraShakeAttenuation::Hit
                | NativeRegionCameraShakeAttenuation::HitSquared => native_region_hit_point_gain(
                    source,
                    context.camera_center,
                    context.visible_world_width,
                    context.local_player_alternate,
                ),
            };
            let scale = if shake.attenuation == NativeRegionCameraShakeAttenuation::HitSquared {
                fround(gain * gain)
            } else {
                gain
            };
            self.write_camera_displacement(
                Vec2::new(
                    fround(shake.displacement.x * scale),
                    fround(shake.displacement.y * scale),
                ),
                event.tick,
            );
        }
        let Some(flash) = event.screen_flash else {
            return;
        };
        if event.screen_flash_only_if_clear && self.alpha > 0.0 {
            return;
        }
        let gain = if flash.point_attenuated {
            context.point_gain(
                event
                    .source_position
                    .expect("Point-attenuated boss flash requires its world position"),
            )
        } else {
            1.0
        };
        self.write_flash(flash, gain, event.tick);
    }

    pub fn consume_primary_camera_displacement(
        &mut self,
        input: NativePrimaryCameraDisplacementInput<'_>,
    ) {
        if input.event_id <= self.last_primary_impact_id {
            return;
        }
        self.last_primary_impact_id = input.event_id;
        if input.world_key != self.world_key {
            return;
        }
        self.advance_to(input.tick);
        self.write_camera_displacement(input.displacement, input.tick);
    }

    pub fn consume_primary_ether_blast(
        &mut self,
        effect: &PrimarySpellEtherBlast,
        context: NativeSecondaryScreenFeedbackContext,
    ) {
        if effect.common.id <= self.last_primary_feedback_id {
            return;
        }
        self.last_primary_feedback_id = effect.common.id;
        if effect.common.world_key != self.world_key {
            return;
        }
        self.advance_to(effect.birth_tick);
        self.camera_magnitude = fround(f64::from(effect.charges) * (0.1_f32 as f64));
        self.alpha = context.point_gain(effect.origin);
        self.blue = 1.0;
        self.decay_per_tick = ETHER_BLAST_FLASH_DECAY;
        self.green = ETHER_BLAST_SCREEN_GREEN;
        self.red = 1.0;
        if effect.birth_tick < self.last_tick {
            let elapsed = self.last_tick - effect.birth_tick;
            self.alpha = repeated_float_decay(self.alpha, self.decay_per_tick, elapsed);
            self.camera_magnitude =
                repeated_float_multiply(self.camera_magnitude, CAMERA_MAGNITUDE_DECAY, elapsed);
        }
    }

    pub fn consume_primary_camera_magnitude(
        &mut self,
        input: NativePrimaryCameraMagnitudeInput<'_>,
    ) {
        if input.event_id <= self.last_primary_magnitude_id {
            return;
        }
        self.last_primary_magnitude_id = input.event_id;
        if input.world_key != self.world_key {
            return;
        }
        self.advance_to(input.tick);
        self.camera_magnitude = fround(input.magnitude);
        if input.tick < self.last_tick {
            self.camera_magnitude = repeated_float_multiply(
                self.camera_magnitude,
                CAMERA_MAGNITUDE_DECAY,
                self.last_tick - input.tick,
            );
        }
    }

    pub fn sample(&mut self, tick: u64) -> Option<NativeSecondaryScreenOverlay> {
        self.advance_to(tick);
        if self.alpha <= 0.0 {
            return None;
        }
        Some(NativeSecondaryScreenOverlay {
            alpha: self.alpha,
            color: pack_normalized_rgb(self.red, self.green, self.blue),
        })
    }

    pub fn sample_camera_magnitude(&mut self, tick: u64) -> f64 {
        self.advance_to(tick);
        self.camera_magnitude
    }

    pub fn sample_camera_displacement(&mut self, tick: u64) -> Vec2 {
        self.advance_to(tick);
        Vec2::new(self.camera_displacement_x, self.camera_displacement_y)
    }

    fn write_flash(
        &mut self,
        flash: NativeSecondaryScreenFlashState,
        point_gain: f64,
        event_tick: u64,
    ) {
        self.alpha = fround(flash.alpha * point_gain);
        self.blue = flash.blue;
        self.decay_per_tick = flash.decay_per_tick;
        self.green = flash.green;
        self.red = flash.red;
        if event_tick < self.last_tick {
            self.alpha =
                repeated_float_decay(self.alpha, self.decay_per_tick, self.last_tick - event_tick);
        }
    }

    fn write_camera_displacement(&mut self, displacement: Vec2, event_tick: u64) {
        let mut x = fround(displacement.x);
        let mut y = fround(displacement.y);
        if event_tick < self.last_tick {
            let elapsed = self.last_tick - event_tick;
            x = repeated_float_multiply(x, CAMERA_DISPLACEMENT_DECAY, elapsed);
            y = repeated_float_multiply(y, CAMERA_DISPLACEMENT_DECAY, elapsed);
        }
        let current_squared = self.camera_displacement_x * self.camera_displacement_x
            + self.camera_displacement_y * self.camera_displacement_y;
        // Ties retain the earlier displacement, across all event producers.
        if x * x + y * y > current_squared {
            self.camera_displacement_x = x;
            self.camera_displacement_y = y;
        }
    }

    fn advance_to(&mut self, tick: u64) {
        if tick <= self.last_tick {
            return;
        }
        let elapsed = tick - self.last_tick;
        if self.alpha > 0.0 && self.decay_per_tick > 0.0 {
            self.alpha = repeated_float_decay(self.alpha, self.decay_per_tick, elapsed);
        }
        self.camera_magnitude =
            repeated_float_multiply(self.camera_magnitude, CAMERA_MAGNITUDE_DECAY, elapsed);
        if self.camera_magnitude < CAMERA_MAGNITUDE_CUTOFF {
            self.camera_magnitude = 0.0;
        }
        self.camera_displacement_x =
            repeated_float_multiply(self.camera_displacement_x, CAMERA_DISPLACEMENT_DECAY, elapsed);
        self.camera_displacement_y =
            repeated_float_multiply(self.camera_displacement_y, CAMERA_DISPLACEMENT_DECAY, elapsed);
        if self.camera_displacement_x * self.camera_displacement_x
            + self.camera_displacement_y * self.camera_displacement_y
            <= 0.25
        {
            self.camera_displacement_x = 0.0;
            self.camera_displacement_y = 0.0;
        }
        self.last_tick = tick;
    }
}

impl NativeSecondaryScreenFeedbackContext {
    fn point_gain(self, position: Vec2) -> f64 {
        native_region_point_gain(
            position,
            self.camera_center,
            self.visible_world_width,
            self.local_player_alternate,
        )
    }
}
