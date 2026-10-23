//! Source `play-routing-dialog.css` and `game-save-mod-mismatch.css` flow and paint.
//! Admission, request identity and cancellation stay in the parent controller.
use super::{Prompt, RoutingView};
use crate::gpu_ui::{FontRunOptions, ImageShadow, NativeFontRun, StockAtlas, UiFrame};
use crate::menu::widgets;
use egui::{Align, Color32, FontFamily, FontId, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};
use solomon_sim::social::GameSessionKind;
use std::sync::Arc;

struct Text {
    source: String,
    run: Arc<NativeFontRun>,
    centered: bool,
}
impl Text {
    fn paint(
        &self,
        frame: &UiFrame<'_>,
        ui: &Ui,
        rect: Rect,
        visible: &mut Vec<String>,
    ) -> Result<(), String> {
        if rect.intersects(ui.clip_rect()) {
            visible.push(self.source.clone());
        }
        self.run.paint_aligned(
            frame,
            ui.painter(),
            rect,
            if self.centered { Align::Center } else { Align::Min },
            1.0,
        )
    }
}

#[derive(Clone, Copy)]
enum Typography {
    Heading,
    Body,
    Section,
    Warning,
    MismatchHeading,
    MismatchBody,
    MismatchWarning,
    GroupHeading,
    Mono,
}
fn text(ui: &Ui, source: &str, kind: Typography, width: f32, scale: f32) -> Result<Text, String> {
    let (family, size, color, height, spacing, centered) = match kind {
        Typography::Heading => ("Cinzel Bold", 24.0, 0xf0d281, None, 0.0, true),
        Typography::Body => ("IM FELL English", 16.0, 0xded1ac, Some(24.0), 0.0, false),
        Typography::Section => ("Cinzel Bold", 11.0, 0xcdb574, None, 1.32, false),
        Typography::Warning => ("Cinzel Bold", 12.0, 0xe1ad87, None, 1.2, true),
        Typography::MismatchHeading => ("Cinzel", 27.0, 0xd7bd78, Some(40.5), 0.0, true),
        Typography::MismatchBody => ("IM FELL English", 17.0, 0xc6b990, None, 0.0, true),
        Typography::MismatchWarning => ("IM FELL English", 15.0, 0xd5997e, None, 0.0, true),
        Typography::GroupHeading => ("Cinzel", 13.0, 0xd7bd78, Some(19.5), 2.08, false),
        Typography::Mono => ("JetBrains Mono", 14.0, 0xeee0bc, None, 0.0, false),
    };
    // These source blocks have CSS white-space:normal, including external metadata.
    let source = source
        .split([' ', '\t', '\n', '\r', '\u{000c}'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let run = crate::gpu_ui::font_run_with_options(
        ui.ctx(),
        FontId::new(size * scale, FontFamily::Name(family.into())),
        widgets::color(color),
        &source,
        FontRunOptions {
            wrap_width: Some(width),
            line_height: height.map(|height| height * scale),
            letter_spacing: Some(spacing * scale),
            ..Default::default()
        },
    )?;
    Ok(Text { source, run, centered })
}

enum Block {
    Text(Text),
    Gap(f32),
    List { items: Vec<Text>, marker: Text, indent: f32, maximum: f32 },
    Group { heading: Text, items: Vec<Text>, marker: Text, scale: f32 },
    Progress { label: Text, percent: f32, scale: f32 },
}
impl Block {
    fn height(&self) -> f32 {
        match self {
            Self::Text(text) => text.run.size().y,
            Self::Gap(height) => *height,
            Self::List { items, maximum, .. } => {
                items.iter().map(|item| item.run.size().y).sum::<f32>().min(*maximum)
            }
            Self::Group { heading, items, scale, .. } => {
                26.0 * scale
                    + heading.run.size().y
                    + 7.0 * scale
                    + items.iter().map(|item| item.run.size().y).sum::<f32>()
            }
            Self::Progress { scale, .. } => 46.0 * scale,
        }
    }
    fn paint(
        &self,
        frame: &UiFrame<'_>,
        ui: &mut Ui,
        width: f32,
        index: usize,
        visible: &mut Vec<String>,
    ) -> Result<(), String> {
        let (_, rect) = ui.allocate_space(Vec2::new(width, self.height()));
        match self {
            Self::Text(text) => text.paint(frame, ui, rect, visible)?,
            Self::Gap(_) => {}
            Self::List { items, marker, indent, maximum } => {
                let mut child = ui.new_child(
                    egui::UiBuilder::new().id_salt(("routing-list", index)).max_rect(rect),
                );
                child.set_clip_rect(rect.intersect(ui.clip_rect()));
                child.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
                child.spacing_mut().scroll.bar_width = 10.0;
                child.spacing_mut().scroll.bar_inner_margin = 0.0;
                child.spacing_mut().scroll.bar_outer_margin = 0.0;
                egui::ScrollArea::vertical()
                    .id_salt(("routing-list-scroll", index))
                    .max_height(*maximum)
                    .auto_shrink([false, true])
                    .show(&mut child, |ui| {
                        for item in items {
                            let (_, row) = ui.allocate_space(Vec2::new(width, item.run.size().y));
                            marker.paint(
                                frame,
                                ui,
                                Rect::from_min_size(row.min, Vec2::new(*indent, item.run.size().y)),
                                &mut Vec::new(),
                            )?;
                            item.paint(
                                frame,
                                ui,
                                Rect::from_min_max(row.min + Vec2::new(*indent, 0.0), row.max),
                                visible,
                            )?;
                        }
                        Ok::<(), String>(())
                    })
                    .inner?;
            }
            Self::Group { heading, items, marker, scale } => {
                ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(115));
                ui.painter().rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(*scale, widgets::color(0x4f452e)),
                    StrokeKind::Inside,
                );
                let inner = rect.shrink2(Vec2::new(17.0, 13.0) * *scale);
                heading.paint(
                    frame,
                    ui,
                    Rect::from_min_size(inner.min, Vec2::new(inner.width(), heading.run.size().y)),
                    visible,
                )?;
                let mut top = inner.top() + heading.run.size().y + 7.0 * scale;
                for item in items {
                    let row = Rect::from_min_size(
                        egui::pos2(inner.left(), top),
                        Vec2::new(inner.width(), item.run.size().y),
                    );
                    marker.paint(
                        frame,
                        ui,
                        Rect::from_min_size(row.min, Vec2::new(18.0 * scale, row.height())),
                        &mut Vec::new(),
                    )?;
                    item.paint(
                        frame,
                        ui,
                        Rect::from_min_max(row.min + Vec2::new(18.0 * scale, 0.0), row.max),
                        visible,
                    )?;
                    top += row.height();
                }
            }
            Self::Progress { label, percent, scale } => {
                let bar = Rect::from_min_max(rect.min + Vec2::new(0.0, 16.0 * scale), rect.max);
                ui.painter().rect_filled(bar, 0.0, widgets::color(0x080704));
                let fill = Rect::from_min_size(
                    bar.min,
                    Vec2::new(bar.width() * percent / 100.0, bar.height()),
                );
                let mut mesh = egui::Mesh::default();
                mesh.colored_vertex(fill.left_top(), widgets::color(0x4b3919));
                mesh.colored_vertex(fill.left_bottom(), widgets::color(0x4b3919));
                mesh.colored_vertex(fill.right_bottom(), widgets::color(0xb68d3d));
                mesh.colored_vertex(fill.right_top(), widgets::color(0xb68d3d));
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                ui.painter().add(egui::Shape::mesh(mesh));
                ui.painter().rect_stroke(
                    bar,
                    0.0,
                    Stroke::new(*scale, widgets::color(0x66552f)),
                    StrokeKind::Inside,
                );
                let text_rect = Rect::from_center_size(
                    bar.center(),
                    Vec2::new(bar.width(), label.run.size().y),
                );
                frame.paint_group(
                    ui.painter(),
                    crate::gpu_ui::GroupEffect {
                        shadows: vec![ImageShadow {
                            offset: Vec2::new(0.0, *scale),
                            blur: *scale,
                            color: Color32::BLACK,
                        }],
                        ..Default::default()
                    },
                    |group, painter| {
                        label.run.paint_aligned(group, painter, text_rect, Align::Center, 1.0)
                    },
                )?;
                if bar.intersects(ui.clip_rect()) {
                    visible.push(label.source.clone());
                }
            }
        }
        Ok(())
    }
}

struct Action {
    id: &'static str,
    label: &'static str,
    run: Arc<NativeFontRun>,
}
struct Presentation {
    heading: String,
    blocks: Vec<Block>,
    actions: Vec<Action>,
    mismatch: bool,
    scale: f32,
    width: f32,
    content_width: f32,
}
impl Presentation {
    fn new(routing: &RoutingView, ui: &Ui, view: Rect, scrollbar: bool) -> Result<Self, String> {
        let prompt = routing.prompt.as_ref().expect("painted routing prompt");
        let mismatch = matches!(prompt, Prompt::Mismatch(_));
        let scale = if mismatch { (view.width() / 1600.0).min(view.height() / 900.0) } else { 1.0 };
        let width = if mismatch { 720.0 * scale } else { 680.0_f32.min(view.width() - 36.0) };
        let padding = if mismatch { 49.0 * scale } else { 26.0 };
        let content = width - padding * 2.0 - if scrollbar { 10.0 * scale } else { 0.0 };
        let mut blocks = Vec::new();
        let mut buttons = vec![(
            "routing-back",
            if matches!(prompt, Prompt::Cheats | Prompt::Mismatch(_)) { "CANCEL" } else { "BACK" },
        )];
        let heading;
        let continue_label;
        match prompt {
            Prompt::Party { target, requires_vanilla, signed_in } => {
                heading = format!("{}'S COLLEGE", target.leader.to_uppercase());
                continue_label = if routing.busy {
                    "PREPARING…"
                } else if *requires_vanilla {
                    "DISABLE & JOIN"
                } else if !target.content.mods.is_empty() {
                    if *signed_in { "SYNC MODS & JOIN" } else { "DOWNLOAD & JOIN ONCE" }
                } else {
                    "JOIN PARTY"
                };
                blocks.push(Block::Text(text(ui,if target.kind==GameSessionKind::GlobalHub{"This is a vanilla global-Hub party."}else{"The room decides its content and cheat policy. Your unrelated mods and settings are ignored for this session."},Typography::Body,content,scale)?));
                if !target.content.mods.is_empty() {
                    blocks.push(Block::Gap(18.0));
                    blocks.push(Block::Text(text(
                        ui,
                        &format!(
                            "{} {}",
                            target.content.mods.len(),
                            if target.content.mods.len() == 1 { "MOD" } else { "MODS" }
                        ),
                        Typography::Section,
                        content,
                        scale,
                    )?));
                    blocks.push(Block::Gap(6.0));
                    blocks.push(mod_list(
                        ui,
                        target
                            .content
                            .mods
                            .iter()
                            .map(|content| format!("{} · v{}", content.name, content.version)),
                        content,
                    )?);
                }
                for (show, warning) in [
                    (target.cheats_enabled, "CHEATS ENABLED FOR THIS COLLEGE"),
                    (
                        target.kind == GameSessionKind::PrivateCollege,
                        "LOCAL HALL ONLY · GLOBAL SCORES OFF",
                    ),
                    (*requires_vanilla, "ACTIVE MODS AND CHEATS WILL BE DISABLED"),
                ] {
                    if show {
                        blocks.push(Block::Text(text(
                            ui,
                            warning,
                            Typography::Warning,
                            content,
                            scale,
                        )?));
                    }
                }
            }
            Prompt::Modded { active_mods, cheats_enabled } => {
                heading = "LOCAL PLAY IS ACTIVE".to_owned();
                continue_label = if routing.busy { "PREPARING…" } else { "CONTINUE LOCAL" };
                blocks.push(Block::Text(text(ui,"Mods and cheats use a private College. These runs stay in your local Hall of Fame.",Typography::Body,content,scale)?));
                if !active_mods.is_empty() {
                    blocks.push(Block::Gap(18.0));
                    blocks.push(Block::Text(text(
                        ui,
                        "ACTIVE MODS",
                        Typography::Section,
                        content,
                        scale,
                    )?));
                    blocks.push(Block::Gap(6.0));
                    blocks.push(mod_list(
                        ui,
                        active_mods
                            .iter()
                            .map(|content| format!("{} · v{}", content.name, content.version)),
                        content,
                    )?);
                }
                if *cheats_enabled {
                    blocks.push(Block::Text(text(
                        ui,
                        "CHEATS ENABLED",
                        Typography::Warning,
                        content,
                        scale,
                    )?));
                }
                buttons.push((
                    "routing-vanilla",
                    if !active_mods.is_empty() && *cheats_enabled {
                        "DISABLE MODS & CHEATS"
                    } else if !active_mods.is_empty() {
                        "DISABLE ALL MODS"
                    } else {
                        "DISABLE CHEATS"
                    },
                ));
            }
            Prompt::Cheats => {
                heading = "CHEATS USE PRIVATE COLLEGES".to_owned();
                continue_label = "LEAVE & CONTINUE";
                blocks.push(Block::Text(text(
                    ui,
                    "Leave the global Hub, then use Last Game to continue this wizard locally.",
                    Typography::Body,
                    content,
                    scale,
                )?));
            }
            Prompt::Mismatch(mismatch) => {
                heading = "THE MOD LIST HAS CHANGED".to_owned();
                continue_label = "CONTINUE";
                blocks.push(Block::Text(text(ui,"This save was made with a different Dark Cloud. Continue with the currently enabled mods?",Typography::MismatchBody,content,scale)?));
                blocks.push(Block::Gap(24.0 * scale));
                let groups = [
                    (
                        "ADDED",
                        mismatch
                            .added
                            .iter()
                            .map(|item| format!("{} v{}", item.id, item.version))
                            .collect::<Vec<_>>(),
                    ),
                    (
                        "MISSING",
                        mismatch
                            .removed
                            .iter()
                            .map(|item| format!("{} v{}", item.id, item.version))
                            .collect(),
                    ),
                    (
                        "CHANGED",
                        mismatch
                            .changed
                            .iter()
                            .map(|item| {
                                format!(
                                    "{} v{} → v{}",
                                    item.saved.id, item.saved.version, item.active.version
                                )
                            })
                            .collect(),
                    ),
                ];
                let mut first = true;
                for (title, items) in groups {
                    if items.is_empty() {
                        continue;
                    }
                    if !first {
                        blocks.push(Block::Gap(12.0 * scale));
                    }
                    first = false;
                    let inner = content - 34.0 * scale;
                    blocks.push(Block::Group {
                        heading: text(ui, title, Typography::GroupHeading, inner, scale)?,
                        items: items
                            .iter()
                            .map(|item| {
                                text(ui, item, Typography::Mono, inner - 18.0 * scale, scale)
                            })
                            .collect::<Result<_, _>>()?,
                        marker: text(ui, "•", Typography::Mono, 18.0 * scale, scale)?,
                        scale,
                    });
                }
                blocks.push(Block::Gap(24.0 * scale));
                blocks.push(Block::Text(text(ui,"Missing or changed mod state will be discarded. If the save is inside a missing mod Boneyard, the wizard returns to the College.",Typography::MismatchWarning,content,scale)?));
            }
        }
        if let Some((done, total, active)) = &*routing.progress.borrow() {
            let percent = (*done as f64 / *total as f64 * 100.0 + 0.5).floor().min(100.0) as f32;
            let label = format!(
                "{} · {percent}%",
                active
                    .as_ref()
                    .map_or("Content ready".to_owned(), |id| format!("Downloading {id}"))
            );
            let label = Text {
                source: label.clone(),
                run: crate::gpu_ui::font_run_with_options(
                    ui.ctx(),
                    FontId::new(12.8 * scale, FontFamily::Name("IM FELL English".into())),
                    widgets::color(0xfff0bf),
                    &label,
                    FontRunOptions { wrap_width: Some(content), ..Default::default() },
                )?,
                centered: true,
            };
            blocks.push(Block::Progress { label, percent, scale });
        }
        if let Some(error) = &routing.error {
            blocks.push(Block::Gap(16.0 * scale));
            blocks.push(Block::Text(text(ui, error, Typography::Warning, content, scale)?));
        }
        let mut heading_block = vec![Block::Text(text(
            ui,
            &heading,
            if mismatch { Typography::MismatchHeading } else { Typography::Heading },
            content,
            scale,
        )?)];
        if !mismatch {
            heading_block.push(Block::Gap(12.0));
        }
        heading_block.append(&mut blocks);
        buttons.push(("routing-continue", continue_label));
        let actions = buttons
            .into_iter()
            .map(|(id, label)| {
                Ok(Action {
                    id,
                    label,
                    run: crate::gpu_ui::font_run(
                        ui.ctx(),
                        FontId::new(
                            if mismatch { 16.0 * scale } else { 11.0 },
                            FontFamily::Name("Cinzel Bold".into()),
                        ),
                        widgets::color(if mismatch {
                            if id == "routing-continue" { 0xd8efcf } else { 0xd7bd78 }
                        } else {
                            0xefd68f
                        }),
                        label,
                        None,
                    )?,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self {
            heading,
            blocks: heading_block,
            actions,
            mismatch,
            scale,
            width,
            content_width: content,
        })
    }
    fn footer(&self) -> (Vec<Rect>, f32) {
        let width = self.content_width;
        if self.mismatch {
            let button_width = (width - 18.0 * self.scale) / 2.0;
            (
                vec![
                    Rect::from_min_size(Pos2::ZERO, Vec2::new(button_width, 54.0 * self.scale)),
                    Rect::from_min_size(
                        egui::pos2(button_width + 18.0 * self.scale, 0.0),
                        Vec2::new(button_width, 54.0 * self.scale),
                    ),
                ],
                54.0 * self.scale,
            )
        } else {
            footer_layout(
                width,
                &self.actions.iter().map(|action| action.run.size().x + 34.0).collect::<Vec<_>>(),
                44.0,
                8.0,
            )
        }
    }
}
fn mod_list(ui: &Ui, items: impl Iterator<Item = String>, width: f32) -> Result<Block, String> {
    let items = items.collect::<Vec<_>>();
    let mut shaped = items
        .iter()
        .map(|item| text(ui, item, Typography::Body, width - 22.0, 1.0))
        .collect::<Result<Vec<_>, _>>()?;
    if shaped.iter().map(|item| item.run.size().y).sum::<f32>() > 230.0 {
        shaped = items
            .iter()
            .map(|item| text(ui, item, Typography::Body, width - 32.0, 1.0))
            .collect::<Result<_, _>>()?;
    }
    Ok(Block::List {
        items: shaped,
        marker: text(ui, "•", Typography::Body, 22.0, 1.0)?,
        indent: 22.0,
        maximum: 230.0,
    })
}

fn footer_layout(width: f32, widths: &[f32], height: f32, gap: f32) -> (Vec<Rect>, f32) {
    let mut rows = vec![Vec::<(usize, f32)>::new()];
    let mut used = 0.0;
    for (index, requested) in widths.iter().copied().enumerate() {
        let item = requested.min(width);
        if used > 0.0 && used + gap + item > width {
            rows.push(Vec::new());
            used = 0.0;
        }
        if used > 0.0 {
            used += gap;
        }
        rows.last_mut().expect("a footer row").push((index, item));
        used += item;
    }
    let total = rows.len() as f32 * height + (rows.len() - 1) as f32 * gap;
    let mut rectangles = vec![Rect::NOTHING; widths.len()];
    for (row_index, row) in rows.into_iter().enumerate() {
        let row_width =
            row.iter().map(|(_, width)| width).sum::<f32>() + (row.len() - 1) as f32 * gap;
        let mut x = (width - row_width) / 2.0;
        for (index, item) in row {
            rectangles[index] = Rect::from_min_size(
                egui::pos2(x, row_index as f32 * (height + gap)),
                Vec2::new(item, height),
            );
            x += item + gap;
        }
    }
    (rectangles, total)
}

pub(super) fn paint(
    routing: &RoutingView,
    frame: &mut UiFrame<'_>,
    ui: &mut Ui,
    view: Rect,
    actions: &mut Vec<String>,
) -> Result<(), String> {
    if routing.prompt.is_none() {
        return Ok(());
    }
    let mut presentation = Presentation::new(routing, ui, view, false)?;
    let scale = presentation.scale;
    let border = if presentation.mismatch { 3.0 * scale } else { 1.0 };
    let horizontal = if presentation.mismatch { 49.0 * scale } else { 26.0 };
    let vertical = if presentation.mismatch { 37.0 * scale } else { 26.0 };
    let maximum = if presentation.mismatch { 720.0 * scale } else { view.height() - 36.0 };
    let footer_margin = if presentation.mismatch { 25.0 * scale } else { 22.0 };
    let (mut buttons, mut footer_height) = presentation.footer();
    let mut natural = presentation.blocks.iter().map(Block::height).sum::<f32>()
        + footer_margin
        + footer_height
        + vertical * 2.0;
    if natural > maximum {
        presentation = Presentation::new(routing, ui, view, true)?;
        (buttons, footer_height) = presentation.footer();
        natural = presentation.blocks.iter().map(Block::height).sum::<f32>()
            + footer_margin
            + footer_height
            + vertical * 2.0;
    }
    let width = presentation.content_width;
    let panel =
        Rect::from_center_size(view.center(), Vec2::new(presentation.width, natural.min(maximum)));
    ui.painter().rect_filled(view, 0.0, Color32::from_black_alpha(209));
    background(frame, ui, panel, presentation.mismatch, scale)?;
    let viewport = panel.shrink(border);
    let mut child =
        ui.new_child(egui::UiBuilder::new().id_salt("routing-scroll-viewport").max_rect(viewport));
    child.set_clip_rect(viewport.intersect(ui.clip_rect()));
    child.spacing_mut().item_spacing = Vec2::ZERO;
    widgets::scrollbar_style(&mut child, 0x241e2e, 0x08070b, false);
    child.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
    child.spacing_mut().scroll.bar_width = 10.0 * scale;
    child.spacing_mut().scroll.bar_inner_margin = 0.0;
    child.spacing_mut().scroll.bar_outer_margin = 0.0;
    let mut visible = Vec::new();
    egui::ScrollArea::vertical()
        .id_salt("routing-scroll")
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            let origin = ui.cursor().min + Vec2::new(horizontal - border, vertical - border);
            let mut content = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("routing-content")
                    .max_rect(Rect::from_min_size(origin, Vec2::new(width, natural))),
            );
            content.set_clip_rect(ui.clip_rect());
            content.spacing_mut().item_spacing = Vec2::ZERO;
            for (index, block) in presentation.blocks.iter().enumerate() {
                block.paint(frame, &mut content, width, index, &mut visible)?;
            }
            content.add_space(footer_margin);
            let (_, footer) = content.allocate_space(Vec2::new(width, footer_height));
            for (action, rect) in presentation.actions.iter().zip(&buttons) {
                let rect = rect.translate(footer.min.to_vec2());
                if button(frame, &content, action, rect, &presentation, !routing.busy)?.clicked()
                    && !routing.busy
                {
                    actions.push(action.id.to_owned());
                }
            }
            ui.allocate_space(Vec2::new(viewport.width(), natural - border * 2.0));
            Ok::<(), String>(())
        })
        .inner?;
    frame.record_rect("routing/dialog", &presentation.heading, "dialog", panel, !routing.busy);
    frame.record_value("routing/dialog", &visible.join("\n"), None);
    Ok(())
}

fn background(
    frame: &UiFrame<'_>,
    ui: &Ui,
    panel: Rect,
    mismatch: bool,
    scale: f32,
) -> Result<(), String> {
    let painter = ui.painter();
    frame.paint_outer_shadow(
        painter,
        ImageShadow {
            offset: Vec2::new(0.0, if mismatch { 20.0 * scale } else { 18.0 }),
            blur: if mismatch { 35.0 * scale } else { 30.0 },
            color: Color32::BLACK,
        },
        |painter| {
            painter.rect_filled(panel, 0.0, Color32::WHITE);
            Ok(())
        },
    )?;
    if mismatch {
        painter.rect_filled(panel.expand(10.0 * scale), 0.0, widgets::color(0x050505));
        let path = "assets/stock/settings-leather.png";
        let size = frame.image_size(path).ok_or("routing leather is not ready")?;
        StockAtlas::from_image(path, size)?.paint_tiles(
            frame,
            painter,
            0,
            panel,
            scale,
            Color32::WHITE,
        )?;
    } else {
        painter.rect_filled(panel, 0.0, widgets::color(0x090806));
        let center = panel.center_top();
        let radius = Vec2::new(panel.width() / 2.0, panel.height()).length() * 0.72;
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(center, widgets::color(0x352a16));
        for index in 0..=64 {
            let angle = index as f32 / 64.0 * std::f32::consts::TAU;
            mesh.colored_vertex(
                center + Vec2::new(angle.cos(), angle.sin()) * radius,
                widgets::color(0x090806),
            );
            if index > 0 {
                mesh.add_triangle(0, index, index + 1);
            }
        }
        painter.with_clip_rect(panel.intersect(ui.clip_rect())).add(egui::Shape::mesh(mesh));
    }
    let border = if mismatch { 3.0 * scale } else { 1.0 };
    frame.paint_inner_shadow(
        painter,
        ImageShadow {
            offset: Vec2::ZERO,
            blur: if mismatch { 30.0 * scale } else { 17.5 },
            color: if mismatch { Color32::BLACK } else { Color32::from_black_alpha(153) },
        },
        |painter| {
            painter.rect_filled(panel.shrink(border), 0.0, Color32::WHITE);
            Ok(())
        },
    )?;
    painter.rect_stroke(
        panel,
        0.0,
        Stroke::new(border, widgets::color(if mismatch { 0xc8a75b } else { 0x9c8249 })),
        StrokeKind::Inside,
    );
    Ok(())
}
fn button(
    frame: &UiFrame<'_>,
    ui: &Ui,
    action: &Action,
    rect: Rect,
    presentation: &Presentation,
    enabled: bool,
) -> Result<egui::Response, String> {
    let response =
        widgets::interact_unstyled(frame, ui, action.id, action.label, "button", rect, enabled);
    frame.paint_group(
        ui.painter(),
        crate::gpu_ui::GroupEffect {
            opacity: if enabled { 1.0 } else { 0.55 },
            ..Default::default()
        },
        |frame, painter| {
            let border = if presentation.mismatch { 2.0 * presentation.scale } else { 1.0 };
            let color = if presentation.mismatch {
                if action.id == "routing-continue" { 0x74926e } else { 0x8e773e }
            } else {
                0x9c8249
            };
            if presentation.mismatch {
                painter.rect_filled(
                    rect,
                    0.0,
                    widgets::color(if action.id == "routing-continue" {
                        0x1c2c1c
                    } else {
                        0x090908
                    }),
                );
            } else {
                let mut mesh = egui::Mesh::default();
                mesh.colored_vertex(rect.left_top(), widgets::color(0x3a2f1a));
                mesh.colored_vertex(rect.right_top(), widgets::color(0x3a2f1a));
                mesh.colored_vertex(rect.right_bottom(), widgets::color(0x151007));
                mesh.colored_vertex(rect.left_bottom(), widgets::color(0x151007));
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                painter.add(egui::Shape::mesh(mesh));
            }
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(border, widgets::color(color)),
                StrokeKind::Inside,
            );
            action.run.paint(frame, painter, rect.center() - action.run.size() / 2.0, 1.0)
        },
    )?;
    widgets::focus_outline(
        frame,
        ui,
        &response,
        2.0 * presentation.scale,
        2.0 * presentation.scale,
        widgets::alpha(0x41e3ff, 0.7),
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_footer_keeps_intrinsic_buttons_in_one_centered_row() {
        let (rectangles, height) = footer_layout(628.0, &[70.0, 162.0, 170.0], 44.0, 8.0);
        assert_eq!(height, 44.0);
        assert_eq!(rectangles[0].left(), 105.0);
        assert_eq!(rectangles[1].left(), 183.0);
        assert_eq!(rectangles[2].left(), 353.0);
        assert_eq!(rectangles[2].right(), 523.0);
    }
    #[test]
    fn narrow_footer_wraps_whole_actions_and_centers_each_actual_row() {
        let (rectangles, height) = footer_layout(250.0, &[70.0, 162.0, 170.0], 44.0, 8.0);
        assert_eq!(height, 96.0);
        assert_eq!(rectangles[0].left(), 5.0);
        assert_eq!(rectangles[1].left(), 83.0);
        assert_eq!(rectangles[2].min, egui::pos2(40.0, 52.0));
    }
}
