//! GPU Join Party preserves `JoinPartyScene.tsx`, `party-directory.ts` and
//! `party-join.ts`. Only the Website coordinator admits actual sessions.
use super::HostedMenuAction;
use crate::gpu_ui::UiFrame;
use crate::hosted::{PartyActivity, PublicGameParty};
use egui::{Rect, Ui};
use solomon_sim::menu::party_directory::{
    DirectoryPartyAction, complete_party_code, directory_party_action, normalize_party_code,
};
mod presentation;

pub(super) struct JoinView {
    code: String,
    parties: Vec<PublicGameParty>,
    active: bool,
    loading: bool,
    busy: bool,
    next_request: u64,
    directory_request: Option<u64>,
    join_request: Option<u64>,
    pending_listing: Option<String>,
    directory_error: Option<String>,
    join_error: Option<String>,
    refresh_at: f64,
    viewport_height: f64,
    actions: Vec<HostedMenuAction>,
}
impl JoinView {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            code: String::new(),
            parties: Vec::new(),
            active: false,
            loading: false,
            busy: false,
            next_request: 0,
            directory_request: None,
            join_request: None,
            pending_listing: None,
            directory_error: None,
            join_error: None,
            refresh_at: 0.0,
            viewport_height: 0.0,
            actions: Vec::new(),
        })
    }
    pub fn open(&mut self, now: f64) -> Result<(), String> {
        self.active = true;
        self.code.clear();
        self.parties.clear();
        self.busy = false;
        self.join_request = None;
        self.pending_listing = None;
        self.join_error = None;
        self.directory_error = None;
        self.refresh(now)
    }

    pub fn close(&mut self) {
        self.active = false;
        self.directory_request = None;
        self.actions.clear();
        if let Some(request_id) = self.join_request.take() {
            self.actions.push(HostedMenuAction::CancelPartyJoin { request_id });
        }
        self.busy = false;
    }

    pub fn update(&mut self, now: f64, viewport_height: f64) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        if now >= self.refresh_at {
            self.refresh(now)?;
        }
        self.viewport_height = viewport_height;
        Ok(())
    }

    pub fn activate(&mut self, action: &str, now: f64) -> Result<bool, String> {
        if action == "join-refresh" {
            self.refresh(now)?;
            return Ok(true);
        }
        if action == "join-code" {
            let code = normalize_party_code(&self.code);
            if !self.busy && complete_party_code(&code) {
                let request_id = self.begin_join(None);
                self.actions.push(HostedMenuAction::ResolvePartyCode { request_id, code });
            }
            return Ok(true);
        }
        let Some(listing_id) = action.strip_prefix("join-listing:") else {
            return Ok(false);
        };
        if self.busy {
            return Ok(true);
        }
        let Some(party) = self.parties.iter().find(|party| party.id == listing_id) else {
            return Ok(true);
        };
        let action = directory_party_action(
            party.visibility,
            matches!(party.activity, PartyActivity::Playing { .. }),
        );
        if action == DirectoryPartyAction::Wait {
            return Ok(true);
        }
        let request_id = self.begin_join(if action == DirectoryPartyAction::Request {
            Some(listing_id.to_owned())
        } else {
            None
        });
        let listing_id = listing_id.to_owned();
        self.actions.push(match action {
            DirectoryPartyAction::Join => {
                HostedMenuAction::JoinPublicParty { request_id, listing_id }
            }
            DirectoryPartyAction::Request => {
                HostedMenuAction::RequestPartyInvite { request_id, listing_id }
            }
            DirectoryPartyAction::Wait => {
                unreachable!("playing listings returned before starting admission")
            }
        });
        Ok(true)
    }

    pub fn complete_directory(
        &mut self,
        request_id: u64,
        result: Result<Vec<PublicGameParty>, String>,
    ) -> Result<bool, String> {
        if !self.active || self.directory_request != Some(request_id) {
            return Ok(false);
        }
        self.directory_request = None;
        self.loading = false;
        match result {
            Ok(parties) => {
                self.parties = parties;
                self.directory_error = None;
            }
            Err(error) => self.directory_error = Some(error),
        }
        Ok(true)
    }

    /// Success only ends the request's busy state. Root retains the real resolution
    /// and chooses the source consent/loading flow; a view never admits membership.
    pub fn complete_join(
        &mut self,
        request_id: u64,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        if !self.active || self.join_request != Some(request_id) {
            return Ok(false);
        }
        self.join_request = None;
        self.pending_listing = None;
        self.busy = false;
        self.join_error = result.err();
        Ok(true)
    }

    pub fn take_actions(&mut self) -> Vec<HostedMenuAction> {
        std::mem::take(&mut self.actions)
    }

    fn refresh(&mut self, now: f64) -> Result<(), String> {
        self.next_request += 1;
        self.directory_request = Some(self.next_request);
        self.refresh_at = now + 15_000.0;
        self.loading = true;
        self.actions.push(HostedMenuAction::LoadParties { request_id: self.next_request });
        Ok(())
    }

    fn begin_join(&mut self, listing: Option<String>) -> u64 {
        self.next_request += 1;
        self.join_request = Some(self.next_request);
        self.pending_listing = listing;
        self.join_error = None;
        self.busy = true;
        self.next_request
    }

    pub fn paint_gpu(
        &mut self,
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
    fn closing_the_directory_cancels_admission_and_rejects_late_results() {
        let mut join = JoinView::new().expect("pure directory state");
        join.open(100.0).expect("open directory");
        let request = join.take_actions();
        assert_eq!(request, vec![HostedMenuAction::LoadParties { request_id: 1 }]);
        join.code = "ABCD-EFGH".to_owned();
        join.activate("join-code", 100.0).expect("enter party code");
        assert_eq!(
            join.take_actions(),
            vec![HostedMenuAction::ResolvePartyCode {
                request_id: 2,
                code: "ABCD-EFGH".to_owned()
            }]
        );
        join.close();
        assert_eq!(join.take_actions(), vec![HostedMenuAction::CancelPartyJoin { request_id: 2 }]);
        assert!(!join.complete_join(2, Ok(())).expect("late admission"));
        assert!(!join.complete_directory(1, Ok(Vec::new())).expect("late directory"));
    }

    #[test]
    fn a_new_refresh_supersedes_the_old_directory_without_replaying_admission() {
        let mut join = JoinView::new().expect("pure directory state");
        join.open(0.0).expect("open directory");
        join.take_actions();
        join.update(15_000.0, 900.0).expect("automatic refresh");
        assert_eq!(join.take_actions(), vec![HostedMenuAction::LoadParties { request_id: 2 }]);
        assert!(!join.complete_directory(1, Err("old failure".to_owned())).expect("stale result"));
        assert!(join.complete_directory(2, Ok(Vec::new())).expect("current result"));
        assert_eq!(join.directory_error, None);
        assert!(!join.loading);
    }
}
