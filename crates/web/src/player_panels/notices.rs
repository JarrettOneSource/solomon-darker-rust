//! Source `hub-inventory-notices.ts` variants. One notice owns its content and
//! presentation branch; result tint/summary do not live in a separate sidecar.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerPanelNotice {
    Standard { title: String, body: String },
    UnforgeConfirmation,
    UnforgeResult { title: String, body: String, summary: String, outcome_tint: u32 },
}

use super::PlayerPanels;
use super::chrome::PanelCanvas;
use crate::gpu_ui::UiFrame;
use solomon_sim::Vec2;
use solomon_sim::hud::HudRect;
use solomon_sim::native_ui_text::messages::{
    NativeUiMessageDataLineSpec, layout_native_ui_single_action_message,
};
use solomon_sim::native_ui_text::{
    NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec, measure_native_ui_text,
};
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
        c.semantic_value(
            "panel:notice-title",
            title,
            "alert",
            HudRect::new(540.0, 350.0, 524.0, 60.0),
            title,
        );
        match notice {
            PlayerPanelNotice::Standard { .. } => {
                let mut lines = vec![NativeUiMessageDataLineSpec {
                    font: NativeUiFontName::Menu,
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
                // The source's four standard notice factories retain these authored
                // anchors and trailing blank DataLines; they are not CSS margins.
                let clothing = title == "A WIZARD WOULD NEVER REMOVE HIS HAT!"
                    || title == "A WIZARD WOULD NEVER REMOVE HIS ROBE!";
                if clothing || title == "NOT ENOUGH GOLD!" {
                    lines.push(data_line(""));
                }
                let layout = layout_native_ui_single_action_message(
                    Vec2::new(800.0, if clothing { 450.0 } else { 350.0 }),
                    &lines,
                )?;
                self.notice_frame(c, layout.frame_bounds)?;
                for line in layout.lines {
                    c.text(&NativeUiTextSpec {
                        align: NativeUiTextAlign::Left,
                        tint: line.tint,
                        line_height: Some(line.line_height),
                        ..NativeUiTextSpec::new(line.font, &line.text, line.x, line.baseline_y)
                    });
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
                self.notice_text(
                    c,
                    NativeUiFontName::Menu,
                    title,
                    Vec2::new(614.0, 478.0),
                    0xffffff,
                )?;
                c.text(&NativeUiTextSpec {
                    align: NativeUiTextAlign::Left,
                    line_height: Some(17.0),
                    max_width: Some(376.0),
                    ..NativeUiTextSpec::new(NativeUiFontName::Medium, body, 614.0, 511.0)
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
                let line_width = measure_native_ui_text(title, NativeUiFontName::Menu, 1.0)
                    .max(measure_native_ui_text(summary, NativeUiFontName::Medium, 1.0))
                    .max(measure_native_ui_text(body, NativeUiFontName::Medium, 1.0));
                let inner_width = line_width + 141.0;
                let left = 801.5 - inner_width / 2.0;
                self.notice_frame(c, HudRect::new(left - 5.0, 391.5, inner_width + 10.0, 318.0))?;
                self.notice_text(
                    c,
                    NativeUiFontName::Menu,
                    title,
                    Vec2::new(left + 70.5, 485.0),
                    0xffffff,
                )?;
                self.notice_text(
                    c,
                    NativeUiFontName::Medium,
                    summary,
                    Vec2::new(left + 70.5, 520.0),
                    0xd9ba70,
                )?;
                self.notice_text(
                    c,
                    NativeUiFontName::Medium,
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
            "panel-notice",
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
        font: NativeUiFontName,
        text: &str,
        position: Vec2,
        tint: u32,
    ) -> Result<(), String> {
        c.text(&NativeUiTextSpec {
            align: NativeUiTextAlign::Left,
            tint,
            ..NativeUiTextSpec::new(font, text, position.x, position.y)
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
        let button = c.native_button_art(&self.ui, action, label, r)?;
        button.text(&NativeUiTextSpec {
            tint: 0xd9ba70,
            ..NativeUiTextSpec::new(NativeUiFontName::Menu, label, r.width / 2.0, baseline - r.y)
        });
        Ok(())
    }
}
fn data_line(text: &str) -> NativeUiMessageDataLineSpec<'_> {
    NativeUiMessageDataLineSpec {
        font: NativeUiFontName::Medium,
        gap_after: 0.0,
        text,
        tint: 0xffffff,
    }
}
