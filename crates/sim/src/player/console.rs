//! Source developer acquisitions from `player-entity-store.ts`1700–1773.

use super::*;

impl PlayerState {
    pub fn grant_skill_ranks(&mut self, skill_id: usize, ranks: u32, rng: &mut NativeRng) {
        let previous = self.book.clone();
        self.book.grant_ranks(skill_id, ranks);
        let acquired =
            self.book.permanent_ranks[skill_id].saturating_sub(previous.permanent_ranks[skill_id]);
        self.progression.apply_acquisition_offer_seeds(rng, acquired as usize);
        if self.book == previous {
            return;
        }
        self.refresh(&previous);
        self.refresh_console_skill_offer(rng);
    }

    pub fn grant_weld_build(&mut self, build_id: u32, rng: &mut NativeRng) {
        let previous = self.book.clone();
        self.book.grant_weld_build(build_id);
        let mut acquired = 0;
        for (next, old) in self.book.permanent_ranks.iter().zip(&previous.permanent_ranks) {
            acquired += next.saturating_sub(*old) as usize;
        }
        self.progression.apply_acquisition_offer_seeds(rng, acquired);
        self.refresh(&previous);
        self.refresh_console_skill_offer(rng);
    }

    fn refresh_console_skill_offer(&mut self, rng: &mut NativeRng) {
        let Some(offer) = &self.progression.pending_offer else {
            return;
        };
        self.progression.pending_offer = Some(crate::profile::build_player_skill_offer(
            &self.progression,
            &self.book,
            self.combat.maximum_mana,
            offer.sequence,
            rng,
        ));
        self.progression.revision += 1;
        self.mark_new_offer_insight(None, rng);
    }
}
