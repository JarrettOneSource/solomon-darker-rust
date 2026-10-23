//! Website `MainMenuScene` player-card resolution and `CollegeInvitationDialog`.
//! The borrowed host records are presentation data. Website resolution and session
//! replacement leave through this queue and remain with the Game coordinator.

use solomon_protocol::hosted::{HostedCommand, HostedServerMessage};
use solomon_sim::social::{GameCollegeInvitation, GamePlayerCardProfile};

use super::{SocialUi, Surface};

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
                self.hosted_notice = None;
            }
            HostedServerMessage::CollegeInvitations { invitations } => {
                if self.college_invitations != *invitations {
                    self.college_invitations.clone_from(invitations);
                    self.focus_college = true;
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
        self.hosted_notice = result.as_ref().err().cloned();
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
        self.hosted_notice = Some("Opening Player Card…".into());
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
        self.hosted_notice = None;
        self.hosted_actions
            .push(HostedSocialAction::ResolveCollegeInvitation { invitation: invitation.clone() });
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
    }

    pub(super) fn paint_college_invitation(
        &mut self,
        frame: &mut crate::gpu_ui::UiFrame<'_>,
        ui: &mut egui::Ui,
    ) -> Result<(), String> {
        let Some(invitation) = self.college_invitations.first() else { return Ok(()) };
        let inviter = invitation.inviter.display_name.clone();
        let message = format!("{inviter} invited you to their private College.");
        let details =
            "You will preview its mods and cheat policy before leaving your current session.";
        let busy = self.resolving_college.is_some();
        let width = (self.size.0 as f32 - 40.0).clamp(220.0, 560.0);
        let bounds = egui::Rect::from_center_size(
            frame.ctx.content_rect().center(),
            egui::vec2(width, 260.0),
        );
        ui.interact(bounds, egui::Id::unique("social/college-panel-block"), egui::Sense::click());
        let response = ui
            .scope_builder(egui::UiBuilder::new().max_rect(bounds), |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(12, 13, 20))
                    .stroke(egui::Stroke::new(1.0, super::gpu::GOLD))
                    .inner_margin(24.0)
                    .show(ui, |ui| {
                        ui.set_min_width(width - 48.0);
                        ui.label(super::gpu::rich("COLLEGE INVITATION", 21.0, super::gpu::GOLD));
                        ui.add_space(18.0);
                        ui.label(super::gpu::rich(&message, 17.0, super::gpu::PALE));
                        ui.add_space(12.0);
                        ui.label(super::gpu::rich(details, 15.0, super::gpu::PALE));
                        ui.add_space(20.0);
                        let mut action = None;
                        ui.horizontal(|ui| {
                            let decline = super::gpu::browser_button(
                                frame,
                                ui,
                                "college-decline",
                                "DECLINE",
                                egui::vec2(140.0, 36.0),
                                !busy,
                            );
                            if self.focus_college && !busy {
                                decline.request_focus();
                                self.focus_college = false;
                            }
                            if decline.clicked() {
                                action = Some(false);
                            }
                            if super::gpu::browser_button(
                                frame,
                                ui,
                                "college-accept",
                                if busy { "RESOLVING…" } else { "VIEW & JOIN" },
                                egui::vec2(160.0, 36.0),
                                !busy,
                            )
                            .clicked()
                            {
                                action = Some(true);
                            }
                        });
                        action
                    })
                    .inner
            })
            .inner;
        frame.record_rect(
            "social/college-invitation",
            "Private College invitation",
            "dialog",
            bounds,
            true,
        );
        frame.record_value("social/college-invitation", &format!("{message} {details}"), None);
        match response {
            Some(true) => self.accept_college_invitation(),
            Some(false) => self.decline_college_invitation(),
            None => {}
        }
        Ok(())
    }
}
