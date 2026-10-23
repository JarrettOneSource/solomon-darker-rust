//! The pinned title confirmations retain the stock message art and modal choices.
use super::widgets::{self, Stage};
use crate::gpu_ui::{StockAtlas, UiFrame, message_frame};
use egui::{Align2, Color32, Rect, Ui, Vec2};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TitlePromptKind {
    Tutorial,
    KillWizard,
}
pub(super) struct TitlePrompt {
    pub kind: Option<TitlePromptKind>,
    atlas: StockAtlas,
}
impl TitlePrompt {
    pub async fn new() -> Result<Self, String> {
        Ok(Self { kind: None, atlas: StockAtlas::load("UI").await? })
    }
    pub fn show(&mut self, kind: TitlePromptKind) -> Result<(), String> {
        self.kind = Some(kind);
        Ok(())
    }
    pub fn hide(&mut self) {
        self.kind = None;
    }
    pub fn paint(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &Ui,
        view: Rect,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        let Some(kind) = self.kind else { return Ok(()) };
        ui.painter().rect_filled(view, 0.0, Color32::from_black_alpha(187));
        let native = (view.width() / 1600.0).min(view.height() / 900.0);
        let scale = if native < 44.0 / 69.0 {
            ((view.width() - 24.0) / 500.0).min((view.height() - 24.0) / 500.0)
        } else {
            native
        };
        let stage = Stage { origin: view.center() - Vec2::new(250.0, 182.0) * scale, scale };
        message_frame(frame, ui.painter(), &self.atlas, stage.rect(0.0, 0.0, 500.0, 362.0))?;
        let (title, body) = match kind {
            TitlePromptKind::Tutorial => (
                "PLAY THE TUTORIAL?",
                "Learn the controls and confront\nSolomon Dark before beginning your\nfirst game.",
            ),
            TitlePromptKind::KillWizard => (
                "Kill character?",
                "Starting a new game will kill off your\ncurrent game and character (Lucritius\nwill scavenge his equipment)!\nAre you sure you want to do this?",
            ),
        };
        widgets::label(
            ui,
            title,
            stage.at(76.0, 71.0),
            Align2::LEFT_TOP,
            "Palatino",
            24.0 * scale,
            Color32::WHITE,
        );
        widgets::label(
            ui,
            body,
            stage.at(76.0, 113.5),
            Align2::LEFT_TOP,
            "Palatino",
            17.0 * scale,
            Color32::WHITE,
        );
        for (text, action, x) in
            [("YES", "prompt-primary", 45.0), ("NO", "prompt-secondary", 261.0)]
        {
            if widgets::stock_button(
                frame,
                ui,
                action,
                text,
                stage.rect(x, 216.0, 200.0, 69.0),
                scale,
                true,
            )
            .clicked()
            {
                actions.push(action.to_owned());
            }
        }
        Ok(())
    }
}
