//! The first pending Party invitation, shown in the Hub.
//! The host retains invitation membership; this view only paints the borrowed row
//! and sends its original ID back through the canonical accept/deny actions.

use solomon_sim::social::party_ui::{
    PARTY_INVITATION_FRAME, PARTY_INVITATION_TITLE, party_invitation_action_bounds,
    party_invitation_body,
};
use solomon_sim::social::{GameChatWorldKind, PartyInvitationView, SocialAction};
use solomon_sim::ui_text::{UiFontName, UiTextAlign, UiTextSpec};

use super::super::{SocialUi, Surface};

impl SocialUi {
    fn first_party_invitation(&self) -> Option<&PartyInvitationView> {
        self.snapshot.as_ref()?.party_state.as_ref()?.invitations.first()
    }

    pub(in crate::social) fn party_invitation_visible(&self) -> bool {
        self.context.visible
            && !self.context.input_suspended
            && self
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.world_kind == GameChatWorldKind::Hub)
            && !matches!(self.surface, Some(Surface::Card(_) | Surface::HubPlayer(_)))
            && !self.college_visible()
            && self.first_party_invitation().is_some()
    }

    pub(in crate::social) fn deny_party_invitation(&mut self) {
        if let Some(invitation) = self.first_party_invitation() {
            let invitation_id = invitation.id.clone();
            self.actions.push(SocialAction::DenyInvitation { invitation_id });
        }
    }

    pub(in crate::social) fn paint_party_invitation(
        &mut self,
        frame: &mut crate::gpu_ui::UiFrame<'_>,
        ui: &mut egui::Ui,
    ) -> Result<(), String> {
        let Some(invitation) = self.first_party_invitation() else { return Ok(()) };
        let id = invitation.id.clone();
        let body = party_invitation_body(&invitation.inviter.display_name);
        let opening = self.shown_party_invitation.as_deref() != Some(&id);
        let (origin, scale) = super::super::gpu::stage(self.size);
        let bounds = super::super::gpu::rect(PARTY_INVITATION_FRAME, origin, scale);
        ui.interact(bounds, egui::Id::unique("social/invitation-panel-block"), egui::Sense::CLICK);
        crate::gpu_ui::message_frame_scaled(frame, ui.painter(), &self.ui, bounds, scale)?;
        frame.record_rect("social/party-invitation", &body, "dialog", bounds, true);
        for (text, font, baseline, line_height) in [
            (PARTY_INVITATION_TITLE, UiFontName::Menu, 95.0, None),
            (body.as_str(), UiFontName::Medium, 130.5, Some(17.0)),
        ] {
            super::super::gpu::paint_stock_text(
                frame,
                ui.painter(),
                &UiTextSpec {
                    align: UiTextAlign::Left,
                    max_width: Some(400.0),
                    line_height,
                    ..UiTextSpec::new(
                        font,
                        text,
                        PARTY_INVITATION_FRAME.x + 76.0,
                        PARTY_INVITATION_FRAME.y + baseline,
                    )
                },
                origin,
                scale,
            );
        }
        let mut selected = None;
        for ((label, answer), bounds) in [("ACCEPT", "accept"), ("DENY", "deny")]
            .into_iter()
            .zip(party_invitation_action_bounds())
        {
            let action = format!("party-invitation-{answer}:{id}");
            let response = super::super::gpu::stock_button(
                frame,
                ui,
                &action,
                label,
                super::super::gpu::rect(bounds, origin, scale),
                true,
            )?;
            if opening && answer == "accept" {
                response.request_focus();
            }
            if response.clicked() {
                selected = Some(action);
            }
        }
        self.shown_party_invitation = Some(id);
        if let Some(action) = selected {
            self.activate(&action)?;
        }
        Ok(())
    }
}
