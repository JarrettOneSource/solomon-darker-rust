//! GPU versions of `PartyJoinConsentDialog`, `ModdedPlayDialog` and
//! `GameSaveModMismatchDialog`. Only the coordinator owns actual admission.
use super::{HostedMenuAction, widgets};
use crate::gpu_ui::UiFrame;
use crate::hosted::{ActiveWebMod, PartyJoinTarget};
use egui::{Align2, Color32, Rect, Ui, Vec2};
use solomon_sim::menu::save_mods::GameSaveModMismatch;
use solomon_sim::social::GameSessionKind;
use std::cell::RefCell;
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
        let Some(prompt) = &self.prompt else { return Ok(()) };
        ui.painter().rect_filled(view, 0.0, Color32::from_black_alpha(210));
        let width = 680.0_f32.min(view.width() - 32.0);
        let panel = Rect::from_center_size(
            view.center(),
            Vec2::new(width, 640.0_f32.min(view.height() - 40.0)),
        );
        widgets::gradient(ui, panel, widgets::color(0x241e13), widgets::color(0x080706));
        ui.painter().rect_stroke(
            panel,
            0.0,
            egui::Stroke::new(2.0, widgets::color(0x947b42)),
            egui::StrokeKind::Inside,
        );
        let inner = panel.shrink2(Vec2::new(24.0, 22.0));
        let mut lines = Vec::<(String, bool)>::new();
        let mut vanilla = None;
        let (heading, continue_label) = match prompt {
            Prompt::Party { target, requires_vanilla, signed_in } => {
                lines.push((if target.kind==GameSessionKind::GlobalHub{"This is a vanilla global-Hub party.".to_owned()}else{"The room decides its content and cheat policy. Your unrelated mods and settings are ignored for this session.".to_owned()},false));
                if !target.content.mods.is_empty() {
                    lines.push((
                        format!(
                            "{} {}",
                            target.content.mods.len(),
                            if target.content.mods.len() == 1 { "MOD" } else { "MODS" }
                        ),
                        true,
                    ));
                    for content in &target.content.mods {
                        lines.push((format!("{} · v{}", content.name, content.version), false));
                    }
                }
                if target.cheats_enabled {
                    lines.push(("CHEATS ENABLED FOR THIS COLLEGE".to_owned(), true));
                }
                if target.kind == GameSessionKind::PrivateCollege {
                    lines.push(("LOCAL HALL ONLY · GLOBAL SCORES OFF".to_owned(), true));
                }
                if *requires_vanilla {
                    lines.push(("ACTIVE MODS AND CHEATS WILL BE DISABLED".to_owned(), true));
                }
                (
                    format!("{}'S COLLEGE", target.leader.to_uppercase()),
                    if self.busy {
                        "PREPARING…"
                    } else if *requires_vanilla {
                        "DISABLE & JOIN"
                    } else if !target.content.mods.is_empty() && *signed_in {
                        "SYNC MODS & JOIN"
                    } else if !target.content.mods.is_empty() {
                        "DOWNLOAD & JOIN ONCE"
                    } else {
                        "JOIN PARTY"
                    },
                )
            }
            Prompt::Modded { active_mods, cheats_enabled } => {
                lines.push(("Mods and cheats use a private College. These runs stay in your local Hall of Fame.".to_owned(),false));
                if !active_mods.is_empty() {
                    lines.push(("ACTIVE MODS".to_owned(), true));
                    for content in active_mods {
                        lines.push((format!("{} · v{}", content.name, content.version), false));
                    }
                }
                if *cheats_enabled {
                    lines.push(("CHEATS ENABLED".to_owned(), true));
                }
                vanilla = Some(if !active_mods.is_empty() && *cheats_enabled {
                    "DISABLE MODS & CHEATS"
                } else if !active_mods.is_empty() {
                    "DISABLE ALL MODS"
                } else {
                    "DISABLE CHEATS"
                });
                (
                    "LOCAL PLAY IS ACTIVE".to_owned(),
                    if self.busy { "PREPARING…" } else { "CONTINUE LOCAL" },
                )
            }
            Prompt::Cheats => {
                lines.push((
                    "Leave the global Hub, then use Last Game to continue this wizard locally."
                        .to_owned(),
                    false,
                ));
                ("CHEATS USE PRIVATE COLLEGES".to_owned(), "LEAVE & CONTINUE")
            }
            Prompt::Mismatch(m) => {
                lines.push(("This save was made with a different Dark Cloud. Continue with the currently enabled mods?".to_owned(),false));
                for (title, rows) in [
                    (
                        "ADDED",
                        m.added
                            .iter()
                            .map(|e| format!("{} v{}", e.id, e.version))
                            .collect::<Vec<_>>(),
                    ),
                    (
                        "MISSING",
                        m.removed.iter().map(|e| format!("{} v{}", e.id, e.version)).collect(),
                    ),
                    (
                        "CHANGED",
                        m.changed
                            .iter()
                            .map(|e| {
                                format!(
                                    "{} v{} → v{}",
                                    e.saved.id, e.saved.version, e.active.version
                                )
                            })
                            .collect(),
                    ),
                ] {
                    if !rows.is_empty() {
                        lines.push((title.to_owned(), true));
                        for row in rows {
                            lines.push((row, false));
                        }
                    }
                }
                lines.push(("Missing or changed mod state will be discarded. If the save is inside a missing mod Boneyard, the wizard returns to the College.".to_owned(),true));
                ("THE MOD LIST HAS CHANGED".to_owned(), "CONTINUE")
            }
        };
        frame.record_rect("routing/dialog", &heading, "dialog", panel, !self.busy);
        frame.record_value(
            "routing/dialog",
            &std::iter::once(heading.as_str())
                .chain(lines.iter().map(|(text, _)| text.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
            None,
        );
        widgets::label(
            ui,
            &heading,
            egui::pos2(panel.center().x, inner.top()),
            Align2::CENTER_TOP,
            "Cinzel Bold",
            20.0,
            widgets::GOLD,
        );
        let bottom = panel.bottom() - if vanilla.is_some() { 126.0 } else { 72.0 };
        let content =
            Rect::from_min_max(inner.min + Vec2::new(0.0, 48.0), egui::pos2(inner.right(), bottom));
        let mut child =
            ui.new_child(egui::UiBuilder::new().id_salt("routing-content").max_rect(content));
        child.set_clip_rect(content);
        egui::ScrollArea::vertical().id_salt("routing-scroll").auto_shrink([false, false]).show(
            &mut child,
            |ui| {
                for (text, warning) in &lines {
                    let (_, r) = ui.allocate_space(Vec2::new(
                        content.width(),
                        if *warning { 30.0 } else { 52.0 },
                    ));
                    widgets::paragraph(
                        ui,
                        text,
                        r,
                        if *warning { "Cinzel Bold" } else { "IM FELL English" },
                        if *warning { 12.0 } else { 17.0 },
                        if *warning { widgets::color(0xe6a078) } else { widgets::color(0xe4d8b6) },
                    );
                }
                if let Some((done, total, active)) = &*self.progress.borrow() {
                    let percent = (*done as f64 / *total as f64 * 100.0 + 0.5).floor().min(100.0);
                    let (_, r) = ui.allocate_space(Vec2::new(content.width(), 48.0));
                    ui.painter().rect_filled(
                        r.shrink2(Vec2::new(0.0, 16.0)),
                        0.0,
                        widgets::color(0x19150d),
                    );
                    let fill = Rect::from_min_size(
                        r.min + Vec2::new(0.0, 16.0),
                        Vec2::new(r.width() * percent as f32 / 100.0, 16.0),
                    );
                    ui.painter().rect_filled(fill, 0.0, widgets::GOLD);
                    widgets::label(
                        ui,
                        &format!(
                            "{} · {percent}%",
                            active.as_ref().map_or("Content ready".to_owned(), |id| format!(
                                "Downloading {id}"
                            ))
                        ),
                        r.min,
                        Align2::LEFT_TOP,
                        "IM FELL English",
                        14.0,
                        widgets::GOLD,
                    );
                }
                if let Some(error) = &self.error {
                    let (_, r) = ui.allocate_space(Vec2::new(content.width(), 56.0));
                    widgets::paragraph(ui, error, r, "IM FELL English", 16.0, widgets::ERROR);
                }
            },
        );
        if let Some(label) = vanilla {
            let r = Rect::from_min_size(
                egui::pos2(inner.left(), panel.bottom() - 116.0),
                Vec2::new(inner.width(), 44.0),
            );
            if widgets::flat_button(frame, ui, "routing-vanilla", label, r, !self.busy).clicked()
                && !self.busy
            {
                actions.push("routing-vanilla".to_owned());
            }
        }
        for (label, id, x, w) in [
            (
                if matches!(prompt, Prompt::Cheats | Prompt::Mismatch(_)) {
                    "CANCEL"
                } else {
                    "BACK"
                },
                "routing-back",
                inner.left(),
                112.0,
            ),
            (continue_label, "routing-continue", inner.left() + 124.0, inner.width() - 124.0),
        ] {
            let r = Rect::from_min_size(egui::pos2(x, panel.bottom() - 60.0), Vec2::new(w, 44.0));
            if widgets::flat_button(frame, ui, id, label, r, !self.busy).clicked() && !self.busy {
                actions.push(id.to_owned());
            }
        }
        Ok(())
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
