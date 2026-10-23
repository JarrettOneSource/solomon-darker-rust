//! GPU catalog, directory and developer presence rows from `DarkCloudRows.tsx`.
use super::{super::widgets, art::CloudArt, images::CloudImages};
use crate::gpu_ui::UiFrame;
use crate::hosted::{
    ConnectedGamePlayer, ConnectedPlayerActivity, DeveloperGameMatch, ModSubscription, ModSummary,
    PartyActivity, PublicGameParty,
};
use egui::{Align2, Rect, Ui, Vec2};
use solomon_sim::menu::party_directory::{
    DirectoryPartyAction, directory_party_action, directory_party_presentation,
};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextAlign};
use solomon_sim::social::{GameSessionKind, PartyVisibility};
fn ink(
    frame: &UiFrame<'_>,
    ui: &Ui,
    text: &str,
    font: NativeUiFontName,
    scale: f32,
    tint: u32,
    rect: Rect,
    center: bool,
) {
    let spec = solomon_sim::native_ui_text::NativeUiTextSpec {
        scale: f64::from(scale),
        tint,
        align: if center { NativeUiTextAlign::Center } else { NativeUiTextAlign::Left },
        ..solomon_sim::native_ui_text::NativeUiTextSpec::new(
            font,
            text,
            f64::from(if center { rect.center().x } else { rect.left() }),
            f64::from(rect.center().y + 6.0 * scale),
        )
    };
    let layout = solomon_sim::native_ui_text::layout_native_ui_text(&spec);
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    if layout.unsupported_code_points.is_empty() {
        crate::gpu_ui::paint_text_layout(frame, &painter, &layout);
    } else {
        let font = egui::FontId::new(
            solomon_sim::native_ui_text::native_ui_font(font).metrics[0] as f32 * scale,
            egui::FontFamily::Name("Cinzel".into()),
        );
        painter.text(
            if center { rect.center() } else { rect.left_center() },
            if center { Align2::CENTER_CENTER } else { Align2::LEFT_CENTER },
            text,
            font,
            widgets::color(tint),
        );
    }
}
pub(super) fn mod_row(
    frame: &mut UiFrame<'_>,
    ui: &mut Ui,
    _art: &CloudArt,
    content: &ModSummary,
    subscription: Option<&ModSubscription>,
    selected: bool,
    busy: bool,
    subscribed: bool,
    images: &mut CloudImages,
    width: f32,
    viewport_width: f32,
    enabled: bool,
    actions: &mut Vec<String>,
) -> Result<(), String> {
    let phone = viewport_width <= 700.0;
    let action_height = if phone && selected { 56.0 } else { 0.0 };
    let height = if phone { 92.0 + action_height } else { 76.0 };
    let (_, row) = ui.allocate_space(Vec2::new(width, height));
    if selected {
        ui.painter().rect_filled(row, 0.0, widgets::alpha(0xbfffbf, 0.06));
    }
    ui.painter().line_segment(
        [row.left_bottom(), row.right_bottom()],
        egui::Stroke::new(1.0, widgets::alpha(0xd2b263, 0.12)),
    );
    let action_width = if viewport_width <= 880.0 {
        200.0
    } else if viewport_width <= 1300.0 {
        270.0
    } else {
        300.0
    };
    let main = Rect::from_min_size(
        row.min,
        Vec2::new(
            if phone { width } else { width - action_width },
            if phone { 92.0 } else { height },
        ),
    );
    let id = format!("cloud-select:mod:{}", content.slug);
    let response = widgets::interact(
        frame,
        ui,
        &id,
        &format!("Select {}", content.name),
        "button",
        main,
        enabled,
    );
    frame.record_value(&id, &content.slug, Some(selected));
    if enabled {
        if response.double_clicked() {
            actions.push(format!("cloud-open:mod:{}", content.slug));
        } else if response.clicked() {
            actions.push(id);
        }
    }
    let thumb = Rect::from_min_size(
        main.min + Vec2::new(3.0, if phone { 12.0 } else { 10.0 }),
        Vec2::new(if phone { 44.0 } else { 56.0 }, if phone { 48.0 } else { 56.0 }),
    );
    images.paint(frame, ui, content.thumbnail_url.as_deref(), &content.name, thumb, false);
    let title_left = thumb.right() + 9.0;
    let author_w = if viewport_width <= 880.0 {
        80.0
    } else if viewport_width <= 1300.0 {
        95.0
    } else {
        120.0
    };
    let status_w = if viewport_width <= 1300.0 { 135.0 } else { 165.0 };
    let version_w = if viewport_width <= 1050.0 {
        0.0
    } else if viewport_width <= 1300.0 {
        75.0
    } else {
        90.0
    };
    let copy_right =
        if phone { main.right() } else { main.right() - author_w - status_w - version_w - 24.0 };
    let copy = Rect::from_min_max(
        egui::pos2(title_left, main.top() + 7.0),
        egui::pos2(copy_right, main.top() + 33.0),
    );
    ink(
        frame,
        ui,
        &content.name,
        NativeUiFontName::Menu,
        0.85,
        if selected { 0xbfffbf } else { 0xd9ba70 },
        copy,
        false,
    );
    let summary_rect = Rect::from_min_max(
        egui::pos2(title_left, main.top() + 34.0),
        egui::pos2(copy_right, main.top() + 52.0),
    );
    let painter = ui.painter().with_clip_rect(summary_rect.intersect(ui.clip_rect()));
    painter.text(
        summary_rect.left_center(),
        Align2::LEFT_CENTER,
        &content.summary,
        egui::FontId::new(
            if phone { 13.0 } else { 14.0 },
            egui::FontFamily::Name("IM FELL English".into()),
        ),
        widgets::color(0xb2a78d),
    );
    if !phone {
        ink(
            frame,
            ui,
            &content.tags.iter().take(3).cloned().collect::<Vec<_>>().join(", "),
            NativeUiFontName::Medium,
            0.85,
            0xa99a70,
            Rect::from_min_max(
                egui::pos2(title_left, main.top() + 54.0),
                egui::pos2(copy_right, main.top() + 72.0),
            ),
            false,
        );
    }
    let author = if phone {
        Rect::from_min_size(
            egui::pos2(title_left, main.top() + 52.0),
            Vec2::new(main.right() - title_left, 17.0),
        )
    } else {
        Rect::from_min_size(egui::pos2(copy_right + 8.0, main.top()), Vec2::new(author_w, height))
    };
    ink(
        frame,
        ui,
        &content.author.username,
        NativeUiFontName::Medium,
        if phone { 0.85 } else { 1.0 },
        0xd9ba70,
        author,
        !phone,
    );
    if !phone && version_w > 0.0 {
        ink(
            frame,
            ui,
            &format!("v{}", content.latest_version),
            NativeUiFontName::Medium,
            1.0,
            0xd9ba70,
            Rect::from_min_size(
                egui::pos2(author.right() + 8.0, main.top()),
                Vec2::new(version_w, height),
            ),
            true,
        );
    }
    let on = subscription.is_some_and(|s| s.enabled);
    let state = if on {
        "ENABLED"
    } else if subscription.is_some() {
        "DISABLED"
    } else {
        "NOT SUBSCRIBED"
    };
    let status = if phone {
        Rect::from_min_size(
            egui::pos2(title_left, main.top() + 70.0),
            Vec2::new(main.right() - title_left, 18.0),
        )
    } else {
        Rect::from_min_size(
            egui::pos2(main.right() - status_w, main.top()),
            Vec2::new(status_w, height),
        )
    };
    ink(
        frame,
        ui,
        state,
        NativeUiFontName::Medium,
        0.85,
        if on { 0xa9d29d } else { 0xa99a70 },
        status,
        !phone,
    );
    if !phone || selected {
        let mut choices = vec![("VIEW", format!("cloud-open-mod:{}", content.slug), true)];
        if subscribed && let Some(s) = subscription {
            choices.push((
                if s.enabled { "DISABLE" } else { "ENABLE" },
                format!(
                    "cloud-subscription:{}:{}",
                    if s.enabled { "disable" } else { "enable" },
                    content.slug
                ),
                !busy,
            ));
            choices.push((
                "REMOVE",
                format!("cloud-subscription:unsubscribe:{}", content.slug),
                !busy,
            ));
        } else if subscription.is_none() {
            choices.push((
                if busy { "WORKING..." } else { "SUBSCRIBE" },
                format!("cloud-subscription:subscribe:{}", content.slug),
                !busy,
            ));
        }
        let r = if phone {
            Rect::from_min_size(row.min + Vec2::new(4.0, 96.0), Vec2::new(width - 8.0, 44.0))
        } else {
            Rect::from_min_size(
                egui::pos2(main.right() + 8.0, row.center().y - 22.0),
                Vec2::new(action_width - 16.0, 44.0),
            )
        };
        let w = (r.width() - (choices.len() - 1) as f32 * 10.0) / choices.len() as f32;
        for (index, (label, id, admit)) in choices.into_iter().enumerate() {
            let b = Rect::from_min_size(
                r.min + Vec2::new(index as f32 * (w + 10.0), 0.0),
                Vec2::new(w, 44.0),
            );
            if widgets::stock_button(frame, ui, &id, label, b, 0.55, enabled && admit).clicked()
                && enabled
                && admit
            {
                actions.push(id);
            }
        }
    }
    Ok(())
}
pub(super) fn party_row(
    frame: &mut UiFrame<'_>,
    ui: &mut Ui,
    _art: &CloudArt,
    party: &PublicGameParty,
    selected: bool,
    busy: bool,
    pending: bool,
    width: f32,
    viewport_width: f32,
    enabled: bool,
    actions: &mut Vec<String>,
) -> Result<(), String> {
    let phone = viewport_width <= 700.0;
    let height = if phone { 116.0 + if selected { 54.0 } else { 0.0 } } else { 88.0 };
    let (_, row) = ui.allocate_space(Vec2::new(width, height));
    if selected {
        ui.painter().rect_filled(row, 0.0, widgets::alpha(0xbfffbf, 0.06));
    }
    ui.painter().line_segment(
        [row.left_bottom(), row.right_bottom()],
        egui::Stroke::new(1.0, widgets::alpha(0xd2b263, 0.12)),
    );
    let main = Rect::from_min_size(
        row.min,
        Vec2::new(if phone { width } else { width - 160.0 }, if phone { 116.0 } else { 88.0 }),
    );
    let id = format!("cloud-select:party:{}", party.id);
    let response = widgets::interact(
        frame,
        ui,
        &id,
        &format!("Select {}'s party", party.leader),
        "button",
        main,
        enabled,
    );
    if enabled {
        if response.double_clicked() {
            actions.push(format!("cloud-open:party:{}", party.id));
        } else if response.clicked() {
            actions.push(id);
        }
    }
    let mark = Rect::from_min_size(
        main.min + Vec2::new(3.0, 12.0),
        Vec2::splat(if phone { 44.0 } else { 50.0 }),
    );
    ink(
        frame,
        ui,
        &party.leader.chars().next().unwrap_or(' ').to_uppercase().to_string(),
        NativeUiFontName::Menu,
        1.0,
        0xd9ba70,
        mark,
        true,
    );
    let left = mark.right() + 9.0;
    let copy_right = if phone { main.right() } else { main.right() - 365.0 };
    ink(
        frame,
        ui,
        &format!("{}'s party", party.leader),
        NativeUiFontName::Menu,
        0.85,
        if selected { 0xbfffbf } else { 0xd9ba70 },
        Rect::from_min_max(
            egui::pos2(left, main.top() + 6.0),
            egui::pos2(copy_right, main.top() + 30.0),
        ),
        false,
    );
    let mut flags = Vec::new();
    if party.session_kind == GameSessionKind::PrivateCollege {
        flags.push("PRIVATE COLLEGE".to_owned());
    }
    if party.mod_count > 0 {
        flags.push(format!("MODDED ({})", party.mod_count));
    }
    if party.cheats_enabled {
        flags.push("CHEATS".to_owned());
    }
    ink(
        frame,
        ui,
        &flags.join(" · "),
        NativeUiFontName::Medium,
        0.85,
        0xd9ba70,
        Rect::from_min_max(
            egui::pos2(left, main.top() + 30.0),
            egui::pos2(copy_right, main.top() + 48.0),
        ),
        false,
    );
    let members = Rect::from_min_max(
        egui::pos2(left, main.top() + 50.0),
        egui::pos2(copy_right, main.top() + 68.0),
    );
    ui.painter().with_clip_rect(members.intersect(ui.clip_rect())).text(
        members.left_center(),
        Align2::LEFT_CENTER,
        party.members.join(", "),
        egui::FontId::new(14.0, egui::FontFamily::Name("IM FELL English".into())),
        widgets::color(0xb2a78d),
    );
    let location = match &party.activity {
        PartyActivity::Hub => None,
        PartyActivity::Playing { boneyard_name } => Some(boneyard_name.as_str()),
    };
    let p = directory_party_presentation(party.member_count, party.max_members, location);
    let action = directory_party_action(party.visibility, location.is_some());
    if phone {
        ink(
            frame,
            ui,
            &format!("{} WIZARDS · {}", p.squad, p.status),
            NativeUiFontName::Medium,
            0.85,
            0xd9ba70,
            Rect::from_min_size(
                egui::pos2(left, main.top() + 72.0),
                Vec2::new(main.right() - left, 18.0),
            ),
            false,
        );
        ink(
            frame,
            ui,
            p.location,
            NativeUiFontName::Medium,
            0.85,
            0xd9ba70,
            Rect::from_min_size(
                egui::pos2(left, main.top() + 92.0),
                Vec2::new(main.right() - left, 18.0),
            ),
            false,
        );
    } else {
        for (text, x, w) in [
            (p.squad.as_str(), main.right() - 365.0, 95.0),
            (p.status, main.right() - 270.0, 90.0),
            (p.location, main.right() - 180.0, 165.0),
        ] {
            ink(
                frame,
                ui,
                text,
                NativeUiFontName::Medium,
                0.85,
                0xd9ba70,
                Rect::from_min_size(egui::pos2(x, main.top()), Vec2::new(w, main.height())),
                true,
            );
        }
    }
    if !phone || selected {
        let label = if pending {
            "REQUESTED"
        } else {
            match action {
                DirectoryPartyAction::Join => "JOIN",
                DirectoryPartyAction::Request => "REQUEST",
                DirectoryPartyAction::Wait => "IN GAME",
            }
        };
        let b = if phone {
            Rect::from_min_size(row.min + Vec2::new(4.0, 120.0), Vec2::new(width - 8.0, 44.0))
        } else {
            Rect::from_min_size(
                egui::pos2(main.right() + 18.0, row.center().y - 22.0),
                Vec2::new(130.0, 44.0),
            )
        };
        let id = format!("cloud-join-party:{}", party.id);
        let admitted = enabled && !busy && action != DirectoryPartyAction::Wait;
        if widgets::stock_button(frame, ui, &id, label, b, 0.55, admitted).clicked() && admitted {
            actions.push(id);
        }
    }
    Ok(())
}
pub(super) fn developer_presence(
    frame: &mut UiFrame<'_>,
    ui: &mut Ui,
    _art: &CloudArt,
    players: &[ConnectedGamePlayer],
    matches: &[DeveloperGameMatch],
    loading: bool,
    error: Option<&str>,
    observing: Option<&str>,
    width: f32,
    enabled: bool,
    actions: &mut Vec<String>,
) -> Result<(), String> {
    ui.add_space(24.0);
    let (_, header) = ui.allocate_space(Vec2::new(width, 44.0));
    ink(
        frame,
        ui,
        &format!(
            "DEVELOPER SIGHT   {} CONNECTED {}",
            players.len(),
            if players.len() == 1 { "WIZARD" } else { "WIZARDS" }
        ),
        NativeUiFontName::Medium,
        1.0,
        0xd9ba70,
        header,
        false,
    );
    if let Some(error) = error {
        let (_, r) = ui.allocate_space(Vec2::new(width, 50.0));
        widgets::paragraph(ui, error, r, "IM FELL English", 15.0, widgets::ERROR);
    }
    if loading && players.is_empty() {
        let (_, r) = ui.allocate_space(Vec2::new(width, 40.0));
        ink(frame, ui, "SCRYING...", NativeUiFontName::Medium, 1.0, 0xd9ba70, r, false);
    } else if !loading && error.is_none() && players.is_empty() {
        let (_, r) = ui.allocate_space(Vec2::new(width, 40.0));
        ink(frame, ui, "NO WIZARDS CONNECTED.", NativeUiFontName::Medium, 1.0, 0xd9ba70, r, false);
    }
    let phone = width < 600.0;
    for game in matches {
        let (_, r) = ui.allocate_space(Vec2::new(width, if phone { 132.0 } else { 76.0 }));
        let info = Rect::from_min_size(r.min, Vec2::new(r.width() - 130.0, 48.0));
        ink(
            frame,
            ui,
            &game.boneyard_name,
            NativeUiFontName::Medium,
            1.0,
            0xd9ba70,
            Rect::from_min_size(info.min, Vec2::new(info.width(), 24.0)),
            false,
        );
        ink(
            frame,
            ui,
            &game.players.join(", "),
            NativeUiFontName::Medium,
            0.85,
            0xd9ba70,
            Rect::from_min_size(info.min + Vec2::new(0.0, 26.0), Vec2::new(info.width(), 22.0)),
            false,
        );
        let status = if game.wave_number > 0 {
            format!("WAVE {}", game.wave_number)
        } else {
            "STAGING".to_owned()
        };
        let visibility = match game.visibility {
            PartyVisibility::Public => "PUBLIC",
            PartyVisibility::InviteOnly => "INVITE-ONLY",
            PartyVisibility::Private => "PRIVATE",
        };
        ink(
            frame,
            ui,
            &format!("{status} · {visibility} · {}", session_label(game.session)),
            NativeUiFontName::Medium,
            0.85,
            0xd9ba70,
            Rect::from_min_size(r.min + Vec2::new(0.0, 50.0), Vec2::new(r.width() - 130.0, 22.0)),
            false,
        );
        let b = Rect::from_min_size(
            egui::pos2(r.right() - 120.0, r.top() + if phone { 80.0 } else { 10.0 }),
            Vec2::new(110.0, 44.0),
        );
        let id = format!("cloud-observe:{}", game.id);
        let admitted = enabled && observing.is_none();
        if widgets::stock_button(
            frame,
            ui,
            &id,
            if observing == Some(game.id.as_str()) { "OPENING..." } else { "OBSERVE" },
            b,
            0.55,
            admitted,
        )
        .clicked()
            && admitted
        {
            actions.push(id);
        }
    }
    let (_, r) = ui.allocate_space(Vec2::new(width, 36.0));
    ink(
        frame,
        ui,
        &format!("CONNECTED WIZARDS   {}", players.len()),
        NativeUiFontName::Medium,
        1.0,
        0xd9ba70,
        r,
        false,
    );
    for player in players {
        let (_, r) = ui.allocate_space(Vec2::new(width, if phone { 130.0 } else { 80.0 }));
        let detail = if player.bot {
            "ML BOT"
        } else {
            player.account_username.as_deref().unwrap_or("GUEST WIZARD")
        };
        let (status, location) = match &player.activity {
            ConnectedPlayerActivity::Hub => ("IN HUB".to_owned(), "COLLEGE COURTYARD"),
            ConnectedPlayerActivity::Boneyard { boneyard_name, wave_number } => (
                if *wave_number > 0 { format!("WAVE {wave_number}") } else { "STAGING".to_owned() },
                boneyard_name.as_str(),
            ),
        };
        let party = match (&player.party_leader, player.party_size) {
            (Some(leader), Some(size)) => format!("{leader}'s party of {size}"),
            _ => "NO PARTY".to_owned(),
        };
        for (index, text) in [
            player.display_name.clone(),
            format!(
                "{detail}{} · {}",
                if player.developer { ", DEV" } else { "" },
                session_label(player.session)
            ),
            format!("{status} · {location}"),
            party,
        ]
        .iter()
        .enumerate()
        {
            ink(
                frame,
                ui,
                text,
                NativeUiFontName::Medium,
                if index == 0 { 1.0 } else { 0.85 },
                0xd9ba70,
                Rect::from_min_size(
                    r.min + Vec2::new(0.0, index as f32 * 20.0),
                    Vec2::new(width, 20.0),
                ),
                false,
            );
        }
    }
    Ok(())
}
fn session_label(kind: GameSessionKind) -> &'static str {
    if kind == GameSessionKind::GlobalHub { "GLOBAL HUB" } else { "PRIVATE COLLEGE" }
}
