//! GPU Join Party preserves `JoinPartyScene.tsx`, `party-directory.ts` and
//! `party-join.ts`. Only the Website coordinator admits actual sessions.
use super::HostedMenuAction;
use super::widgets;
use crate::gpu_ui::UiFrame;
use crate::hosted::{PartyActivity, PublicGameParty};
use egui::{Align2, Rect, Ui, Vec2};
use solomon_sim::menu::party_directory::{
    DirectoryPartyAction, complete_party_code, directory_party_action,
    directory_party_presentation, normalize_party_code,
};
use solomon_sim::social::GameSessionKind;
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
        mut view: Rect,
        actions: &mut Vec<String>,
    ) -> Result<(), String> {
        view.max.y = view.min.y + view.height().min(self.viewport_height as f32);
        widgets::gradient(ui, view, widgets::color(0x2b2212), widgets::color(0x040508));
        let compact = view.width() < 700.0 || view.height() < 620.0;
        let pad = if compact { 12.0 } else { 24.0 };
        widgets::label(
            ui,
            "JOIN PARTY",
            view.min + Vec2::new(view.width() / 2.0, pad),
            Align2::CENTER_TOP,
            "Cinzel Bold",
            if compact { 24.0 } else { 30.0 },
            widgets::color(0xf4d887),
        );
        widgets::label(
            ui,
            "Enter a Party ID or join a listed party.",
            view.min + Vec2::new(view.width() / 2.0, pad + 40.0),
            Align2::CENTER_TOP,
            "IM FELL English",
            16.0,
            widgets::color(0xaaa088),
        );
        let width = 560.0_f32.min(view.width() - pad * 2.0);
        let left = view.center().x - width / 2.0;
        let code_top = view.top() + pad + 72.0;
        widgets::label(
            ui,
            "PARTY ID",
            egui::pos2(left, code_top),
            Align2::LEFT_TOP,
            "Cinzel Bold",
            11.0,
            widgets::GOLD,
        );
        let field =
            Rect::from_min_size(egui::pos2(left, code_top + 20.0), Vec2::new(width - 90.0, 44.0));
        ui.painter().rect_filled(field, 0.0, widgets::color(0x080704));
        ui.painter().rect_stroke(
            field,
            0.0,
            egui::Stroke::new(1.0, widgets::color(0x947b42)),
            egui::StrokeKind::Inside,
        );
        let edited = widgets::edit(
            frame,
            ui,
            "join-party-code",
            "Party ID",
            &mut self.code,
            field,
            "JetBrains Mono Bold",
            20.0,
            !self.busy,
            false,
        );
        if edited.changed() {
            self.code = normalize_party_code(&self.code);
        }
        if edited.has_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter))
            && complete_party_code(&self.code)
            && !self.busy
        {
            actions.push("join-code".to_owned());
        }
        let rect = Rect::from_min_size(
            egui::pos2(field.right() + 8.0, field.top()),
            Vec2::new(82.0, 44.0),
        );
        if widgets::flat_button(
            frame,
            ui,
            "join-code",
            "JOIN",
            rect,
            !self.busy && complete_party_code(&self.code),
        )
        .clicked()
            && !self.busy
            && complete_party_code(&self.code)
        {
            actions.push("join-code".to_owned());
        }
        let error = self.join_error.as_deref().or(self.directory_error.as_deref());
        if let Some(error) = error {
            widgets::label(
                ui,
                error,
                egui::pos2(view.center().x, field.bottom() + 12.0),
                Align2::CENTER_TOP,
                "IM FELL English",
                16.0,
                widgets::ERROR,
            );
        }
        let list = Rect::from_min_max(
            egui::pos2(
                view.center().x - (900.0_f32.min(view.width() - pad * 2.0)) / 2.0,
                field.bottom() + if error.is_some() { 48.0 } else { 20.0 },
            ),
            egui::pos2(
                view.center().x + (900.0_f32.min(view.width() - pad * 2.0)) / 2.0,
                view.bottom() - pad - 58.0,
            ),
        );
        ui.painter().rect_filled(list, 0.0, egui::Color32::from_black_alpha(90));
        ui.painter().rect_stroke(
            list,
            0.0,
            egui::Stroke::new(1.0, widgets::alpha(0xbf9d4e, 0.45)),
            egui::StrokeKind::Inside,
        );
        frame.record_rect("join-party-list", "Party directory", "region", list, true);
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("join-list")
                .max_rect(list)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(list);
        egui::ScrollArea::vertical().id_salt("join-party-list").auto_shrink([false, false]).show(
            &mut child,
            |ui| {
                if self.parties.is_empty() {
                    let text = if self.loading {
                        "CONSULTING THE DARK CLOUD…"
                    } else {
                        "NO PARTIES ARE FORMING RIGHT NOW."
                    };
                    ui.add_space(32.0);
                    widgets::label(
                        ui,
                        text,
                        egui::pos2(list.center().x, ui.cursor().top()),
                        Align2::CENTER_TOP,
                        "IM FELL English",
                        16.0,
                        widgets::GOLD,
                    );
                    ui.add_space(48.0);
                }
                for party in &self.parties {
                    let height = if compact { 128.0 } else { 92.0 };
                    let (_, row) = ui.allocate_space(Vec2::new(list.width() - 16.0, height));
                    ui.painter().line_segment(
                        [row.left_bottom(), row.right_bottom()],
                        egui::Stroke::new(1.0, widgets::alpha(0xbf9d4e, 0.2)),
                    );
                    widgets::label(
                        ui,
                        &party.leader.to_uppercase(),
                        row.min + Vec2::new(12.0, 10.0),
                        Align2::LEFT_TOP,
                        "IM FELL English",
                        18.0,
                        widgets::color(0xead9aa),
                    );
                    let mut flags = Vec::new();
                    if party.session_kind == GameSessionKind::PrivateCollege {
                        flags.push("PRIVATE COLLEGE".to_owned());
                    }
                    if party.mod_count > 0 {
                        flags.push(format!("MODDED · {}", party.mod_count));
                    }
                    if party.cheats_enabled {
                        flags.push("CHEATS".to_owned());
                    }
                    widgets::label(
                        ui,
                        &flags.join(" · "),
                        row.min + Vec2::new(12.0, 34.0),
                        Align2::LEFT_TOP,
                        "Cinzel Bold",
                        8.0,
                        widgets::color(0xc8ad69),
                    );
                    widgets::label(
                        ui,
                        &party.members.join(" · "),
                        row.min + Vec2::new(12.0, 50.0),
                        Align2::LEFT_TOP,
                        "IM FELL English",
                        14.0,
                        widgets::color(0x9e9580),
                    );
                    let location = match &party.activity {
                        PartyActivity::Hub => None,
                        PartyActivity::Playing { boneyard_name } => Some(boneyard_name.as_str()),
                    };
                    let p = directory_party_presentation(
                        party.member_count,
                        party.max_members,
                        location,
                    );
                    widgets::label(
                        ui,
                        &format!("{} · {}", p.status, p.location),
                        row.min + Vec2::new(12.0, 68.0),
                        Align2::LEFT_TOP,
                        "IM FELL English",
                        14.0,
                        widgets::color(0xb6a77f),
                    );
                    let action = directory_party_action(party.visibility, location.is_some());
                    let label = if action == DirectoryPartyAction::Wait {
                        "IN GAME"
                    } else if self.pending_listing.as_deref() == Some(party.id.as_str()) {
                        "REQUESTED"
                    } else if action == DirectoryPartyAction::Request {
                        "REQUEST"
                    } else {
                        "JOIN"
                    };
                    let r = Rect::from_min_size(
                        if compact {
                            row.min + Vec2::new(row.width() - 110.0, 80.0)
                        } else {
                            egui::pos2(row.right() - 110.0, row.center().y - 22.0)
                        },
                        Vec2::new(98.0, 44.0),
                    );
                    widgets::label(
                        ui,
                        &p.squad,
                        r.min - Vec2::new(16.0, -22.0),
                        Align2::RIGHT_CENTER,
                        "IM FELL English",
                        16.0,
                        widgets::color(0xcbb985),
                    );
                    let id = format!("join-listing:{}", party.id);
                    let enabled = !self.busy && action != DirectoryPartyAction::Wait;
                    if widgets::flat_button(frame, ui, &id, label, r, enabled).clicked() && enabled
                    {
                        actions.push(id);
                    }
                }
            },
        );
        for (label, action, x) in [
            ("BACK", "join-back", view.center().x - 110.0),
            ("REFRESH", "join-refresh", view.center().x + 4.0),
        ] {
            if widgets::flat_button(
                frame,
                ui,
                action,
                label,
                Rect::from_min_size(
                    egui::pos2(x, view.bottom() - pad - 44.0),
                    Vec2::new(106.0, 44.0),
                ),
                true,
            )
            .clicked()
            {
                actions.push(action.to_owned());
            }
        }
        Ok(())
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
