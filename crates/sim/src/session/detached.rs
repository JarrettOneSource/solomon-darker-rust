//! Detached skill transactions from the public Game API. Only one participant
//! is staged; enemies, scenery, spells and the running party remain borrowed.

use super::membership::{import_detached_character, synchronize_detached_player};
use super::snapshot::GameSnapshotParticipants;
use super::*;
use crate::profile::{PlayerSkillChoice, SharedPlayerLevelMilestone};

pub struct DetachedSessionProjection {
    pub participant: SessionParticipant,
    pub(super) next_entity_id: u64,
    pub(super) gameplay_rng: NativeRng,
    painter_order: WorldPainterOrder,
    participant_order: Vec<String>,
    run: GameRunLifecycle,
    level_up_barrier: Option<PlayerLevelUpBarrier>,
    next_level_up_barrier_id: u64,
}

enum DetachedSkillCommand {
    Choose(PlayerSkillChoice),
    Reroll(u64),
    Defer(u64),
}

impl GameSession {
    pub fn project_detached(
        &self,
        detached: &DetachedSessionParticipant,
    ) -> Result<DetachedSessionProjection, String> {
        self.stage_detached(detached, None)
    }

    /// The returned JSON is an ordinary GameCheckpoint with the rejoining
    /// participant visible. The live Session and all of its RNG words stay put.
    pub fn snapshot_with_detached<'a>(
        &'a self,
        projected: &'a DetachedSessionProjection,
    ) -> GameSnapshot<'a> {
        let mut snapshot = self.snapshot();
        snapshot.participants = GameSnapshotParticipants {
            retained: &self.participants,
            detached: Some(&projected.participant),
        };
        snapshot.participant_order = &projected.participant_order;
        snapshot.next_entity_id = &projected.next_entity_id;
        snapshot.gameplay_rng = &projected.gameplay_rng;
        snapshot.painter_order = &projected.painter_order;
        snapshot.run = &projected.run;
        snapshot.level_up_barrier = &projected.level_up_barrier;
        snapshot.next_level_up_barrier_id = &projected.next_level_up_barrier_id;
        snapshot
    }

    pub fn choose_detached_skill(
        &mut self,
        detached: &mut DetachedSessionParticipant,
        choice: PlayerSkillChoice,
    ) -> Result<bool, String> {
        self.transact_detached_skill(detached, DetachedSkillCommand::Choose(choice))
    }

    pub fn reroll_detached_skill_offer(
        &mut self,
        detached: &mut DetachedSessionParticipant,
        sequence: u64,
    ) -> Result<bool, String> {
        self.transact_detached_skill(detached, DetachedSkillCommand::Reroll(sequence))
    }

    pub fn defer_detached_skill_choice(
        &mut self,
        detached: &mut DetachedSessionParticipant,
        sequence: u64,
    ) -> Result<bool, String> {
        self.transact_detached_skill(detached, DetachedSkillCommand::Defer(sequence))
    }

    fn transact_detached_skill(
        &mut self,
        detached: &mut DetachedSessionParticipant,
        command: DetachedSkillCommand,
    ) -> Result<bool, String> {
        let mut projected = self.stage_detached(detached, None)?;
        let previous = projected.participant.selected_primary_id();
        let player = &mut projected.participant.player;
        let accepted = match command {
            DetachedSkillCommand::Choose(choice) => {
                player.choose_skill(choice, &mut projected.gameplay_rng)
            }
            DetachedSkillCommand::Reroll(sequence) => {
                player.reroll_skill_offer(sequence, &mut projected.gameplay_rng)
            }
            DetachedSkillCommand::Defer(sequence) => {
                player.defer_skill_choice(sequence, &mut projected.gameplay_rng)
            }
        };
        if accepted {
            projected.participant.synchronize_primary_selection(previous);
            self.gameplay_rng = projected.gameplay_rng;
            detached.participant = projected.participant;
            detached.next_entity_id = projected.next_entity_id;
        }
        // Rejected stale/invalid choices discard the complete staged draw sequence.
        Ok(accepted)
    }

    pub(super) fn stage_detached(
        &self,
        detached: &DetachedSessionParticipant,
        milestone: Option<&SharedPlayerLevelMilestone>,
    ) -> Result<DetachedSessionProjection, String> {
        self.validate_rejoin(detached, milestone)?;
        let mut participant = detached.participant.clone();
        participant.entity_id = self.next_entity_id;
        let mut painter_order = self.painter_order;
        let mut gameplay_rng = self.gameplay_rng.clone();
        import_detached_character(
            &mut participant,
            self.boneyard.as_ref().unwrap(),
            &mut painter_order,
        );
        if let Some(milestone) = milestone
            && !milestone.crossed_levels.is_empty()
        {
            synchronize_detached_player(&mut participant, milestone, &mut gameplay_rng)?;
        }
        if participant.player.economy.npc.boast.uses_random_skill_choices()
            && let Some(offer) = &participant.player.progression.pending_offer
            && offer.automatic_choice_index.is_none()
        {
            let choice = gameplay_rng.draw_integer(offer.options.len() as i32) as usize;
            assert!(participant.player.progression.set_automatic_choice(choice));
        }
        let (barrier, next_id) = self.project_rejoin_barrier(&participant, milestone);
        let mut participant_order = self.participant_order.clone();
        participant_order.push(participant.id.clone());
        let mut run = self.run.clone();
        run.synchronize_participants(&participant_order);
        Ok(DetachedSessionProjection {
            participant,
            next_entity_id: self.next_entity_id + 1,
            gameplay_rng,
            painter_order,
            participant_order,
            run,
            level_up_barrier: barrier,
            next_level_up_barrier_id: next_id,
        })
    }
}
