//! GPU versions of `PartyJoinConsentDialog`, `ModdedPlayDialog` and
//! `GameSaveModMismatchDialog`. Only the coordinator owns actual admission.
use super::HostedMenuAction;
use crate::gpu_ui::UiFrame;
use crate::hosted::{ActiveWebMod, PartyJoinTarget};
use egui::{Rect, Ui};

use solomon_sim::menu::save_mods::GameSaveModMismatch;
use std::cell::RefCell;

mod presentation;
pub struct RoutingDownloadProgress<'a> {
    pub completed_bytes: u64,
    pub total_bytes: u64,
    pub active_mod_id: Option<&'a str>,
}
enum Prompt {
    Party { target: PartyJoinTarget, requires_vanilla: bool, signed_in: bool },
    Modded { active_mods: Vec<ActiveWebMod>, cheats_enabled: bool },
    Mismatch(GameSaveModMismatch),
    Cheats,
}
pub(super) struct RoutingView {
    request_id: Option<u64>,
    prompt: Option<Prompt>,
    busy: bool,
    progress: RefCell<Option<(u64, u64, Option<String>)>>,
    error: Option<String>,
    actions: Vec<HostedMenuAction>,
}
impl RoutingView {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            request_id: None,
            prompt: None,
            busy: false,
            progress: RefCell::new(None),
            error: None,
            actions: Vec::new(),
        })
    }
    pub fn show_party(
        &mut self,
        request_id: u64,
        target: PartyJoinTarget,
        requires_vanilla: bool,
        signed_in: bool,
    ) -> Result<(), String> {
        self.show(request_id, Prompt::Party { target, requires_vanilla, signed_in })
    }

    pub fn show_modded(
        &mut self,
        request_id: u64,
        active_mods: Vec<ActiveWebMod>,
        cheats_enabled: bool,
    ) -> Result<(), String> {
        self.show(request_id, Prompt::Modded { active_mods, cheats_enabled })
    }

    pub fn show_mismatch(
        &mut self,
        request_id: u64,
        mismatch: GameSaveModMismatch,
    ) -> Result<(), String> {
        self.show(request_id, Prompt::Mismatch(mismatch))
    }

    pub fn show_cheats(&mut self) -> Result<(), String> {
        // This prompt has no asynchronous request. The retained scene owner alone
        // commits the preference and leaves after explicit continuation.
        self.show(0, Prompt::Cheats)
    }

    pub fn active(&self) -> bool {
        self.prompt.is_some()
    }

    fn show(&mut self, request_id: u64, prompt: Prompt) -> Result<(), String> {
        self.request_id = Some(request_id);
        self.prompt = Some(prompt);
        self.busy = false;
        self.error = None;
        self.progress.borrow_mut().take();
        Ok(())
    }

    /// A successful coordinator transition unmounts the source dialog. Closing
    /// invalidates late progress/completion without cancelling an accepted session.
    pub fn close(&mut self) {
        self.request_id = None;
        self.prompt = None;
        self.busy = false;
        self.error = None;
        self.progress.borrow_mut().take();
    }

    pub fn back(&mut self) -> bool {
        if self.busy || !self.active() {
            return false;
        }
        if matches!(self.prompt, Some(Prompt::Cheats)) {
            self.close();
            return true;
        }
        let request_id = self.request_id.expect("active routing has its request");
        self.actions.push(match self.prompt {
            Some(Prompt::Party { .. }) => HostedMenuAction::CancelPartyJoin { request_id },
            Some(Prompt::Modded { .. }) => HostedMenuAction::CancelModdedPlay { request_id },
            Some(Prompt::Mismatch(_)) => HostedMenuAction::CancelSaveModMismatch { request_id },
            Some(Prompt::Cheats) => unreachable!("the local cheat prompt closed above"),
            None => unreachable!("active routing has its prompt"),
        });
        self.close();
        true
    }

    pub fn activate(&mut self, action: &str) -> Result<bool, String> {
        if !self.active() {
            return Ok(false);
        }
        if action == "routing-back" {
            self.back();
            return Ok(true);
        }
        if self.busy {
            return Ok(true);
        }
        if matches!(self.prompt, Some(Prompt::Cheats)) && action == "routing-continue" {
            self.actions.push(HostedMenuAction::ContinueCheatsInPrivateCollege);
            self.close();
            return Ok(true);
        }
        let request_id = self.request_id.expect("active routing has its request");
        let action = match (self.prompt.as_ref(), action) {
            (Some(Prompt::Party { .. }), "routing-continue") => {
                HostedMenuAction::ContinuePartyJoin { request_id }
            }
            (Some(Prompt::Modded { .. }), "routing-continue") => {
                HostedMenuAction::ContinueModdedPlay { request_id }
            }
            (Some(Prompt::Modded { .. }), "routing-vanilla") => {
                HostedMenuAction::PlayVanilla { request_id }
            }
            (Some(Prompt::Mismatch(_)), "routing-continue") => {
                HostedMenuAction::ContinueSaveModMismatch { request_id }
            }
            _ => return Ok(true),
        };
        self.actions.push(action);
        self.busy = true;
        Ok(true)
    }

    pub fn progress(
        &self,
        request_id: u64,
        progress: Option<RoutingDownloadProgress<'_>>,
    ) -> Result<bool, String> {
        if self.request_id != Some(request_id) {
            return Ok(false);
        }
        *self.progress.borrow_mut() = progress
            .filter(|p| p.total_bytes > 0)
            .map(|p| (p.completed_bytes, p.total_bytes, p.active_mod_id.map(str::to_owned)));
        Ok(true)
    }

    pub fn complete(
        &mut self,
        request_id: u64,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        if self.request_id != Some(request_id) {
            return Ok(false);
        }
        match result {
            Ok(()) => self.close(),
            Err(message) => {
                self.busy = false;
                self.error = Some(message);
            }
        }
        Ok(true)
    }

    pub fn take_actions(&mut self) -> Vec<HostedMenuAction> {
        std::mem::take(&mut self.actions)
    }

    pub fn paint(
        &self,
        frame: &mut UiFrame<'_>,
        ui: &mut Ui,
        view: Rect,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        presentation::paint(self, frame, ui, view, actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_college_cheats_wait_for_an_explicit_continue() {
        let mut routing = RoutingView::new().expect("pure routing state");
        routing.show_cheats().expect("open local consent");
        assert!(routing.take_actions().is_empty());
        routing.activate("routing-continue").expect("explicit continuation");
        assert_eq!(routing.take_actions(), vec![HostedMenuAction::ContinueCheatsInPrivateCollege]);
        assert!(!routing.active());
        assert!(!routing.complete(0, Err("late callback".to_owned())).expect("closed completion"));
    }

    #[test]
    fn cancelling_a_local_cheat_prompt_does_not_request_a_setting_change() {
        let mut routing = RoutingView::new().expect("pure routing state");
        routing.show_cheats().expect("open consent");
        assert!(routing.back());
        assert!(routing.take_actions().is_empty());
        assert!(!routing.active());
    }
}
