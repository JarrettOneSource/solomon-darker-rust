//! Display-only interpretation of accepted native snapshots. Source:
//! `client/{hub,boneyard}-presentation-timeline.ts` and `game-client-session.ts`.
//! The authority is copied, never stepped, restored, acknowledged or saved here.

mod boneyard;
mod enemies;
mod hub;
mod math;
mod prediction;
mod primary;
mod scenery;
mod secondary;

use std::collections::{BTreeMap, VecDeque};

use solomon_protocol::flame_presentation::{FlamePresentationFrame, RetainedFlamePresentation};
use solomon_sim::game_run::{GameRunPhase, game_run_world_tick};
use solomon_sim::painter::PainterRegistration;
use solomon_sim::session::{GameSession, GameSessionInput};
use solomon_sim::spells::weld::NativeFlameCameraProposal;

pub use enemies::EnemyPresentationMetadata;
use prediction::LocalHubPrediction;
pub use scenery::scenery_actor_poses;

const MAX_BUFFERED_SNAPSHOTS: usize = 8;

/// Fractions remain in the renderer rather than weakening simulation tick types.
/// Keys always refer to the real actors in `PresentationFrame::session`.
#[derive(Clone, Debug, Default)]
pub struct PresentationClocks {
    pub primary: BTreeMap<u64, f64>,
    pub primary_contact: BTreeMap<u64, f64>,
    pub primary_ground_spark: BTreeMap<u64, f64>,
    pub primary_release: BTreeMap<u64, f64>,
    pub primary_remaining: BTreeMap<u64, f64>,
    pub secondary: BTreeMap<u64, f64>,
    pub player_damage_x4: BTreeMap<String, f64>,
    pub puppet_hits: BTreeMap<String, f64>,
    pub puppet_hit_timers: BTreeMap<String, f64>,
}

impl PresentationClocks {
    #[must_use]
    pub fn from_session(session: &GameSession) -> Self {
        let mut clocks = Self::default();
        for projectile in &session.spells.projectiles {
            let common = projectile.common();
            clocks.primary.insert(common.id, common.age_ticks as f64);
        }
        for transient in &session.spells.transients {
            let common = transient.common();
            clocks.primary.insert(common.id, common.age_ticks as f64);
            if let solomon_sim::spells::model::PrimarySpellTransient::FireGoodImp(imp) = transient
                && let Some(age) = imp.contact_age_ticks
            {
                clocks.primary_contact.insert(common.id, age as f64);
            }
        }
        for actor in &session.secondary.actors {
            clocks.secondary.insert(actor.id, actor.age_ticks as f64);
        }
        for (id, participant) in &session.participants {
            clocks.player_damage_x4.insert(
                id.clone(),
                participant.player.progression.damage_x4_ticks_remaining as f64,
            );
        }
        if let Some(world) = &session.boneyard {
            for hit in &world.enemies.puppet_hits {
                clocks
                    .puppet_hits
                    .insert(hit.target_id.clone(), hit.feedback.alpha(session.ticks as f64));
                clocks
                    .puppet_hit_timers
                    .insert(hit.target_id.clone(), hit.feedback.timer_at(session.ticks as f64));
            }
        }
        clocks
    }
}

/// One sampled render frame. Never submit this private display copy to persistence
/// or commands: those continue to borrow Game's accepted `self.session`.
pub struct PresentationFrame {
    pub session: Box<GameSession>,
    pub tick: f64,
    pub clocks: PresentationClocks,
    pub enemies: Option<EnemyPresentationMetadata>,
    pub flame_epoch: Option<u64>,
    pub flame_camera_displacements: Vec<NativeFlameCameraProposal>,
}

struct TimedSession {
    session: Box<GameSession>,
    enemies: Option<EnemyPresentationMetadata>,
    received_at_ms: f64,
    presentation_start_tick: f64,
    flame_epoch: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum WorldScope {
    Hub { phase: GameRunPhase, local_registration: Option<PainterRegistration> },
    Boneyard(String),
}

impl WorldScope {
    /// `sameHubPresentationOwner` in `client/game-client-session.ts`: resident
    /// College and post-run Create may reuse a world kind but have distinct clocks.
    fn same_owner(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Hub { phase, local_registration: Some(registration) },
                Self::Hub { phase: next_phase, local_registration: Some(next_registration) },
            ) => phase == next_phase && registration == next_registration,
            (Self::Boneyard(run_id), Self::Boneyard(next_run_id)) => run_id == next_run_id,
            _ => false,
        }
    }
}

fn scope(session: &GameSession, local_player_id: &str) -> WorldScope {
    if let Some(world) = &session.boneyard {
        WorldScope::Boneyard(world.loaded.run_id.clone())
    } else {
        WorldScope::Hub {
            phase: session.run.phase,
            local_registration: session
                .participants
                .get(local_player_id)
                .map(|participant| participant.lighting.light_registration),
        }
    }
}

pub struct NativePresentationTimeline {
    history: VecDeque<TimedSession>,
    tick_rate: u32,
    snapshot_rate: u32,
    interval_ticks: f64,
    scope: Option<WorldScope>,
    local_prediction: Option<LocalHubPrediction>,
    current_input: GameSessionInput,
    prediction_held: bool,
    primary_memory: primary::PrimaryPresentationMemory,
    flame_presentation: Option<RetainedFlamePresentation>,
}

impl NativePresentationTimeline {
    /// Rates come from the validated native Welcome, not a second simulation clock.
    #[must_use]
    pub fn new(server_tick_rate: u32, snapshot_rate: u32) -> Self {
        assert!(server_tick_rate > 0 && snapshot_rate > 0, "presentation rates must be positive");
        Self {
            history: VecDeque::new(),
            tick_rate: server_tick_rate,
            snapshot_rate,
            interval_ticks: (f64::from(server_tick_rate) / f64::from(snapshot_rate)).max(1.0),
            scope: None,
            local_prediction: None,
            current_input: GameSessionInput::default(),
            prediction_held: false,
            primary_memory: primary::PrimaryPresentationMemory::default(),
            flame_presentation: None,
        }
    }

    /// New admission/disconnect clears view-owned clocks even when actor IDs repeat.
    pub fn reset(&mut self) {
        self.history.clear();
        self.scope = None;
        self.local_prediction = None;
        self.current_input = GameSessionInput::default();
        self.prediction_held = false;
        self.primary_memory = primary::PrimaryPresentationMemory::default();
        self.flame_presentation = None;
    }

    /// Delivered positions only: Tree scans must not advance the stateful display
    /// sampler or substitute the oldest retained endpoint for missing history.
    #[must_use]
    pub fn sample_player_position_at_tick(&self, tick: f64, id: &str) -> Option<solomon_sim::Vec2> {
        let (older, newer, blend) = self.retained_position_interval(tick)?;
        scenery::player_position(&older.session, &newer.session, blend, id)
    }

    #[must_use]
    pub fn sample_scenery_actor_poses_at_tick(
        &self,
        tick: f64,
    ) -> Option<solomon_sim::boneyard::scenery::NativeTreeActorPoses> {
        let (older, newer, blend) = self.retained_position_interval(tick)?;
        Some(scenery::interpolate(older, newer, blend))
    }

    fn retained_position_interval(&self, tick: f64) -> Option<(&TimedSession, &TimedSession, f64)> {
        let first = self.history.front()?;
        let last = self.history.back()?;
        if !tick.is_finite()
            || tick < first.session.ticks as f64
            || tick > last.session.ticks as f64
            || first.session.boneyard.is_none()
        {
            return None;
        }
        let (older, newer) = bracket(&self.history, tick);
        let older = &self.history[older];
        let newer = &self.history[newer];
        let span = newer.session.ticks - older.session.ticks;
        let blend = if span == 0 {
            1.0
        } else {
            ((tick - older.session.ticks as f64) / span as f64).clamp(0.0, 1.0)
        };
        Some((older, newer, blend))
    }

    pub fn push(
        &mut self,
        session: &GameSession,
        received_at_ms: f64,
        local_player_id: &str,
        flame_frame: Option<&FlamePresentationFrame>,
    ) -> Result<(), String> {
        assert!(received_at_ms.is_finite(), "presentation receipt time must be finite");
        if let Some(frame) = flame_frame {
            let world_key = session.boneyard.as_ref().map_or_else(
                || "hub".to_owned(),
                |world| format!("boneyard:{}", world.loaded.run_id),
            );
            let world_tick = game_run_world_tick(session.ticks as f64, &session.run) as u64;
            frame.validate_snapshot(&world_key, world_tick, session.spells.next_id)?;
        }
        let next_scope = scope(session, local_player_id);
        if self.scope.as_ref().is_some_and(|previous| !previous.same_owner(&next_scope)) {
            self.reset();
        }
        self.scope = Some(next_scope);
        if let Some(flames) = &mut self.flame_presentation {
            let oldest = self.history.front().map_or(0, |oldest| {
                game_run_world_tick(oldest.session.ticks as f64, &oldest.session.run)
                    .max(0.0)
                    .floor() as u64
            });
            if flames.accept(flame_frame, oldest)? {
                self.history.clear();
            }
        } else {
            self.flame_presentation = Some(RetainedFlamePresentation::new(flame_frame)?);
        }
        if let Some(latest) = self.history.back()
            && session.ticks < latest.session.ticks
        {
            return Ok(());
        }
        if session.boneyard.is_some() || self.prediction_held || session.level_up_barrier.is_some()
        {
            self.local_prediction = None;
        } else if let Some(prediction) = &mut self.local_prediction {
            prediction.reconcile(
                session,
                received_at_ms,
                self.current_input,
                self.tick_rate,
                self.snapshot_rate,
            );
        }
        let enemies = boneyard::capture_enemies(session);
        if let Some(latest) = self.history.back_mut()
            && session.ticks == latest.session.ticks
        {
            // Same-tick feedback replaces data without restarting elapsed time.
            latest.session.as_mut().clone_from(session);
            latest.enemies = enemies;
            latest.flame_epoch = flame_frame.map(FlamePresentationFrame::epoch);
            return Ok(());
        }
        let presentation_start_tick = self.history.back().map_or(session.ticks as f64, |latest| {
            (session.ticks as f64 - self.interval_ticks).max(target_tick(
                latest,
                received_at_ms,
                self.tick_rate,
            ))
        });
        self.history.push_back(TimedSession {
            session: Box::new(session.clone()),
            enemies,
            received_at_ms,
            presentation_start_tick,
            flame_epoch: flame_frame.map(FlamePresentationFrame::epoch),
        });
        if self.history.len() > MAX_BUFFERED_SNAPSHOTS {
            self.history.pop_front();
        }
        Ok(())
    }

    /// Boneyard samples received states only. Hub alone may predict its local
    /// character for one snapshot interval, using the canonical movement kernel.
    pub fn sample(
        &mut self,
        now_ms: f64,
        local_player_id: &str,
        current_input: GameSessionInput,
        prediction_held: bool,
    ) -> Result<Option<PresentationFrame>, String> {
        assert!(now_ms.is_finite(), "presentation sample time must be finite");
        let Some(newest) = self.history.back() else { return Ok(None) };
        let tick = if self.history.len() == 1 {
            newest.session.ticks as f64
        } else {
            target_tick(newest, now_ms, self.tick_rate)
        };
        let (older_index, newer_index) = bracket(&self.history, tick);
        let older = &self.history[older_index];
        let newer = &self.history[newer_index];
        let span = newer.session.ticks - older.session.ticks;
        let blend = if span == 0 {
            1.0
        } else {
            ((tick - older.session.ticks as f64) / span as f64).clamp(0.0, 1.0)
        };
        let discrete = if blend < 1.0 { &older.session } else { &newer.session };
        let mut output = discrete.clone();
        let mut clocks = PresentationClocks::from_session(&output);
        hub::project_players(&mut output, &older.session, &newer.session, blend, &mut clocks);
        if older_index == newer_index {
            primary::copy(
                &mut output.spells,
                older.session.ticks,
                &mut self.primary_memory,
                &mut clocks,
            );
        } else {
            primary::project(
                &mut output.spells,
                &older.session.spells,
                &newer.session.spells,
                older.session.ticks,
                newer.session.ticks,
                tick,
                blend,
                &mut self.primary_memory,
                &mut clocks,
            );
        }
        secondary::project(
            &mut output.secondary,
            &older.session.secondary,
            &newer.session.secondary,
            blend,
            &mut clocks,
        );
        let enemies = if output.boneyard.is_some() {
            Some(boneyard::project(
                &mut output,
                &older.session,
                &newer.session,
                older.enemies.as_ref().expect("Boneyard endpoint metadata"),
                newer.enemies.as_ref().expect("Boneyard endpoint metadata"),
                blend,
                tick,
                &mut clocks,
            ))
        } else {
            hub::project_world(&mut output, &older.session, &newer.session, blend);
            let elapsed = ((now_ms - newest.received_at_ms) * f64::from(self.tick_rate) / 1000.0)
                .clamp(0.0, self.interval_ticks);
            hub::project_local(&mut output, &newest.session, local_player_id, elapsed, &mut clocks);
            None
        };
        let stopped_tick =
            tick.min(game_run_world_tick(newest.session.ticks as f64, &newest.session.run));
        let flames =
            self.flame_presentation.as_ref().expect("buffered snapshot owns its Flame replay");
        let flame_camera_displacements =
            flames.camera_displacements(&output.spells, stopped_tick, tick)?;
        flames.sample(&mut output.spells, stopped_tick)?;
        let live_primary_ids: std::collections::BTreeSet<_> = output
            .spells
            .projectiles
            .iter()
            .map(|actor| actor.common().id)
            .chain(output.spells.transients.iter().map(|actor| actor.common().id))
            .collect();
        clocks.primary.retain(|id, _| live_primary_ids.contains(id));
        for effect in &output.spells.transients {
            if solomon_protocol::flame_presentation::flame_presentation_birth(effect)?.is_some() {
                clocks.primary.insert(effect.common().id, effect.common().age_ticks as f64);
            }
        }
        // Only explicitly floored display clocks occupy the canonical integer slots.
        // Continuous consumers receive `tick` and the actor clock maps separately.
        output.ticks = tick.floor() as u64;
        if let Some(world) = &mut output.boneyard {
            world.ticks = tick.floor() as u64;
        }
        let held = prediction_held || newest.session.level_up_barrier.is_some();
        if output.boneyard.is_none() && !held {
            let reset = self.prediction_held
                || self
                    .local_prediction
                    .as_ref()
                    .is_none_or(|prediction| prediction.id != local_player_id);
            if reset {
                self.local_prediction = LocalHubPrediction::new(
                    &newest.session,
                    local_player_id,
                    newest.received_at_ms,
                    self.snapshot_rate,
                );
            }
            if let Some(prediction) = &mut self.local_prediction {
                // sendInput advances the old held input before changing it. This
                // matters when direction changes between accepted snapshots.
                if current_input != self.current_input {
                    prediction.advance(
                        now_ms,
                        self.current_input,
                        self.tick_rate,
                        self.snapshot_rate,
                    );
                }
                prediction.advance(now_ms, current_input, self.tick_rate, self.snapshot_rate);
                prediction.apply(&mut output, now_ms);
            }
        } else {
            self.local_prediction = None;
        }
        self.current_input = current_input;
        self.prediction_held = held;
        Ok(Some(PresentationFrame {
            session: output,
            tick,
            clocks,
            enemies,
            flame_epoch: newest.flame_epoch,
            flame_camera_displacements,
        }))
    }
}

fn target_tick(newest: &TimedSession, now_ms: f64, tick_rate: u32) -> f64 {
    let elapsed = (now_ms - newest.received_at_ms).max(0.0) * f64::from(tick_rate) / 1000.0;
    (newest.presentation_start_tick + elapsed).min(newest.session.ticks as f64)
}

fn bracket(history: &VecDeque<TimedSession>, tick: f64) -> (usize, usize) {
    if tick <= history[0].session.ticks as f64 {
        return (0, 0);
    }
    for (index, snapshot) in history.iter().enumerate().skip(1) {
        if tick <= snapshot.session.ticks as f64 {
            return (index - 1, index);
        }
    }
    (history.len() - 1, history.len() - 1)
}
