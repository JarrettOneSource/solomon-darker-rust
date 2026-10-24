//! `JoinPartyScene.tsx` and `join-party.css`: native text, fluid grid and badges.
use super::JoinView;
use crate::gpu_ui::{FontRunOptions, NativeFontRun, UiFrame};
use crate::hosted::{PartyActivity, PublicGameParty};
use crate::menu::widgets;
use egui::{Align, Color32, FontFamily, FontId, Rect, Stroke, StrokeKind, Ui, Vec2};
use solomon_sim::menu::party_directory::{
    DirectoryPartyAction, complete_party_code, directory_party_action,
    directory_party_presentation, normalize_party_code,
};
use solomon_sim::social::GameSessionKind;
use std::sync::Arc;

#[derive(Clone, Copy)]
enum Style {
    Heading(bool),
    Subtitle,
    Caption,
    Leader,
    Squad(bool),
    Flag(bool),
    Small(u32),
    Body,
    Error,
    Button,
}
fn run(
    ui: &Ui,
    text: &str,
    style: Style,
    width: Option<f32>,
) -> Result<Arc<NativeFontRun>, String> {
    let (family, size, color, line_height, spacing) = match style {
        Style::Heading(compact) => {
            let size = if compact { 20.0 } else { 30.0 };
            ("Cinzel Bold", size, 0xf4d887, None, size * 0.14)
        }
        Style::Subtitle => ("IM FELL English", 16.0, 0xaaa088, Some(24.0), 0.0),
        Style::Caption => ("Cinzel Bold", 11.0, 0xead9aa, None, 1.32),
        Style::Leader => ("IM FELL English Bold", 16.0, 0xead9aa, Some(24.0), 0.0),
        Style::Squad(compact) => {
            let size = if compact { 11.0 } else { 16.0 };
            ("IM FELL English", size, 0xcbb985, Some(size * 1.5), 0.0)
        }
        Style::Flag(cheats) => {
            ("Cinzel Bold", 8.0, if cheats { 0xe6a078 } else { 0xc8ad69 }, None, 0.48)
        }
        Style::Small(color) => ("IM FELL English", 12.8, color, Some(19.2), 0.0),
        Style::Body => ("IM FELL English", 16.0, 0xead9aa, Some(24.0), 0.0),
        Style::Error => ("IM FELL English", 16.0, 0xe8a99e, Some(24.0), 0.0),
        Style::Button => ("Cinzel Bold", 11.0, 0xf0d690, None, 0.0),
    };
    crate::gpu_ui::font_run_with_options(
        ui.ctx(),
        FontId::new(size, FontFamily::Name(family.into())),
        widgets::color(color),
        text,
        FontRunOptions {
            wrap_width: width,
            line_height,
            letter_spacing: Some(spacing),
            ..Default::default()
        },
    )
}
fn centered(frame: &UiFrame<'_>, ui: &Ui, run: &NativeFontRun, rect: Rect) -> Result<(), String> {
    run.paint_aligned(frame, ui.painter(), rect, Align::Center, 1.0)
}
fn button_size(run: &NativeFontRun) -> Vec2 {
    Vec2::new(run.size().x + 38.0, 44.0)
}
fn button(
    frame: &UiFrame<'_>,
    ui: &Ui,
    id: &str,
    label: &str,
    run: &NativeFontRun,
    rect: Rect,
    enabled: bool,
) -> Result<egui::Response, String> {
    let response = widgets::interact_unstyled(frame, ui, id, label, "button", rect, enabled);
    frame.paint_group(
        ui.painter(),
        crate::gpu_ui::GroupEffect {
            opacity: if enabled { 1.0 } else { 0.5 },
            ..Default::default()
        },
        |frame, painter| {
            let mut mesh = egui::Mesh::default();
            mesh.colored_vertex(rect.left_top(), widgets::color(0x3c301a));
            mesh.colored_vertex(rect.right_top(), widgets::color(0x3c301a));
            mesh.colored_vertex(rect.right_bottom(), widgets::color(0x171108));
            mesh.colored_vertex(rect.left_bottom(), widgets::color(0x171108));
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            painter.add(egui::Shape::mesh(mesh));
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(1.0, widgets::color(0x947b42)),
                StrokeKind::Inside,
            );
            run.paint(frame, painter, rect.center() - run.size() / 2.0, 1.0)
        },
    )?;
    widgets::focus_outline(frame, ui, &response, 2.0, 2.0, widgets::alpha(0x41e3ff, 0.7));
    Ok(response)
}

pub(super) fn paint(
    join: &mut JoinView,
    frame: &mut UiFrame<'_>,
    ui: &mut Ui,
    mut view: Rect,
    actions: &mut Vec<String>,
) -> Result<(), String> {
    view.max.y = view.min.y + view.height().min(join.viewport_height as f32);
    let center = egui::pos2(view.center().x, view.top() + view.height() * 0.2);
    let radius = Vec2::new(view.width() / 2.0, view.height() * 0.8).length() * 0.68;
    widgets::radial_gradient(
        ui,
        view,
        center,
        radius,
        widgets::alpha(0x2b2212, 0.97),
        widgets::alpha(0x040508, 0.99),
    );
    let compact = view.width() <= 700.0 || view.height() <= 620.0;
    let gap = if compact { 8.0 } else { 14.0 };
    let mut top = view.top() + if compact { 10.0 } else { 24.0 };
    let heading = run(ui, "JOIN PARTY", Style::Heading(compact), None)?;
    let heading_rect = Rect::from_center_size(
        egui::pos2(view.center().x, top + heading.size().y / 2.0),
        heading.size(),
    );
    centered(frame, ui, &heading, heading_rect)?;
    frame.record_clipped_rect(
        "join-heading",
        "JOIN PARTY",
        "heading",
        heading_rect,
        ui.clip_rect(),
        false,
    );
    top += heading.size().y;
    if !compact {
        let text = "Enter a Party ID or join a listed party.";
        let subtitle = run(ui, text, Style::Subtitle, None)?;
        let rect = Rect::from_min_size(
            egui::pos2(view.left() + 24.0, top + 5.0),
            Vec2::new(view.width() - 48.0, subtitle.size().y),
        );
        centered(frame, ui, &subtitle, rect)?;
        frame.record_clipped_rect("join-subtitle", text, "status", rect, ui.clip_rect(), false);
        top += 5.0 + subtitle.size().y;
    }
    top += gap;
    let width = 560.0_f32.min(view.width() - 48.0);
    let left = view.center().x - width / 2.0;
    let caption = run(ui, "PARTY ID", Style::Caption, None)?;
    caption.paint(frame, ui.painter(), egui::pos2(left, top), 1.0)?;
    top += caption.size().y + 5.0;
    let join_button = run(ui, "JOIN", Style::Button, None)?;
    let join_size = button_size(&join_button);
    let field =
        Rect::from_min_size(egui::pos2(left, top), Vec2::new(width - join_size.x - 8.0, 44.0));
    ui.painter().rect_filled(field, 0.0, widgets::color(0x080704));
    ui.painter().rect_stroke(
        field,
        0.0,
        Stroke::new(1.0, widgets::color(0x947b42)),
        StrokeKind::Inside,
    );
    let edited =
        crate::gpu_ui::NativeTextEdit::singleline(&mut join.code, "join-party-code", "Party ID")
            .font(FontId::new(20.0, FontFamily::Name("JetBrains Mono Bold".into())))
            .color(widgets::color(0xfff0bd))
            .margin(Vec2::new(15.0, 9.0))
            .letter_spacing(3.2)
            .hint_text("XXXX-XXXX")
            .enabled(true)
            .rules(crate::gpu_ui::EditorRules {
                utf16_limit: Some(128),
                normalize: Some(normalize_party_code),
                ..Default::default()
            })
            .show(frame, ui, field);
    frame.record_value("join-party-code", &join.code, None);
    widgets::focus_outline(frame, ui, &edited, 2.0, 2.0, widgets::alpha(0x41e3ff, 0.7));
    if edited.submitted && complete_party_code(&join.code) && !join.busy {
        actions.push("join-code".to_owned());
    }
    let action = Rect::from_min_size(egui::pos2(field.right() + 8.0, top), join_size);
    if button(
        frame,
        ui,
        "join-code",
        "JOIN",
        &join_button,
        action,
        !join.busy && complete_party_code(&join.code),
    )?
    .clicked()
        && !join.busy
        && complete_party_code(&join.code)
    {
        actions.push("join-code".to_owned());
    }
    top += 44.0 + gap;
    if let Some(error) = join.join_error.as_deref().or(join.directory_error.as_deref()) {
        let width = 760.0_f32.min(view.width() - 48.0);
        let error_run = run(ui, error, Style::Error, Some(width))?;
        let rect = Rect::from_min_size(
            egui::pos2(view.center().x - width / 2.0, top),
            Vec2::new(width, error_run.size().y),
        );
        centered(frame, ui, &error_run, rect)?;
        frame.record_clipped_rect("join-error", error, "alert", rect, ui.clip_rect(), false);
        top += error_run.size().y;
    }
    top += gap;
    let back = run(ui, "BACK", Style::Button, None)?;
    let refresh = run(ui, "REFRESH", Style::Button, None)?;
    let back_size = button_size(&back);
    let refresh_size = button_size(&refresh);
    let footer_top = view.bottom() - 20.0 - 44.0;
    let footer_left = view.center().x - (back_size.x + refresh_size.x + 10.0) / 2.0;
    for (label, id, run, x, size) in [
        ("BACK", "join-back", &back, footer_left, back_size),
        ("REFRESH", "join-refresh", &refresh, footer_left + back_size.x + 10.0, refresh_size),
    ] {
        if button(
            frame,
            ui,
            id,
            label,
            run,
            Rect::from_min_size(egui::pos2(x, footer_top), size),
            true,
        )?
        .clicked()
        {
            actions.push(id.to_owned());
        }
    }
    let list_width = 900.0_f32.min(view.width() - 48.0);
    let list = Rect::from_min_max(
        egui::pos2(view.center().x - list_width / 2.0, top),
        egui::pos2(view.center().x + list_width / 2.0, footer_top - gap),
    );
    ui.painter().rect_filled(list, 0.0, Color32::from_black_alpha(89));
    ui.painter().rect_stroke(
        list,
        0.0,
        Stroke::new(1.0, widgets::alpha(0xbf9d4e, 0.45)),
        StrokeKind::Inside,
    );
    frame.record_rect("join-party-list", "Party directory", "region", list, true);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("join-list")
            .max_rect(list.shrink(1.0))
            .layout(egui::Layout::top_down(Align::Min)),
    );
    child.set_clip_rect(list.shrink(1.0).intersect(ui.clip_rect()));
    child.spacing_mut().item_spacing = Vec2::ZERO;
    widgets::scrollbar_style(&mut child, 0x241e2e, 0x08070b, false);
    child.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
    child.spacing_mut().scroll.bar_width = 10.0;
    child.spacing_mut().scroll.bar_inner_margin = 0.0;
    child.spacing_mut().scroll.bar_outer_margin = 0.0;
    egui::ScrollArea::vertical()
        .animated(false)
        .id_salt("join-party-list")
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            if join.parties.is_empty() {
                let text = if join.loading {
                    "CONSULTING THE DARK CLOUD…"
                } else {
                    "NO PARTIES ARE FORMING RIGHT NOW."
                };
                let line = run(ui, text, Style::Body, Some(ui.available_width() - 64.0))?;
                let (_, rect) =
                    ui.allocate_space(Vec2::new(ui.available_width(), line.size().y + 64.0));
                centered(frame, ui, &line, rect.shrink(32.0))?;
                frame.record_clipped_rect(
                    "join-status",
                    text,
                    "status",
                    rect,
                    ui.clip_rect(),
                    false,
                );
            }
            for party in &join.parties {
                row(join, frame, ui, party, compact, actions)?;
            }
            Ok::<(), String>(())
        })
        .inner?;
    Ok(())
}

struct Badge {
    label: Arc<NativeFontRun>,
    rect: Rect,
    cheats: bool,
}
fn badges(ui: &Ui, party: &PublicGameParty, width: f32) -> Result<(Vec<Badge>, f32), String> {
    let mut labels = Vec::new();
    if party.session_kind == GameSessionKind::PrivateCollege {
        labels.push(("PRIVATE COLLEGE".to_owned(), false));
    }
    if party.mod_count > 0 {
        labels.push((format!("MODDED · {}", party.mod_count), false));
    }
    if party.cheats_enabled {
        labels.push(("CHEATS".to_owned(), true));
    }
    let mut badges = Vec::new();
    let mut x = 0.0;
    let mut y = 0.0;
    let mut height = 0.0_f32;
    for (text, cheats) in labels {
        let label = run(ui, &text, Style::Flag(cheats), None)?;
        let size = label.size() + Vec2::new(10.0, 4.0);
        if x > 0.0 && x + size.x > width {
            x = 0.0;
            y += height + 3.0;
            height = 0.0;
        }
        let rect = Rect::from_min_size(egui::pos2(x, y), size);
        badges.push(Badge { label, rect, cheats });
        x += size.x + 3.0;
        height = height.max(size.y);
    }
    Ok((badges, y + height + 4.0))
}
fn row(
    join: &JoinView,
    frame: &UiFrame<'_>,
    ui: &mut Ui,
    party: &PublicGameParty,
    compact: bool,
    actions: &mut Vec<String>,
) -> Result<(), String> {
    let location = match &party.activity {
        PartyActivity::Hub => None,
        PartyActivity::Playing { boneyard_name } => Some(boneyard_name.as_str()),
    };
    let presentation =
        directory_party_presentation(party.member_count, party.max_members, location);
    let policy = directory_party_action(party.visibility, location.is_some());
    let label = if policy == DirectoryPartyAction::Wait {
        "IN GAME"
    } else if join.pending_listing.as_deref() == Some(party.id.as_str()) {
        "REQUESTED"
    } else if policy == DirectoryPartyAction::Request {
        "REQUEST"
    } else {
        "JOIN"
    };
    let action = run(ui, label, Style::Button, None)?;
    let action_size = button_size(&action);
    let squad = run(ui, &presentation.squad, Style::Squad(compact), None)?;
    let inner_width = ui.available_width() - 24.0;
    let copy_width =
        (inner_width - action_size.x - if compact { 10.0 } else { 28.0 + squad.size().x }).max(0.0);
    let leader = run(ui, &party.leader.to_uppercase(), Style::Leader, Some(copy_width))?;
    let (flags, flag_height) = badges(ui, party, copy_width)?;
    let copy_height = leader.size().y + flag_height + 38.4;
    let main_height = if compact {
        copy_height + 4.0 + squad.size().y
    } else {
        copy_height.max(44.0).max(squad.size().y)
    };
    let (_, rect) = ui.allocate_space(Vec2::new(ui.available_width(), main_height + 21.0));
    let copy =
        Rect::from_min_size(rect.min + Vec2::new(12.0, 10.0), Vec2::new(copy_width, copy_height));
    leader.paint(frame, ui.painter(), copy.min, 1.0)?;
    for badge in flags {
        let box_rect =
            badge.rect.translate(copy.min.to_vec2() + Vec2::new(0.0, leader.size().y + 2.0));
        ui.painter().rect_stroke(
            box_rect,
            0.0,
            Stroke::new(
                1.0,
                widgets::alpha(
                    if badge.cheats { 0xd37346 } else { 0xbe9c4c },
                    if badge.cheats { 0.48 } else { 0.38 },
                ),
            ),
            StrokeKind::Inside,
        );
        badge.label.paint(frame, ui.painter(), box_rect.min + Vec2::new(5.0, 2.0), 1.0)?;
    }
    let members = Rect::from_min_size(
        copy.min + Vec2::new(0.0, leader.size().y + flag_height),
        Vec2::new(copy_width, 19.2),
    );
    widgets::narrative_line(
        frame,
        ui,
        &party.members.join(" · "),
        members,
        12.8,
        widgets::color(0x9e9580),
    )?;
    let status_rect = members.translate(Vec2::new(0.0, 19.2));
    let status = run(
        ui,
        presentation.status,
        Style::Small(if location.is_some() { 0xdeb66b } else { 0x88af81 }),
        None,
    )?;
    let divider = run(ui, " · ", Style::Small(0x9e9580), None)?;
    let copy_painter = ui.painter().with_clip_rect(copy.intersect(ui.clip_rect()));
    status.paint(frame, &copy_painter, status_rect.min, 1.0)?;
    divider.paint(frame, &copy_painter, status_rect.min + Vec2::new(status.size().x, 0.0), 1.0)?;
    widgets::narrative_line(
        frame,
        ui,
        presentation.location,
        Rect::from_min_max(
            status_rect.min + Vec2::new(status.size().x + divider.size().x, 0.0),
            status_rect.max,
        ),
        12.8,
        widgets::color(0xb6a77f),
    )?;
    let squad_origin = if compact {
        copy.min + Vec2::new(0.0, copy_height + 4.0)
    } else {
        egui::pos2(copy.right() + 14.0, rect.center().y - squad.size().y / 2.0)
    };
    squad.paint(frame, ui.painter(), squad_origin, 1.0)?;
    let button_rect = Rect::from_min_size(
        egui::pos2(
            rect.right() - 12.0 - action_size.x,
            rect.top() + 10.0 + (main_height - 44.0) / 2.0,
        ),
        action_size,
    );
    let id = format!("join-listing:{}", party.id);
    let enabled = !join.busy && policy != DirectoryPartyAction::Wait;
    if button(frame, ui, &id, label, &action, button_rect, enabled)?.clicked() && enabled {
        actions.push(id);
    }
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), rect.bottom() - 0.5),
            egui::pos2(rect.right(), rect.bottom() - 0.5),
        ],
        Stroke::new(1.0, widgets::alpha(0xbf9d4e, 0.2)),
    );
    Ok(())
}
