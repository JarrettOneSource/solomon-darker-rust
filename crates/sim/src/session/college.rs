//! Participant College admission and loadout commands from `game-simulation.ts`.

use super::*;
use crate::game_run::GameRunPhase;
use crate::hub::college_intro::{
    NativeCollegeIntroState, acknowledge_hub_college_intro_dialogue,
    confirm_hub_college_intro_loadout, hub_college_admission_primary_unset,
    plan_hub_college_intro_arm,
};
use crate::hub::regions::HubRegion;
use crate::profile::NewGameSelection;

impl GameSession {
    pub fn arm_college_intro(&mut self, id: &str) -> bool {
        if self.boneyard.is_some() || self.run.phase != GameRunPhase::Hub {
            return false;
        }
        let Some(participant) = self.participants.get_mut(id) else {
            return false;
        };
        let selected =
            participant.wizard.primary_cast.selected_primary_id.map_or(-1, |id| id as i32);
        let Some(plan) = plan_hub_college_intro_arm(
            &participant.wizard,
            participant.college_intro.as_ref(),
            participant.player.economy.college_intro_pending,
            participant.player.progression.offer_seed,
            selected,
        ) else {
            return false;
        };
        if participant.player.economy.apply_starter_appearance(plan.appearance) {
            participant.player.refresh_economy();
        }
        if let Some(spawn) = plan.spawn {
            participant.wizard = HubWizard::at(spawn.position);
            participant.wizard.set_facing(spawn.heading_degrees);
            participant.college_intro = Some(NativeCollegeIntroState::new());
        }
        if plan.reset_primary_cast {
            participant.wizard.primary_cast = Default::default();
        }
        true
    }

    pub fn acknowledge_college_intro_dialogue(&mut self, id: &str) -> bool {
        if self.boneyard.is_some() || self.level_up_barrier.is_some() {
            return false;
        }
        let Some(participant) = self.participants.get_mut(id) else {
            return false;
        };
        if participant.wizard.transition.is_some() {
            return false;
        }
        acknowledge_hub_college_intro_dialogue(&mut participant.college_intro)
    }

    pub fn decline_tutorial(&mut self, id: &str) -> bool {
        if self.boneyard.is_some() || self.run.phase != GameRunPhase::Hub {
            return false;
        }
        let Some(participant) = self.participants.get_mut(id) else {
            return false;
        };
        if !participant.player.economy.tutorial_pending
            || !participant.player.economy.college_intro_pending
            || participant.wizard.region != HubRegion::Courtyard
            || participant.wizard.transition.is_some()
            || participant.college_intro.is_some()
        {
            return false;
        }
        participant.player.economy.college_intro_pending = false;
        participant.player.economy.tutorial_pending = false;
        participant.player.economy.revision += 1;
        true
    }

    pub(super) fn hub_primary_unset(&self, id: &str) -> bool {
        self.boneyard.is_none()
            && self.participants.get(id).is_some_and(|participant| {
                hub_college_admission_primary_unset(
                    &participant.wizard,
                    participant.college_intro.as_ref(),
                    participant.player.economy.college_intro_pending,
                )
            })
    }

    pub fn confirm_loadout(&mut self, id: &str, selection: NewGameSelection) -> bool {
        if self.boneyard.is_some() {
            return false;
        }
        let Some(participant) = self.participants.get_mut(id) else {
            return false;
        };
        if self.run.phase == GameRunPhase::Loadout {
            if !self.run.confirm_post_run_loadout(id) {
                return false;
            }
            let owner = selection.element.into();
            let seed =
                crate::profile::draw_native_player_creation_offer_seed(&mut self.gameplay_rng);
            participant.player.replace_loadout(selection, seed, Some(owner));
            participant.player.refresh_hagatha_skill_effects(&mut self.gameplay_rng);
            reset_loadout_character(participant);
            return true;
        }
        if !confirm_hub_college_intro_loadout(&mut participant.wizard) {
            return false;
        }
        let seed = crate::profile::draw_native_player_creation_offer_seed(&mut self.gameplay_rng);
        participant.player.replace_loadout(selection, seed, None);
        participant.player.refresh_hagatha_skill_effects(&mut self.gameplay_rng);
        participant.player.economy.college_intro_pending = false;
        participant.player.economy.tutorial_pending = false;
        participant.player.economy.revision += 1;
        reset_loadout_character(participant);
        true
    }
}

fn reset_loadout_character(participant: &mut SessionParticipant) {
    participant.wizard = HubWizard {
        region: participant.wizard.region,
        transition: participant.wizard.transition,
        ..HubWizard::at(participant.wizard.position)
    };
    // replacePlayerLoadout installs its selected primary before the next cast epoch.
    super::lifecycle::reset_selected_primary(&mut participant.wizard, &participant.player);
}
