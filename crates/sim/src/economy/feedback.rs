//! `HubActionFeedback` and its producer in `core-server/game-simulation.ts`.
//! The receipt belongs to the canonical economy and survives rejected transactions.
//! Service admission failures and NPC hint acknowledgements do not replace it.

use super::{
    HubEconomyRejection, HubEconomyResult, HubEconomyState, HubInventoryAction,
    NativeUnforgeOutcome, TransferDirection,
};
use crate::profile::NATIVE_SKILL_CATALOG;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum HubTransferGesture {
    DoubleActivation,
    #[default]
    Drag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum HubActionFeedbackAction {
    ActivateBeltSlot,
    BindBeltItem,
    BuyDowsing,
    BuyFomentius,
    BuyHagatha,
    BuyTeacherSpell,
    CloseDowsing,
    CloseDye,
    CloseHagatha,
    Consume,
    Dye,
    Dowse,
    Equip,
    InteractGoodie,
    MoveInventoryItem,
    OpenDye,
    ReadLibrarianBook,
    ReadSkillBook,
    RemoveHagatha,
    RenameSack,
    SelectBoast,
    Transfer,
    Unforge,
    Unequip,
}

impl HubActionFeedbackAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActivateBeltSlot => "activate-belt-slot",
            Self::BindBeltItem => "bind-belt-item",
            Self::BuyDowsing => "buy-dowsing",
            Self::BuyFomentius => "buy-fomentius",
            Self::BuyHagatha => "buy-hagatha",
            Self::BuyTeacherSpell => "buy-teacher-spell",
            Self::CloseDowsing => "close-dowsing",
            Self::CloseDye => "close-dye",
            Self::CloseHagatha => "close-hagatha",
            Self::Consume => "consume",
            Self::Dye => "dye",
            Self::Dowse => "dowse",
            Self::Equip => "equip",
            Self::InteractGoodie => "interact-goodie",
            Self::MoveInventoryItem => "move-inventory-item",
            Self::OpenDye => "open-dye",
            Self::ReadLibrarianBook => "read-librarian-book",
            Self::ReadSkillBook => "read-skill-book",
            Self::RemoveHagatha => "remove-hagatha",
            Self::RenameSack => "rename-sack",
            Self::SelectBoast => "select-boast",
            Self::Transfer => "transfer",
            Self::Unforge => "unforge",
            Self::Unequip => "unequip",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeSkillBookOutcome {
    Choice,
    Rank { skill_id: Option<usize> },
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubActionFeedback {
    pub accepted: bool,
    pub action: HubActionFeedbackAction,
    pub dowsing_pitch: Option<f64>,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub dye_session_id: Option<String>,
    pub reason: Option<HubEconomyRejection>,
    pub sequence: u64,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub skill_book_outcome: Option<NativeSkillBookOutcome>,
    pub transfer_direction: Option<TransferDirection>,
    pub transfer_gesture: Option<HubTransferGesture>,
    pub unforge_outcome: Option<NativeUnforgeOutcome>,
}

/// `skill-book-feedback.ts::NativeSkillBookFeedbackCursor`: retained receipts never replay.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeSkillBookFeedbackCursor {
    sequence: u64,
}

/// One newer accepted native book outcome, shared by the modal and world-text consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeSkillBookReceipt {
    pub sequence: u64,
    pub outcome: NativeSkillBookOutcome,
}

impl NativeSkillBookFeedbackCursor {
    #[must_use]
    pub fn new(initial: Option<&HubActionFeedback>) -> Self {
        Self { sequence: initial.map_or(0, |feedback| feedback.sequence) }
    }

    pub fn consume(
        &mut self,
        feedback: Option<&HubActionFeedback>,
    ) -> Option<NativeSkillBookReceipt> {
        let feedback = feedback?;
        if feedback.sequence <= self.sequence {
            return None;
        }
        self.sequence = feedback.sequence;
        if !feedback.accepted || feedback.action != HubActionFeedbackAction::ReadSkillBook {
            return None;
        }
        feedback
            .skill_book_outcome
            .map(|outcome| NativeSkillBookReceipt { sequence: feedback.sequence, outcome })
    }
}

/// `nativeSkillBookResultText`: only the authored rank-award rows 8..81 are valid.
pub fn native_skill_book_result_text(skill_id: usize) -> Result<String, String> {
    if !(8..=81).contains(&skill_id) {
        return Err("Book result must name a native rank-award skill".to_owned());
    }
    Ok(format!("{} +1", NATIVE_SKILL_CATALOG[skill_id].name))
}

impl HubEconomyState {
    pub(crate) fn record_action_feedback(
        &mut self,
        action: &HubInventoryAction,
        result: &HubEconomyResult,
    ) {
        let action_kind = match action {
            HubInventoryAction::AcknowledgeNpcHint { .. } | HubInventoryAction::Discard { .. } => {
                return;
            }
            HubInventoryAction::BuyFomentius { .. } => HubActionFeedbackAction::BuyFomentius,
            HubInventoryAction::BuyHagatha { .. } => HubActionFeedbackAction::BuyHagatha,
            HubInventoryAction::CloseHagatha => HubActionFeedbackAction::CloseHagatha,
            HubInventoryAction::RemoveHagatha { .. } => HubActionFeedbackAction::RemoveHagatha,
            HubInventoryAction::BuyDowsing { .. } => HubActionFeedbackAction::BuyDowsing,
            HubInventoryAction::Dowse { .. } => HubActionFeedbackAction::Dowse,
            HubInventoryAction::CloseDowsing => HubActionFeedbackAction::CloseDowsing,
            HubInventoryAction::Equip { .. } => HubActionFeedbackAction::Equip,
            HubInventoryAction::Unequip { .. } => HubActionFeedbackAction::Unequip,
            HubInventoryAction::EquipSackContents { .. } => {
                HubActionFeedbackAction::ActivateBeltSlot
            }
            HubInventoryAction::Transfer { .. } => HubActionFeedbackAction::Transfer,
            HubInventoryAction::Move { .. } => HubActionFeedbackAction::MoveInventoryItem,
            HubInventoryAction::RenameSack { .. } => HubActionFeedbackAction::RenameSack,
            HubInventoryAction::Consume { .. } => HubActionFeedbackAction::Consume,
            HubInventoryAction::ReadSkillBook { .. } => HubActionFeedbackAction::ReadSkillBook,
            HubInventoryAction::Dye { .. } => HubActionFeedbackAction::Dye,
            HubInventoryAction::OpenDye { .. } => HubActionFeedbackAction::OpenDye,
            HubInventoryAction::CloseDye { .. } => HubActionFeedbackAction::CloseDye,
            HubInventoryAction::Unforge { .. } => HubActionFeedbackAction::Unforge,
            HubInventoryAction::SelectBoast { .. } => HubActionFeedbackAction::SelectBoast,
            HubInventoryAction::ReadLibrarianBook { .. } => {
                HubActionFeedbackAction::ReadLibrarianBook
            }
            HubInventoryAction::BuyTeacherSpell { .. } => HubActionFeedbackAction::BuyTeacherSpell,
        };
        let (transfer_direction, transfer_gesture) = match action {
            HubInventoryAction::Transfer { direction, gesture, .. } => {
                (Some(*direction), Some(*gesture))
            }
            _ => (None, None),
        };
        self.action_feedback = Some(HubActionFeedback {
            accepted: result.accepted,
            action: action_kind,
            dowsing_pitch: result.dowsing_pitch,
            dye_session_id: match action {
                HubInventoryAction::OpenDye { session_id, .. }
                | HubInventoryAction::CloseDye { session_id }
                | HubInventoryAction::Dye { session_id, .. } => Some(session_id.clone()),
                _ => None,
            },
            reason: result.reason,
            sequence: self.action_feedback.as_ref().map_or(1, |feedback| feedback.sequence + 1),
            skill_book_outcome: None,
            transfer_direction,
            transfer_gesture,
            unforge_outcome: result.unforge_outcome.clone(),
        });
    }
}
