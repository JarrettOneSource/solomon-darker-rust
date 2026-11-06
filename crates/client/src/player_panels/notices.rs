//! Inventory notices: standard messages, the unforge confirmation and unforge results. One
//! notice owns its content and presentation branch; result tint/summary do not live in a
//! separate sidecar.

/// A modal notice over the panels: a standard message, the unforge confirmation or its result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerPanelNotice {
    Standard { title: String, body: String },
    UnforgeConfirmation,
    UnforgeResult { title: String, body: String, summary: String, outcome_tint: u32 },
}

pub(super) const NOTICE_MODAL_SCOPE: &str = "panel-notice";

use super::PlayerPanels;
use super::chrome::PanelCanvas;
use crate::gpu_ui::UiFrame;
use solomon_sim::Vec2;
use solomon_sim::hud::HudRect;
use solomon_sim::ui_text::messages::{UiMessageDataLineSpec, layout_ui_single_action_message};
use solomon_sim::ui_text::{UiFontName, UiTextAlign, UiTextSpec, measure_ui_text};
impl PlayerPanels {
    pub(crate) fn draw_notice(
        &self,
        c: &PanelCanvas<'_, '_>,
        notice: &PlayerPanelNotice,
    ) -> Result<(), String> {
        let (title, body, summary) = match notice {
            PlayerPanelNotice::Standard { title, body } => (title.as_str(), body.as_str(), ""),
            PlayerPanelNotice::UnforgeConfirmation => (
                "REALLY UNFORGE THIS?",
                "Unforging grants you a permanent small bonus to your stats, but utterly destroys the item.",
                "",
            ),
            PlayerPanelNotice::UnforgeResult { title, body, summary, .. } => {
                (title.as_str(), body.as_str(), summary.as_str())
            }
        };
        c.frame.announce("panel:notice", &format!("{title}. {summary} {body}"));
        match notice {
            PlayerPanelNotice::Standard { .. } => {
                let mut lines = vec![UiMessageDataLineSpec {
                    font: UiFontName::Menu,
                    text: title,
                    gap_after: 10.0,
                    tint: 0xffffff,
                }];
                for (index, paragraph) in body.split("\n\n").enumerate() {
                    if index > 0 {
                        lines.push(data_line(""));
                    }
                    lines.push(data_line(paragraph));
                }
                // Standard notices keep their authored anchors and trailing blank data
                // lines, which belong to the message layout rather than outer spacing.
                let clothing = title == "A WIZARD WOULD NEVER REMOVE HIS HAT!"
                    || title == "A WIZARD WOULD NEVER REMOVE HIS ROBE!";
                if clothing || title == "NOT ENOUGH GOLD!" {
                    lines.push(data_line(""));
                }
                let layout = layout_ui_single_action_message(
                    Vec2::new(800.0, if clothing { 450.0 } else { 350.0 }),
                    &lines,
                )?;
                self.notice_frame(c, layout.frame_bounds)?;
                for (index, line) in layout.lines.iter().enumerate() {
                    let spec = UiTextSpec {
                        align: UiTextAlign::Left,
                        tint: line.tint,
                        line_height: Some(line.line_height),
                        ..UiTextSpec::new(line.font, &line.text, line.x, line.baseline_y)
                    };
                    if index == 0 {
                        c.labeled_text("panel:notice-title", &spec);
                    } else {
                        c.text(&spec);
                    }
                }
                self.notice_button(
                    c,
                    "OKAY",
                    "notice-okay",
                    layout.action_bounds,
                    layout.action_text_baseline_y,
                    true,
                )?;
            }
            PlayerPanelNotice::UnforgeConfirmation => {
                self.notice_frame(c, HudRect::new(539.5, 382.5, 524.0, 336.0))?;
                c.labeled_text(
                    "panel:notice-title",
                    &UiTextSpec {
                        align: UiTextAlign::Left,
                        ..UiTextSpec::new(UiFontName::Menu, title, 614.0, 478.0)
                    },
                );
                c.text(&UiTextSpec {
                    align: UiTextAlign::Left,
                    line_height: Some(17.0),
                    max_width: Some(376.0),
                    ..UiTextSpec::new(UiFontName::Medium, body, 614.0, 511.0)
                });
                self.notice_button(
                    c,
                    "UNFORGE",
                    "unforge-confirm",
                    HudRect::new(595.0, 573.0, 197.0, 69.0),
                    618.0,
                    true,
                )?;
                self.notice_button(
                    c,
                    "CANCEL",
                    "unforge-cancel",
                    HudRect::new(811.0, 573.0, 197.0, 69.0),
                    618.0,
                    false,
                )?;
            }
            PlayerPanelNotice::UnforgeResult { outcome_tint, .. } => {
                let line_width = measure_ui_text(title, UiFontName::Menu, 1.0)
                    .max(measure_ui_text(summary, UiFontName::Medium, 1.0))
                    .max(measure_ui_text(body, UiFontName::Medium, 1.0));
                let inner_width = line_width + 141.0;
                let left = 801.5 - inner_width / 2.0;
                self.notice_frame(c, HudRect::new(left - 5.0, 391.5, inner_width + 10.0, 318.0))?;
                c.labeled_text(
                    "panel:notice-title",
                    &UiTextSpec {
                        align: UiTextAlign::Left,
                        ..UiTextSpec::new(UiFontName::Menu, title, left + 70.5, 485.0)
                    },
                );
                self.notice_text(
                    c,
                    UiFontName::Medium,
                    summary,
                    Vec2::new(left + 70.5, 520.0),
                    0xd9ba70,
                )?;
                self.notice_text(
                    c,
                    UiFontName::Medium,
                    body,
                    Vec2::new(left + 70.5, 537.0),
                    *outcome_tint,
                )?;
                self.notice_button(
                    c,
                    "OKAY",
                    "notice-okay",
                    HudRect::new(703.0, 564.0, 197.0, 69.0),
                    609.0,
                    true,
                )?;
            }
        }
        c.frame.record_value("panel:notice-title", title, None);
        Ok(())
    }
    pub(super) fn paint_notice_modal(
        &mut self,
        frame: &mut UiFrame<'_>,
        notice: &PlayerPanelNotice,
    ) -> Result<(), String> {
        let scale = (self.viewport_size.0 as f32 / 1600.0).min(self.viewport_size.1 as f32 / 900.0);
        self.modal_canvas(
            frame,
            NOTICE_MODAL_SCOPE,
            scale,
            egui::pos2(
                (self.viewport_size.0 as f32 - 1600.0 * scale) / 2.0,
                (self.viewport_size.1 as f32 - 900.0 * scale) / 2.0,
            ),
            0.75,
            |panels, c| panels.draw_notice(c, notice),
        )
    }
    fn notice_frame(&self, c: &PanelCanvas<'_, '_>, r: HudRect) -> Result<(), String> {
        crate::gpu_ui::message_frame(c.frame, &c.painter, &self.ui, c.rect(r))
    }
    fn notice_text(
        &self,
        c: &PanelCanvas<'_, '_>,
        font: UiFontName,
        text: &str,
        position: Vec2,
        tint: u32,
    ) -> Result<(), String> {
        c.text(&UiTextSpec {
            align: UiTextAlign::Left,
            tint,
            ..UiTextSpec::new(font, text, position.x, position.y)
        });
        Ok(())
    }
    fn notice_button(
        &self,
        c: &PanelCanvas<'_, '_>,
        label: &str,
        action: &str,
        r: HudRect,
        baseline: f64,
        _default_focus: bool,
    ) -> Result<(), String> {
        let button = c.button_art(&self.ui, action, label, r)?;
        button.text(&solomon_sim::ui_text::buttons::fit_ui_button_label(
            UiTextSpec {
                tint: 0xd9ba70,
                ..UiTextSpec::new(UiFontName::Menu, label, r.width / 2.0, baseline - r.y)
            },
            HudRect::new(0.0, 0.0, r.width, r.height),
            r.height / 69.0,
        ));
        Ok(())
    }
}
fn data_line(text: &str) -> UiMessageDataLineSpec<'_> {
    UiMessageDataLineSpec { font: UiFontName::Medium, gap_after: 0.0, text, tint: 0xffffff }
}
