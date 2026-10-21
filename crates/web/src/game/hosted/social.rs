//! `MainMenuScene` resolves a received College invitation through the actual Website API.

use super::*;
use crate::social::HostedSocialAction;
use solomon_sim::social::GameSessionKind;

impl Game {
    pub(super) fn poll_hosted_social(&mut self) -> Result<(), String> {
        for action in self.social.take_hosted_actions() {
            match action {
                HostedSocialAction::Command(command) => {
                    self.send_native_message(&ClientMessage::Hosted { command: *command })?;
                }
                HostedSocialAction::ResolveCollegeInvitation { invitation } => {
                    self.hosted
                        .as_mut()
                        .unwrap()
                        .pending
                        .push(HostedRequest::CollegeInvitation(invitation));
                }
            }
        }
        Ok(())
    }

    pub(super) fn complete_hosted_college_invitation(
        &mut self,
        invitation_id: &str,
        result: Result<PartyJoinResolution, String>,
    ) -> Result<(), String> {
        let result = result.and_then(|resolution| {
            if resolution.target.kind == GameSessionKind::GlobalHub
                && !game_shared_hub_enabled(self.menu.settings())
            {
                Err("The shared Hub is disabled in Game Settings.".into())
            } else {
                Ok(resolution)
            }
        });
        self.social.complete_college_invitation(
            invitation_id,
            &result.as_ref().map(|_| ()).map_err(Clone::clone),
        );
        if let Ok(resolution) = result {
            let hosted = self.hosted.as_mut().unwrap();
            hosted.routing_sequence += 1;
            let request_id = hosted.routing_sequence;
            self.offer_resolved_party(request_id, resolution)?;
        }
        Ok(())
    }
}
