//! Stock Party marble, bracket rows and actual host membership actions.

mod interaction;
mod invitation;
mod menu;

pub(super) use interaction::{PartyMenuInteraction, copy_party_code, listen_to_party};

use solomon_sim::social::{GameChatWorldKind, party_member_display_name};
use web_sys::{Document, HtmlElement};

use super::{SocialUi, Surface, action_button, attribute};
use crate::ui::{StockAtlas, add_style, append, element, native_button, style};

impl SocialUi {
    pub(super) fn draw_party(&self) -> Result<(), String> {
        use crate::ui::text::native_text;
        use solomon_sim::native_ui_text::{
            NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec, measure_native_ui_text,
        };
        use solomon_sim::social::party_ui::{
            fit_native_party_menu_text, native_party_menu_name_font,
        };

        self.chip.set_text_content(None);
        let Some(snapshot) = &self.snapshot else {
            self.chip.set_hidden(true);
            return Ok(());
        };
        let Some(state) = &snapshot.party_state else {
            self.chip.set_hidden(true);
            return Ok(());
        };
        let visible =
            snapshot.world_kind == GameChatWorldKind::Hub && !self.context.input_suspended;
        self.chip.set_hidden(!visible);
        if !visible {
            return Ok(());
        }
        let expanded = !self.coarse || self.party_expanded;
        let leader = state.party.leader_player_id == snapshot.local_player_id;
        attribute(&self.chip, "data-party-expanded", &expanded.to_string())?;
        attribute(&self.chip, "data-party-id", &state.party.id)?;
        let mut rows = Vec::new();
        if expanded {
            for id in &state.party.member_player_ids {
                let mut tags = Vec::new();
                if *id == snapshot.local_player_id {
                    tags.push(("YOU", 0xaaa2a6));
                }
                if *id == state.party.leader_player_id {
                    tags.push(("LEADER", 0xd9ba70));
                }
                if state.party_roster.iter().any(|row| row.player_id == *id && !row.connected) {
                    tags.push(("OFFLINE", 0xaaa2a6));
                }
                rows.push((format!("member:{id}"), party_member_display_name(state, id), tags));
            }
            if leader {
                for request in &state.join_requests {
                    rows.push((
                        format!("request:{}", request.id),
                        &request.requester.display_name,
                        vec![("WANTS TO JOIN", 0xd9ba70)],
                    ));
                }
            }
        }
        let error = self
            .party_status
            .as_deref()
            .map(|error| fit_native_party_menu_text(error, NativeUiFontName::Medium, 188.0, 0.85))
            .unwrap_or_default();
        let mut y = 54.0 + if error.is_empty() { 0.0 } else { 22.0 };
        let tall = !rows.is_empty() || !error.is_empty();
        let height = if tall { y + rows.len() as f64 * 36.0 - 4.0 + 12.0 } else { 50.0 };
        style(&self.chip, &format!("width:236px;height:{height}px"))?;
        append(&self.chip, &self.ui.tiles(&self.document, 49, 236.0, height)?)?;
        append(
            &self.chip,
            &scaled_border(&self.document, &self.ui, 236.0, height, if tall { 0.4 } else { 0.3 })?,
        )?;
        let skull = self.ui.sprite(&self.document, 38, "social-chip-skull")?;
        add_style(
            &skull,
            "position:absolute;left:14px;top:12.25px;transform:scale(.5);transform-origin:top left",
        )?;
        append(&self.chip, &skull)?;
        let header = action_button(
            &self.document,
            "PARTY",
            "party",
            "social-chip-header native-ui-party-chip-action",
        )?;
        attribute(&header, "aria-expanded", &expanded.to_string())?;
        header.set_text_content(None);
        append(
            &header,
            &native_text(
                &self.document,
                &NativeUiTextSpec {
                    align: NativeUiTextAlign::Left,
                    tint: 0xd9ba70,
                    ..NativeUiTextSpec::new(NativeUiFontName::Menu, "PARTY", 42.0, 33.0)
                },
                "",
            )?,
        )?;
        append(&self.chip, &header)?;
        if leader
            || state.party.member_player_ids.len() > 1
            || snapshot.session_kind == solomon_sim::social::GameSessionKind::PrivateCollege
        {
            let gear = action_button(
                &self.document,
                "Party settings",
                "party-settings",
                "social-chip-gear native-ui-party-chip-action",
            )?;
            gear.set_text_content(None);
            let art = self.bonedit.mask_sprite(&self.document, 54, 0xd9ba70, "")?;
            let record = self.bonedit.record(54)?;
            add_style(
                &art,
                &format!(
                    "position:absolute;left:{}px;top:{}px;transform:scale(.6);transform-origin:top left",
                    30.0 - f64::from(record.logical_width) * 0.6,
                    22.0 - f64::from(record.logical_height) * 0.3
                ),
            )?;
            append(&gear, &art)?;
            append(&self.chip, &gear)?;
        }
        let arrow = self.control_panel.sprite(&self.document, 0, "social-chip-arrow")?;
        add_style(
            &arrow,
            &format!(
                "position:absolute;left:215px;top:25px;transform:translate(-50%,-50%) rotate({}deg)",
                if self.coarse && expanded { 90 } else { 0 }
            ),
        )?;
        append(&self.chip, &arrow)?;
        if !error.is_empty() {
            append(
                &self.chip,
                &native_text(
                    &self.document,
                    &NativeUiTextSpec {
                        align: NativeUiTextAlign::Left,
                        scale: 0.85,
                        tint: 0xff9a8a,
                        ..NativeUiTextSpec::new(NativeUiFontName::Medium, &error, 24.0, 67.0)
                    },
                    "",
                )?,
            )?;
        }
        for (action, name, tags) in rows {
            let row = action_button(
                &self.document,
                name,
                &action,
                "social-chip-member native-ui-party-chip-action",
            )?;
            row.set_text_content(None);
            style(&row, &format!("left:12px;top:{y}px;width:212px;height:32px"))?;
            append(&row, &brackets(&self.document, &self.ui, 212.0, 32.0)?)?;
            let mut tags_width = 0.0;
            for (index, (label, _)) in tags.iter().enumerate() {
                tags_width += measure_native_ui_text(label, NativeUiFontName::Body, 1.0)
                    + if index > 0 { 8.0 } else { 0.0 };
            }
            let tags_left = 212.0 - tags_width;
            let limit = if tags.is_empty() { 188.0 } else { tags_left - 8.0 - 24.0 };
            let font = native_party_menu_name_font(name);
            let name = fit_native_party_menu_text(name, font, limit, 0.65);
            append(
                &row,
                &native_text(
                    &self.document,
                    &NativeUiTextSpec {
                        align: NativeUiTextAlign::Left,
                        scale: 0.65,
                        tint: 0xffffff,
                        ..NativeUiTextSpec::new(font, &name, 12.0, 21.0)
                    },
                    "",
                )?,
            )?;
            let mut tag_x = tags_left - 12.0;
            for (label, tint) in tags {
                append(
                    &row,
                    &native_text(
                        &self.document,
                        &NativeUiTextSpec {
                            align: NativeUiTextAlign::Left,
                            tint,
                            ..NativeUiTextSpec::new(NativeUiFontName::Body, label, tag_x, 21.0)
                        },
                        "",
                    )?,
                )?;
                tag_x += measure_native_ui_text(label, NativeUiFontName::Body, 1.0) + 8.0;
            }
            append(&self.chip, &row)?;
            y += 36.0;
        }
        Ok(())
    }

    pub(super) fn draw_modal(&self) -> Result<(), String> {
        let focused = self
            .document
            .active_element()
            .and_then(|node| node.get_attribute("data-party-focus-key"));
        self.modal.set_text_content(None);
        self.modal.set_hidden(self.surface.is_none() || self.context.input_suspended);
        match &self.surface {
            Some(Surface::Party(tab)) => self.draw_native_party_menu(*tab, focused.as_deref()),
            Some(Surface::Card(_) | Surface::HubPlayer(_)) => {
                self.party_ui.borrow_mut().close();
                self.draw_card()
            }
            None => {
                self.party_ui.borrow_mut().close();
                Ok(())
            }
        }
    }
}

fn stock_button(document: &Document, label: &str, action: &str) -> Result<HtmlElement, String> {
    let button = native_button(document, label, "")?;
    button.remove_attribute("data-menu-action").map_err(|_| "social native action producer")?;
    attribute(&button, "data-social-action", action)?;
    button.set_class_name("native-button");
    Ok(button)
}

fn scaled_border(
    document: &Document,
    ui: &StockAtlas,
    width: f64,
    height: f64,
    scale: f64,
) -> Result<HtmlElement, String> {
    let border = ui.nine_slice_border(document, 17, width / scale, height / scale)?;
    border
        .style()
        .set_property("transform", &format!("scale({scale})"))
        .map_err(|_| "party frame scale")?;
    border
        .style()
        .set_property("transform-origin", "top left")
        .map_err(|_| "party frame origin")?;
    Ok(border)
}

fn brackets(
    document: &Document,
    ui: &StockAtlas,
    width: f64,
    height: f64,
) -> Result<HtmlElement, String> {
    let frame = element(document, "span", "social-row-brackets")?;
    style(
        &frame,
        &format!("position:absolute;inset:0;width:{width}px;height:{height}px;pointer-events:none"),
    )?;
    // UI.50 is four independent corner brackets, with no connecting frame edges.
    for (x, y, scale_x, scale_y) in
        [(0.0, 0.0, 1, 1), (width, 0.0, -1, 1), (0.0, height, 1, -1), (width, height, -1, -1)]
    {
        let corner = ui.sprite(document, 50, "")?;
        add_style(
            &corner,
            &format!(
                "position:absolute;left:{x}px;top:{y}px;transform:scale({scale_x},{scale_y});transform-origin:top left"
            ),
        )?;
        append(&frame, &corner)?;
    }
    Ok(frame)
}

fn tab_chrome(
    document: &Document,
    ui: &StockAtlas,
    width: f64,
    height: f64,
    selected: bool,
) -> Result<HtmlElement, String> {
    let scale = height / 69.0;
    let top = if selected { 0.0 } else { 8.0 * scale };
    let source_height = if selected { 65.0 } else { 51.0 };
    let drawn_height = source_height * scale;
    let bracket_width = 34.0 * scale;
    let chrome = element(document, "span", "social-tab-chrome")?;
    style(&chrome, "position:absolute;inset:0;pointer-events:none")?;
    for (source, target, mirror) in [
        ([0.0, 0.0, 34.0, source_height], [0.0, top, bracket_width, drawn_height], false),
        (
            [0.0, 0.0, 34.0, source_height],
            [width - bracket_width, top, bracket_width, drawn_height],
            true,
        ),
        (
            [34.0 * 0.95, 0.0, 34.0 * 0.05, source_height],
            [bracket_width, top, width - 2.0 * bracket_width, drawn_height],
            false,
        ),
    ] {
        let piece = ui.slice(document, 13, source, target)?;
        if mirror {
            piece.style().set_property("transform", "scaleX(-1)").map_err(|_| "tab mirror")?;
        }
        append(&chrome, &piece)?;
    }
    Ok(chrome)
}
