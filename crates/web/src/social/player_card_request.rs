//! Website `PlayerCardRequests` owns latest-wins lookup feedback per session.
//! The five-second notice and ten-second lookup deadline are product UI choices,
//! independent of retail gameplay ticks (Website social ledger 136).

const FEEDBACK_MS: f64 = 5_000.0;
const REQUEST_TIMEOUT_MS: f64 = 10_000.0;

struct PendingRequest {
    id: u64,
    until: f64,
}

struct Feedback {
    message: &'static str,
    until: f64,
}

#[derive(Default)]
pub(crate) struct PlayerCardRequests {
    next_id: u64,
    pending: Option<PendingRequest>,
    feedback: Option<Feedback>,
    dismiss_focus: Option<egui::Id>,
}

impl PlayerCardRequests {
    pub fn begin(&mut self, now: f64) -> u64 {
        self.next_id += 1;
        self.pending = Some(PendingRequest { id: self.next_id, until: now + REQUEST_TIMEOUT_MS });
        self.feedback = None;
        self.next_id
    }

    pub fn complete(&mut self, id: u64, available: bool, now: f64) -> bool {
        self.advance(now);
        if self.pending.as_ref().is_none_or(|pending| pending.id != id) {
            return false;
        }
        self.pending = None;
        self.feedback = (!available).then_some(Feedback {
            message: "That wizard is no longer available.",
            until: now + FEEDBACK_MS,
        });
        true
    }

    pub fn advance(&mut self, now: f64) {
        if self.pending.as_ref().is_some_and(|pending| now >= pending.until) {
            self.pending = None;
            self.feedback = Some(Feedback {
                message: "The Player Card request timed out.",
                until: now + FEEDBACK_MS,
            });
        }
        if self.feedback.as_ref().is_some_and(|feedback| now >= feedback.until) {
            self.feedback = None;
        }
    }

    pub fn dismiss(&mut self) {
        self.pending = None;
        self.feedback = None;
    }

    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn blocks_input(&self) -> bool {
        self.pending() || self.feedback.is_some() && self.dismiss_focus.is_some()
    }

    pub fn message(&self) -> Option<&str> {
        if self.pending() {
            Some("Opening Player Card…")
        } else {
            self.feedback.as_ref().map(|feedback| feedback.message)
        }
    }

    /// Covering the notice releases its button without discarding its application deadline.
    pub fn hide(&mut self, ctx: &egui::Context) {
        if let Some(id) = self.dismiss_focus.take() {
            ctx.memory_mut(|memory| memory.surrender_focus(id));
        }
    }

    pub fn paint(
        &mut self,
        frame: &mut crate::gpu_ui::UiFrame<'_>,
        coarse: bool,
    ) -> Result<(), String> {
        let feedback = self.feedback.is_some();
        if !feedback {
            self.hide(frame.ctx);
        }
        let Some(message) = self.message() else { return Ok(()) };
        let id = if feedback { "social/card-feedback" } else { "social/card-pending" };
        let response = paint_notice(frame, id, message, feedback, coarse)?;
        if let Some(response) = response {
            self.dismiss_focus = response.has_focus().then_some(response.id);
            if response.has_focus() {
                frame.input_scope("social/card-feedback", super::SocialUi::notice_layer());
            }
            if response.clicked() {
                self.dismiss();
                response.surrender_focus();
                self.dismiss_focus = None;
            }
        }
        Ok(())
    }
}

pub(super) fn paint_notice(
    frame: &mut crate::gpu_ui::UiFrame<'_>,
    id: &str,
    notice: &str,
    dismissible: bool,
    coarse: bool,
) -> Result<Option<egui::Response>, String> {
    let ctx = frame.ctx.clone();
    let screen = ctx.content_rect();
    let font_size = screen.width() * 0.015;
    let padding = egui::vec2(screen.width() * 0.015, screen.height() * 0.007);
    let label = crate::gpu_ui::font_run_with_options(
        frame.ctx,
        egui::FontId::new(font_size, egui::FontFamily::Name("Cinzel".into())),
        egui::Color32::from_rgb(234, 216, 170),
        notice,
        crate::gpu_ui::FontRunOptions {
            wrap_width: Some((screen.width() * 0.7 - padding.x * 2.0 - 2.0).max(1.0)),
            letter_spacing: Some(font_size * 0.08),
            ..Default::default()
        },
    )?;
    let button_size = egui::vec2((font_size * 8.0).max(84.0), if coarse { 44.0 } else { 32.0 });
    let footer_height = if dismissible { button_size.y + 8.0 } else { 0.0 };
    let size = egui::vec2(
        label.size().x.max(if dismissible { button_size.x } else { 0.0 }),
        label.size().y + footer_height,
    ) + padding * 2.0
        + egui::vec2(2.0, 2.0);
    let layer = super::SocialUi::notice_layer();
    egui::Area::new(layer.id)
        .fade_in(false)
        .background_interactable(false)
        .order(layer.order)
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -screen.height() * 0.04))
        .show(&ctx, |ui| -> Result<Option<egui::Response>, String> {
            let (bounds, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            ui.painter().rect_filled(
                bounds,
                0.0,
                egui::Color32::from_rgba_unmultiplied(7, 6, 14, 224),
            );
            ui.painter().rect_stroke(
                bounds,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(211, 177, 101, 153)),
                egui::StrokeKind::Inside,
            );
            frame.paint_group(
                ui.painter(),
                crate::gpu_ui::GroupEffect {
                    shadows: vec![crate::gpu_ui::ImageShadow {
                        offset: egui::vec2(0.0, 2.0),
                        blur: 1.0,
                        color: egui::Color32::BLACK,
                    }],
                    ..Default::default()
                },
                |frame, painter| {
                    label.paint_aligned(
                        frame,
                        painter,
                        egui::Rect::from_min_size(
                            egui::pos2(
                                bounds.center().x - label.size().x / 2.0,
                                bounds.top() + padding.y + 1.0,
                            ),
                            label.size(),
                        ),
                        egui::Align::Center,
                        1.0,
                    )
                },
            )?;
            frame.record_rect(id, notice, "status", bounds, true);
            if !dismissible {
                return Ok(None);
            }
            let button_bounds = egui::Rect::from_min_size(
                egui::pos2(
                    bounds.center().x - button_size.x / 2.0,
                    bounds.bottom() - padding.y - 1.0 - button_size.y,
                ),
                button_size,
            );
            let response = ui.put(
                button_bounds,
                egui::Button::new(
                    egui::RichText::new("Dismiss")
                        .font(egui::FontId::new(
                            font_size.max(12.0),
                            egui::FontFamily::Name("Cinzel".into()),
                        ))
                        .color(egui::Color32::from_rgb(234, 216, 170)),
                )
                .min_size(button_size),
            );
            frame.record(
                "social/card-feedback-dismiss",
                "Dismiss Player Card message",
                "button",
                &response,
            );
            Ok(Some(response))
        })
        .inner
}

#[cfg(test)]
mod tests {
    use super::PlayerCardRequests;

    #[test]
    fn unavailable_player_card_feedback_is_visible_for_five_application_seconds() {
        let mut requests = PlayerCardRequests::default();
        let id = requests.begin(1_000.0);
        assert!(requests.complete(id, false, 1_250.0));
        assert_eq!(requests.message(), Some("That wizard is no longer available."));
        requests.advance(6_249.0);
        assert_eq!(requests.message(), Some("That wizard is no longer available."));
        requests.advance(6_250.0);
        assert_eq!(requests.message(), None);
        assert!(!requests.pending());
    }

    #[test]
    fn only_the_latest_player_card_request_can_complete() {
        let mut requests = PlayerCardRequests::default();
        let old = requests.begin(0.0);
        let current = requests.begin(100.0);
        assert_ne!(old, current);
        for available in [true, false] {
            assert!(!requests.complete(old, available, 200.0));
            assert!(requests.pending());
            assert_eq!(requests.message(), Some("Opening Player Card…"));
        }
        assert!(requests.complete(current, true, 300.0));
        assert!(!requests.pending());
        assert_eq!(requests.message(), None);
        assert!(!requests.complete(old, false, 400.0));
        assert_eq!(requests.message(), None);
    }

    #[test]
    fn lookup_timeout_has_its_own_feedback_deadline_and_ignores_late_reply() {
        let mut requests = PlayerCardRequests::default();
        let id = requests.begin(10.0);
        requests.advance(10_009.0);
        assert!(requests.pending());
        requests.advance(10_010.0);
        assert!(!requests.pending());
        assert_eq!(requests.message(), Some("The Player Card request timed out."));
        assert!(!requests.complete(id, true, 10_011.0));
        assert_eq!(requests.message(), Some("The Player Card request timed out."));
        requests.advance(15_009.0);
        assert_eq!(requests.message(), Some("The Player Card request timed out."));
        requests.advance(15_010.0);
        assert_eq!(requests.message(), None);
    }

    #[test]
    fn a_reply_at_or_after_the_lookup_deadline_cannot_complete_without_prior_ui_advance() {
        for now in [10_000.0, 10_001.0, 180_000.0] {
            for available in [true, false] {
                let mut requests = PlayerCardRequests::default();
                let id = requests.begin(0.0);
                assert!(!requests.complete(id, available, now), "late reply at {now}");
                assert!(!requests.pending());
                assert_eq!(requests.message(), Some("The Player Card request timed out."));
                requests.advance(now + 4_999.0);
                assert_eq!(requests.message(), Some("The Player Card request timed out."));
                requests.advance(now + 5_000.0);
                assert_eq!(requests.message(), None);
            }
        }
    }

    #[test]
    fn dismissal_and_session_reset_invalidate_replies_without_reusing_request_ids() {
        let mut requests = PlayerCardRequests::default();
        let old = requests.begin(0.0);
        requests.dismiss();
        assert!(!requests.complete(old, false, 1.0));
        assert_eq!(requests.message(), None);
        let current = requests.begin(2.0);
        assert_ne!(old, current);
        assert!(!requests.complete(old, true, 3.0));
        assert!(requests.complete(current, false, 4.0));
        requests.dismiss();
        requests.advance(100_000.0);
        assert_eq!(requests.message(), None);
        assert!(!requests.blocks_input());
    }

    #[test]
    fn a_new_request_cancels_feedback_expiry_without_clearing_its_pending_state() {
        let mut requests = PlayerCardRequests::default();
        let old = requests.begin(0.0);
        assert!(requests.complete(old, false, 1.0));
        let current = requests.begin(2.0);
        requests.advance(5_001.0);
        assert!(requests.pending());
        assert_eq!(requests.message(), Some("Opening Player Card…"));
        assert!(requests.complete(current, true, 5_002.0));
        assert_eq!(requests.message(), None);
    }
}
