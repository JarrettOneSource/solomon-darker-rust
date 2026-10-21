//! Stock Party layout from `native-ui/native-ui-party-menu.ts` and
//! `native-ui/native-ui-party-chip.ts`. Accepted membership
//! remains in LocalPartyState; this plan owns only measured text, clips and hit regions.

use crate::hud::HudRect;
use crate::native_ui_text::{
    NativeUiFontName, NativeUiTextAlign, is_javascript_whitespace, measure_native_ui_text,
    native_ui_font,
};

use super::PartyVisibility;

pub const NATIVE_PARTY_MENU_FRAME: HudRect = HudRect::new(390.0, 110.0, 820.0, 680.0);
pub const NATIVE_PARTY_MENU_VIEWPORT: HudRect = HudRect::new(460.0, 278.0, 680.0, 367.0);
pub const NATIVE_PARTY_MENU_ROW_PITCH: f64 = 66.0;
pub const NATIVE_PARTY_MENU_WHEEL_STEP: f64 = 25.0;
pub const NATIVE_PARTY_INVITATION_FRAME: HudRect = HudRect::new(550.0, 268.0, 500.0, 362.0);
pub const NATIVE_PARTY_INVITATION_TITLE: &str = "PARTY INVITATION";
pub const NATIVE_PARTY_VISIBILITIES: [PartyVisibility; 3] =
    [PartyVisibility::Public, PartyVisibility::InviteOnly, PartyVisibility::Private];

/// `nativeUiPartyInvitationActionBounds`: the two stock message actions use the
/// Party footer's 125px bottom offset and 36px gap, rather than the default footer.
#[must_use]
pub const fn native_party_invitation_action_bounds() -> [HudRect; 2] {
    [HudRect::new(590.0, 505.0, 192.0, 69.0), HudRect::new(818.0, 505.0, 192.0, 69.0)]
}

#[must_use]
pub fn native_party_invitation_body(inviter: &str) -> String {
    format!("{inviter} invited you to their party.")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativePartyMenuTab {
    Members,
    Mods,
    Settings,
}

impl NativePartyMenuTab {
    pub const ALL: [Self; 3] = [Self::Members, Self::Mods, Self::Settings];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Members => "members",
            Self::Mods => "mods",
            Self::Settings => "settings",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Members => "Members",
            Self::Mods => "Mods",
            Self::Settings => "Settings",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativePartyMenuTag {
    You,
    Leader,
    Offline,
}

impl NativePartyMenuTag {
    #[must_use]
    pub const fn presentation(self) -> (&'static str, u32) {
        match self {
            Self::You => ("YOU", 0xaaa2a6),
            Self::Leader => ("LEADER", 0xd9ba70),
            Self::Offline => ("OFFLINE", 0xaaa2a6),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativePartyMenuMember {
    pub id: String,
    pub name: String,
    pub removable: bool,
    pub tags: Vec<NativePartyMenuTag>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativePartyMenuRequest {
    pub id: String,
    pub name: String,
}

pub struct NativePartyMenuSpec<'a> {
    pub code: Option<&'a str>,
    pub copied: bool,
    pub error: Option<&'a str>,
    pub leader: bool,
    pub leave_label: Option<&'a str>,
    pub members: &'a [NativePartyMenuMember],
    pub requests: &'a [NativePartyMenuRequest],
    pub scroll_y: f64,
    pub selected_tab: NativePartyMenuTab,
    pub visibility: PartyVisibility,
    pub visibility_options: &'a [PartyVisibility],
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativePartyMenuText {
    pub label: String,
    pub text: String,
    pub font: NativeUiFontName,
    pub align: NativeUiTextAlign,
    pub scale: f64,
    pub tint: u32,
    pub x: f64,
    pub y: f64,
    pub clipped: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativePartyMenuAction {
    pub bounds: HudRect,
    pub disabled: bool,
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativePartyMenuRow {
    pub bounds: HudRect,
    pub id: String,
    pub visible_bounds: Option<HudRect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativePartyMenuRowAction {
    pub bounds: HudRect,
    pub id: String,
    pub label: String,
    pub row_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativePartyMenuPlan {
    pub content_height: f64,
    pub maximum_scroll_y: f64,
    pub scroll_y: f64,
    pub rows: Vec<NativePartyMenuRow>,
    pub settings_rows: Vec<HudRect>,
    /// Every row action remains addressable by keyboard, even outside the clip.
    pub row_actions: Vec<NativePartyMenuRowAction>,
    /// Visible hit regions, in the source plan's order.
    pub actions: Vec<NativePartyMenuAction>,
    pub texts: Vec<NativePartyMenuText>,
}

#[must_use]
pub fn native_party_menu_name_font(name: &str) -> NativeUiFontName {
    let font = native_ui_font(NativeUiFontName::WorldAndRoster);
    for character in name.chars() {
        if character != ' ' && font.glyph(u32::from(character)).is_none() {
            return NativeUiFontName::Menu;
        }
    }
    NativeUiFontName::WorldAndRoster
}

#[must_use]
pub fn fit_native_party_menu_text(
    text: &str,
    font: NativeUiFontName,
    max_width: f64,
    scale: f64,
) -> String {
    let mut candidate = text.to_owned();
    while !candidate.is_empty() && measure_native_ui_text(&candidate, font, scale) > max_width {
        candidate.pop();
        candidate.truncate(candidate.trim_end_matches(is_javascript_whitespace).len());
    }
    candidate
}

#[must_use]
pub fn native_party_menu_row_button_bounds(right_edge: f64, row_top: f64) -> (HudRect, f64) {
    let scale = 0.55;
    let surround = 6.0 * scale;
    let width = 353.0 * scale;
    let height = 69.0 * scale;
    let surround_width = width + surround * 2.0;
    let surround_height = 85.0 * scale;
    let top = row_top + (56.0 - surround_height) / 2.0 + surround;
    (HudRect::new(right_edge - surround_width + surround, top, width, height), surround_width)
}

#[must_use]
pub fn clamp_native_party_menu_scroll(requested: f64, content_height: f64) -> f64 {
    assert!(requested.is_finite(), "native SwipeBox offset must be finite");
    requested.max(0.0).min((content_height - NATIVE_PARTY_MENU_VIEWPORT.height).max(0.0))
}

#[must_use]
pub fn plan_native_party_menu(spec: &NativePartyMenuSpec<'_>) -> NativePartyMenuPlan {
    let mut plan = NativePartyMenuPlan {
        content_height: 0.0,
        maximum_scroll_y: 0.0,
        scroll_y: 0.0,
        rows: Vec::new(),
        settings_rows: Vec::new(),
        row_actions: Vec::new(),
        actions: Vec::new(),
        texts: Vec::new(),
    };
    plan.texts.push(centered_text(
        "party:title",
        "PARTY",
        NativeUiFontName::Menu,
        0xd9ba70,
        800.0,
        162.0,
    ));
    for (index, tab) in NativePartyMenuTab::ALL.iter().enumerate() {
        plan.actions.push(NativePartyMenuAction {
            bounds: HudRect::new(431.0 + index as f64 * 246.0, 190.0, 246.0, 69.0),
            disabled: false,
            id: format!("tab:{}", tab.id()),
            label: tab.label().to_owned(),
        });
    }
    match spec.selected_tab {
        NativePartyMenuTab::Members => member_list(spec, &mut plan),
        NativePartyMenuTab::Settings => settings(spec, &mut plan),
        NativePartyMenuTab::Mods => plan.texts.push(centered_text(
            "party:empty",
            "Nothing here yet.",
            NativeUiFontName::Medium,
            0xaaa2a6,
            800.0,
            318.0,
        )),
    }
    let footer = if let Some(label) = spec.leave_label.filter(|label| !label.is_empty()) {
        vec![("leave", label), ("close", "CLOSE")]
    } else {
        vec![("close", "CLOSE")]
    };
    let count = footer.len();
    let width = if count == 1 { 353.0 } else { 260.0 };
    let total_width = width * count as f64 + 36.0 * (count - 1) as f64;
    for (index, (id, label)) in footer.into_iter().enumerate() {
        plan.actions.push(NativePartyMenuAction {
            bounds: HudRect::new(
                800.0 - total_width / 2.0 + index as f64 * (width + 36.0),
                665.0,
                width,
                69.0,
            ),
            disabled: false,
            id: id.to_owned(),
            label: label.to_owned(),
        });
    }
    if let Some(error) = spec.error.filter(|error| !error.is_empty()) {
        let error = fit_native_party_menu_text(error, NativeUiFontName::Medium, 680.0, 1.0);
        plan.texts.push(centered_text(
            "party:error",
            &error,
            NativeUiFontName::Medium,
            0xff9a8a,
            800.0,
            764.0,
        ));
    }
    plan
}

fn member_list(spec: &NativePartyMenuSpec<'_>, plan: &mut NativePartyMenuPlan) {
    let count = spec.requests.len() + spec.members.len();
    plan.content_height = if count == 0 { 0.0 } else { count as f64 * 66.0 - 10.0 };
    plan.maximum_scroll_y = (plan.content_height - NATIVE_PARTY_MENU_VIEWPORT.height).max(0.0);
    plan.scroll_y = clamp_native_party_menu_scroll(spec.scroll_y, plan.content_height);
    let mut index = 0;
    for request in spec.requests {
        let id = format!("request:{}", request.id);
        let buttons = [
            (format!("deny:{}", request.id), "DENY"),
            (format!("accept:{}", request.id), "ACCEPT"),
        ];
        member_row(plan, index, &id, &request.name, &[("WANTS TO JOIN", 0xd9ba70)], &buttons);
        index += 1;
    }
    for member in spec.members {
        let id = format!("member:{}", member.id);
        let tags = member.tags.iter().map(|tag| tag.presentation()).collect::<Vec<_>>();
        let buttons = if member.removable {
            vec![(format!("kick:{}", member.id), "KICK")]
        } else {
            Vec::new()
        };
        member_row(plan, index, &id, &member.name, &tags, &buttons);
        index += 1;
    }
    if count == 0 {
        plan.texts.push(centered_text(
            "party:empty",
            "No members yet.",
            NativeUiFontName::Medium,
            0xaaa2a6,
            800.0,
            318.0,
        ));
    }
}

fn member_row(
    plan: &mut NativePartyMenuPlan,
    index: usize,
    id: &str,
    name: &str,
    tags: &[(&str, u32)],
    buttons: &[(String, &str)],
) {
    let viewport = NATIVE_PARTY_MENU_VIEWPORT;
    let bounds = HudRect::new(
        viewport.x,
        viewport.y + index as f64 * 66.0 - plan.scroll_y,
        viewport.width,
        56.0,
    );
    let visible_bounds = intersect(bounds, viewport);
    plan.rows.push(NativePartyMenuRow { bounds, id: id.to_owned(), visible_bounds });
    let mut right_edge = bounds.x + bounds.width - 12.0;
    let mut row_buttons = Vec::new();
    for (action, label) in buttons {
        let (body, surround_width) = native_party_menu_row_button_bounds(right_edge, bounds.y);
        right_edge -= surround_width + 16.0;
        plan.row_actions.push(NativePartyMenuRowAction {
            bounds: body,
            id: action.clone(),
            label: (*label).to_owned(),
            row_id: id.to_owned(),
        });
        row_buttons.push((action, label, body));
    }
    if visible_bounds.is_none() {
        return;
    }
    let name_x = bounds.x + 26.0;
    let baseline = bounds.y + 36.0;
    let mut tags_width = 0.0;
    for (index, (tag, _)) in tags.iter().enumerate() {
        tags_width += measure_native_ui_text(tag, NativeUiFontName::Body, 1.0)
            + if index > 0 { 8.0 } else { 0.0 };
    }
    let name_limit = right_edge - name_x - if tags.is_empty() { 0.0 } else { tags_width + 12.0 };
    let font = native_party_menu_name_font(name);
    let name = fit_native_party_menu_text(name, font, name_limit, 1.0);
    if !name.is_empty() {
        let mut text =
            centered_text(&format!("{id}:name"), &name, font, 0xffffff, name_x, baseline);
        text.align = NativeUiTextAlign::Left;
        text.clipped = true;
        plan.texts.push(text);
    }
    let mut tag_x = name_x + measure_native_ui_text(&name, font, 1.0) + 12.0;
    for (index, (label, tint)) in tags.iter().enumerate() {
        let mut text = centered_text(
            &format!("{id}:tag-{index}"),
            label,
            NativeUiFontName::Body,
            *tint,
            tag_x,
            baseline,
        );
        text.align = NativeUiTextAlign::Left;
        text.clipped = true;
        plan.texts.push(text);
        tag_x += measure_native_ui_text(label, NativeUiFontName::Body, 1.0) + 8.0;
    }
    for (id, label, body) in row_buttons {
        if let Some(bounds) = intersect(body, viewport) {
            plan.actions.push(NativePartyMenuAction {
                bounds,
                disabled: false,
                id: id.clone(),
                label: (*label).to_owned(),
            });
        }
    }
}

fn settings(spec: &NativePartyMenuSpec<'_>, plan: &mut NativePartyMenuPlan) {
    settings_row(plan, "visibility", 0, "Visibility");
    let count = spec.visibility_options.len();
    let width = count as f64 * 141.0 + count.saturating_sub(1) as f64 * 4.0;
    let left = 1128.0 - width;
    for (index, visibility) in spec.visibility_options.iter().enumerate() {
        let x = left + index as f64 * 145.0;
        let id = format!("visibility:{}", visibility_id(*visibility));
        plan.actions.push(NativePartyMenuAction {
            bounds: HudRect::new(x, 285.5, 141.0, 41.0),
            disabled: !spec.leader,
            id: id.clone(),
            label: visibility.label().to_owned(),
        });
        let mut label = centered_text(
            &format!("{id}:label"),
            visibility.label(),
            NativeUiFontName::ControlPanel,
            if *visibility == spec.visibility { 0xffffff } else { 0x1a1712 },
            x + 70.5,
            312.5,
        );
        label.scale = 1.15;
        plan.texts.push(label);
    }
    if !spec.leader {
        plan.texts.push(centered_text(
            "party:leader-note",
            "Only the party leader can change these settings.",
            NativeUiFontName::Medium,
            0xaaa2a6,
            800.0,
            380.0,
        ));
        return;
    }
    let Some(code) = spec.code else { return };
    settings_row(plan, "party-id", 1, "Party ID");
    let mut text = centered_text(
        "party-id:code",
        code,
        NativeUiFontName::Menu,
        0xd9ba70,
        486.0 + measure_native_ui_text("Party ID", NativeUiFontName::WorldAndRoster, 1.0) + 18.0,
        380.0,
    );
    text.align = NativeUiTextAlign::Left;
    plan.texts.push(text);
    let (bounds, _) = native_party_menu_row_button_bounds(1128.0, 344.0);
    plan.actions.push(NativePartyMenuAction {
        bounds,
        disabled: false,
        id: "copy".to_owned(),
        label: if spec.copied { "COPIED" } else { "COPY" }.to_owned(),
    });
    settings_row(plan, "new-party-id", 2, "New party ID");
    let (bounds, _) = native_party_menu_row_button_bounds(1128.0, 410.0);
    plan.actions.push(NativePartyMenuAction {
        bounds,
        disabled: false,
        id: "generate".to_owned(),
        label: "GENERATE".to_owned(),
    });
}

fn settings_row(plan: &mut NativePartyMenuPlan, id: &str, index: usize, label: &str) {
    let bounds = HudRect::new(460.0, 278.0 + index as f64 * 66.0, 680.0, 56.0);
    plan.settings_rows.push(bounds);
    let mut text = centered_text(
        &format!("{id}:label"),
        label,
        NativeUiFontName::WorldAndRoster,
        0xffffff,
        bounds.x + 26.0,
        bounds.y + 36.0,
    );
    text.align = NativeUiTextAlign::Left;
    plan.texts.push(text);
}

fn centered_text(
    label: &str,
    text: &str,
    font: NativeUiFontName,
    tint: u32,
    x: f64,
    y: f64,
) -> NativePartyMenuText {
    NativePartyMenuText {
        label: label.to_owned(),
        text: text.to_owned(),
        font,
        align: NativeUiTextAlign::Center,
        scale: 1.0,
        tint,
        x,
        y,
        clipped: false,
    }
}

#[must_use]
pub const fn visibility_id(visibility: PartyVisibility) -> &'static str {
    match visibility {
        PartyVisibility::Public => "public",
        PartyVisibility::InviteOnly => "invite-only",
        PartyVisibility::Private => "private",
    }
}

fn intersect(first: HudRect, second: HudRect) -> Option<HudRect> {
    let left = first.x.max(second.x);
    let top = first.y.max(second.y);
    let width = (first.x + first.width).min(second.x + second.width) - left;
    let height = (first.y + first.height).min(second.y + second.height) - top;
    (width > 0.0 && height > 0.0).then(|| HudRect::new(left, top, width, height))
}
