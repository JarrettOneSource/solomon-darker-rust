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
        let screen = frame.ctx.content_rect();
        let width = (screen.width() - 36.0).min(680.0);
        let content_width = width - 52.0;
        let heading = crate::gpu_ui::font_run(
            frame.ctx,
            egui::FontId::new(24.0, egui::FontFamily::Name("Cinzel Bold".into())),
            egui::Color32::from_rgb(240, 210, 129),
            "COLLEGE INVITATION",
            Some(content_width),
        )?;
        let message_run = crate::gpu_ui::font_run_with_options(
            frame.ctx,
            egui::FontId::new(16.0, egui::FontFamily::Name("IM FELL English".into())),
            egui::Color32::from_rgb(222, 209, 172),
            &message,
            crate::gpu_ui::FontRunOptions {
                wrap_width: Some(content_width),
                line_height: Some(24.0),
                ..Default::default()
            },
        )?;
        let detail_run = crate::gpu_ui::font_run_with_options(
            frame.ctx,
            egui::FontId::new(16.0, egui::FontFamily::Name("IM FELL English".into())),
            egui::Color32::from_rgb(222, 209, 172),
            details,
            crate::gpu_ui::FontRunOptions {
                wrap_width: Some(content_width),
                line_height: Some(24.0),
                ..Default::default()
            },
        )?;
        let accept_label = if busy { "RESOLVING…" } else { "VIEW & JOIN" };
        let button_sizes = [
            crate::menu::routing::consent_button_size(frame.ctx, "DECLINE")?,
            crate::menu::routing::consent_button_size(frame.ctx, accept_label)?,
        ];
        let wrapped_footer = button_sizes[0].x + button_sizes[1].x + 8.0 > content_width;
        let footer_height = if wrapped_footer { 96.0 } else { 44.0 };
        let body_height = heading.size().y + 12.0 + message_run.size().y + detail_run.size().y;
        let panel_height = (body_height + 22.0 + footer_height + 52.0).min(screen.height() - 36.0);
        let bounds = egui::Rect::from_center_size(screen.center(), egui::vec2(width, panel_height));
        ui.interact(bounds, egui::Id::unique("social/college-panel-block"), egui::Sense::click());
        crate::menu::routing::consent_frame(frame, ui, bounds)?;
        let content = bounds.shrink(26.0);
        let mut response = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| -> Result<(), String> {
            ui.set_clip_rect(content.intersect(ui.clip_rect()));
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            egui::ScrollArea::vertical()
                .id_salt("social/college-scroll")
                .max_height(content.height())
                .min_scrolled_height(0.0)
                .auto_shrink([false, true])
                .show(ui, |ui| -> Result<(), String> {
                    let width = ui.available_width();
                    for (id, text, run, align) in [
                        ("heading", "COLLEGE INVITATION", &heading, egui::Align::Center),
                        ("message", message.as_str(), &message_run, egui::Align::Min),
                        ("details", details, &detail_run, egui::Align::Min),
                    ] {
                        let (_, rect) = ui.allocate_space(egui::vec2(width, run.size().y));
                        run.paint_aligned(frame, ui.painter(), rect, align, 1.0)?;
                        frame.record_rect(
                            &format!("social/college-{id}"),
                            text,
                            if id == "heading" { "heading" } else { "text" },
                            rect,
                            true,
                        );
                        if id == "heading" {
                            ui.add_space(12.0);
                        }
                    }
                    ui.add_space(22.0);
                    let (_, footer) = ui.allocate_space(egui::vec2(width, footer_height));
                    let total_width = button_sizes[0].x + 8.0 + button_sizes[1].x;
                    for (index, (id, label, size)) in [
                        ("social/college-decline", "DECLINE", button_sizes[0]),
                        ("social/college-accept", accept_label, button_sizes[1]),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let origin = if wrapped_footer {
                            egui::pos2(
                                footer.center().x - size.x / 2.0,
                                footer.top() + index as f32 * 52.0,
                            )
                        } else {
                            egui::pos2(
                                footer.center().x - total_width / 2.0
                                    + if index == 0 { 0.0 } else { button_sizes[0].x + 8.0 },
                                footer.top(),
                            )
                        };
                        let action = crate::menu::routing::consent_button(
                            frame,
                            ui,
                            id,
                            label,
                            egui::Rect::from_min_size(origin, size),
                            !busy,
                        )?;
                        if index == 0 && self.focus_college && !busy && !ui.is_sizing_pass() {
                            action.request_focus();
                            self.focus_college = false;
                        }
                        if action.clicked() {
                            response = Some(index == 1);
                        }
                    }
                    Ok(())
                })
                .inner?;
            Ok(())
        })
        .inner?;
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
