//! Website `DarkCloudRows.tsx`: catalog and admitted directory projections.

use solomon_sim::menu::party_directory::{
    DirectoryPartyAction, directory_party_action, directory_party_presentation,
};
use solomon_sim::native_ui_text::NativeUiFontName::{Medium, Menu};
use solomon_sim::social::GameSessionKind;
use web_sys::{Document, HtmlElement};

use super::images::CloudImages;
use super::widgets::{self, narrative, text};
use crate::hosted::{
    ConnectedGamePlayer, ConnectedPlayerActivity, DeveloperGameMatch, ModSubscription, ModSummary,
    PartyActivity, PublicGameParty,
};
use crate::ui::{append, element};

pub(super) fn mod_row(
    document: &Document,
    content: &ModSummary,
    subscription: Option<&ModSubscription>,
    selected: bool,
    busy: bool,
    subscribed: bool,
    images: &mut CloudImages,
) -> Result<HtmlElement, String> {
    let row = element(
        document,
        "article",
        &format!("dark-cloud-row dark-cloud-mod-row{}", if selected { " selected" } else { "" }),
    )?;
    row.set_attribute("data-mod-slug", &content.slug).map_err(|_| "mod row identity")?;
    row.set_attribute("role", "listitem").map_err(|_| "mod row role")?;
    let main = selector(
        document,
        &format!("Select {}", content.name),
        &format!("mod:{}", content.slug),
        selected,
    )?;
    append(
        &main,
        &images.media(
            document,
            content.thumbnail_url.as_deref(),
            &content.name,
            "dark-cloud-row-thumbnail",
            false,
            false,
        )?,
    )?;
    let copy = element(document, "span", "dark-cloud-row-copy")?;
    text(
        document,
        &copy,
        "strong",
        "",
        &content.name,
        Menu,
        0.85,
        if selected { 0xbfffbf } else { 0xd9ba70 },
        true,
    )?;
    narrative(document, &copy, "small", "", &content.summary)?;
    text(
        document,
        &copy,
        "span",
        "dark-cloud-row-tags",
        &content.tags.iter().take(3).cloned().collect::<Vec<_>>().join(", "),
        Medium,
        0.85,
        0xa99a70,
        true,
    )?;
    append(&main, &copy)?;
    text(
        document,
        &main,
        "span",
        "dark-cloud-row-author",
        &content.author.username,
        Medium,
        1.0,
        0xd9ba70,
        true,
    )?;
    text(
        document,
        &main,
        "span",
        "dark-cloud-row-version",
        &format!("v{}", content.latest_version),
        Medium,
        1.0,
        0xd9ba70,
        true,
    )?;
    let enabled = subscription.is_some_and(|subscription| subscription.enabled);
    text(
        document,
        &main,
        "span",
        if enabled { "dark-cloud-row-state enabled" } else { "dark-cloud-row-state" },
        if enabled {
            "ENABLED"
        } else if subscription.is_some() {
            "DISABLED"
        } else {
            "NOT SUBSCRIBED"
        },
        Medium,
        0.85,
        if enabled { 0xa9d29d } else { 0xa99a70 },
        false,
    )?;
    append(&row, &main)?;
    let actions = element(document, "div", "dark-cloud-row-actions")?;
    let view = widgets::button(
        document,
        "VIEW",
        &format!("cloud-open-mod:{}", content.slug),
        "",
        None,
        44.0,
        0.55,
        false,
    )?;
    view.set_attribute("aria-label", &format!("View {}", content.name))
        .map_err(|_| "mod View label")?;
    append(&actions, &view)?;
    if subscribed && let Some(subscription) = subscription {
        let (label, action) =
            if subscription.enabled { ("DISABLE", "disable") } else { ("ENABLE", "enable") };
        let change = widgets::button(
            document,
            label,
            &format!("cloud-subscription:{action}:{}", content.slug),
            "",
            None,
            44.0,
            0.55,
            busy,
        )?;
        change
            .set_attribute(
                "aria-label",
                &format!(
                    "{} {}",
                    if subscription.enabled { "Disable" } else { "Enable" },
                    content.name
                ),
            )
            .map_err(|_| "mod change label")?;
        append(&actions, &change)?;
        let remove = widgets::button(
            document,
            "REMOVE",
            &format!("cloud-subscription:unsubscribe:{}", content.slug),
            "",
            None,
            44.0,
            0.55,
            busy,
        )?;
        remove
            .set_attribute("aria-label", &format!("Unsubscribe from {}", content.name))
            .map_err(|_| "mod remove label")?;
        append(&actions, &remove)?;
    } else if subscription.is_none() {
        append(
            &actions,
            &widgets::button(
                document,
                if busy { "WORKING..." } else { "SUBSCRIBE" },
                &format!("cloud-subscription:subscribe:{}", content.slug),
                "",
                None,
                44.0,
                0.55,
                busy,
            )?,
        )?;
    }
    append(&row, &actions)?;
    Ok(row)
}

pub(super) fn party_row(
    document: &Document,
    party: &PublicGameParty,
    selected: bool,
    busy: bool,
    pending: bool,
) -> Result<HtmlElement, String> {
    let playing = matches!(party.activity, PartyActivity::Playing { .. });
    let action = directory_party_action(party.visibility, playing);
    let location = match &party.activity {
        PartyActivity::Hub => None,
        PartyActivity::Playing { boneyard_name } => Some(boneyard_name.as_str()),
    };
    let presentation =
        directory_party_presentation(party.member_count, party.max_members, location);
    let row = element(
        document,
        "article",
        &format!("dark-cloud-row dark-cloud-party-row{}", if selected { " selected" } else { "" }),
    )?;
    row.set_attribute("data-party-id", &party.id).map_err(|_| "party row identity")?;
    row.set_attribute("role", "listitem").map_err(|_| "party row role")?;
    let mut label = format!("Select {}'s party", party.leader);
    if party.mod_count > 0 {
        label.push_str(&format!(
            ", modded with {} {}",
            party.mod_count,
            if party.mod_count == 1 { "mod" } else { "mods" }
        ));
    }
    if party.cheats_enabled {
        label.push_str(", cheats enabled");
    }
    let main = selector(document, &label, &format!("party:{}", party.id), selected)?;
    let initial: String =
        js_sys::JsString::from(party.leader.as_str()).slice(0, 1).to_upper_case().into();
    text(document, &main, "span", "dark-cloud-party-mark", &initial, Menu, 1.0, 0xd9ba70, true)?;
    let copy = element(document, "span", "dark-cloud-row-copy")?;
    text(
        document,
        &copy,
        "strong",
        "",
        &format!("{}'s party", party.leader),
        Menu,
        0.85,
        if selected { 0xbfffbf } else { 0xd9ba70 },
        true,
    )?;
    let flags = element(document, "span", "dark-cloud-party-flags")?;
    if party.session_kind == GameSessionKind::PrivateCollege {
        text(document, &flags, "em", "", "PRIVATE COLLEGE", Medium, 0.85, 0xd9ba70, false)?;
    }
    if party.mod_count > 0 {
        text(
            document,
            &flags,
            "em",
            "",
            &format!("MODDED ({})", party.mod_count),
            Medium,
            0.85,
            0xd9ba70,
            false,
        )?;
    }
    if party.cheats_enabled {
        text(document, &flags, "em", "cheats", "CHEATS", Medium, 0.85, 0xe6a078, false)?;
    }
    append(&copy, &flags)?;
    narrative(document, &copy, "small", "", &party.members.join(", "))?;
    append(&main, &copy)?;
    text(
        document,
        &main,
        "span",
        "dark-cloud-party-members",
        &presentation.squad,
        Medium,
        0.85,
        0xd9ba70,
        false,
    )?;
    text(
        document,
        &main,
        "span",
        if playing { "dark-cloud-party-status playing" } else { "dark-cloud-party-status hub" },
        presentation.status,
        Medium,
        0.85,
        0xd9ba70,
        false,
    )?;
    text(
        document,
        &main,
        "span",
        "dark-cloud-party-location",
        presentation.location,
        Medium,
        0.85,
        0xd9ba70,
        true,
    )?
    .set_attribute("title", presentation.location)
    .map_err(|_| "party location title")?;
    append(&row, &main)?;
    let actions = element(document, "div", "dark-cloud-row-actions")?;
    let label = if pending {
        "REQUESTED"
    } else {
        match action {
            DirectoryPartyAction::Join => "JOIN",
            DirectoryPartyAction::Request => "REQUEST",
            DirectoryPartyAction::Wait => "IN GAME",
        }
    };
    append(
        &actions,
        &widgets::button(
            document,
            label,
            &format!("cloud-join-party:{}", party.id),
            "",
            None,
            44.0,
            0.55,
            busy || action == DirectoryPartyAction::Wait,
        )?,
    )?;
    append(&row, &actions)?;
    Ok(row)
}

fn selector(
    document: &Document,
    label: &str,
    key: &str,
    selected: bool,
) -> Result<HtmlElement, String> {
    let button = element(document, "button", "dark-cloud-row-main")?;
    for (name, value) in [
        ("type", "button"),
        ("aria-label", label),
        ("aria-pressed", if selected { "true" } else { "false" }),
    ] {
        button.set_attribute(name, value).map_err(|_| "Cloud row selector semantics")?;
    }
    button
        .set_attribute("data-menu-action", &format!("cloud-select:{key}"))
        .map_err(|_| "Cloud selection action")?;
    button.set_attribute("data-cloud-row-key", key).map_err(|_| "Cloud double click target")?;
    Ok(button)
}

pub(super) struct PresenceSnapshot<'a> {
    pub players: &'a [ConnectedGamePlayer],
    pub matches: &'a [DeveloperGameMatch],
    pub loading: bool,
    pub error: Option<&'a str>,
    pub observing: Option<&'a str>,
}

pub(super) fn developer_presence(
    document: &Document,
    snapshot: PresenceSnapshot<'_>,
) -> Result<HtmlElement, String> {
    let root = element(document, "section", "dark-cloud-dev-presence")?;
    root.set_attribute("aria-label", "All connected players (developer)")
        .map_err(|_| "developer presence label")?;
    let header = element(document, "header", "dark-cloud-dev-presence-heading")?;
    text(document, &header, "span", "", "DEVELOPER SIGHT", Medium, 1.0, 0xd9ba70, false)?;
    let count = if snapshot.loading && snapshot.players.is_empty() {
        "SCRYING...".to_owned()
    } else {
        format!(
            "{} CONNECTED {}",
            snapshot.players.len(),
            if snapshot.players.len() == 1 { "WIZARD" } else { "WIZARDS" }
        )
    };
    text(
        document,
        &header,
        "span",
        "dark-cloud-dev-presence-count",
        &count,
        Medium,
        1.0,
        0xd9ba70,
        false,
    )?;
    append(&root, &header)?;
    if let Some(error) = snapshot.error {
        narrative(document, &root, "p", "dark-cloud-dev-presence-note", error)?
            .set_attribute("role", "alert")
            .map_err(|_| "developer error role")?;
    }
    if !snapshot.loading && snapshot.error.is_none() && snapshot.players.is_empty() {
        text(
            document,
            &root,
            "p",
            "dark-cloud-dev-presence-note",
            "NO CONNECTED WIZARDS.",
            Medium,
            1.0,
            0xd9ba70,
            false,
        )?;
    }
    let matches = element(document, "div", "dark-cloud-dev-matches")?;
    presence_heading(document, &matches, "ACTIVE MATCHES", snapshot.matches.len())?;
    if !snapshot.loading && snapshot.error.is_none() && snapshot.matches.is_empty() {
        text(
            document,
            &matches,
            "p",
            "dark-cloud-dev-presence-note",
            "NO ACTIVE MATCHES.",
            Medium,
            1.0,
            0xd9ba70,
            false,
        )?;
    }
    for game in snapshot.matches {
        let row = element(document, "div", "dark-cloud-dev-match-row")?;
        let names = element(document, "span", "")?;
        text(document, &names, "strong", "", &game.boneyard_name, Medium, 1.0, 0xd9ba70, true)?;
        text(
            document,
            &names,
            "small",
            "",
            &game.players.join(", "),
            Medium,
            0.85,
            0xd9ba70,
            true,
        )?;
        append(&row, &names)?;
        let status = if game.wave_number > 0 {
            format!("WAVE {}", game.wave_number)
        } else {
            "STAGING".to_owned()
        };
        text(document, &row, "span", "", &status, Medium, 0.85, 0xd9ba70, false)?;
        text(
            document,
            &row,
            "span",
            "",
            match game.visibility {
                solomon_sim::social::PartyVisibility::Public => "PUBLIC",
                solomon_sim::social::PartyVisibility::InviteOnly => "INVITE-ONLY",
                solomon_sim::social::PartyVisibility::Private => "PRIVATE",
            },
            Medium,
            0.85,
            0xd9ba70,
            false,
        )?;
        text(
            document,
            &row,
            "span",
            "",
            session_label(game.session),
            Medium,
            0.85,
            0xd9ba70,
            false,
        )?;
        append(
            &row,
            &widgets::button(
                document,
                if snapshot.observing == Some(game.id.as_str()) { "OPENING..." } else { "OBSERVE" },
                &format!("cloud-observe:{}", game.id),
                "",
                None,
                44.0,
                0.55,
                snapshot.observing.is_some(),
            )?,
        )?;
        append(&matches, &row)?;
    }
    append(&root, &matches)?;
    presence_heading(document, &root, "CONNECTED WIZARDS", snapshot.players.len())?;
    for player in snapshot.players {
        let row = element(document, "div", "dark-cloud-dev-presence-row")?;
        let wizard = element(document, "span", "dark-cloud-dev-presence-wizard")?;
        text(document, &wizard, "strong", "", &player.display_name, Medium, 1.0, 0xd9ba70, true)?;
        let detail = if player.bot {
            "ML BOT"
        } else {
            player.account_username.as_deref().unwrap_or("GUEST WIZARD")
        };
        text(
            document,
            &wizard,
            "small",
            "",
            &format!("{detail}{}", if player.developer { ", DEV" } else { "" }),
            Medium,
            0.85,
            0xd9ba70,
            true,
        )?;
        append(&row, &wizard)?;
        text(
            document,
            &row,
            "span",
            "dark-cloud-dev-presence-session",
            session_label(player.session),
            Medium,
            1.0,
            0xd9ba70,
            false,
        )?;
        let (activity, location, status) = match &player.activity {
            ConnectedPlayerActivity::Hub => ("hub", "COLLEGE COURTYARD", "IN HUB".to_owned()),
            ConnectedPlayerActivity::Boneyard { boneyard_name, wave_number } => (
                "boneyard",
                boneyard_name.as_str(),
                if *wave_number > 0 { format!("WAVE {wave_number}") } else { "STAGING".to_owned() },
            ),
        };
        text(
            document,
            &row,
            "span",
            &format!("dark-cloud-dev-presence-status {activity}"),
            &status,
            Medium,
            0.85,
            0xd9ba70,
            false,
        )?;
        text(
            document,
            &row,
            "span",
            "dark-cloud-dev-presence-location",
            location,
            Medium,
            0.85,
            0xd9ba70,
            true,
        )?
        .set_attribute("title", location)
        .map_err(|_| "developer location title")?;
        let party = match (&player.party_leader, player.party_size) {
            (Some(leader), Some(size)) => format!("{leader}'s party of {size}"),
            _ => "NO PARTY".to_owned(),
        };
        text(
            document,
            &row,
            "span",
            "dark-cloud-dev-presence-party",
            &party,
            Medium,
            0.85,
            0xd9ba70,
            true,
        )?
        .set_attribute("title", &party)
        .map_err(|_| "developer party title")?;
        append(&root, &row)?;
    }
    Ok(root)
}

fn presence_heading(
    document: &Document,
    parent: &HtmlElement,
    title: &str,
    count: usize,
) -> Result<(), String> {
    let row = element(document, "div", "dark-cloud-dev-subheading")?;
    text(document, &row, "span", "", title, Medium, 1.0, 0xd9ba70, false)?;
    text(document, &row, "span", "", &count.to_string(), Medium, 1.0, 0xd9ba70, false)?;
    append(parent, &row)
}

fn session_label(kind: GameSessionKind) -> &'static str {
    if kind == GameSessionKind::GlobalHub { "GLOBAL HUB" } else { "PRIVATE COLLEGE" }
}
