//! Health rows from the host's canonical AllyHud projection. No actor state lives here.

use solomon_sim::social::{AllyHudIdentity, AllyHudRow, clamp_ally_hud_health_ratio};
use web_sys::HtmlElement;

use super::{SocialUi, attribute};
use crate::ui::{append, element, style};

pub(super) struct AllyRowView {
    model: AllyHudRow,
    root: HtmlElement,
    bar: HtmlElement,
    fill: HtmlElement,
}

impl SocialUi {
    pub(super) fn draw_allies(&mut self) -> Result<(), String> {
        let in_hub = self.snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.world_kind == solomon_sim::social::GameChatWorldKind::Hub
        });
        self.allies.set_hidden(
            self.context.input_suspended || (self.coarse && in_hub && self.party_expanded),
        );
        attribute(&self.allies, "data-ally-count", &self.ally_rows.len().to_string())?;
        attribute(&self.allies, "data-world-kind", if in_hub { "hub" } else { "boneyard" })?;
        let structure_changed = self.ally_nodes.len() != self.ally_rows.len()
            || self.ally_nodes.iter().zip(&self.ally_rows).any(|(view, model)| {
                view.model.id != model.id
                    || view.model.identity != model.identity
                    || view.model.connected != model.connected
                    || view.model.dead != model.dead
            });
        if structure_changed {
            self.allies.set_text_content(None);
            self.ally_nodes.clear();
            for model in &self.ally_rows {
                let root = element(&self.document, "div", "hub-hud-ally-row")?;
                for (name, value) in [
                    ("role", "listitem".to_owned()),
                    ("aria-label", model.accessible_status()),
                    ("data-ally-id", model.id.clone()),
                    ("data-ally-connected", model.connected.to_string()),
                    ("data-ally-dead", model.dead.to_string()),
                ] {
                    attribute(&root, name, &value)?;
                }
                let chip = element(&self.document, "span", "hub-hud-ally-chip")?;
                match &model.identity {
                    AllyHudIdentity::Player { element, .. } => {
                        attribute(&root, "data-ally-kind", "player")?;
                        attribute(&root, "data-ally-element", element.as_str())?;
                        let body = self.portraits.body(&self.document, *element, 12)?;
                        style(
                            &body,
                            "position:absolute;left:0;top:0;transform:translate(-25.4px,-11.7px) scale(.45);transform-origin:top left",
                        )?;
                        append(&chip, &body)?;
                    }
                    AllyHudIdentity::Golem => {
                        attribute(&root, "data-ally-kind", "golem")?;
                        chip.set_class_name("hub-hud-ally-chip hub-hud-ally-chip-golem");
                        let skull = element(&self.document, "img", "")?;
                        attribute(&skull, "src", "assets/stock/skull-white.png")?;
                        attribute(&skull, "alt", "")?;
                        append(&chip, &skull)?;
                    }
                }
                append(&root, &chip)?;
                let main = element(&self.document, "span", "hub-hud-ally-main")?;
                let name = element(&self.document, "span", "hub-hud-ally-name")?;
                name.set_text_content(Some(model.accessible_name()));
                append(&main, &name)?;
                let bar = element(&self.document, "span", "hub-hud-ally-bar")?;
                attribute(&bar, "role", "progressbar")?;
                attribute(&bar, "aria-label", &format!("{} health", model.accessible_name()))?;
                attribute(&bar, "aria-valuemin", "0")?;
                attribute(&bar, "aria-valuemax", "100")?;
                let fill = element(&self.document, "span", "hub-hud-ally-bar-fill")?;
                append(&bar, &fill)?;
                append(&main, &bar)?;
                append(&root, &main)?;
                if !model.connected || model.dead {
                    let status = element(&self.document, "span", "hub-hud-ally-status")?;
                    status.set_text_content(Some(if !model.connected {
                        "DISCONNECTED"
                    } else {
                        "DEAD"
                    }));
                    append(&root, &status)?;
                }
                append(&self.allies, &root)?;
                self.ally_nodes.push(AllyRowView { model: model.clone(), root, bar, fill });
            }
        }
        for (view, model) in self.ally_nodes.iter_mut().zip(&self.ally_rows) {
            let ratio = clamp_ally_hud_health_ratio(model.health_ratio);
            attribute(&view.root, "data-health-ratio", &ratio.to_string())?;
            attribute(&view.bar, "aria-valuenow", &(ratio * 100.0).to_string())?;
            view.fill
                .style()
                .set_property("width", &format!("{}%", ratio * 100.0))
                .map_err(|_| "ally health fill")?;
            view.model = model.clone();
        }
        Ok(())
    }
}
