//! Pinned fa08 `skill-book-feedback.ts`: a retained receipt is not a new outcome.
use solomon_sim::economy::{
    HubActionFeedback, HubActionFeedbackAction, NativeSkillBookFeedbackCursor,
    NativeSkillBookOutcome, native_skill_book_result_text,
};

fn receipt(
    sequence: u64,
    accepted: bool,
    action: HubActionFeedbackAction,
    outcome: Option<NativeSkillBookOutcome>,
) -> HubActionFeedback {
    HubActionFeedback {
        sequence,
        accepted,
        action,
        skill_book_outcome: outcome,
        dowsing_pitch: None,
        dye_session_id: None,
        reason: None,
        transfer_direction: None,
        transfer_gesture: None,
        unforge_outcome: None,
    }
}

#[test]
fn retained_rejected_and_unrelated_book_receipts_never_replay() {
    let rank = Some(NativeSkillBookOutcome::Rank { skill_id: Some(8) });
    let initial = receipt(3, true, HubActionFeedbackAction::ReadSkillBook, rank);
    let mut cursor = NativeSkillBookFeedbackCursor::new(Some(&initial));
    assert_eq!(cursor.consume(Some(&initial)), None);
    assert_eq!(cursor.consume(None), None);
    for (sequence, action, accepted) in [
        (4, HubActionFeedbackAction::ReadSkillBook, false),
        (5, HubActionFeedbackAction::Consume, true),
    ] {
        assert_eq!(cursor.consume(Some(&receipt(sequence, accepted, action, rank))), None);
        assert_eq!(
            cursor.consume(Some(&receipt(
                sequence,
                true,
                HubActionFeedbackAction::ReadSkillBook,
                rank
            ))),
            None
        );
    }
    let choice = cursor
        .consume(Some(&receipt(
            6,
            true,
            HubActionFeedbackAction::ReadSkillBook,
            Some(NativeSkillBookOutcome::Choice),
        )))
        .unwrap();
    assert_eq!(choice.sequence, 6);
    assert_eq!(choice.outcome, NativeSkillBookOutcome::Choice);
    let no_rank = cursor
        .consume(Some(&receipt(
            7,
            true,
            HubActionFeedbackAction::ReadSkillBook,
            Some(NativeSkillBookOutcome::Rank { skill_id: None }),
        )))
        .unwrap();
    assert_eq!(no_rank.outcome, NativeSkillBookOutcome::Rank { skill_id: None });
    let accepted = receipt(8, true, HubActionFeedbackAction::ReadSkillBook, rank);
    assert_eq!(cursor.consume(Some(&accepted)).unwrap().outcome, rank.unwrap());
    assert_eq!(cursor.consume(Some(&accepted)), None);
}

#[test]
fn book_result_text_uses_the_exact_native_catalog_and_source_range() {
    assert_eq!(native_skill_book_result_text(8).unwrap(), "Magic Missile +1");
    assert_eq!(native_skill_book_result_text(80).unwrap(), "Plane Orb +1");
    assert_eq!(native_skill_book_result_text(81).unwrap(), "Reserved +1");
    for id in [0, 7, 82, usize::MAX] {
        assert_eq!(
            native_skill_book_result_text(id).unwrap_err(),
            "Book result must name a native rank-award skill"
        );
    }
}
