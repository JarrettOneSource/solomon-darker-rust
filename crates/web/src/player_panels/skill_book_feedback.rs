//! `skill-book-feedback.ts` and `use-skill-book-feedback.ts` at fa08ae24.
//! The accepted rank receipt owns a result that outlives the Inventory close.
use super::{PanelEvent, PlayerPanel, PlayerPanels, Target};
use crate::gpu_ui::UiFrame;
use solomon_sim::Vec2;
use solomon_sim::audio::{AudioCue, AudioEvent};
use solomon_sim::economy::{
    HubActionFeedback, NativeSkillBookFeedbackCursor, NativeSkillBookOutcome,
    native_skill_book_result_text,
};
use solomon_sim::native_ui_text::messages::{
    NativeUiMessageDataLineSpec, NativeUiSingleActionMessageLayout,
    layout_native_ui_single_action_message,
};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec};

pub(super) struct SkillBookFeedback {
    cursor: NativeSkillBookFeedbackCursor,
    result: Option<SkillBookResult>,
}
struct SkillBookResult {
    sequence: u64,
    skill_id: usize,
    text: String,
    pause_panel: PlayerPanel,
}
impl SkillBookFeedback {
    pub fn new() -> Self {
        Self { cursor: NativeSkillBookFeedbackCursor::new(None), result: None }
    }
    fn reset(&mut self, feedback: Option<&HubActionFeedback>) {
        self.cursor = NativeSkillBookFeedbackCursor::new(feedback);
        self.result = None;
    }
    fn dismiss(&mut self) -> bool {
        self.result.take().is_some()
    }
    fn observe(
        &mut self,
        feedback: Option<&HubActionFeedback>,
        pause_panel: PlayerPanel,
    ) -> Result<bool, String> {
        let Some(receipt) = self.cursor.consume(feedback) else { return Ok(false) };
        self.result = match receipt.outcome {
            NativeSkillBookOutcome::Rank { skill_id: Some(skill_id) } => Some(SkillBookResult {
                sequence: receipt.sequence,
                skill_id,
                text: native_skill_book_result_text(skill_id)?,
                pause_panel,
            }),
            NativeSkillBookOutcome::Choice | NativeSkillBookOutcome::Rank { skill_id: None } => {
                None
            }
        };
        Ok(true)
    }
}

impl PlayerPanels {
    /// A session/world boundary seeds its retained receipt; loading never replays it.
    pub fn reset_skill_book_feedback(&mut self, feedback: Option<&HubActionFeedback>) {
        self.skill_book_feedback.reset(feedback);
        self.audio_events.push(AudioEvent::StopStream(AudioCue::MagicBookGet));
    }
    pub fn observe_skill_book_feedback(
        &mut self,
        feedback: Option<&HubActionFeedback>,
        now: f64,
    ) -> Result<(), String> {
        let prior_pause = self
            .pause_panel()
            .filter(|panel| {
                matches!(
                    panel,
                    PlayerPanel::Inventory | PlayerPanel::Skills | PlayerPanel::SkillSelector(_)
                )
            })
            .unwrap_or(PlayerPanel::Inventory);
        if self.skill_book_feedback.observe(feedback, prior_pause)? {
            self.sound(AudioCue::MagicBookGet);
            if self.panel == Some(PlayerPanel::Inventory) {
                self.close(now, None);
            }
        }
        Ok(())
    }
    pub fn skill_book_result_open(&self) -> bool {
        self.skill_book_feedback.result.is_some()
    }
    /// Keeps the original book's native pause source after its visual close.
    pub fn pause_panel(&self) -> Option<PlayerPanel> {
        self.skill_book_feedback.result.as_ref().map(|result| result.pause_panel).or(self.panel)
    }
    pub(super) fn apply_skill_book_result_event(&mut self, event: &PanelEvent) {
        let dismiss = match event {
            PanelEvent::Activate(Target::SkillBookResultOkay) => true,
            PanelEvent::Key(key) => !key.repeat && key.code == "Escape",
            _ => false,
        };
        if dismiss && self.skill_book_feedback.dismiss() {
            self.sound(AudioCue::Click);
            self.navigation.release();
        }
    }
    pub(super) fn paint_skill_book_result(
        &mut self,
        frame: &mut UiFrame<'_>,
    ) -> Result<(), String> {
        let Some(result) = &self.skill_book_feedback.result else { return Ok(()) };
        let layout = result_layout(&result.text)?;
        let scope = format!("skill-book-result:{}", result.sequence);
        let value =
            serde_json::json!({"skillId":result.skill_id,"sequence":result.sequence}).to_string();
        if !self.input_suspended {
            self.navigation.focus(frame.ctx, &scope, Some("panel:skill-book-result-okay"));
        }
        frame.announce(&format!("Skill improved. {}. OKAY.", result.text));
        let scale = (self.viewport_size.0 as f32 / 1600.0).min(self.viewport_size.1 as f32 / 900.0);
        self.modal_canvas(
            frame,
            &scope,
            scale,
            egui::pos2(
                (self.viewport_size.0 as f32 - 1600.0 * scale) / 2.0,
                (self.viewport_size.1 as f32 - 900.0 * scale) / 2.0,
            ),
            0.75,
            |panels, c| {
                crate::gpu_ui::message_frame(
                    c.frame,
                    &c.painter,
                    &panels.ui,
                    c.rect(layout.frame_bounds),
                )?;
                c.semantic_value(
                    "panel:skill-book-result",
                    "Skill improved",
                    "dialog",
                    layout.frame_bounds,
                    &value,
                );
                for (index, line) in layout.lines.iter().enumerate() {
                    c.labeled_text(
                        if index == 0 {
                            "panel:skill-book-result-title"
                        } else {
                            "panel:skill-book-result-text"
                        },
                        &NativeUiTextSpec {
                            align: NativeUiTextAlign::Left,
                            tint: line.tint,
                            line_height: Some(line.line_height),
                            ..NativeUiTextSpec::new(line.font, &line.text, line.x, line.baseline_y)
                        },
                    );
                }
                let r = layout.action_bounds;
                let button =
                    c.native_button_art(&panels.ui, "skill-book-result-okay", "OKAY", r)?;
                button.text(&NativeUiTextSpec {
                    tint: 0xd9ba70,
                    ..NativeUiTextSpec::new(
                        NativeUiFontName::Menu,
                        "OKAY",
                        r.width / 2.0,
                        layout.action_text_baseline_y - r.y,
                    )
                });
                Ok(())
            },
        )
    }
}

fn result_layout(text: &str) -> Result<NativeUiSingleActionMessageLayout, String> {
    layout_native_ui_single_action_message(
        Vec2::new(800.0, 450.0),
        &[
            NativeUiMessageDataLineSpec {
                font: NativeUiFontName::Menu,
                text: "Skill improved",
                gap_after: 10.0,
                tint: 0xb2b2ff,
            },
            NativeUiMessageDataLineSpec {
                font: NativeUiFontName::Medium,
                text,
                gap_after: 0.0,
                tint: 0xb2b2ff,
            },
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use solomon_sim::economy::HubActionFeedbackAction;

    fn receipt(sequence: u64, outcome: NativeSkillBookOutcome) -> HubActionFeedback {
        HubActionFeedback {
            accepted: true,
            action: HubActionFeedbackAction::ReadSkillBook,
            dowsing_pitch: None,
            dye_session_id: None,
            reason: None,
            sequence,
            skill_book_outcome: Some(outcome),
            transfer_direction: None,
            transfer_gesture: None,
            unforge_outcome: None,
        }
    }

    #[test]
    fn result_retains_its_book_pause_until_dismiss_and_does_not_replay() {
        let restored = receipt(7, NativeSkillBookOutcome::Rank { skill_id: Some(32) });
        let mut feedback = SkillBookFeedback::new();
        feedback.reset(Some(&restored));
        assert!(!feedback.observe(Some(&restored), PlayerPanel::Inventory).unwrap());
        let accepted = receipt(8, NativeSkillBookOutcome::Rank { skill_id: Some(35) });
        assert!(feedback.observe(Some(&accepted), PlayerPanel::Skills).unwrap());
        let result = feedback.result.as_ref().unwrap();
        assert_eq!(result.pause_panel, PlayerPanel::Skills);
        assert_eq!(result.skill_id, 35);
        assert_eq!(result.sequence, 8);
        assert!(!feedback.observe(Some(&accepted), PlayerPanel::Inventory).unwrap());
        assert_eq!(feedback.result.as_ref().unwrap().pause_panel, PlayerPanel::Skills);
        assert!(feedback.dismiss());
        assert!(!feedback.dismiss());
        assert!(!feedback.observe(Some(&accepted), PlayerPanel::Inventory).unwrap());
        assert!(feedback.result.is_none());
        feedback.reset(Some(&accepted));
        assert!(!feedback.observe(Some(&accepted), PlayerPanel::Inventory).unwrap());
        assert!(feedback.result.is_none());
    }

    #[test]
    fn choice_and_exhausted_rank_play_feedback_without_a_result() {
        let mut feedback = SkillBookFeedback::new();
        for (sequence, outcome) in [
            (1, NativeSkillBookOutcome::Rank { skill_id: Some(32) }),
            (2, NativeSkillBookOutcome::Choice),
            (3, NativeSkillBookOutcome::Rank { skill_id: Some(35) }),
            (4, NativeSkillBookOutcome::Rank { skill_id: None }),
        ] {
            assert!(
                feedback
                    .observe(Some(&receipt(sequence, outcome)), PlayerPanel::Inventory)
                    .unwrap()
            );
            assert_eq!(
                feedback.result.is_some(),
                matches!(outcome, NativeSkillBookOutcome::Rank { skill_id: Some(_) })
            );
        }
    }

    #[test]
    fn all_native_rank_results_match_the_unmodified_source_layout() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../sim/tests/fixtures/website/skill-book-feedback-goldens.json"
        ))
        .unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 74);
        for case in cases {
            let skill = case["skillId"].as_u64().unwrap() as usize;
            let text = native_skill_book_result_text(skill).unwrap();
            assert_eq!(text, case["text"].as_str().unwrap());
            let actual = result_layout(&text).unwrap();
            let expected = &case["layout"];
            for (rect, name) in [
                (actual.frame_bounds, "frameBounds"),
                (actual.panel_bounds, "panelBounds"),
                (actual.action_bounds, "actionBounds"),
            ] {
                assert_eq!(
                    [rect.x, rect.y, rect.width, rect.height],
                    [
                        expected[name]["left"].as_f64().unwrap(),
                        expected[name]["top"].as_f64().unwrap(),
                        expected[name]["width"].as_f64().unwrap(),
                        expected[name]["height"].as_f64().unwrap()
                    ],
                    "skill {skill} {name}"
                );
            }
            assert_eq!(
                actual.action_text_baseline_y,
                expected["actionTextBaselineY"].as_f64().unwrap()
            );
            assert_eq!(actual.lines.len(), expected["lines"].as_array().unwrap().len());
            for (line, source) in actual.lines.iter().zip(expected["lines"].as_array().unwrap()) {
                assert_eq!(line.font.as_str(), source["font"].as_str().unwrap());
                assert_eq!(line.text, source["text"].as_str().unwrap());
                assert_eq!(line.tint, source["tint"].as_u64().unwrap() as u32);
                assert_eq!(
                    [
                        line.x,
                        line.baseline_y,
                        line.gap_after,
                        line.height,
                        line.line_height,
                        line.width
                    ],
                    [
                        source["x"].as_f64().unwrap(),
                        source["baselineY"].as_f64().unwrap(),
                        source["gapAfter"].as_f64().unwrap(),
                        source["height"].as_f64().unwrap(),
                        source["lineHeight"].as_f64().unwrap(),
                        source["width"].as_f64().unwrap()
                    ]
                );
            }
        }
    }
}
