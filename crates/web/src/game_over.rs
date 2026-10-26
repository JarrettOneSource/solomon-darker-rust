//! Stock terminal-run overlay from `GameOverOverlay.tsx` and `boneyard.css`.
//! The session owns every clock and admission check; this view only emits Continue.

use crate::gpu_ui::{UiFrame, native_text};
use solomon_sim::Vec2;
use solomon_sim::game_run::presentation::{
    GameOverPresentation, SolomonRiffPresentation, game_over_presentation,
    solomon_riff_presentation,
};
use solomon_sim::game_run::{GameRunLifecycle, GameRunPhase};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextSpec};

#[derive(Clone, Debug)]
pub struct ContinueGameOver {
    pub run_id: String,
    pub event_id: u64,
}

/// Screen coordinates and camera scale of the local wizard's final position.
#[derive(Clone, Copy)]
pub struct GameOverAnchor {
    pub position: Vec2,
    pub zoom: f64,
}

pub struct GameOverOverlay {
    view: Option<(GameOverPresentation, SolomonRiffPresentation, GameOverAnchor)>,
    accepted: Option<ContinueGameOver>,
    actions: Vec<ContinueGameOver>,
    focus_next: bool,
}

impl GameOverOverlay {
    pub fn layer_id(&self) -> egui::LayerId {
        egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("game-over"))
    }
    pub fn attach() -> Result<Self, String> {
        Ok(Self { view: None, accepted: None, actions: Vec::new(), focus_next: false })
    }

    pub fn take_actions(&mut self) -> Vec<ContinueGameOver> {
        std::mem::take(&mut self.actions)
    }

    pub fn update(
        &mut self,
        run: Option<&GameRunLifecycle>,
        anchor: GameOverAnchor,
        _now: f64,
    ) -> Result<(), String> {
        let Some(run) = run.filter(|run| run.phase == GameRunPhase::GameOver) else {
            self.view = None;
            self.accepted = None;
            self.actions.clear();
            return Ok(());
        };
        let view = game_over_presentation(
            run.game_over_ticks,
            run.game_over_exit_ticks,
            run.game_over_exit_kind,
        );
        self.focus_next = view.accepts_input && self.accepted.is_none();
        self.accepted = view.accepts_input.then(|| ContinueGameOver {
            run_id: run.run_id.as_ref().expect("Game Over belongs to an active run").clone(),
            event_id: run.game_over_event_id,
        });
        self.view = Some((view, solomon_riff_presentation(run.game_over_ticks), anchor));
        Ok(())
    }

    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let Some((view, riff, anchor)) = self.view else { return Ok(()) };
        frame.input_scope("game-over", self.layer_id());
        let bounds = frame.ctx.content_rect();
        let scale = solomon_sim::viewport::game_viewport(Vec2::new(
            bounds.width() as f64,
            bounds.height() as f64,
        ))
        .display_scale as f32;
        let layer = self.layer_id();
        egui::Area::new(layer.id).fade_in(false).order(layer.order).fixed_pos(bounds.min).show(
            frame.ctx,
            |ui| {
                let mut input =
                    crate::player_panels::chrome::child_ui(ui, "game-over-input", bounds);
                if !view.accepts_input {
                    input.disable();
                }
                let response = input.interact(
                    bounds,
                    egui::Id::unique("game-over:continue"),
                    egui::Sense::click(),
                );
                if self.focus_next {
                    response.request_focus();
                    self.focus_next = false;
                }
                frame.record(
                    "game-over:continue",
                    if view.accepts_input {
                        "Game over. Continue to loadout."
                    } else {
                        "Game over."
                    },
                    "button",
                    &response,
                );
                if view.accepts_input
                    && response.clicked()
                    && let Some(action) = &self.accepted
                {
                    self.actions.push(action.clone());
                }
                let painter = ui.painter();
                if let Some(index) = riff.frame_record {
                    let center = egui::pos2(
                        (anchor.position.x + riff.offset.x * anchor.zoom) as f32,
                        (anchor.position.y + riff.offset.y * anchor.zoom) as f32,
                    );
                    frame.image(
                        painter,
                        "assets/stock/anim-solomon-riff.png",
                        egui::Rect::from_center_size(
                            center,
                            egui::Vec2::splat(200.0 * anchor.zoom as f32),
                        ),
                        Some(egui::Rect::from_min_max(
                            egui::pos2((index - 1) as f32 / 12.0, 0.0),
                            egui::pos2(index as f32 / 12.0, 1.0),
                        )),
                        egui::Color32::WHITE,
                    );
                }
                painter.rect_filled(
                    bounds,
                    0.0,
                    egui::Color32::BLACK.gamma_multiply(view.entry_fade_alpha as f32),
                );
                for (file, size, y) in [
                    ("game-over-game", [307.0, 119.0], -175.0),
                    ("game-over-over", [306.0, 120.0], 125.0),
                ] {
                    frame.image(
                        painter,
                        &format!("assets/stock/{file}.png"),
                        egui::Rect::from_center_size(
                            egui::pos2(
                                bounds.center().x,
                                bounds.center().y + y * scale + size[1] * scale / 2.0,
                            ),
                            egui::vec2(size[0] * scale, size[1] * scale),
                        ),
                        None,
                        egui::Color32::WHITE.gamma_multiply(view.title_alpha as f32),
                    );
                }
                native_text(
                    frame,
                    painter,
                    &NativeUiTextSpec {
                        scale: scale as f64,
                        tint: 0xd9ba70,
                        alpha: view.prompt_alpha,
                        ..NativeUiTextSpec::new(
                            NativeUiFontName::Menu,
                            "CLICK TO CONTINUE...",
                            bounds.center().x as f64,
                            bounds.bottom() as f64 - 50.0 * scale as f64,
                        )
                    },
                );
                painter.rect_filled(
                    bounds,
                    0.0,
                    egui::Color32::BLACK.gamma_multiply(view.exit_fade_alpha as f32),
                );
                ui.set_min_size(bounds.size());
            },
        );
        Ok(())
    }
}
