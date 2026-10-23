//! Learned SkillScreen pages, stock leather/chain art and item/skill hover text.

use super::chrome::PanelCanvas;
use super::{PlayerPanelSnapshot, PlayerPanels, Target};
use solomon_sim::belt::native_belt_owned_item;
use solomon_sim::float;
use solomon_sim::hud::HudRect;
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextSpec};
pub(super) use solomon_sim::player_panels::item_text::hub_native_equipment_effect_text as equipment_effect_text;
use solomon_sim::player_panels::item_text::{
    HagathaTooltipOptions, HubTooltipOptions, economy_recipe_indices, hub_contextual_hover_rect,
    hub_hagatha_tooltip_lines, hub_item_tooltip_lines,
};
use solomon_sim::player_panels::native_skill_book_page_placements;
use solomon_sim::player_panels::skill_hover::{
    native_skill_book_tooltip_lines, native_skill_hover_box_layout,
};
use solomon_sim::player_panels::skill_text::{
    native_skill_page_display_name, native_skill_page_text_height, native_skill_page_tint,
    native_skill_page_wrapped_lines,
};
use solomon_sim::profile::{
    NATIVE_SKILL_CATALOG, native_skill_category, native_skill_root, native_weld_build,
};
impl PlayerPanels {
    pub(super) fn draw_skills(
        &self,
        c: &PanelCanvas<'_, '_>,
        s: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        c.tiles(&self.ui, 49, HudRect::new(0.0, 50.0, 1600.0, 760.0), 1.0, 0xffffff)?;
        let tick = ((self.now - self.opened_at) / 10.0).floor().max(0.0);
        for index in 0..8 {
            let angle = index as f64 * 45.0;
            let x = 800.0 + float::sin(angle * 2.0 * std::f64::consts::PI / 180.0) * 40.0;
            c.additive().faded(0.15).art(
                &self.ui,
                3,
                [x, 490.0],
                [1.9, 1.9],
                angle - tick / 60.0,
                0xffffff,
            )?;
        }
        for (record, x, y, sx, rotation) in [
            (33, 80.0, 30.0, 1.0, -90.0),
            (33, 1520.0, 30.0, 1.0, 90.0),
            (31, 200.0, 20.0, -1.0, 0.0),
            (31, 1400.0, 20.0, 1.0, 0.0),
            (30, 0.0, 860.0, 1.0, 0.0),
            (30, 1600.0, 860.0, 1.0, 0.0),
        ] {
            c.art(&self.ui, record, [x, y], [sx, 1.0], rotation, 0xffffff)?;
        }
        let warriors = c.at(0.0, 820.0).clip(HudRect::new(0.0, 0.0, 1600.0, 80.0));
        for (x, sx) in [(60.0, 1.0), (1540.0, -1.0)] {
            warriors.art(&self.ui, 32, [x, 60.0], [sx, 1.0], 0.0, 0xffffff)?;
        }
        for y in [44.0, 807.0] {
            c.tiles(&self.ui, 10, HudRect::new(0.0, y, 1600.0, 19.0), 1.0, 0xffffff)?;
        }
        c.frame(&self.ui, 4, HudRect::new(743.0, 34.0, 114.0, 34.0), 5.0, 0xffffff)?;
        c.shadow_text(&NativeUiTextSpec {
            tint: 0x808080,
            ..NativeUiTextSpec::new(NativeUiFontName::Menu, "SKILLS", 800.0, 60.0)
        });
        let coarse = self
            .document
            .default_view()
            .and_then(|window| window.match_media("(pointer: coarse)").ok().flatten())
            .is_some_and(|query| query.matches());
        for (line, y) in [
            (
                if coarse {
                    "tap a skill icon for more"
                } else {
                    "hover over a skill icon for more"
                },
                165.0,
            ),
            ("information about a skill.", 183.0),
            ("skills with a gold or green border", 701.0),
            ("can be dragged into your belt", 719.0),
        ] {
            c.shadow_text(&NativeUiTextSpec {
                tint: 0x808080,
                alpha: 0.75,
                ..NativeUiTextSpec::new(NativeUiFontName::Menu, line, 800.0, y)
            });
        }
        for (page_index, page) in native_skill_book_page_placements(s.book).into_iter().enumerate()
        {
            let selection =
                page.skills.iter().copied().find(|id| {
                    *id == s.book.primary_skill_id || s.concentrations.contains(&Some(*id))
                });
            let selected = selection.is_some();
            let tint = native_skill_page_tint(
                native_skill_root(page.root_skill_id).expect("native page root"),
            );
            let r = HudRect::new(page.x, page.y, page.width, 300.0);
            c.fill(
                HudRect::new(page.x + 12.0, page.y + 12.0, page.width - 24.0, 276.0),
                tint,
                if selected { 0.5 } else { 0.1 },
            );
            c.faded(if selected { 0.5 } else { 0.1 }).frame(&self.skills, 0, r, 30.0, tint)?;
            for _ in 0..if selected { 2 } else { 1 } {
                c.additive().faded(0.5).frame(&self.skills, 0, r, 30.0, 0xffffff)?;
            }
            for (index, id) in page.skills.iter().copied().enumerate() {
                let def = &NATIVE_SKILL_CATALOG[id];
                let weld = if id == 52 {
                    Some(
                        s.book
                            .weld_build_id
                            .and_then(native_weld_build)
                            .expect("native welding build"),
                    )
                } else {
                    None
                };
                let x =
                    page.x + if index == 0 { 100.0 } else { 280.0 + 160.0 * (index - 1) as f64 };
                let y = page.y + 80.0;
                if index > 0 {
                    c.art(&self.skills, 6, [x - 80.0, y], [1.0, 1.0], 0.0, 0xffffff)?;
                }
                c.art(&self.skills, 13, [x, y], [1.15, 1.15], 0.0, 0xffffff)?;
                if let Some(build) = weld {
                    c.split_art(
                        &self.skills,
                        164,
                        [x, y],
                        [1.15, 1.15],
                        [
                            native_skill_page_tint(build.color_roots[0]),
                            native_skill_page_tint(build.color_roots[1]),
                        ],
                    )?;
                } else {
                    c.art(
                        &self.skills,
                        164,
                        [x, y],
                        [1.15, 1.15],
                        0.0,
                        native_skill_page_tint(native_skill_root(id).expect("native skill root")),
                    )?;
                }
                let category = native_skill_category(id).expect("native category");
                let selected = selection == Some(id);
                c.art(
                    &self.skills,
                    if !selected && (1..=3).contains(&category) { 14 } else { 5 },
                    [x, y],
                    [1.0, 1.0],
                    0.0,
                    if selected { 0x97c797 } else { 0xffffff },
                )?;
                let icon = s.book.skill_icon_record(id);
                c.art(&self.skills, icon, [x + 4.0, y + 4.0], [1.0, 1.0], 0.0, 0)?;
                c.art(&self.skills, icon, [x, y], [1.0, 1.0], 0.0, 0xffffff)?;
                let target = HudRect::new(x - 43.5, y - 43.5, 87.0, 87.0);
                c.button(
                    &format!("skill:{id}"),
                    &format!("{}, rank {}", def.name, s.book.effective_ranks[id]),
                    target,
                );
                c.semantic(
                    &format!("panel:skill-page:{page_index}:{index}"),
                    def.name,
                    "image",
                    target,
                );
                if selected {
                    c.shadow_text(&NativeUiTextSpec {
                        tint: 0x97c797,
                        ..NativeUiTextSpec::new(
                            NativeUiFontName::Body,
                            if id == s.book.primary_skill_id { "casting" } else { "concentrate" },
                            x,
                            y - 45.0,
                        )
                    });
                }
                let name = weld.map_or(def.name, |build| build.synthetic_name);
                let title = native_skill_page_display_name(name, s.book.effective_ranks[id]);
                let names = native_skill_page_wrapped_lines(&title);
                let name_y = page.y + 150.0;
                c.shadow_text(&NativeUiTextSpec::new(
                    NativeUiFontName::Medium,
                    &names.join("\n"),
                    x,
                    name_y,
                ));
                c.shadow_text(&NativeUiTextSpec::new(
                    NativeUiFontName::SkillUppercase,
                    &def.family.to_uppercase(),
                    x,
                    name_y + native_skill_page_text_height(&names),
                ));
                let description = native_skill_page_wrapped_lines(
                    weld.map_or(def.quick_description, |build| build.pair_description),
                );
                c.text(&NativeUiTextSpec::new(
                    NativeUiFontName::Medium,
                    &description.join("\n"),
                    x,
                    page.y + 230.0 - native_skill_page_text_height(&description) / 2.0,
                ));
                let footer = match category {
                    1 => "primary cast",
                    2 => "secondary cast",
                    _ => "",
                };
                if !footer.is_empty() {
                    c.shadow_text(&NativeUiTextSpec::new(
                        NativeUiFontName::Body,
                        footer,
                        x,
                        page.y + 280.0,
                    ));
                }
            }
        }
        c.semantic(
            "panel:surface:skills",
            "Skills",
            "dialog",
            HudRect::new(0.0, 0.0, 1600.0, 900.0),
        );
        Ok(())
    }
    pub(super) fn paint_tooltip(
        &self,
        c: &PanelCanvas<'_, '_>,
        snapshot: &PlayerPanelSnapshot<'_>,
        now: f64,
    ) -> Result<(), String> {
        if self.inventory_flyby_active() || self.press.is_some_and(|press| press.dragging) {
            return Ok(());
        }
        let selected_skill = if self.compact {
            if let Some(Target::Skill(id)) = self.selected { Some(id) } else { None }
        } else {
            None
        };
        let skill = self.hovered_skill.or(selected_skill);
        if let Some(id) = skill {
            if let Some(source) = c.target_rect(Target::Skill(id)) {
                let source = solomon_sim::Vec2::new(
                    source.center().x as f64 - c.origin.x as f64,
                    source.center().y as f64 - c.origin.y as f64,
                );
                let lines = native_skill_book_tooltip_lines(
                    id,
                    snapshot.book.permanent_ranks[id],
                    snapshot.book.effective_ranks[id],
                );
                let layout = native_skill_hover_box_layout(&lines, source);
                let (origin, scale, painter) = self.tooltip_placement(c, layout.rect, source, 50.0);
                crate::ui::skill_hover::paint_skill_hover_box(
                    c.frame, &painter, &layout, id, origin, scale,
                );
                c.frame.record_rect(
                    "panel:tooltip",
                    &lines.iter().map(|line| line.text.as_str()).collect::<Vec<_>>().join("\n"),
                    "tooltip",
                    c.frame.ctx.layer_transform_to_global(painter.layer_id()).unwrap_or_default()
                        * egui::Rect::from_min_size(
                            origin,
                            egui::vec2(layout.rect.width as f32, layout.rect.height as f32) * scale,
                        ),
                    false,
                );
            }
            return Ok(());
        }
        if now - self.selected_at < 200.0 && self.hovered_service.is_none() {
            return Ok(());
        }
        let Some(target) = self.hovered_service.or(self.selected) else { return Ok(()) };
        let service_item = self.service_item(snapshot, target);
        let mut gap = 35.0;
        let lines = match target {
            Target::Hagatha(selector) => {
                let Some(offer) = snapshot
                    .economy
                    .hagatha_offers()
                    .into_iter()
                    .find(|offer| offer.selector == selector)
                else {
                    return Ok(());
                };
                let first_mixed = selector < 0
                    || solomon_sim::economy::HAGATHA_PERKS[selector as usize].base_price
                        == offer.price;
                hub_hagatha_tooltip_lines(&HagathaTooltipOptions {
                    bundle_selectors: &offer.members,
                    cheat_death_charges: None,
                    first_mixed,
                    price: Some(offer.price),
                    selector,
                })
            }
            Target::OwnedPerk(selector) => {
                gap = 25.0;
                hub_hagatha_tooltip_lines(&HagathaTooltipOptions {
                    bundle_selectors: &[],
                    // Source addOwnedPerkInspection uses the one-charge card here.
                    cheat_death_charges: (selector == 7).then_some(1),
                    first_mixed: true,
                    price: None,
                    selector: selector as i32,
                })
            }
            _ => {
                let item = match target {
                    Target::Item(id) => native_belt_owned_item(snapshot.economy, id),
                    Target::Equipment(slot) => snapshot.economy.equipment.at(slot).as_ref(),
                    _ => service_item.as_ref().map(|(item, _)| item),
                };
                let Some(item) = item else { return Ok(()) };
                let owned_recipes = economy_recipe_indices(snapshot.economy);
                hub_item_tooltip_lines(
                    item,
                    &HubTooltipOptions {
                        creativity_rank: snapshot.book.permanent_ranks[63],
                        owned_recipe_indices: &owned_recipes,
                        player_level: Some(snapshot.progression.level),
                        price: service_item.as_ref().and_then(|(_, price)| *price),
                    },
                )
            }
        };
        let Some(bounds) = c.target_rect(target) else { return Ok(()) };
        let source = solomon_sim::Vec2::new(
            bounds.center().x as f64 - c.origin.x as f64,
            bounds.center().y as f64 - c.origin.y as f64,
        );
        let layout = crate::ui::item_hover::item_hover_layout(c.frame, &lines, source, gap)?;
        let (origin, scale, painter) = self.tooltip_placement(
            c,
            hub_contextual_hover_rect(
                solomon_sim::Vec2::new(layout.rect.width, layout.rect.height),
                source,
                gap,
            ),
            source,
            gap,
        );
        crate::ui::item_hover::paint_item_hover_box(
            c.frame, &painter, &lines, &layout, origin, scale,
        )?;
        c.frame.record_rect(
            "panel:tooltip",
            &lines.iter().map(|line| line.text.as_str()).collect::<Vec<_>>().join("\n"),
            "tooltip",
            c.frame.ctx.layer_transform_to_global(painter.layer_id()).unwrap_or_default()
                * egui::Rect::from_min_size(
                    origin,
                    egui::vec2(layout.rect.width as f32, layout.rect.height as f32) * scale,
                ),
            false,
        );
        Ok(())
    }
    fn tooltip_placement(
        &self,
        c: &PanelCanvas<'_, '_>,
        r: HudRect,
        source: solomon_sim::Vec2,
        gap: f64,
    ) -> (egui::Pos2, f32, egui::Painter) {
        if self.compact {
            let scale = ((self.viewport_size.0 as f64 - 24.0) / r.width).min(1.0);
            let width = r.width * scale;
            let height = r.height * scale;
            let source = solomon_sim::Vec2::new(
                self.stage_origin.x + source.x * self.view_scale,
                self.stage_origin.y + source.y * self.view_scale,
            );
            let x = (source.x - width / 2.0)
                .clamp(12.0, (self.viewport_size.0 as f64 - width - 12.0).max(12.0));
            let mut y = source.y - gap * self.view_scale - height;
            if y < 52.0 {
                y = source.y + gap * self.view_scale;
            }
            y = y.clamp(52.0, (self.viewport_size.1 as f64 - height - 12.0).max(52.0));
            let painter = c.frame.ctx.layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::unique("panel-tooltip"),
            ));
            (egui::pos2(x as f32, y as f32), scale as f32, painter)
        } else {
            (egui::pos2(r.x as f32, r.y as f32) + c.origin, 1.0, c.painter.clone())
        }
    }
}
