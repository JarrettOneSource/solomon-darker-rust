//! Participant construction and checkpoint records. Live actors separate identity,
//! character transactions, locomotion, Hub state, lighting, score and feedback.

mod storage;
pub use storage::{ParticipantMut, ParticipantRead, ParticipantRef, Participants};

use crate::HubWizard;
use crate::Vec2;
use crate::economy::HubEconomyState;
use crate::hub::college_intro::NativeCollegeIntroState;
use crate::hub::interaction::HubNpcMarkerEpoch;
use crate::player::PlayerState;
use crate::score::NativeHallOfFameRunState;
use crate::screen_feedback::NativeSecondaryScreenFeedbackPresentation;

use super::PlayerLightingState;

/// Owned boundary record for construction, saves, detach and private prediction.
/// Live session participants are queried through `ParticipantRef` and `ParticipantMut`.
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
        self.view().selected_primary_id()
    }

    pub(super) fn synchronize_primary_selection(&mut self, previous: u32) {
        let selected = self.selected_primary_id();
        if selected != previous {
            self.wizard.primary_cast.reset_for_selection(selected);
        }
    }
}
