//! One authoritative participant. Its character has one locomotion owner in every world;
//! the shared session owns enemies, spells, RNG streams and the painter allocator.

use crate::HubWizard;
use crate::Vec2;
use crate::economy::HubEconomyState;
use crate::hub::college_intro::NativeCollegeIntroState;
use crate::hub::interaction::HubNpcMarkerEpoch;
use crate::player::PlayerState;
use crate::score::NativeHallOfFameRunState;
use crate::screen_feedback::NativeSecondaryScreenFeedbackPresentation;

use super::PlayerLightingState;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SessionParticipant {
    pub id: String,
    /// Source PlayerEntityStore ID, distinct from the stable connection/player ID.
    #[cfg_attr(feature = "persistence", serde(default))]
    pub entity_id: u64,
    // PlayerState has large fixed native tables. Heap ownership keeps nested wasm
    // save visitors from multiplying those tables through every enclosing stack frame.
    pub player: Box<PlayerState>,
    pub wizard: HubWizard,
    pub lighting: PlayerLightingState,
    pub college_intro: Option<NativeCollegeIntroState>,
    pub marker_epoch: HubNpcMarkerEpoch,
    pub score: NativeHallOfFameRunState,
    pub screen_feedback: NativeSecondaryScreenFeedbackPresentation,
    pub world_aim: Option<Vec2>,
    pub tutorial_profile_economy: Option<Box<HubEconomyState>>,
}

impl SessionParticipant {
    pub(super) fn selected_primary_id(&self) -> u32 {
        if self.player.book.primary_skill_id == 52 {
            self.player.book.weld_build_id.expect("selected Weld has its build")
        } else {
            self.player.book.primary_skill_id as u32
        }
    }

    pub(super) fn synchronize_primary_selection(&mut self, previous: u32) {
        let selected = self.selected_primary_id();
        if selected != previous {
            self.wizard.primary_cast.reset_for_selection(selected);
        }
    }
}
