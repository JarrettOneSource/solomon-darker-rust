//! Current Source `renderer/native-screen-feedback.ts` and its Hub/Boneyard
//! intake order. This is a renderer owner; accepted Session feedback is not drawn.

use std::collections::BTreeMap;

use solomon_sim::Vec2;
use solomon_sim::enemies::model::{CameraShakeAttenuation, EnemyEvent};
use solomon_sim::float::fround;
use solomon_sim::region_feedback::{native_region_hit_point_gain, native_region_point_gain};
use solomon_sim::screen_feedback::{
    NativeRegionCameraPresentation, NativeScreenFlashPresentation,
    NativeSecondaryScreenFeedbackContext, NativeSecondaryScreenOverlay,
};
use solomon_sim::screen_flash::NativeScreenFlashState;
use solomon_sim::secondary::presentation::repeated_float_multiply;
use solomon_sim::secondary::{NativeSecondaryEventState, NativeSecondarySimulationState};
use solomon_sim::spells::model::{PrimarySpellProjectilePhase, PrimaryTransientRef, SpellState};
use solomon_sim::spells::weld::{
    NATIVE_FLAME_CAMERA_MAX_PRESENTATION_TICKS, NativeFlameCameraProposal, NativeWeldMeteorPhase,
    WeldPersistentRef, WeldWorldRef,
};

use crate::presentation::PresentationClocks;

const MAGNITUDE_DECAY: f64 = 0.94_f32 as f64;
const MAGNITUDE_CUTOFF: f64 = 0.001_f32 as f64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenFeedbackScene {
    Hub,
    Boneyard,
}

/// One actual display frame and the unshaken camera used for point attenuation.
pub struct WorldScreenFeedbackFrame<'a> {
    pub scene: ScreenFeedbackScene,
    pub world_key: &'a str,
    pub tick: f64,
    pub stopped_world_tick: f64,
    pub flame_epoch: Option<u64>,
    pub flame_camera_displacements: &'a [NativeFlameCameraProposal],
    pub screen_flashes: &'a NativeScreenFlashState,
    pub spells: &'a SpellState,
    pub secondary: &'a NativeSecondarySimulationState,
    pub clocks: Option<&'a PresentationClocks>,
    pub context: NativeSecondaryScreenFeedbackContext,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldScreenFeedbackSample {
    pub overlay: Option<NativeSecondaryScreenOverlay>,
    pub camera_magnitude: f64,
    pub camera_displacement: Vec2,
}

/// Hub retains one lane per private Region; a world replacement resets this owner.
#[derive(Default)]
pub struct WorldScreenFeedback {
    worlds: BTreeMap<String, FeedbackLane>,
    pending_enemy_events: Vec<(String, EnemyEvent)>,
}

impl WorldScreenFeedback {
    pub fn reset(&mut self) {
        self.worlds.clear();
        self.pending_enemy_events.clear();
    }

    /// Accepted enemy callbacks create camera proposals on the next paint, without
    /// waiting for the interpolated event array's endpoint to become visible.
    pub fn consume_enemy_events(&mut self, run_id: &str, events: &[EnemyEvent]) {
        for event in events {
            self.pending_enemy_events.push((run_id.into(), event.clone()));
        }
    }

    pub fn update(&mut self, frame: WorldScreenFeedbackFrame<'_>) -> WorldScreenFeedbackSample {
        let lane = self
            .worlds
            .entry(frame.world_key.into())
            .or_insert_with(|| FeedbackLane::new(tick(frame.tick), frame.world_key.into()));
        lane.synchronize_world_timeline(frame.stopped_world_tick, frame.flame_epoch, frame.tick);
        lane.consume_flame_camera_displacements(frame.flame_camera_displacements);
        if frame.scene == ScreenFeedbackScene::Hub {
            lane.consume_world_camera_displacements(frame.secondary);
        }
        lane.flashes.consume(frame.screen_flashes, frame.context);
        if frame.scene == ScreenFeedbackScene::Boneyard {
            for (run_id, event) in self.pending_enemy_events.drain(..) {
                lane.consume_enemy(&run_id, &event, frame.context);
            }
        }
        for event in &frame.secondary.events {
            lane.consume(event);
        }
        lane.consume_primary(frame.spells, frame.tick, frame.clocks);
        if frame.scene == ScreenFeedbackScene::Boneyard {
            lane.consume_world_camera_displacements(frame.secondary);
        }
        lane.advance_to(tick(frame.tick));
        WorldScreenFeedbackSample {
            overlay: lane.flashes.sample(frame.tick),
            camera_magnitude: lane.camera_magnitude,
            camera_displacement: lane.camera.sample(),
        }
    }
}

struct FeedbackLane {
    camera: NativeRegionCameraPresentation,
    flashes: NativeScreenFlashPresentation,
    camera_magnitude: f64,
    last_enemy_event_id: usize,
    last_event_id: u64,
    last_primary_impact_id: u64,
    last_primary_feedback_id: u64,
    last_primary_magnitude_id: u64,
    last_tick: u64,
    last_world_displacement_tick: i64,
    consumed_flame_births: BTreeMap<(u64, u64), u64>,
    accepted_history_epoch: Option<u64>,
    accepted_world_tick: Option<f64>,
    world_key: String,
}

impl FeedbackLane {
    fn new(initial_tick: u64, world_key: String) -> Self {
        Self {
            camera: NativeRegionCameraPresentation::new(initial_tick),
            flashes: NativeScreenFlashPresentation::new(initial_tick, world_key.clone()),
            camera_magnitude: 0.0,
            last_enemy_event_id: 0,
            last_event_id: 0,
            last_primary_impact_id: 0,
            last_primary_feedback_id: 0,
            last_primary_magnitude_id: 0,
            last_tick: initial_tick,
            last_world_displacement_tick: initial_tick as i64 - 1,
            consumed_flame_births: BTreeMap::new(),
            accepted_history_epoch: None,
            accepted_world_tick: None,
            world_key,
        }
    }

    fn synchronize_world_timeline(
        &mut self,
        world_tick: f64,
        history_epoch: Option<u64>,
        live_tick: f64,
    ) {
        let first_frame_rewinds =
            self.accepted_world_tick.is_none() && live_tick < self.last_tick as f64;
        let replacement_rewinds =
            match (history_epoch, self.accepted_history_epoch, self.accepted_world_tick) {
                (Some(epoch), Some(previous_epoch), Some(previous_tick)) => {
                    epoch > previous_epoch && world_tick < previous_tick
                }
                _ => false,
            };
        if first_frame_rewinds || replacement_rewinds {
            *self = Self::new(tick(live_tick), self.world_key.clone());
        }
        self.accepted_history_epoch = history_epoch;
        self.accepted_world_tick = Some(world_tick);
    }

    fn consume(&mut self, event: &NativeSecondaryEventState) {
        if event.event_id <= self.last_event_id {
            return;
        }
        self.last_event_id = event.event_id;
        if event.world_key != self.world_key {
            return;
        }
        self.advance_to(event.tick);
        if event.camera_magnitude > 0.0 {
            self.write_magnitude(event.camera_magnitude, event.tick);
        }
        if let Some(displacement) = event.camera_displacement {
            self.camera.propose_current(displacement, event.tick);
        }
    }

    fn consume_enemy(
        &mut self,
        run_id: &str,
        event: &EnemyEvent,
        context: NativeSecondaryScreenFeedbackContext,
    ) {
        if event.event_id <= self.last_enemy_event_id {
            return;
        }
        self.last_enemy_event_id = event.event_id;
        if self.world_key.strip_prefix("boneyard:") != Some(run_id) {
            return;
        }
        self.advance_to(event.tick);
        let Some(shake) = event.camera_shake else { return };
        let position =
            event.source_position.expect("Boss camera shake requires its source position");
        let gain = match shake.attenuation {
            CameraShakeAttenuation::Fixed => 1.0,
            CameraShakeAttenuation::Point => native_region_point_gain(
                position,
                context.camera_center,
                context.visible_world_width,
                context.local_player_alternate,
            ),
            CameraShakeAttenuation::Hit | CameraShakeAttenuation::HitSquared => {
                native_region_hit_point_gain(
                    position,
                    context.camera_center,
                    context.visible_world_width,
                    context.local_player_alternate,
                )
            }
        };
        let scale = if shake.attenuation == CameraShakeAttenuation::HitSquared {
            fround(gain * gain)
        } else {
            gain
        };
        self.camera.propose_current(
            Vec2::new(fround(shake.displacement.x * scale), fround(shake.displacement.y * scale)),
            event.tick,
        );
    }

    fn consume_world_camera_displacements(&mut self, secondary: &NativeSecondarySimulationState) {
        for proposal in &secondary.camera_displacements {
            if proposal.world_key != self.world_key
                || proposal.tick as i64 <= self.last_world_displacement_tick
            {
                continue;
            }
            self.last_world_displacement_tick = proposal.tick as i64;
            self.advance_to(proposal.tick);
            self.camera.propose_current(proposal.displacement, proposal.tick);
        }
    }

    fn consume_flame_camera_displacements(&mut self, proposals: &[NativeFlameCameraProposal]) {
        self.consumed_flame_births.retain(|_, until| *until > self.last_tick);
        for proposal in proposals {
            let until = proposal.birth_tick + NATIVE_FLAME_CAMERA_MAX_PRESENTATION_TICKS;
            if proposal.world_key != self.world_key || until <= self.last_tick {
                continue;
            }
            let key = (proposal.birth_tick, proposal.id);
            if self.consumed_flame_births.contains_key(&key) {
                continue;
            }
            self.consumed_flame_births.insert(key, until);
            self.camera.propose_flame(proposal.displacement, proposal.birth_tick, proposal.id);
        }
    }

    fn consume_primary(
        &mut self,
        spells: &SpellState,
        presentation_tick: f64,
        clocks: Option<&PresentationClocks>,
    ) {
        for effect in spells.transients.iter() {
            match effect {
                PrimaryTransientRef::EtherBlast(effect) => {
                    if effect.common.id <= self.last_primary_feedback_id {
                        continue;
                    }
                    self.last_primary_feedback_id = effect.common.id;
                    if effect.common.world_key != self.world_key {
                        continue;
                    }
                    self.advance_to(effect.birth_tick);
                    self.write_magnitude(
                        fround(f64::from(effect.charges) * (0.1_f32 as f64)),
                        effect.birth_tick,
                    );
                }
                PrimaryTransientRef::Weld(WeldWorldRef::Meteor(effect)) => {
                    if effect.phase != NativeWeldMeteorPhase::Impact {
                        continue;
                    }
                    let Some(displacement) = effect.camera_displacement else { continue };
                    self.consume_primary_camera_displacement(
                        effect.common.owner.id,
                        &effect.common.owner.world_key,
                        tick(presentation_tick - effect.impact_age_ticks as f64),
                        displacement,
                    );
                }
                PrimaryTransientRef::Weld(WeldWorldRef::Persistent(
                    WeldPersistentRef::Hailstones(effect),
                )) => {
                    if effect.phase != PrimarySpellProjectilePhase::Flight {
                        continue;
                    }
                    let Some(age) = effect.release_age_ticks else { continue };
                    let age = clocks
                        .and_then(|clocks| clocks.primary_release.get(&effect.common.owner.id))
                        .copied()
                        .unwrap_or(age as f64);
                    self.consume_primary_camera_magnitude(
                        effect.common.owner.id,
                        &effect.common.owner.world_key,
                        tick(presentation_tick - age),
                        0.1_f32 as f64,
                    );
                }
                _ => {}
            }
        }
    }

    fn consume_primary_camera_displacement(
        &mut self,
        id: u64,
        world_key: &str,
        at: u64,
        displacement: Vec2,
    ) {
        if id <= self.last_primary_impact_id {
            return;
        }
        self.last_primary_impact_id = id;
        if world_key != self.world_key {
            return;
        }
        self.advance_to(at);
        self.camera.propose_current(displacement, at);
    }

    fn consume_primary_camera_magnitude(
        &mut self,
        id: u64,
        world_key: &str,
        at: u64,
        magnitude: f64,
    ) {
        if id <= self.last_primary_magnitude_id {
            return;
        }
        self.last_primary_magnitude_id = id;
        if world_key != self.world_key {
            return;
        }
        self.advance_to(at);
        self.write_magnitude(magnitude, at);
    }

    fn write_magnitude(&mut self, magnitude: f64, event_tick: u64) {
        self.camera_magnitude = repeated_float_multiply(
            fround(magnitude),
            MAGNITUDE_DECAY,
            self.last_tick.saturating_sub(event_tick),
        );
    }

    fn advance_to(&mut self, tick: u64) {
        if tick <= self.last_tick {
            return;
        }
        self.camera_magnitude =
            repeated_float_multiply(self.camera_magnitude, MAGNITUDE_DECAY, tick - self.last_tick);
        if self.camera_magnitude < MAGNITUDE_CUTOFF {
            self.camera_magnitude = 0.0;
        }
        self.camera.advance_to(tick);
        self.last_tick = tick;
    }
}

fn tick(value: f64) -> u64 {
    value.trunc().max(0.0) as u64
}

#[cfg(test)]
mod tests;
