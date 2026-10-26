//! Bounded local Hub movement from `client/game-client-session.ts` and
//! `client/hub-prediction.ts`. This private character never steps the session.

use solomon_sim::Vec2;
use solomon_sim::actor_physics::{ActorPhysicsBody, ActorPhysicsWorld, resolve_actor_motion};
use solomon_sim::hub::college_intro::{
    HubParticipantStepOptions, NativeCollegeIntroPhase, plan_hub_participant_movement,
    step_hub_participant_movement,
};
use solomon_sim::hub::regions::HubRegion;
use solomon_sim::hub::{PLAYER_CHARACTER_RADIUS, move_with_segments, plan_hub_wizard_tick};
use solomon_sim::session::{GameSession, GameSessionInput, SessionParticipant};

pub(super) struct LocalHubPrediction {
    pub id: String,
    participant: SessionParticipant,
    received_region: HubRegion,
    collision_rng: u32,
    correction: Vec2,
    correction_started_at_ms: f64,
    correction_duration_ms: f64,
    last_advanced_at_ms: f64,
    predicted_ticks: u32,
    remainder_ms: f64,
}

impl LocalHubPrediction {
    pub fn new(
        session: &GameSession,
        id: &str,
        received_at_ms: f64,
        snapshot_rate: u32,
    ) -> Option<Self> {
        let participant = session.participants.get(id)?.snapshot();
        let received_region = participant.wizard.region;
        Some(Self {
            id: id.to_owned(),
            participant,
            received_region,
            collision_rng: session.collision_rng,
            correction: Vec2::ZERO,
            correction_started_at_ms: received_at_ms,
            correction_duration_ms: 1000.0 / f64::from(snapshot_rate),
            last_advanced_at_ms: received_at_ms,
            predicted_ticks: 0,
            remainder_ms: 0.0,
        })
    }

    pub fn reconcile(
        &mut self,
        session: &GameSession,
        received_at_ms: f64,
        input: GameSessionInput,
        tick_rate: u32,
        snapshot_rate: u32,
    ) {
        let Some(authoritative) = session.participants.get(&self.id) else { return };
        self.advance(received_at_ms, input, tick_rate, snapshot_rate);
        let Some(mut next) = Self::new(session, &self.id, received_at_ms, snapshot_rate) else {
            return;
        };
        if self.received_region == authoritative.wizard.region {
            let displayed = self.displayed_position(received_at_ms);
            next.correction = Vec2::new(
                displayed.x - authoritative.wizard.position.x,
                displayed.y - authoritative.wizard.position.y,
            );
            next.remainder_ms = self.remainder_ms;
            next.participant.wizard.gait_degrees = self.participant.wizard.gait_degrees;
            next.participant.wizard.velocity = self.participant.wizard.velocity;
            next.participant.wizard.walk_cycle_primary = self.participant.wizard.walk_cycle_primary;
            let scripted = authoritative
                .college_intro
                .as_ref()
                .is_some_and(|intro| intro.phase != NativeCollegeIntroPhase::ArchDialogue);
            if !scripted && !authoritative.wizard.primary_cast.owns_facing() {
                next.participant.wizard.set_facing(self.participant.wizard.heading_degrees);
            }
        }
        *self = next;
    }

    pub fn advance(
        &mut self,
        now_ms: f64,
        input: GameSessionInput,
        tick_rate: u32,
        snapshot_rate: u32,
    ) {
        self.remainder_ms += (now_ms - self.last_advanced_at_ms).max(0.0);
        self.last_advanced_at_ms = self.last_advanced_at_ms.max(now_ms);
        let tick_ms = 1000.0 / f64::from(tick_rate);
        let maximum_ticks =
            (f64::from(tick_rate) / f64::from(snapshot_rate)).ceil().max(1.0) as u32;
        while self.remainder_ms >= tick_ms && self.predicted_ticks < maximum_ticks {
            predict_character(&mut self.participant, input, &mut self.collision_rng);
            self.predicted_ticks += 1;
            self.remainder_ms -= tick_ms;
        }
        if self.predicted_ticks == maximum_ticks {
            self.remainder_ms = 0.0;
        }
    }

    pub fn apply(&self, output: &mut GameSession, now_ms: f64) {
        let Some(participant) = output.participants.get_mut(&self.id) else { return };
        // Source keeps the timeline's Hub participant/transition while replacing
        // only the local character with its predicted pose and correction.
        let character = &self.participant.wizard;
        participant.wizard.position = self.displayed_position(now_ms);
        participant.wizard.velocity = character.velocity;
        participant.wizard.gait_degrees = character.gait_degrees;
        participant.wizard.set_facing(character.heading_degrees);
        participant.wizard.walk_cycle_primary = character.walk_cycle_primary;
        output.collision_rng = self.collision_rng;
    }

    fn displayed_position(&self, now_ms: f64) -> Vec2 {
        let remaining = (1.0
            - (now_ms - self.correction_started_at_ms).max(0.0) / self.correction_duration_ms)
            .max(0.0);
        Vec2::new(
            self.participant.wizard.position.x + self.correction.x * remaining,
            self.participant.wizard.position.y + self.correction.y * remaining,
        )
    }
}

fn predict_character(participant: &mut SessionParticipant, input: GameSessionInput, rng: &mut u32) {
    let waiting = participant.college_intro.as_ref().is_some_and(|intro| {
        intro.phase == NativeCollegeIntroPhase::CourtyardWalk && intro.title_cursor == 0.0
    });
    let movement = plan_hub_participant_movement(
        &participant.wizard,
        participant.college_intro.as_ref(),
        !waiting,
    );
    let plan = movement.plan.unwrap_or_else(|| {
        plan_hub_wizard_tick(
            participant.wizard.velocity,
            input.movement,
            participant.player.combat.movement_scale()
                * participant.player.derived_stats().movement_factor,
        )
    });
    let position = if movement.plan.is_some() {
        resolve_scripted(
            &participant.wizard,
            plan.delta,
            participant.player.economy.college_intro_pending,
        )
    } else {
        let moved = move_with_segments(
            participant.wizard.position,
            plan.delta,
            PLAYER_CHARACTER_RADIUS,
            *rng,
            participant.wizard.region.segments(),
        );
        *rng = moved.rng_state;
        moved.position
    };
    participant.wizard.commit(&plan, position);
    step_hub_participant_movement(
        &mut participant.wizard,
        &mut participant.college_intro,
        movement.college_target,
        HubParticipantStepOptions {
            college_intro_pending: participant.player.economy.college_intro_pending,
            renderer_ready: !waiting,
        },
    );
}

fn resolve_scripted(wizard: &solomon_sim::HubWizard, delta: Vec2, college_pending: bool) -> Vec2 {
    let mut bodies = vec![ActorPhysicsBody {
        collision_recipient: true,
        id: "player-local".into(),
        position: wizard.position,
        delta,
        radius: PLAYER_CHARACTER_RADIUS,
        push_resistance: 10.0,
        push_strength: 12.0,
        driven: true,
        push_enabled: true,
    }];
    if wizard.region == HubRegion::Courtyard {
        for (id, position, radius) in [
            ("perk-witch", Vec2::new(1340.0, 280.0), 15.0),
            ("potion-trader", Vec2::new(1397.0, 664.0), 30.0),
            ("annalist", Vec2::new(895.5, 455.5), 8.0),
            ("items-trader", Vec2::new(1700.5, 449.5), 25.0),
            ("teacher", Vec2::new(576.5, 710.5), 25.0),
        ] {
            append_fixed(&mut bodies, id, position, radius);
        }
    } else {
        for actor in wizard.region.room_actors() {
            append_fixed(&mut bodies, actor.id, actor.position, actor.radius);
        }
        for prop in wizard.region.room_props() {
            append_fixed(&mut bodies, prop.id, prop.position, prop.radius);
        }
    }
    if college_pending && wizard.region == HubRegion::Office {
        append_fixed(&mut bodies, "story-office-polisher", Vec2::new(566.0, 735.0), 15.0);
    }
    resolve_actor_motion(&mut bodies, &mut ScriptedWorld, |_, _| true, None);
    bodies[0].position
}

fn append_fixed(bodies: &mut Vec<ActorPhysicsBody>, id: &str, position: Vec2, radius: f64) {
    bodies.push(ActorPhysicsBody {
        collision_recipient: true,
        id: id.into(),
        position,
        delta: Vec2::ZERO,
        radius,
        push_resistance: 90.0,
        push_strength: 0.0,
        driven: false,
        push_enabled: true,
    });
}

struct ScriptedWorld;
impl ActorPhysicsWorld for ScriptedWorld {
    fn can_place(&self, _id: &str, _position: Vec2, _radius: f64) -> bool {
        true
    }
    fn move_body(&mut self, _id: &str, position: Vec2, delta: Vec2, _radius: f64) -> Vec2 {
        Vec2::new(position.x + delta.x, position.y + delta.y)
    }
}
