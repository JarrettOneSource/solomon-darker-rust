//! Learned SkillScreen pages, stock leather/chain art and item/skill hover text.

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
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use super::chrome::{art, bitmap_text, button, place, tint};
use super::{PlayerPanelSnapshot, PlayerPanels, Target};
use crate::ui::{append, element};

impl PlayerPanels {
    pub(super) fn update_skill_ambient(&self, now: f64) -> Result<(), String> {
        let seals = self
            .content
            .query_selector_all("[data-skill-ambient]")
            .map_err(|_| "skill ambient sprites")?;
        let tick = ((now - self.opened_at) / 10.0).floor().max(0.0);
        for index in 0..seals.length() {
            let seal = seals
                .item(index)
                .ok_or("missing skill ambient sprite")?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "skill ambient element")?;
            let angle = f64::from(index) * 45.0;
            let x = 800.0 + float::sin(angle * 2.0 * std::f64::consts::PI / 180.0) * 40.0;
            seal.style().set_property("left", &format!("{x}px")).map_err(|_| "skill ambient x")?;
            seal.style()
                .set_property(
                    "transform",
                    &format!("translate(-50%,-50%) rotate({}deg) scale(1.9)", angle - tick / 60.0),
                )
                .map_err(|_| "skill ambient rotation")?;
        }
        Ok(())
    }

    pub(super) fn draw_skills(&self, snapshot: &PlayerPanelSnapshot<'_>) -> Result<(), String> {
        let leather = self.ui.tiles(&self.document, 49, 1600.0, 760.0)?;
        place(&leather, HudRect::new(0.0, 50.0, 1600.0, 760.0))?;
        append(&self.content, &leather)?;
        for index in 0..8 {
            let seal = art(
                &self.document,
                &self.content,
                &self.ui,
                3,
                [800.0, 490.0],
                [1.9, 1.9],
                index as f64 * 45.0,
            )?;
            seal.set_attribute("data-skill-ambient", &index.to_string())
                .map_err(|_| "skill seal index")?;
            seal.style().set_property("opacity", "0.15").map_err(|_| "skill seal alpha")?;
            seal.style()
                .set_property("mix-blend-mode", "plus-lighter")
                .map_err(|_| "skill seal blend")?;
        }
        for (record, x, y, sx, rotation) in [
            (33, 80.0, 30.0, 1.0, -90.0),
            (33, 1520.0, 30.0, 1.0, 90.0),
            (31, 200.0, 20.0, -1.0, 0.0),
            (31, 1400.0, 20.0, 1.0, 0.0),
            (30, 0.0, 860.0, 1.0, 0.0),
            (30, 1600.0, 860.0, 1.0, 0.0),
        ] {
            art(&self.document, &self.content, &self.ui, record, [x, y], [sx, 1.0], rotation)?;
        }
        let warriors = element(&self.document, "div", "panel-clipped-warriors")?;
        place(&warriors, HudRect::new(0.0, 820.0, 1600.0, 80.0))?;
        for (x, sx) in [(60.0, 1.0), (1540.0, -1.0)] {
            art(&self.document, &warriors, &self.ui, 32, [x, 60.0], [sx, 1.0], 0.0)?;
        }
        append(&self.content, &warriors)?;
        for y in [44.0, 807.0] {
            let chain = self.ui.tiles(&self.document, 10, 1600.0, 19.0)?;
            place(&chain, HudRect::new(0.0, y, 1600.0, 19.0))?;
            append(&self.content, &chain)?;
        }
        let header = self.ui.frame(&self.document, 4, 114.0, 34.0, 5.0)?;
        place(&header, HudRect::new(743.0, 34.0, 114.0, 34.0))?;
        append(&self.content, &header)?;
        shadowed_text(
            &self.document,
            &self.content,
            &NativeUiTextSpec {
                tint: 0x808080,
                ..NativeUiTextSpec::new(NativeUiFontName::Menu, "SKILLS", 800.0, 60.0)
            },
        )?;
        let coarse = self
            .document
            .default_view()
            .ok_or("Skills window")?
            .match_media("(pointer: coarse)")
            .map_err(|_| "Skills pointer capability")?
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
            shadowed_text(
                &self.document,
                &self.content,
                &NativeUiTextSpec {
                    tint: 0x808080,
                    alpha: 0.75,
                    ..NativeUiTextSpec::new(NativeUiFontName::Menu, line, 800.0, y)
                },
            )?;
        }
        for (page_index, page) in
            native_skill_book_page_placements(snapshot.book).into_iter().enumerate()
        {
            // The source highlights only the first selected row on a page. A later
            // primary/concentration on the same page does not add another label.
            let page_selection = page.skills.iter().copied().find(|id| {
                *id == snapshot.book.primary_skill_id
                    || snapshot.concentrations.contains(&Some(*id))
            });
            let page_selected = page_selection.is_some();
            let panel = self.skills.frame(&self.document, 0, page.width, 300.0, 30.0)?;
            place(&panel, HudRect::new(page.x, page.y, page.width, 300.0))?;
            let page_tint = native_skill_page_tint(
                native_skill_root(page.root_skill_id).expect("learned page has a native root"),
            );
            let fill = element(&self.document, "span", "panel-skill-page-fill")?;
            place(&fill, HudRect::new(page.x + 12.0, page.y + 12.0, page.width - 24.0, 276.0))?;
            fill.style()
                .set_property("background", &format!("#{page_tint:06x}"))
                .map_err(|_| "skill page fill")?;
            fill.style()
                .set_property("opacity", if page_selected { "0.5" } else { "0.1" })
                .map_err(|_| "skill page fill alpha")?;
            append(&self.content, &fill)?;
            tint(&self.document, &panel, page_tint)?;
            panel
                .style()
                .set_property("opacity", if page_selected { "0.5" } else { "0.1" })
                .map_err(|_| "page alpha")?;
            append(&self.content, &panel)?;
            for _ in 0..if page_selected { 2 } else { 1 } {
                let edge = self.skills.frame(&self.document, 0, page.width, 300.0, 30.0)?;
                place(&edge, HudRect::new(page.x, page.y, page.width, 300.0))?;
                edge.style().set_property("opacity", "0.5").map_err(|_| "skill page edge alpha")?;
                edge.style()
                    .set_property("mix-blend-mode", "plus-lighter")
                    .map_err(|_| "skill page edge blend")?;
                append(&self.content, &edge)?;
            }
            for (index, id) in page.skills.iter().copied().enumerate() {
                let definition = &NATIVE_SKILL_CATALOG[id];
                let weld = if id == 52 {
                    Some(
                        snapshot
                            .book
                            .weld_build_id
                            .and_then(native_weld_build)
                            .expect("learned Spell Welding has its native build"),
                    )
                } else {
                    None
                };
                let center_x =
                    page.x + if index == 0 { 100.0 } else { 280.0 + 160.0 * (index - 1) as f64 };
                let center_y = page.y + 80.0;
                if index > 0 {
                    art(
                        &self.document,
                        &self.content,
                        &self.skills,
                        6,
                        [center_x - 80.0, center_y],
                        [1.0, 1.0],
                        0.0,
                    )?;
                }
                art(
                    &self.document,
                    &self.content,
                    &self.skills,
                    13,
                    [center_x, center_y],
                    [1.15, 1.15],
                    0.0,
                )?;
                let glow_roots = if let Some(build) = weld {
                    vec![
                        (build.color_roots[0], Some("polygon(0 0,100% 0,0 100%)")),
                        (build.color_roots[1], Some("polygon(100% 0,0 100%,100% 100%)")),
                    ]
                } else {
                    vec![(native_skill_root(id).expect("learned skill root"), None)]
                };
                for (root, clip) in glow_roots {
                    let glow = art(
                        &self.document,
                        &self.content,
                        &self.skills,
                        164,
                        [center_x, center_y],
                        [1.15, 1.15],
                        0.0,
                    )?;
                    tint(&self.document, &glow, native_skill_page_tint(root))?;
                    if let Some(clip) = clip {
                        glow.style()
                            .set_property("clip-path", clip)
                            .map_err(|_| "Weld glow half")?;
                    }
                }
                let category = native_skill_category(id).expect("page skill category");
                let selected = page_selection == Some(id);
                let frame = art(
                    &self.document,
                    &self.content,
                    &self.skills,
                    if !selected && (1..=3).contains(&category) { 14 } else { 5 },
                    [center_x, center_y],
                    [1.0, 1.0],
                    0.0,
                )?;
                if selected {
                    tint(&self.document, &frame, 0x97c797)?;
                }
                let icon = snapshot.book.skill_icon_record(id);
                let shadow = art(
                    &self.document,
                    &self.content,
                    &self.skills,
                    icon,
                    [center_x + 4.0, center_y + 4.0],
                    [1.0, 1.0],
                    0.0,
                )?;
                tint(&self.document, &shadow, 0)?;
                art(
                    &self.document,
                    &self.content,
                    &self.skills,
                    icon,
                    [center_x, center_y],
                    [1.0, 1.0],
                    0.0,
                )?;
                let action = button(
                    &self.document,
                    &self.content,
                    &format!("skill:{id}"),
                    &format!("{}, rank {}", definition.name, snapshot.book.effective_ranks[id]),
                    HudRect::new(center_x - 43.5, center_y - 43.5, 87.0, 87.0),
                )?;
                action
                    .set_attribute("data-tutorial-skill-page", &page_index.to_string())
                    .map_err(|_| "Tutorial skill page")?;
                action
                    .set_attribute("data-tutorial-skill-index", &index.to_string())
                    .map_err(|_| "Tutorial skill position")?;
                action
                    .set_attribute("aria-pressed", if selected { "true" } else { "false" })
                    .map_err(|_| "skill selected")?;
                if index == 0 {
                    action
                        .set_attribute("data-game-default-focus", "true")
                        .map_err(|_| "skill focus")?;
                }
                if selected {
                    shadowed_text(
                        &self.document,
                        &self.content,
                        &NativeUiTextSpec {
                            tint: 0x97c797,
                            ..NativeUiTextSpec::new(
                                NativeUiFontName::Body,
                                if id == snapshot.book.primary_skill_id {
                                    "casting"
                                } else {
                                    "concentrate"
                                },
                                center_x,
                                center_y - 45.0,
                            )
                        },
                    )?;
                }
                let name = weld.map_or(definition.name, |build| build.synthetic_name);
                let rank = snapshot.book.effective_ranks[id];
                let title = native_skill_page_display_name(name, rank);
                let name_lines = native_skill_page_wrapped_lines(&title);
                let name_y = page.y + 150.0;
                shadowed_text(
                    &self.document,
                    &self.content,
                    &NativeUiTextSpec::new(
                        NativeUiFontName::Medium,
                        &name_lines.join("\n"),
                        center_x,
                        name_y,
                    ),
                )?;
                shadowed_text(
                    &self.document,
                    &self.content,
                    &NativeUiTextSpec::new(
                        NativeUiFontName::SkillUppercase,
                        &definition.family.to_uppercase(),
                        center_x,
                        name_y + native_skill_page_text_height(&name_lines),
                    ),
                )?;
                let description =
                    weld.map_or(definition.quick_description, |build| build.pair_description);
                let description_lines = native_skill_page_wrapped_lines(description);
                let description_y =
                    page.y + 230.0 - native_skill_page_text_height(&description_lines) / 2.0;
                bitmap_text(
                    &self.document,
                    &self.content,
                    &NativeUiTextSpec::new(
                        NativeUiFontName::Medium,
                        &description_lines.join("\n"),
                        center_x,
                        description_y,
                    ),
                )?;
                let footer = match category {
                    1 => "primary cast",
                    2 => "secondary cast",
                    _ => "",
                };
                if !footer.is_empty() {
                    shadowed_text(
                        &self.document,
                        &self.content,
                        &NativeUiTextSpec::new(
                            NativeUiFontName::Body,
                            footer,
                            center_x,
                            page.y + 280.0,
                        ),
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn update_tooltip(
        &self,
        snapshot: &PlayerPanelSnapshot<'_>,
        now: f64,
    ) -> Result<(), String> {
        if self.inventory_flyby_active() || self.press.is_some_and(|press| press.dragging) {
            self.tooltip.set_text_content(None);
            self.tooltip
                .remove_attribute("data-native-hover-key")
                .map_err(|_| "retire moving skill detail")?;
            self.tooltip
                .remove_attribute("data-native-item-hover-key")
                .map_err(|_| "retire moving item detail")?;
            self.tooltip
                .style()
                .set_property("display", "none")
                .map_err(|_| "hide moving item tooltip")?;
            return Ok(());
        }
        let focused_skill = self
            .document
            .active_element()
            .and_then(|element| element.get_attribute("data-panel-action"))
            .and_then(|action| {
                action.strip_prefix("skill:").and_then(|id| id.parse::<usize>().ok())
            });
        let selected_skill = if self.compact {
            if let Some(Target::Skill(id)) = self.selected { Some(id) } else { None }
        } else {
            None
        };
        let skill = self.hovered_skill.or(focused_skill).or(selected_skill);
        let inspection = format!("{skill:?}:{:?}:{:?}", self.selected, self.hovered_service);
        self.tooltip.style().set_property("display", "none").map_err(|_| "hide tooltip")?;
        if let Some(id) = skill {
            self.tooltip
                .remove_attribute("data-native-item-hover")
                .map_err(|_| "retire item detail owner")?;
            self.tooltip
                .remove_attribute("data-native-item-hover-key")
                .map_err(|_| "retire item detail cache")?;
            return self.update_skill_hover(id, snapshot, &inspection);
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
        let action = match target {
            Target::Item(id) => format!("item:{id}"),
            Target::Equipment(slot) => format!("equipment:{}", slot.as_str()),
            Target::Shop(id) => format!("shop:{id}"),
            Target::Storage(id) => format!("storage:{id}"),
            Target::Hagatha(selector) => format!("hagatha:{selector}"),
            Target::OwnedPerk(selector) => format!("owned-perk:{selector}"),
            _ => return Ok(()),
        };
        let Some(icon) = self
            .content
            .query_selector(&format!("[data-panel-action='{action}']"))
            .map_err(|_| "native item hover source")?
        else {
            return Ok(());
        };
        let bounds = icon.get_bounding_client_rect();
        let stage = self.stage.get_bounding_client_rect();
        let source = solomon_sim::Vec2::new(
            (bounds.left() + bounds.width() / 2.0 - stage.left()) * 1600.0 / stage.width(),
            (bounds.top() + bounds.height() / 2.0 - stage.top()) * 900.0 / stage.height(),
        );
        let key = format!(
            "{target:?}:{}:{}:{}",
            snapshot.economy.revision,
            snapshot.progression.level,
            snapshot.book.permanent_ranks[63]
        );
        let size =
            if self.tooltip.get_attribute("data-native-item-hover-key").as_deref() == Some(&key) {
                solomon_sim::Vec2::new(
                    self.tooltip
                        .get_attribute("data-native-item-hover-width")
                        .expect("cached hover width")
                        .parse()
                        .expect("native hover width"),
                    self.tooltip
                        .get_attribute("data-native-item-hover-height")
                        .expect("cached hover height")
                        .parse()
                        .expect("native hover height"),
                )
            } else {
                self.tooltip
                    .remove_attribute("data-native-hover")
                    .map_err(|_| "retire skill detail owner")?;
                self.tooltip
                    .remove_attribute("data-native-hover-key")
                    .map_err(|_| "retire skill detail cache")?;
                let layout = crate::ui::item_hover::paint_item_hover_box(
                    &self.document,
                    &self.tooltip,
                    &lines,
                    source,
                    gap,
                )?;
                for (name, value) in [
                    ("data-native-item-hover-key", key),
                    ("data-native-item-hover-width", layout.rect.width.to_string()),
                    ("data-native-item-hover-height", layout.rect.height.to_string()),
                ] {
                    self.tooltip.set_attribute(name, &value).map_err(|_| "item hover cache")?;
                }
                solomon_sim::Vec2::new(layout.rect.width, layout.rect.height)
            };
        self.place_native_tooltip(
            hub_contextual_hover_rect(size, source, gap),
            solomon_sim::Vec2::new(
                bounds.left() + bounds.width() / 2.0,
                bounds.top() + bounds.height() / 2.0,
            ),
            gap,
            &inspection,
        )
    }

    fn update_skill_hover(
        &self,
        id: usize,
        snapshot: &PlayerPanelSnapshot<'_>,
        inspection: &str,
    ) -> Result<(), String> {
        let Some(icon) = self
            .content
            .query_selector(&format!("[data-panel-action='skill:{id}']"))
            .map_err(|_| "native skill hover source")?
        else {
            return Ok(());
        };
        let bounds = icon.get_bounding_client_rect();
        let stage = self.stage.get_bounding_client_rect();
        let source = solomon_sim::Vec2::new(
            (bounds.left() + bounds.width() / 2.0 - stage.left()) * 1600.0 / stage.width(),
            (bounds.top() + bounds.height() / 2.0 - stage.top()) * 900.0 / stage.height(),
        );
        let lines = native_skill_book_tooltip_lines(
            id,
            snapshot.book.permanent_ranks[id],
            snapshot.book.effective_ranks[id],
        );
        let layout = native_skill_hover_box_layout(&lines, source);
        let key = format!(
            "{id}:{}:{}",
            snapshot.book.permanent_ranks[id], snapshot.book.effective_ranks[id]
        );
        if self.tooltip.get_attribute("data-native-hover-key").as_deref() != Some(&key) {
            crate::ui::skill_hover::paint_skill_hover_box(
                &self.document,
                &self.tooltip,
                &layout,
                id,
            )?;
            self.tooltip
                .set_attribute("data-native-hover-key", &key)
                .map_err(|_| "native skill hover cache")?;
        }
        self.place_native_tooltip(
            layout.rect,
            solomon_sim::Vec2::new(
                bounds.left() + bounds.width() / 2.0,
                bounds.top() + bounds.height() / 2.0,
            ),
            50.0,
            inspection,
        )
    }

    fn place_native_tooltip(
        &self,
        rect: HudRect,
        source: solomon_sim::Vec2,
        gap: f64,
        inspection: &str,
    ) -> Result<(), String> {
        let (x, y, scale) = if self.compact {
            let scale = ((f64::from(self.viewport_size.0) - 24.0) / rect.width).min(1.0);
            let width = rect.width * scale;
            let height = rect.height * scale;
            let x = (source.x - width / 2.0)
                .clamp(12.0, (f64::from(self.viewport_size.0) - width - 12.0).max(12.0));
            let mut y = source.y - gap * self.view_scale - height;
            if y < 52.0 {
                y = source.y + gap * self.view_scale;
            }
            (x, y.clamp(52.0, (f64::from(self.viewport_size.1) - height - 12.0).max(52.0)), scale)
        } else {
            (rect.x, rect.y, 1.0)
        };
        crate::ui::style(
            &self.tooltip,
            &format!(
                "display:block;left:{x}px;top:{y}px;width:{}px;height:{}px;max-width:none;max-height:none;overflow:visible;padding:0;border:0;background:none;--native-hover-scale:{scale}",
                rect.width * scale,
                rect.height * scale,
            ),
        )?;
        self.tooltip
            .set_attribute("data-inspection", inspection)
            .map_err(|_| "native inspection")?;
        Ok(())
    }
}

fn shadowed_text(
    document: &web_sys::Document,
    parent: &HtmlElement,
    spec: &NativeUiTextSpec<'_>,
) -> Result<(), String> {
    bitmap_text(
        document,
        parent,
        &NativeUiTextSpec { x: spec.x + 1.0, y: spec.y + 1.0, tint: 0x000000, ..*spec },
    )?;
    bitmap_text(document, parent, spec)?;
    Ok(())
}
