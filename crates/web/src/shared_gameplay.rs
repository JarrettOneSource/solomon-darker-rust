//! GPU shared pause/resume presentation from GameplayPauseMenu/GameplayResumeProgress.
//! Native authority owns the hold clock; this view only extrapolates its displayed progress.

use crate::gpu_ui::UiFrame;
use egui::{Color32, FontFamily, FontId, Id, Order, Rect, Stroke, vec2};
use solomon_sim::menu::native_pause_menu_reveal;
use solomon_sim::menu::shared::{
    GameplayPausePresentation, gameplay_pause_presentation, gameplay_resume_grace_progress,
};
use web_sys::Document;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedPauseView<'a> {
    pub owner_player_id: &'a str,
    pub owner_display_name: &'a str,
    /// Source labels: Inventory, the Skill Book, the skill selector; None for pause menu.
    pub activity: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResumeGraceView<'a> {
    /// Source hyphen IDs, including party-rejoin-wait and skill-picker-closed.
    pub reason: &'a str,
    pub remaining_ms: Option<u64>,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct SharedGameplaySnapshot<'a> {
    pub local_player_id: &'a str,
    pub has_game: bool,
    /// Title/Create/Hall visibility; ordinary pause and player panels do not cover the world.
    pub menu_covers_world: bool,
    pub settings_open: bool,
    pub pause: Option<SharedPauseView<'a>>,
    pub resume_grace: Option<ResumeGraceView<'a>>,
}

struct ResumeClock {
    reason: String,
    remaining_ms: Option<u64>,
    sequence: u64,
    received_at: f64,
}

struct PausePresentation {
    label: String,
    detail: String,
    reveal: f64,
}
struct ResumePresentation {
    label: &'static str,
    progress: Option<f64>,
}

pub struct SharedGameplayUi {
    pause_key: Option<(String, Option<String>)>,
    pause_received_at: f64,
    resume_clock: Option<ResumeClock>,
    pause: Option<PausePresentation>,
    resume: Option<ResumePresentation>,
    size: (i32, i32),
}
impl SharedGameplayUi {
    pub fn attach(_document: &Document) -> Result<Self, String> {
        Ok(Self {
            pause_key: None,
            pause_received_at: 0.0,
            resume_clock: None,
            pause: None,
            resume: None,
            size: (0, 0),
        })
    }
    pub fn update(
        &mut self,
        snapshot: SharedGameplaySnapshot<'_>,
        now_ms: f64,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        self.size = (width, height);
        let visible = snapshot.has_game && !snapshot.menu_covers_world;
        let pause = snapshot.pause.filter(|pause| {
            visible && !snapshot.settings_open && pause.owner_player_id != snapshot.local_player_id
        });
        self.pause = if let Some(pause) = pause {
            let key = (pause.owner_player_id.to_owned(), pause.activity.map(str::to_owned));
            if self.pause_key.as_ref() != Some(&key) {
                self.pause_key = Some(key);
                self.pause_received_at = now_ms;
            }
            match gameplay_pause_presentation(
                pause.owner_player_id,
                snapshot.local_player_id,
                pause.owner_display_name,
                pause.activity,
            ) {
                GameplayPausePresentation::Waiting { label, detail } => Some(PausePresentation {
                    label,
                    detail,
                    reveal: native_pause_menu_reveal(true, now_ms - self.pause_received_at),
                }),
                _ => None,
            }
        } else {
            self.pause_key = None;
            None
        };
        let grace =
            snapshot.resume_grace.filter(|grace| visible && grace.reason != "skill-picker-closed");
        self.resume = if let Some(grace) = grace {
            let same = self.resume_clock.as_ref().is_some_and(|clock| {
                clock.reason == grace.reason
                    && clock.sequence == grace.sequence
                    && clock.remaining_ms == grace.remaining_ms
            });
            if !same {
                self.resume_clock = Some(ResumeClock {
                    reason: grace.reason.to_owned(),
                    remaining_ms: grace.remaining_ms,
                    sequence: grace.sequence,
                    received_at: now_ms,
                });
            }
            let elapsed = now_ms - self.resume_clock.as_ref().expect("received grace").received_at;
            let progress = gameplay_resume_grace_progress(
                grace.remaining_ms.map(|value| value as f64),
                elapsed,
            );
            Some(ResumePresentation {
                label: if progress.is_some() {
                    "RESUMING..."
                } else if grace.reason == "party-rejoin-wait" {
                    "Waiting for players to rejoin"
                } else {
                    "Waiting on players ..."
                },
                progress,
            })
        } else {
            self.resume_clock = None;
            None
        };
        Ok(())
    }
    pub fn pause_layer() -> egui::LayerId {
        egui::LayerId::new(Order::Middle, Id::unique("shared/pause"))
    }
    pub fn resume_layer() -> egui::LayerId {
        egui::LayerId::new(Order::Foreground, Id::unique("shared/resume"))
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let ctx = frame.ctx.clone();
        let screen = ctx.content_rect();
        if let Some(pause) = &self.pause {
            frame.input_scope("shared/pause");
            egui::Area::new(Self::pause_layer().id)
                .fade_in(false)
                .order(Self::pause_layer().order)
                .fixed_pos(egui::Pos2::ZERO)
                .show(&ctx, |ui| {
                    ui.memory_mut(|memory| memory.set_modal_layer(ui.layer_id()));
                    ui.set_min_size(screen.size());
                    ui.interact(screen, Id::unique("shared/pause-hold"), egui::Sense::CLICK);
                    let alpha = pause.reveal as f32;
                    let painter = ui.painter();
                    painter.rect_filled(
                        screen,
                        0.0,
                        Color32::from_black_alpha((alpha * 0.85 * 255.0) as u8),
                    );
                    let scale = fit_panel(self.size.0, self.size.1, 620.0, 300.0) as f32;
                    let bounds =
                        Rect::from_center_size(screen.center(), vec2(620.0, 190.0) * scale);
                    panel(painter, bounds, alpha);
                    painter.text(
                        bounds.center() - vec2(0.0, 22.0 * scale),
                        egui::Align2::CENTER_CENTER,
                        &pause.label,
                        FontId::new(30.0 * scale, FontFamily::Name("Cinzel Bold".into())),
                        Color32::from_rgb(223, 199, 130).gamma_multiply(alpha),
                    );
                    painter.text(
                        bounds.center() + vec2(0.0, 28.0 * scale),
                        egui::Align2::CENTER_CENTER,
                        &pause.detail,
                        FontId::new(19.0 * scale, FontFamily::Name("Cinzel".into())),
                        Color32::from_rgb(201, 185, 138).gamma_multiply(alpha),
                    );
                    frame.record_rect(
                        "shared/pause",
                        &format!("{} {}", pause.label, pause.detail),
                        "status",
                        bounds,
                        true,
                    );
                });
        }
        if let Some(resume) = &self.resume {
            frame.input_scope("shared/resume");
            let scale = fit_panel(self.size.0, self.size.1, 428.0, 240.0) as f32;
            let bounds = Rect::from_center_size(
                screen.center(),
                vec2(428.0, if resume.progress.is_some() { 142.0 } else { 104.0 }) * scale,
            );
            let painter = ctx.layer_painter(Self::resume_layer());
            panel(&painter, bounds, 1.0);
            painter.text(
                bounds.center()
                    - vec2(0.0, if resume.progress.is_some() { 19.0 } else { 0.0 }) * scale,
                egui::Align2::CENTER_CENTER,
                resume.label,
                FontId::new(28.0 * scale, FontFamily::Name("Georgia".into())),
                Color32::from_rgb(223, 199, 130),
            );
            if let Some(progress) = resume.progress {
                let track = Rect::from_center_size(
                    bounds.center() + vec2(0.0, 32.0 * scale),
                    vec2(336.0, 18.0) * scale,
                );
                painter.rect_filled(track, 0.0, Color32::from_rgb(20, 17, 13));
                painter.rect_filled(
                    Rect::from_min_size(
                        track.min,
                        vec2(track.width() * progress as f32, track.height()),
                    ),
                    0.0,
                    Color32::from_rgb(223, 199, 130),
                );
                painter.rect_stroke(
                    track,
                    0.0,
                    Stroke::new(2.0 * scale, Color32::from_rgb(216, 188, 114)),
                    egui::StrokeKind::Inside,
                );
                frame.record_rect(
                    "shared/resume-progress",
                    &format!("{}%", (progress * 100.0).round()),
                    "progressbar",
                    track,
                    true,
                );
            }
            frame.record_rect("shared/resume", resume.label, "status", bounds, true);
            if let Some(clock) = &self.resume_clock {
                frame.record_value("shared/resume", &clock.reason, None);
            }
        }
        Ok(())
    }
}
fn panel(painter: &egui::Painter, bounds: Rect, alpha: f32) {
    painter.rect_filled(bounds, 0.0, Color32::from_black_alpha((209.0 * alpha) as u8));
    painter.rect_stroke(
        bounds,
        0.0,
        Stroke::new(4.0, Color32::from_rgb(216, 188, 114).gamma_multiply(alpha)),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        bounds.shrink(2.0),
        0.0,
        Stroke::new(1.0, Color32::BLACK),
        egui::StrokeKind::Inside,
    );
}
fn fit_panel(width: i32, height: i32, panel_width: f64, panel_height: f64) -> f64 {
    ((f64::from(width) - 24.0) / panel_width)
        .min((f64::from(height) - 24.0) / panel_height)
        .clamp(0.1, 1.0)
}
