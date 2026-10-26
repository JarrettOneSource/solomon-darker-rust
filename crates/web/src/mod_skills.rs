//! GPU learned authored skills and mod choice offer from `mod-ui/ModSkillBook.tsx`.
//! Exact host offer/binding identities remain in typed actions.

use crate::gpu_ui::UiFrame;
use crate::mod_presentation_assets::ModPresentationAssets;
use egui::{Color32, FontFamily, FontId, Id, RichText, Stroke, vec2};
use solomon_sim::modding::LuaObject;
use solomon_sim::modding::assets::GameModAsset;
use solomon_sim::modding::ui::{ModSkillIcon, project_mod_skill_book, project_mod_skill_picker};

pub struct ModSkillsSnapshot<'a> {
    pub runtime: Option<&'a LuaObject>,
    pub player_id: &'a str,
    /// Source mounts the learned panel inside the actual native Skills surface.
    pub skills_open: bool,
    /// Source mounts the mod offer panel only beside an actual native offer.
    pub native_offer_sequence: Option<u64>,
    pub assets: &'a [GameModAsset],
    pub images: Option<&'a ModPresentationAssets>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ModSkillAction {
    QuickbarBind { content_id: String, slot: f64, clear: bool },
    SkillChoose { content_id: String, mod_offer_sequence: f64, native_offer_sequence: u64 },
}

#[derive(Clone, PartialEq)]
struct SkillRow {
    key: String,
    tag: &'static str,
    name: String,
    detail: Option<String>,
    description: Option<String>,
    icon_url: Option<String>,
    subskill: bool,
    disabled: bool,
    action: Option<ModSkillAction>,
}

impl SkillRow {
    fn text(key: &str, tag: &'static str, name: &str) -> Self {
        Self {
            key: key.to_owned(),
            tag,
            name: name.to_owned(),
            detail: None,
            description: None,
            icon_url: None,
            subskill: false,
            disabled: false,
            action: None,
        }
    }
}

pub struct ModSkillsUi {
    book: Vec<SkillRow>,
    choices: Vec<SkillRow>,
    actions: Vec<ModSkillAction>,
}
impl ModSkillsUi {
    pub fn new() -> Result<Self, String> {
        Ok(Self { book: Vec::new(), choices: Vec::new(), actions: Vec::new() })
    }
    pub fn update(&mut self, snapshot: &ModSkillsSnapshot<'_>) -> Result<(), String> {
        let mut book_rows = Vec::new();
        if snapshot.skills_open
            && let Some(book) = project_mod_skill_book(snapshot.runtime)
        {
            book_rows.push(SkillRow::text("heading", "h2", "Mod skills"));
            if book.learned.is_empty() {
                book_rows.push(SkillRow::text("empty", "p", "No mod skills learned yet."));
            }
            for skill in book.learned {
                let mut row =
                    SkillRow::text(&format!("learned:{}", skill.content_id), "article", skill.name);
                row.detail =
                    Some(format!("Rank {} / {}", number(skill.rank), number(skill.maximum_rank)));
                row.description = Some(skill.description.to_owned());
                row.icon_url = icon_url(snapshot, skill.icon)?;
                row.subskill = skill.subskill;
                book_rows.push(row);
            }
            if !book.spells.is_empty() {
                book_rows.push(SkillRow::text("spells", "h3", "Unlocked spells"));
            }
            for spell in book.spells {
                let mut row =
                    SkillRow::text(&format!("binding:{}", spell.content_id), "button", spell.name);
                let slot = spell.slot.map_or_else(|| "NaN".to_owned(), |slot| number(slot + 1.0));
                row.description = Some(if spell.bound {
                    format!("Remove from slot {slot}")
                } else {
                    format!("Add to slot {slot}")
                });
                row.icon_url = icon_url(snapshot, spell.icon)?;
                row.disabled = spell.slot.is_none();
                row.action = spell.slot.map(|slot| ModSkillAction::QuickbarBind {
                    content_id: spell.content_id.to_owned(),
                    slot,
                    clear: spell.bound,
                });
                book_rows.push(row);
            }
        }
        let mut choice_rows = Vec::new();
        if let Some(native_offer_sequence) = snapshot.native_offer_sequence
            && let Some(offer) = project_mod_skill_picker(snapshot.runtime, snapshot.player_id)
        {
            for skill in offer.skills {
                let mut row =
                    SkillRow::text(&format!("offer:{}", skill.content_id), "button", skill.name);
                row.description = Some(if skill.description.is_empty() {
                    format!("Rank {}", number(skill.next_rank))
                } else {
                    skill.description.to_owned()
                });
                row.icon_url = icon_url(snapshot, skill.icon)?;
                row.action = Some(ModSkillAction::SkillChoose {
                    content_id: skill.content_id.to_owned(),
                    mod_offer_sequence: offer.sequence,
                    native_offer_sequence,
                });
                choice_rows.push(row);
            }
        }
        self.book = book_rows;
        self.choices = choice_rows;
        Ok(())
    }

    pub fn hide(&mut self) {
        self.book.clear();
        self.choices.clear();
    }
    pub fn reset(&mut self) {
        self.hide();
        self.actions.clear();
    }
    pub fn take_actions(&mut self) -> Vec<ModSkillAction> {
        std::mem::take(&mut self.actions)
    }
    pub fn book_layer() -> egui::LayerId {
        egui::LayerId::new(egui::Order::Foreground, Id::unique("mod/skill-book"))
    }
    pub fn picker_layer() -> egui::LayerId {
        egui::LayerId::new(egui::Order::Foreground, Id::unique("mod/skill-picker"))
    }
    pub fn paint(&mut self, frame: &mut UiFrame<'_>) -> Result<(), String> {
        let ctx = frame.ctx.clone();
        let small = ctx.content_rect().width() <= 700.0;
        let mut paint_error = None;
        if !self.book.is_empty() {
            frame.input_scope("mod/skill-book", Self::book_layer());
            let anchor = if small { egui::Align2::RIGHT_BOTTOM } else { egui::Align2::RIGHT_TOP };
            let offset = if small {
                vec2(-16.0, -128.0)
            } else {
                vec2(-16.0, ctx.content_rect().height() * 0.12)
            };
            let layer = Self::book_layer();
            let area = egui::Area::new(layer.id)
                .fade_in(false)
                .order(layer.order)
                .anchor(anchor, offset)
                .show(&ctx, |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(9, 10, 14, 240))
                        .stroke(Stroke::new(
                            1.0,
                            Color32::from_rgba_unmultiplied(202, 181, 126, 199),
                        ))
                        .inner_margin(12.0)
                        .show(ui, |ui| {
                            ui.set_width(if small {
                                288.0_f32.min(ctx.content_rect().width() - 32.0) - 26.0
                            } else {
                                288.0_f32.min(ctx.content_rect().width() * 0.42) - 26.0
                            });
                            egui::ScrollArea::vertical()
                                .id_salt("mod/skill-book-scroll")
                                .max_height(
                                    ctx.content_rect().height() * if small { 0.42 } else { 0.72 },
                                )
                                .show(ui, |ui| {
                                    for row in &self.book {
                                        match paint_row(frame, ui, row, false) {
                                            Ok(Some(action)) => self.actions.push(action),
                                            Ok(None) => {}
                                            Err(error) => paint_error = Some(error),
                                        }
                                    }
                                });
                        });
                });
            frame.record_rect(
                "mod/skill-book",
                "Mod skills",
                "complementary",
                area.response.rect,
                true,
            );
        }
        if !self.choices.is_empty() {
            frame.input_scope("mod/skill-picker", Self::picker_layer());
            let layer = Self::picker_layer();
            let area = egui::Area::new(layer.id)
                .fade_in(false)
                .order(layer.order)
                .anchor(egui::Align2::RIGHT_BOTTOM, vec2(-16.0, if small { -72.0 } else { -16.0 }))
                .show(&ctx, |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(8, 9, 13, 235))
                        .stroke(Stroke::new(
                            1.0,
                            Color32::from_rgba_unmultiplied(202, 181, 126, 217),
                        ))
                        .inner_margin(11.2)
                        .show(ui, |ui| {
                            ui.set_width(400.0_f32.min(ctx.content_rect().width() - 32.0) - 24.4);
                            ui.label(copy("Mod skills", 12.8));
                            egui::ScrollArea::vertical()
                                .id_salt("mod/skill-choices-scroll")
                                .max_height(ctx.content_rect().height() * 0.4)
                                .show(ui, |ui| {
                                    let columns =
                                        ((ui.available_width() + 6.4) / 126.4).floor().max(1.0)
                                            as usize;
                                    ui.spacing_mut().item_spacing = vec2(6.4, 6.4);
                                    for rows in self.choices.chunks(columns) {
                                        ui.columns(columns, |cells| {
                                            for (row, cell) in rows.iter().zip(cells) {
                                                match paint_row(frame, cell, row, true) {
                                                    Ok(Some(action)) => self.actions.push(action),
                                                    Ok(None) => {}
                                                    Err(error) => paint_error = Some(error),
                                                }
                                            }
                                        });
                                    }
                                });
                        });
                });
            frame.record_rect(
                "mod/skill-picker",
                "Mod skill choices",
                "complementary",
                area.response.rect,
                true,
            );
        }
        paint_error.map_or(Ok(()), Err)
    }
}
fn copy(text: &str, size: f32) -> RichText {
    RichText::new(text)
        .font(FontId::new(size, FontFamily::Name("Cinzel".into())))
        .color(Color32::from_rgb(232, 220, 185))
}
fn paint_row(
    frame: &mut UiFrame<'_>,
    ui: &mut egui::Ui,
    row: &SkillRow,
    choice: bool,
) -> Result<Option<ModSkillAction>, String> {
    if row.tag != "article" && row.tag != "button" {
        let heading = row.tag == "h3";
        if heading {
            ui.add_space(6.4);
        }
        let text = if heading { row.name.to_uppercase() } else { row.name.clone() };
        let mut style = crate::gpu_ui::NativeLabelStyle::new(
            FontId::new(
                if heading { 10.88 } else { 11.2 },
                FontFamily::Name("Cinzel Semibold".into()),
            ),
            if heading {
                Color32::from_rgb(201, 182, 221)
            } else {
                Color32::from_rgb(232, 220, 185)
            },
        );
        style.wrap_width = Some(ui.available_width());
        style.line_height = Some(if heading { 13.6 } else { 14.0 });
        crate::gpu_ui::shaped_label(
            frame,
            ui,
            &format!("mod/skill-heading:{}", row.key),
            &text,
            style,
        )?;
        return Ok(None);
    }
    ui.horizontal(|ui| -> Result<Option<ModSkillAction>, String> {
        if row.subskill {
            ui.add_space(16.0);
        }
        let width = ui.available_width();
        let has_icon = row.icon_url.is_some();
        let gap = if choice { 2.4 } else { 1.6 };
        let family = if row.tag == "article" { "Cinzel ExtraBold" } else { "Cinzel Semibold" };
        let mut text = vec![(
            row.name.as_str(),
            FontId::new(if choice { 11.52 } else { 11.2 }, FontFamily::Name(family.into())),
            if choice {
                Color32::from_rgb(234, 220, 182)
            } else {
                Color32::from_rgb(232, 220, 185)
            },
            (!choice).then_some(14.0),
        )];
        if let Some(detail) = &row.detail {
            text.push((
                detail,
                FontId::new(11.2, FontFamily::Name("Cinzel Semibold".into())),
                Color32::from_rgb(232, 220, 185),
                Some(14.0),
            ));
        }
        if let Some(description) = &row.description {
            text.push((
                description,
                FontId::new(9.92, FontFamily::Name("Alegreya Medium".into())),
                if choice {
                    Color32::from_rgb(191, 179, 208)
                } else {
                    Color32::from_rgb(184, 174, 195)
                },
                Some(11.904),
            ));
        }
        let runs = text
            .into_iter()
            .enumerate()
            .map(|(index, (text, font, color, line_height))| {
                let column = if has_icon { 1 } else { index % 2 };
                let wrap_width = if column == 0 { 32.0 } else { (width - 54.0).max(1.0) };
                crate::gpu_ui::font_run_with_options(
                    frame.ctx,
                    font,
                    color,
                    text,
                    crate::gpu_ui::FontRunOptions {
                        wrap_width: Some(wrap_width),
                        line_height,
                        ..Default::default()
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let rows = skill_grid_rows(
            &runs.iter().map(|run| run.size().y).collect::<Vec<_>>(),
            has_icon,
            choice,
        );
        let content_height = rows.iter().sum::<f32>() + gap * (rows.len().saturating_sub(1)) as f32;
        let (bounds, response) = ui
            .add_enabled_ui(!row.disabled, |ui| {
                let (bounds, _) = ui
                    .allocate_exact_size(vec2(width, content_height + 14.8), egui::Sense::hover());
                let response = ui.interact(
                    bounds,
                    Id::unique(("mod/skill", &row.key)),
                    if row.action.is_some() { egui::Sense::click() } else { egui::Sense::hover() },
                );
                (bounds, response)
            })
            .inner;
        let hover =
            response.hovered() && row.action.is_some() || choice && frame.focus_visible(&response);
        ui.painter().rect_filled(
            bounds,
            0.0,
            if choice {
                Color32::from_rgba_unmultiplied(27, 20, 38, 240)
            } else {
                Color32::from_rgba_unmultiplied(27, 22, 34, 230)
            },
        );
        ui.painter().rect_stroke(
            bounds,
            0.0,
            Stroke::new(
                1.0,
                if hover {
                    Color32::from_rgb(229, 204, 255)
                } else if choice {
                    Color32::from_rgba_unmultiplied(178, 147, 218, 179)
                } else {
                    Color32::from_rgba_unmultiplied(139, 119, 160, 158)
                },
            ),
            egui::StrokeKind::Inside,
        );
        let origin = bounds.min + vec2(7.4, 7.4);
        if let Some(url) = &row.icon_url {
            let icon_box = egui::Rect::from_min_size(
                origin + vec2(0.0, (content_height - 32.0) / 2.0),
                vec2(32.0, 32.0),
            );
            let size = frame
                .image_size(url)
                .map(|size| size * (32.0 / size.x).min(32.0 / size.y))
                .unwrap_or(vec2(32.0, 32.0));
            let icon = egui::Rect::from_center_size(icon_box.center(), size);
            frame.image(ui.painter(), url, icon, None, Color32::WHITE);
            let id = format!("mod/skill-icon:{}", row.key);
            frame.record_rect(&id, &format!("{} icon", row.name), "img", icon, true);
            if let Some(size) = frame.image_size(url) {
                frame.record_value(&id, &format!("{}x{} {url}", size.x, size.y), None);
            }
        }
        for (index, run) in runs.iter().enumerate() {
            let column = if has_icon { 1 } else { index % 2 };
            let row_index = if has_icon { index } else { index / 2 };
            let top = rows[..row_index].iter().sum::<f32>() + gap * row_index as f32;
            run.paint(
                frame,
                ui.painter(),
                origin
                    + vec2(
                        if column == 0 { 0.0 } else { 39.2 },
                        top + (rows[row_index] - run.size().y) / 2.0,
                    ),
                1.0,
            )?;
        }
        let id = format!("mod/skill:{}", row.key);
        frame.record(
            &id,
            &row.name,
            if row.action.is_some() { "button" } else { "article" },
            &response,
        );
        frame.record_value(
            &id,
            &[Some(row.name.as_str()), row.detail.as_deref(), row.description.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("\n"),
            None,
        );
        Ok(if response.clicked() && !row.disabled { row.action.clone() } else { None })
    })
    .inner
}

fn skill_grid_rows(heights: &[f32], has_icon: bool, choice: bool) -> Vec<f32> {
    let count = if has_icon { if choice { 2 } else { 3 } } else { heights.len().div_ceil(2) };
    let mut rows = vec![0.0_f32; count];
    for (index, height) in heights.iter().enumerate() {
        let row = if has_icon { index } else { index / 2 };
        rows[row] = rows[row].max(*height);
    }
    let gap = if choice { 2.4 } else { 1.6 };
    let current = rows.iter().sum::<f32>() + gap * count.saturating_sub(1) as f32;
    let minimum = if choice {
        36.4
    } else if has_icon {
        32.0
    } else {
        0.0
    };
    let extra = (minimum - current).max(0.0) / count as f32;
    for row in &mut rows {
        *row += extra;
    }
    rows
}

fn icon_url(
    snapshot: &ModSkillsSnapshot<'_>,
    icon: ModSkillIcon<'_>,
) -> Result<Option<String>, String> {
    // Source first finds an exact-case image descriptor, then uses the whole
    // verified asset. These skill icons do not apply custom sprite-sheet crops.
    let asset = snapshot.assets.iter().find(|asset| {
        asset.mod_id == icon.mod_id && asset.path == icon.path && asset.kind == "image"
    });
    let Some(asset) = asset else { return Ok(None) };
    let images = snapshot.images.ok_or("Mod skill images are not installed")?;
    Ok(Some(images.image(&asset.mod_id, &asset.path)?.url().to_owned()))
}

fn number(value: f64) -> String {
    crate::ui::javascript_number(value)
}
