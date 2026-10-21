//! `HubScene`'s actual first Party invitation and `NativeUiPartyInvitation.tsx`.
//! The host retains invitation membership; this view only paints the borrowed row
//! and sends its original ID back through the canonical accept/deny actions.

use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec};
use solomon_sim::social::party_ui::{
    NATIVE_PARTY_INVITATION_FRAME, NATIVE_PARTY_INVITATION_TITLE,
    native_party_invitation_action_bounds, native_party_invitation_body,
};
use solomon_sim::social::{GameChatWorldKind, PartyInvitationView, SocialAction};
use web_sys::HtmlElement;

use super::super::{SocialUi, Surface, attribute};
use crate::ui::text::native_text;
use crate::ui::{append, element, message_frame, style};

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

    pub(in crate::social) fn draw_party_invitation(&mut self) -> Result<(), String> {
        self.party_invitation.set_hidden(!self.party_invitation_visible());
        if !self.party_invitation_visible() {
            self.shown_party_invitation = None;
            self.party_invitation.set_text_content(None);
            self.party_invitation
                .remove_attribute("data-party-invitation")
                .map_err(|_| "hidden Party invitation identity")?;
            return Ok(());
        }
        let invitation = self.first_party_invitation().expect("visible Party invitation");
        let id = invitation.id.clone();
        let body = native_party_invitation_body(&invitation.inviter.display_name);
        let focus = self.shown_party_invitation.as_deref() != Some(id.as_str());
        let retained_focus = self
            .document
            .active_element()
            .and_then(|node| node.get_attribute("data-party-invitation-answer"));
        self.party_invitation.set_text_content(None);
        attribute(&self.party_invitation, "data-party-invitation", &id)?;
        let stage = element(&self.document, "section", "hub-party-menu-stage")?;
        for (name, value) in [
            ("role", "dialog"),
            ("aria-modal", "true"),
            ("aria-label", "Party invitation"),
            ("aria-describedby", "social-party-invitation-body"),
            ("data-native-ui-message-box", ""),
        ] {
            attribute(&stage, name, value)?;
        }
        let scale = (f64::from(self.size.0) / 1600.0).min(f64::from(self.size.1) / 900.0);
        let left = (f64::from(self.size.0) - 1600.0 * scale) / 2.0;
        let top = (f64::from(self.size.1) - 900.0 * scale) / 2.0;
        style(&stage, &format!("left:{left}px;top:{top}px;transform:scale({scale})"))?;
        let bounds = NATIVE_PARTY_INVITATION_FRAME;
        let frame = message_frame(&self.document, &self.ui, bounds.width, bounds.height)?;
        style(
            &frame,
            &format!(
                "position:absolute;left:{}px;top:{}px;width:{}px;height:{}px;pointer-events:none",
                bounds.x, bounds.y, bounds.width, bounds.height
            ),
        )?;
        append(&stage, &frame)?;
        let description = element(&self.document, "p", "native-ui-sr-only")?;
        attribute(&description, "id", "social-party-invitation-body")?;
        description.set_text_content(Some(&body));
        append(&stage, &description)?;
        for (text, font, baseline, line_height) in [
            (NATIVE_PARTY_INVITATION_TITLE, NativeUiFontName::Menu, 95.0, None),
            (body.as_str(), NativeUiFontName::Medium, 130.5, Some(17.0)),
        ] {
            append(
                &stage,
                &native_text(
                    &self.document,
                    &NativeUiTextSpec {
                        align: NativeUiTextAlign::Left,
                        max_width: Some(400.0),
                        line_height,
                        ..NativeUiTextSpec::new(font, text, bounds.x + 76.0, bounds.y + baseline)
                    },
                    "",
                )?,
            )?;
        }
        let mut focused: Option<HtmlElement> = None;
        for ((label, answer), bounds) in [("ACCEPT", "accept"), ("DENY", "deny")]
            .into_iter()
            .zip(native_party_invitation_action_bounds())
        {
            let button = self.party_stock_action(
                label,
                &format!("party-invitation-{answer}:{id}"),
                bounds,
                1.0,
            )?;
            attribute(&button, "data-party-invitation-answer", answer)?;
            attribute(
                &button,
                if answer == "accept" { "data-game-default-focus" } else { "data-game-back" },
                "true",
            )?;
            let should_focus =
                if focus { answer == "accept" } else { retained_focus.as_deref() == Some(answer) };
            if should_focus {
                focused = Some(button.clone());
            }
            append(&stage, &button)?;
        }
        append(&self.party_invitation, &stage)?;
        self.shown_party_invitation = Some(id);
        if let Some(button) = focused {
            button.focus().map_err(|_| "Party invitation focus")?;
        }
        Ok(())
    }
}
