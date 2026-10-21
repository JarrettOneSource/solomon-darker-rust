//! A rejoining wizard with pending offers stays detached until those choices finish.
//! Source: `host/game-host.ts` staged-party-rejoin select/level-up branches.

use solomon_protocol::GameCommand;
use solomon_sim::session::DetachedSessionParticipant;

use super::authority::NativeAuthority;
use super::persistence::SavedParticipant;

impl NativeAuthority {
    pub(super) fn staged_detached(&self, id: &str) -> Option<&DetachedSessionParticipant> {
        let slot = self.metadata.resume_slots.get(id)?;
        if slot.peer_id.is_none()
            || self.session.as_ref().is_some_and(|session| session.participant(id).is_some())
        {
            return None;
        }
        match &slot.saved {
            Some(SavedParticipant::Run { detached }) => Some(detached),
            _ => None,
        }
    }

    pub(super) fn apply_detached_command(
        &mut self,
        id: &str,
        command: GameCommand,
    ) -> Result<bool, String> {
        let session = self.session.as_mut().expect("a staged rejoin has its retained world");
        let Some(SavedParticipant::Run { detached }) =
            self.metadata.resume_slots.get_mut(id).and_then(|slot| slot.saved.as_mut())
        else {
            return Err("The staged rejoin has no detached participant".into());
        };
        match command {
            GameCommand::ChooseSkill { choice } => session.choose_detached_skill(detached, choice),
            GameCommand::RerollSkillOffer { offer_sequence } => {
                session.reroll_detached_skill_offer(detached, offer_sequence)
            }
            GameCommand::DeferSkillChoice { offer_sequence } => {
                session.defer_detached_skill_choice(detached, offer_sequence)
            }
            _ => Ok(false),
        }
    }

    pub(super) fn finish_staged_rejoin(&mut self, id: &str) -> Result<(), String> {
        let Some(detached) = self.staged_detached(id) else { return Ok(()) };
        if detached.participant.player.progression.pending_offer.is_some() {
            return Ok(());
        }
        // Copy one participant for the infrequent admission transaction. A rejected
        // materialization must preserve the durable slot; the shared world is borrowed.
        let detached = detached.clone();
        self.session
            .as_mut()
            .expect("a staged rejoin retains its world")
            .rejoin_participant(detached, None)?;
        self.metadata.resume_slots.get_mut(id).expect("the staged slot remains retained").saved =
            None;
        self.social_dirty = true;
        Ok(())
    }
}
