//! DOM painter for the canonical stock Party plan. The 1600×900 stage follows
//! HubScene's native UI placement and never substitutes browser font measurements.

use solomon_sim::hud::HudRect;
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextSpec};
use solomon_sim::social::party_ui::{
    NATIVE_PARTY_MENU_FRAME, NATIVE_PARTY_MENU_VIEWPORT, NATIVE_PARTY_VISIBILITIES,
    NativePartyMenuMember, NativePartyMenuRequest, NativePartyMenuSpec, NativePartyMenuTag,
    plan_native_party_menu, visibility_id,
};
use solomon_sim::social::{party_member_display_name, party_menu_leave_label};
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use super::super::PartyTab;
use super::{SocialUi, action_button, attribute, brackets, stock_button, tab_chrome};
use crate::ui::text::native_text;
use crate::ui::{append, element, message_frame, style};

impl SocialUi {
    pub(in crate::social) fn draw_native_party_menu(
        &self,
        tab: PartyTab,
        focused: Option<&str>,
    ) -> Result<(), String> {
        let Some(snapshot) = &self.snapshot else { return Ok(()) };
        let Some(state) = &snapshot.party_state else { return Ok(()) };
        let leader = state.party.leader_player_id == snapshot.local_player_id;
        let mut members = Vec::new();
        for id in &state.party.member_player_ids {
            let mut tags = Vec::new();
            if *id == snapshot.local_player_id {
                tags.push(NativePartyMenuTag::You);
            }
            if *id == state.party.leader_player_id {
                tags.push(NativePartyMenuTag::Leader);
            }
            if state.party_roster.iter().any(|player| player.player_id == *id && !player.connected)
            {
                tags.push(NativePartyMenuTag::Offline);
            }
            members.push(NativePartyMenuMember {
                id: id.clone(),
                name: party_member_display_name(state, id).to_owned(),
                removable: leader && *id != snapshot.local_player_id,
                tags,
            });
        }
        let requests = if leader {
            state
                .join_requests
                .iter()
                .map(|request| NativePartyMenuRequest {
                    id: request.id.clone(),
                    name: request.requester.display_name.clone(),
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let controls = self.party_ui.borrow();
        let plan = plan_native_party_menu(&NativePartyMenuSpec {
            code: if leader { state.party.join_code.as_deref() } else { None },
            copied: controls.copied,
            error: self.party_status.as_deref(),
            leader,
            leave_label: party_menu_leave_label(
                state.party.member_player_ids.len(),
                snapshot.session_kind,
            ),
            members: &members,
            requests: &requests,
            scroll_y: controls.scroll_y,
            selected_tab: tab,
            visibility: state.party.visibility,
            visibility_options: if snapshot.directory_available {
                &NATIVE_PARTY_VISIBILITIES
            } else {
                &[]
            },
        });
        drop(controls);
        let scale = (f64::from(self.size.0) / 1600.0).min(f64::from(self.size.1) / 900.0);
        let origin = [
            (f64::from(self.size.0) - 1600.0 * scale) / 2.0,
            (f64::from(self.size.1) - 900.0 * scale) / 2.0,
        ];
        self.party_ui.borrow_mut().configure(&plan, origin, scale);
        let stage =
            element(&self.document, "section", "hub-party-menu-stage native-ui-party-menu")?;
        for (name, value) in [
            ("role", "dialog"),
            ("aria-modal", "true"),
            ("aria-label", "PARTY"),
            ("data-native-ui-party-menu", ""),
        ] {
            attribute(&stage, name, value)?;
        }
        attribute(&stage, "data-native-ui-party-scroll-max", &plan.maximum_scroll_y.to_string())?;
        attribute(&stage, "data-native-ui-party-scroll-y", &plan.scroll_y.to_string())?;
        attribute(&stage, "data-native-ui-party-tab", tab.id())?;
        style(
            &stage,
            &format!("left:{}px;top:{}px;transform:scale({scale})", origin[0], origin[1]),
        )?;
        let frame = element(&self.document, "div", "")?;
        place(&frame, NATIVE_PARTY_MENU_FRAME)?;
        append(&frame, &message_frame(&self.document, &self.ui, 820.0, 680.0)?)?;
        append(&stage, &frame)?;

        let tabs = element(&self.document, "div", "")?;
        attribute(&tabs, "role", "tablist")?;
        attribute(&tabs, "aria-label", "Party menu sections")?;
        place(&tabs, HudRect::new(431.0, 190.0, 738.0, 69.0))?;
        for (index, candidate) in PartyTab::ALL.iter().enumerate() {
            let selected = *candidate == tab;
            let button = action_button(
                &self.document,
                candidate.label(),
                &format!("party-{}", candidate.id()),
                "native-ui-party-menu-action",
            )?;
            attribute(&button, "role", "tab")?;
            attribute(&button, "aria-selected", &selected.to_string())?;
            attribute(&button, "tabindex", if selected { "0" } else { "-1" })?;
            attribute(&button, "data-native-ui-party-tab-action", candidate.id())?;
            attribute(&button, "data-party-focus-key", &format!("tab:{}", candidate.id()))?;
            button.set_text_content(None);
            place(&button, HudRect::new(index as f64 * 246.0, 0.0, 246.0, 69.0))?;
            append(&button, &tab_chrome(&self.document, &self.ui, 246.0, 69.0, selected)?)?;
            append(
                &button,
                &native_text(
                    &self.document,
                    &NativeUiTextSpec {
                        tint: if selected { 0xffffff } else { 0xaaa2a6 },
                        ..NativeUiTextSpec::new(
                            NativeUiFontName::Menu,
                            candidate.label(),
                            123.0,
                            if selected { 45.0 } else { 53.0 },
                        )
                    },
                    "",
                )?,
            )?;
            append(&tabs, &button)?;
        }
        append(&stage, &tabs)?;
        let viewport = element(&self.document, "div", "native-ui-party-menu-viewport")?;
        attribute(&viewport, "data-native-ui-party-viewport", "")?;
        place(&viewport, NATIVE_PARTY_MENU_VIEWPORT)?;
        for row in &plan.rows {
            if row.visible_bounds.is_none() {
                continue;
            }
            self.party_row_frame(&viewport, offset(row.bounds, NATIVE_PARTY_MENU_VIEWPORT))?;
        }
        for bounds in &plan.settings_rows {
            self.party_row_frame(&stage, *bounds)?;
        }
        for text in &plan.texts {
            if text.label.starts_with("visibility:") && text.label.ends_with(":label") {
                continue;
            }
            let origin = if text.clipped {
                [NATIVE_PARTY_MENU_VIEWPORT.x, NATIVE_PARTY_MENU_VIEWPORT.y]
            } else {
                [0.0; 2]
            };
            let node = native_text(
                &self.document,
                &NativeUiTextSpec {
                    align: text.align,
                    scale: text.scale,
                    tint: text.tint,
                    ..NativeUiTextSpec::new(
                        text.font,
                        &text.text,
                        text.x - origin[0],
                        text.y - origin[1],
                    )
                },
                "",
            )?;
            attribute(&node, "data-native-ui-node-label", &text.label)?;
            append(if text.clipped { &viewport } else { &stage }, &node)?;
        }
        for action in &plan.row_actions {
            let row = members
                .iter()
                .find(|member| action.row_id == format!("member:{}", member.id))
                .map(|member| member.name.as_str())
                .or_else(|| {
                    requests
                        .iter()
                        .find(|request| action.row_id == format!("request:{}", request.id))
                        .map(|request| request.name.as_str())
                })
                .unwrap_or("");
            let button = self.party_stock_action(
                &action.label,
                &action.id,
                offset(action.bounds, NATIVE_PARTY_MENU_VIEWPORT),
                0.55,
            )?;
            attribute(&button, "aria-label", format!("{} {row}", action.label).trim())?;
            attribute(&button, "data-native-ui-party-row-id", &action.row_id)?;
            append(&viewport, &button)?;
        }
        if tab == PartyTab::Members {
            append(&stage, &viewport)?;
        }
        let visibility = element(&self.document, "div", "")?;
        attribute(&visibility, "role", "radiogroup")?;
        attribute(&visibility, "aria-label", "Visibility")?;
        for action in &plan.actions {
            if action.id.starts_with("tab:")
                || plan.row_actions.iter().any(|row| row.id == action.id)
            {
                continue;
            }
            if let Some(id) = action.id.strip_prefix("visibility:") {
                let selected = id == visibility_id(state.party.visibility);
                let button = action_button(
                    &self.document,
                    &action.label,
                    &action.id,
                    "native-ui-party-menu-action",
                )?;
                button.set_text_content(None);
                attribute(&button, "role", "radio")?;
                attribute(&button, "aria-checked", &selected.to_string())?;
                attribute(&button, "tabindex", if selected { "0" } else { "-1" })?;
                attribute(&button, "data-native-ui-party-visibility", id)?;
                attribute(
                    &button,
                    "data-party-selected-visibility",
                    visibility_id(state.party.visibility),
                )?;
                attribute(&button, "data-party-focus-key", &action.id)?;
                button
                    .toggle_attribute_with_force("disabled", action.disabled)
                    .map_err(|_| "party visibility admission")?;
                place(&button, action.bounds)?;
                let plate = self.ui.sprite(&self.document, if selected { 105 } else { 67 }, "")?;
                style(
                    &plate,
                    &format!(
                        "position:absolute;left:0;top:0;width:{}px;height:{}px;pointer-events:none",
                        action.bounds.width, action.bounds.height
                    ),
                )?;
                append(&button, &plate)?;
                // The plan's label shares the same pen but must paint over the plate.
                let label = plan
                    .texts
                    .iter()
                    .find(|text| text.label == format!("{}:label", action.id))
                    .expect("each visibility action has its authored label");
                append(
                    &button,
                    &native_text(
                        &self.document,
                        &NativeUiTextSpec {
                            scale: label.scale,
                            tint: label.tint,
                            ..NativeUiTextSpec::new(
                                label.font,
                                &label.text,
                                label.x - action.bounds.x,
                                label.y - action.bounds.y,
                            )
                        },
                        "",
                    )?,
                )?;
                append(&visibility, &button)?;
                continue;
            }
            let producer = match action.id.as_str() {
                "copy" => "party-copy",
                "generate" => "party-generate",
                id => id,
            };
            let scale = if matches!(action.id.as_str(), "copy" | "generate") { 0.55 } else { 1.0 };
            let button = self.party_stock_action(&action.label, producer, action.bounds, scale)?;
            attribute(&button, "data-native-ui-party-action", &action.id)?;
            attribute(&button, "data-party-focus-key", &action.id)?;
            if action.id == "close" {
                attribute(&button, "data-social-focus", "true")?;
                attribute(&button, "data-game-default-focus", "true")?;
                attribute(&button, "data-game-back", "true")?;
            }
            if action.id == "copy" {
                attribute(
                    &button,
                    "aria-label",
                    if self.party_ui.borrow().copied { "Party ID copied" } else { "Copy party ID" },
                )?;
                if let Some(code) = &state.party.join_code {
                    attribute(&button, "data-party-code", code)?;
                }
            } else if action.id == "generate" {
                attribute(&button, "aria-label", "Generate a new party ID")?;
            }
            append(&stage, &button)?;
        }
        if tab == PartyTab::Settings {
            append(&stage, &visibility)?;
        }
        append(&self.modal, &stage)?;
        let focus =
            self.party_ui.borrow_mut().focus_action.take().or_else(|| focused.map(str::to_owned));
        if let Some(focus) = focus {
            let nodes = stage
                .query_selector_all("[data-party-focus-key]")
                .map_err(|_| "party retained focus")?;
            for index in 0..nodes.length() {
                let Some(node) =
                    nodes.item(index).and_then(|node| node.dyn_into::<HtmlElement>().ok())
                else {
                    continue;
                };
                if node.get_attribute("data-party-focus-key").as_deref() == Some(&focus) {
                    let options = web_sys::FocusOptions::new();
                    options.set_prevent_scroll(true);
                    node.focus_with_options(&options).map_err(|_| "restore party focus")?;
                    break;
                }
            }
        }
        Ok(())
    }

    fn party_row_frame(&self, parent: &HtmlElement, bounds: HudRect) -> Result<(), String> {
        let row = element(&self.document, "div", "")?;
        place(&row, bounds)?;
        append(&row, &brackets(&self.document, &self.ui, bounds.width, bounds.height)?)?;
        append(parent, &row)
    }

    pub(in crate::social) fn party_stock_action(
        &self,
        label: &str,
        action: &str,
        bounds: HudRect,
        scale: f64,
    ) -> Result<HtmlElement, String> {
        let button = stock_button(&self.document, label, action)?;
        button.set_class_name("native-button party-native-button native-ui-party-menu-action");
        attribute(&button, "data-native-ui-party-action", action)?;
        attribute(&button, "data-party-focus-key", action)?;
        style(
            &button,
            &format!(
                "position:absolute;left:{}px;top:{}px;width:{}px;height:{}px;transform:scale({scale});transform-origin:top left",
                bounds.x,
                bounds.y,
                bounds.width / scale,
                bounds.height / scale
            ),
        )?;
        Ok(button)
    }
}

fn offset(bounds: HudRect, origin: HudRect) -> HudRect {
    HudRect::new(bounds.x - origin.x, bounds.y - origin.y, bounds.width, bounds.height)
}

fn place(node: &HtmlElement, bounds: HudRect) -> Result<(), String> {
    style(
        node,
        &format!(
            "position:absolute;left:{}px;top:{}px;width:{}px;height:{}px",
            bounds.x, bounds.y, bounds.width, bounds.height
        ),
    )
}
