//! Website `MainMenuScene` player-card resolution and `CollegeInvitationDialog`.
//! The borrowed host records are presentation data. Website resolution and session
//! replacement leave through this queue and remain with the Game coordinator.

use solomon_protocol::hosted::{HostedCommand, HostedServerMessage};
use solomon_sim::social::{GameCollegeInvitation, GamePlayerCardProfile};
use web_sys::HtmlElement;

use super::{SocialUi, Surface, action_button, attribute};
use crate::ui::{append, element};

#[derive(Clone, Debug)]
pub enum HostedSocialAction {
    Command(Box<HostedCommand>),
    ResolveCollegeInvitation { invitation: GameCollegeInvitation },
}

impl SocialUi {
    /// True only while the actual hosted control-plane connection is installed.
    /// A direct native session keeps its existing snapshot/card command route.
    pub fn set_hosted_session(&mut self, hosted: bool) {
        if self.hosted == hosted {
            return;
        }
        self.hosted = hosted;
        self.hub_players.clear();
        self.hosted_card_request = None;
        self.college_invitations.clear();
        self.resolving_college = None;
        self.hosted_actions.clear();
        self.party_dirty = true;
    }

    pub fn take_hosted_actions(&mut self) -> Vec<HostedSocialAction> {
        std::mem::take(&mut self.hosted_actions)
    }

    /// False means this message belongs to another actual consumer (Lua, content,
    /// saves or admission); the caller must keep routing those messages there.
    pub fn receive_hosted_message(
        &mut self,
        message: &HostedServerMessage,
    ) -> Result<bool, String> {
        match message {
            HostedServerMessage::PlayerCard { profile, request_id } => {
                if self.hosted_card_request != Some(*request_id) {
                    return Ok(true);
                }
                self.hosted_card_request = None;
                self.requested_card = None;
                self.surface = profile.clone().map(Surface::Card);
                self.focus_modal = self.surface.is_some();
                self.party_dirty = true;
                self.hosted_notice.set_text_content(None);
            }
            HostedServerMessage::CollegeInvitations { invitations } => {
                if self.college_invitations != *invitations {
                    self.college_invitations.clone_from(invitations);
                    self.focus_college = true;
                    self.party_dirty = true;
                }
            }
            HostedServerMessage::PartyAction { accepted, reason, .. } => {
                self.party_status = if *accepted {
                    None
                } else {
                    Some(
                        reason
                            .map_or("That party is no longer available.", |reason| reason.text())
                            .to_owned(),
                    )
                };
                self.party_dirty = true;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub fn complete_college_invitation(
        &mut self,
        invitation_id: &str,
        result: &Result<(), String>,
    ) {
        if self.resolving_college.as_deref() != Some(invitation_id) {
            return;
        }
        self.resolving_college = None;
        self.dismiss_college_invitation(invitation_id);
        self.hosted_notice.set_text_content(result.as_ref().err().map(String::as_str));
        self.party_dirty = true;
    }

    pub(super) fn request_hosted_card(&mut self, by_reference: bool, target: &str) {
        if self.hosted_card_request.is_some() {
            return;
        }
        let reference = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot.player_cards.iter().find(|card| {
                    if by_reference {
                        card.player_reference == target
                    } else {
                        card.player_id == target
                    }
                })
            })
            .map_or_else(|| target.to_owned(), |card| card.player_reference.clone());
        self.next_hosted_request += 1;
        self.hosted_card_request = Some(self.next_hosted_request);
        self.hosted_notice.set_text_content(Some("Opening Player Card…"));
        self.hosted_actions.push(HostedSocialAction::Command(Box::new(
            HostedCommand::RequestPlayerCard {
                request_id: self.next_hosted_request,
                player_reference: reference,
            },
        )));
    }

    pub(super) fn invite_to_college(&mut self, card: &GamePlayerCardProfile) {
        self.hosted_actions.push(HostedSocialAction::Command(Box::new(
            HostedCommand::InviteCollege { player_reference: card.player_reference.clone() },
        )));
    }

    pub(super) fn college_visible(&self) -> bool {
        self.hosted
            && self.context.visible
            && !self.context.input_suspended
            && self.snapshot.is_some()
            && !matches!(self.surface, Some(Surface::Card(_) | Surface::HubPlayer(_)))
            && !self.college_invitations.is_empty()
    }

    pub(super) fn accept_college_invitation(&mut self) {
        if self.resolving_college.is_some() {
            return;
        }
        let Some(invitation) = self.college_invitations.first() else { return };
        self.resolving_college = Some(invitation.id.clone());
        self.hosted_notice.set_text_content(None);
        self.hosted_actions
            .push(HostedSocialAction::ResolveCollegeInvitation { invitation: invitation.clone() });
        self.party_dirty = true;
    }

    pub(super) fn decline_college_invitation(&mut self) {
        if self.resolving_college.is_some() {
            return;
        }
        let Some(invitation) = self.college_invitations.first() else { return };
        let id = invitation.id.clone();
        self.dismiss_college_invitation(&id);
    }

    fn dismiss_college_invitation(&mut self, invitation_id: &str) {
        self.college_invitations.retain(|invitation| invitation.id != invitation_id);
        self.hosted_actions.push(HostedSocialAction::Command(Box::new(
            HostedCommand::DismissCollegeInvitation { invitation_id: invitation_id.to_owned() },
        )));
        self.focus_college = true;
        self.party_dirty = true;
    }

    pub(super) fn draw_college_invitation(&mut self) -> Result<(), String> {
        self.college_dialog.set_text_content(None);
        self.college_dialog.set_hidden(!self.college_visible());
        if !self.college_visible() {
            return Ok(());
        }
        let invitation = &self.college_invitations[0];
        let dialog = element(&self.document, "section", "play-routing-dialog")?;
        attribute(&dialog, "role", "dialog")?;
        attribute(&dialog, "aria-modal", "true")?;
        attribute(&dialog, "aria-label", "Private College invitation")?;
        copy(&self.document, &dialog, "h2", "COLLEGE INVITATION")?;
        let invitation_text = element(&self.document, "p", "")?;
        copy(&self.document, &invitation_text, "strong", &invitation.inviter.display_name)?;
        copy(&self.document, &invitation_text, "span", " invited you to their private College.")?;
        append(&dialog, &invitation_text)?;
        copy(
            &self.document,
            &dialog,
            "p",
            "You will preview its mods and cheat policy before leaving your current session.",
        )?;
        let footer = element(&self.document, "footer", "")?;
        let busy = self.resolving_college.is_some();
        let decline = action_button(&self.document, "DECLINE", "college-decline", "")?;
        attribute(&decline, "data-game-back", "true")?;
        attribute(&decline, "data-college-focus", "true")?;
        decline
            .toggle_attribute_with_force("disabled", busy)
            .map_err(|_| "College decline availability")?;
        append(&footer, &decline)?;
        let accept = action_button(
            &self.document,
            if busy { "RESOLVING…" } else { "VIEW & JOIN" },
            "college-accept",
            "",
        )?;
        accept
            .toggle_attribute_with_force("disabled", busy)
            .map_err(|_| "College acceptance availability")?;
        append(&footer, &accept)?;
        append(&dialog, &footer)?;
        append(&self.college_dialog, &dialog)?;
        if self.focus_college && !busy {
            decline.focus().map_err(|_| "College invitation focus")?;
            self.focus_college = false;
        }
        Ok(())
    }
}

fn copy(
    document: &web_sys::Document,
    parent: &HtmlElement,
    tag: &str,
    text: &str,
) -> Result<(), String> {
    let node = element(document, tag, "")?;
    node.set_text_content(Some(text));
    append(parent, &node)
}
