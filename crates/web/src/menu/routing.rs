//! Website `PartyJoinConsentDialog.tsx` / `ModdedPlayDialog.tsx`.
//! The coordinator retains admission intents and performs preparation. This view
//! only explains the actual policy and emits the user's explicit choice.

use solomon_sim::menu::save_mods::GameSaveModMismatch;
use solomon_sim::social::GameSessionKind;
use web_sys::{Document, HtmlElement};

use super::HostedMenuAction;
use crate::hosted::{ActiveWebMod, PartyJoinTarget};
use crate::ui::{append, element};

/// Borrowed projection of bytes reported by the accepted content loader.
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
    pub root: HtmlElement,
    document: Document,
    request_id: Option<u64>,
    prompt: Option<Prompt>,
    busy: bool,
    progress: HtmlElement,
    error: HtmlElement,
    actions: Vec<HostedMenuAction>,
}

impl RoutingView {
    pub fn new(document: &Document) -> Result<Self, String> {
        let root = element(document, "div", "play-routing-backdrop")?;
        root.set_attribute("role", "presentation").map_err(|_| "routing backdrop role")?;
        root.set_hidden(true);
        Ok(Self {
            root,
            document: document.clone(),
            request_id: None,
            prompt: None,
            busy: false,
            progress: element(document, "div", "play-routing-progress")?,
            error: element(document, "p", "play-routing-error")?,
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

    pub fn resize(&self, width: f64, height: f64) -> Result<(), String> {
        if matches!(self.prompt, Some(Prompt::Mismatch(_))) {
            let scale = (width / 1600.0).min(height / 900.0);
            crate::ui::style(
                &self.root,
                &format!(
                    "left:{}px;top:{}px;width:1600px;height:900px;transform:scale({scale});transform-origin:top left",
                    (width - 1600.0 * scale) / 2.0,
                    (height - 900.0 * scale) / 2.0
                ),
            )?;
        }
        Ok(())
    }

    pub fn active(&self) -> bool {
        self.prompt.is_some()
    }

    fn show(&mut self, request_id: u64, prompt: Prompt) -> Result<(), String> {
        self.root.remove_attribute("style").map_err(|_| "routing stage reset")?;
        self.root.set_class_name(match prompt {
            Prompt::Mismatch(_) => "main-menu-native-stage game-save-mod-mismatch-stage",
            Prompt::Cheats => "play-routing-backdrop cheat-college-consent-backdrop",
            _ => "play-routing-backdrop",
        });
        self.request_id = Some(request_id);
        self.prompt = Some(prompt);
        self.busy = false;
        self.paint()?;
        self.root.set_hidden(false);
        Ok(())
    }

    /// A successful coordinator transition unmounts the source dialog. Closing
    /// invalidates late progress/completion without cancelling an accepted session.
    pub fn close(&mut self) {
        self.request_id = None;
        self.prompt = None;
        self.busy = false;
        self.root.set_hidden(true);
        self.root.set_text_content(None);
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
        self.paint()?;
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
        self.progress.set_text_content(None);
        let Some(progress) = progress.filter(|progress| progress.total_bytes > 0) else {
            self.progress.set_hidden(true);
            return Ok(true);
        };
        let percent = (progress.completed_bytes as f64 / progress.total_bytes as f64 * 100.0 + 0.5)
            .floor()
            .min(100.0);
        for (name, value) in [
            ("role", "progressbar".to_owned()),
            ("aria-valuemin", "0".to_owned()),
            ("aria-valuemax", "100".to_owned()),
            ("aria-valuenow", percent.to_string()),
        ] {
            self.progress.set_attribute(name, &value).map_err(|_| "content progress values")?;
        }
        let fill = element(&self.document, "span", "")?;
        fill.style()
            .set_property("width", &format!("{percent}%"))
            .map_err(|_| "content progress fill")?;
        append(&self.progress, &fill)?;
        let label = progress
            .active_mod_id
            .map_or_else(|| "Content ready".to_owned(), |id| format!("Downloading {id}"));
        text(&self.document, &self.progress, "small", "", &format!("{label} · {percent}%"))?;
        self.progress.set_hidden(false);
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
                self.paint()?;
                self.error.set_text_content(Some(&message));
                self.error.set_hidden(false);
            }
        }
        Ok(true)
    }

    pub fn take_actions(&mut self) -> Vec<HostedMenuAction> {
        std::mem::take(&mut self.actions)
    }

    fn paint(&mut self) -> Result<(), String> {
        self.root.set_text_content(None);
        if let Some(Prompt::Mismatch(mismatch)) = &self.prompt {
            return self.paint_mismatch(mismatch);
        }
        let document = &self.document;
        let dialog = element(document, "section", "play-routing-dialog")?;
        dialog.set_attribute("role", "dialog").map_err(|_| "routing dialog role")?;
        dialog.set_attribute("aria-modal", "true").map_err(|_| "routing dialog modal")?;
        let continue_label;
        let mut vanilla_label = None;
        match self.prompt.as_ref().ok_or("routing has no prompt")? {
            Prompt::Party { target, requires_vanilla, signed_in } => {
                dialog
                    .set_attribute("aria-label", "Join party consent")
                    .map_err(|_| "party consent label")?;
                text(
                    document,
                    &dialog,
                    "h2",
                    "",
                    &format!("{}'S COLLEGE", target.leader.to_uppercase()),
                )?;
                text(
                    document,
                    &dialog,
                    "p",
                    "",
                    if target.kind == GameSessionKind::GlobalHub {
                        "This is a vanilla global-Hub party."
                    } else {
                        "The room decides its content and cheat policy. Your unrelated mods and settings are ignored for this session."
                    },
                )?;
                let mods = &target.content.mods;
                if !mods.is_empty() {
                    text(
                        document,
                        &dialog,
                        "h3",
                        "",
                        &format!("{} {}", mods.len(), if mods.len() == 1 { "MOD" } else { "MODS" }),
                    )?;
                    let list = element(document, "ul", "")?;
                    for content in mods {
                        text(
                            document,
                            &list,
                            "li",
                            "",
                            &format!("{} · v{}", content.name, content.version),
                        )?;
                    }
                    append(&dialog, &list)?;
                }
                if target.cheats_enabled {
                    warning(document, &dialog, "CHEATS ENABLED FOR THIS COLLEGE")?;
                }
                if target.kind == GameSessionKind::PrivateCollege {
                    warning(document, &dialog, "LOCAL HALL ONLY · GLOBAL SCORES OFF")?;
                }
                if *requires_vanilla {
                    warning(document, &dialog, "ACTIVE MODS AND CHEATS WILL BE DISABLED")?;
                }
                continue_label = if self.busy {
                    "PREPARING…"
                } else if *requires_vanilla {
                    "DISABLE & JOIN"
                } else if !mods.is_empty() && *signed_in {
                    "SYNC MODS & JOIN"
                } else if !mods.is_empty() {
                    "DOWNLOAD & JOIN ONCE"
                } else {
                    "JOIN PARTY"
                };
            }
            Prompt::Modded { active_mods, cheats_enabled } => {
                dialog
                    .set_attribute("aria-label", "Local play is active")
                    .map_err(|_| "local play label")?;
                text(document, &dialog, "h2", "", "LOCAL PLAY IS ACTIVE")?;
                text(
                    document,
                    &dialog,
                    "p",
                    "",
                    "Mods and cheats use a private College. These runs stay in your local Hall of Fame.",
                )?;
                if !active_mods.is_empty() {
                    text(document, &dialog, "h3", "", "ACTIVE MODS")?;
                    let list = element(document, "ul", "")?;
                    for content in active_mods {
                        text(
                            document,
                            &list,
                            "li",
                            "",
                            &format!("{} · v{}", content.name, content.version),
                        )?;
                    }
                    append(&dialog, &list)?;
                }
                if *cheats_enabled {
                    warning(document, &dialog, "CHEATS ENABLED")?;
                }
                vanilla_label = Some(if !active_mods.is_empty() && *cheats_enabled {
                    "DISABLE MODS & CHEATS"
                } else if !active_mods.is_empty() {
                    "DISABLE ALL MODS"
                } else {
                    "DISABLE CHEATS"
                });
                continue_label = if self.busy { "PREPARING…" } else { "CONTINUE LOCAL" };
            }
            Prompt::Cheats => {
                dialog
                    .set_attribute("aria-label", "Cheats require a private College")
                    .map_err(|_| "cheat College prompt label")?;
                text(document, &dialog, "h2", "", "CHEATS USE PRIVATE COLLEGES")?;
                text(
                    document,
                    &dialog,
                    "p",
                    "",
                    "Leave the global Hub, then use Last Game to continue this wizard locally.",
                )?;
                continue_label = "LEAVE & CONTINUE";
            }
            Prompt::Mismatch(_) => unreachable!("mismatch uses its authored surface"),
        }
        self.progress.set_text_content(None);
        self.progress.set_hidden(true);
        append(&dialog, &self.progress)?;
        self.error.set_attribute("role", "alert").map_err(|_| "routing error role")?;
        self.error.set_hidden(true);
        append(&dialog, &self.error)?;
        let footer = element(document, "footer", "")?;
        let back = button(
            document,
            if matches!(self.prompt, Some(Prompt::Cheats)) { "CANCEL" } else { "BACK" },
            "routing-back",
            self.busy,
        )?;
        back.set_attribute("data-game-back", "true").map_err(|_| "routing back action")?;
        append(&footer, &back)?;
        if let Some(label) = vanilla_label {
            append(&footer, &button(document, label, "routing-vanilla", self.busy)?)?;
        }
        append(&footer, &button(document, continue_label, "routing-continue", self.busy)?)?;
        append(&dialog, &footer)?;
        append(&self.root, &dialog)
    }

    fn paint_mismatch(&self, mismatch: &GameSaveModMismatch) -> Result<(), String> {
        let document = &self.document;
        let dialog = element(document, "section", "game-save-mod-mismatch")?;
        dialog.set_attribute("role", "dialog").map_err(|_| "save mismatch role")?;
        dialog.set_attribute("aria-modal", "true").map_err(|_| "save mismatch modal")?;
        dialog
            .set_attribute("aria-label", "Saved mod list changed")
            .map_err(|_| "save mismatch label")?;
        text(document, &dialog, "h2", "", "THE MOD LIST HAS CHANGED")?;
        text(
            document,
            &dialog,
            "p",
            "",
            "This save was made with a different Dark Cloud. Continue with the currently enabled mods?",
        )?;
        let groups = element(document, "div", "game-save-mod-mismatch-groups")?;
        for (title, rows) in [
            (
                "ADDED",
                mismatch
                    .added
                    .iter()
                    .map(|entry| format!("{} v{}", entry.id, entry.version))
                    .collect::<Vec<_>>(),
            ),
            (
                "MISSING",
                mismatch
                    .removed
                    .iter()
                    .map(|entry| format!("{} v{}", entry.id, entry.version))
                    .collect(),
            ),
            (
                "CHANGED",
                mismatch
                    .changed
                    .iter()
                    .map(|entry| {
                        format!(
                            "{} v{} → v{}",
                            entry.saved.id, entry.saved.version, entry.active.version
                        )
                    })
                    .collect(),
            ),
        ] {
            if rows.is_empty() {
                continue;
            }
            let group = element(document, "section", "")?;
            text(document, &group, "h3", "", title)?;
            let list = element(document, "ul", "")?;
            for row in rows {
                text(document, &list, "li", "", &row)?;
            }
            append(&group, &list)?;
            append(&groups, &group)?;
        }
        append(&dialog, &groups)?;
        text(
            document,
            &dialog,
            "p",
            "game-save-mod-mismatch-warning",
            "Missing or changed mod state will be discarded. If the save is inside a missing mod Boneyard, the wizard returns to the College.",
        )?;
        let actions = element(document, "div", "game-save-mod-mismatch-actions")?;
        let cancel = button(document, "CANCEL", "routing-back", self.busy)?;
        cancel.set_attribute("data-game-back", "true").map_err(|_| "save mismatch cancel")?;
        append(&actions, &cancel)?;
        append(&actions, &button(document, "CONTINUE", "routing-continue", self.busy)?)?;
        append(&dialog, &actions)?;
        append(&self.root, &dialog)
    }
}

fn warning(document: &Document, parent: &HtmlElement, content: &str) -> Result<(), String> {
    text(document, parent, "p", "play-routing-warning", content)?;
    Ok(())
}

fn text(
    document: &Document,
    parent: &HtmlElement,
    tag: &str,
    class: &str,
    content: &str,
) -> Result<HtmlElement, String> {
    let node = element(document, tag, class)?;
    node.set_text_content(Some(content));
    append(parent, &node)?;
    Ok(node)
}

fn button(
    document: &Document,
    label: &str,
    action: &str,
    disabled: bool,
) -> Result<HtmlElement, String> {
    let node = element(document, "button", "")?;
    node.set_attribute("type", "button").map_err(|_| "routing button type")?;
    node.set_attribute("data-menu-action", action).map_err(|_| "routing button action")?;
    node.toggle_attribute_with_force("disabled", disabled)
        .map_err(|_| "routing button admission")?;
    node.set_text_content(Some(label));
    Ok(node)
}
