//! Native InventoryScreen: outward stat/equipment panes, column-first backpack and
//! the stock equipment sinks. Geometry comes from `renderer/hub-inventory-render-contract.ts`.

use solomon_sim::economy::{EquipmentSlot, EquipmentType};
use solomon_sim::equipment::native_equipment_completed_sets;
use solomon_sim::hud::HudRect;
use solomon_sim::native_ui_text::{
    NativeUiFontName as Font, NativeUiTextAlign as Align, NativeUiTextRun,
    NativeUiTextSpec as TextSpec, layout_native_ui_text, layout_native_ui_text_runs,
    measure_native_ui_text, native_ui_font,
};
use solomon_sim::player_panels::equipment_slot_rects;
use solomon_sim::player_panels::sack_rename::inventory_sack_name_needs_browser_font;
use solomon_sim::profile::native_wizard_class_title;
use solomon_sim::profile::primary::{
    NativePrimarySpellDamageUnit, NativePrimarySpellManaUnit, native_primary_spell_descriptor,
    native_primary_spell_summary_with_stat_book,
};
use web_sys::HtmlElement;

use super::chrome::{art, bitmap_text, button, place};
use super::{PlayerPanel, PlayerPanelSnapshot, PlayerPanels, Target};
use crate::ui::text::paint_text_layout;
use crate::ui::{append, element, style};

impl PlayerPanels {
    pub(super) fn draw_inventory(
        &mut self,
        snapshot: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        self.draw_side_panel(true)?;
        self.draw_side_panel(false)?;
        if self.panel == Some(PlayerPanel::Service(solomon_sim::economy::HubTraderId::Hagatha)) {
            self.draw_hagatha_perks(snapshot)?;
        } else {
            self.draw_stats(snapshot)?;
        }
        self.draw_equipment(snapshot)?;
        if self.panel == Some(PlayerPanel::Inventory) {
            let seal = art(
                &self.document,
                &self.content,
                &self.ui,
                62,
                [800.0, 249.0],
                [1.25, 1.25],
                0.0,
            )?;
            seal.style().set_property("opacity", "0.32").map_err(|_| "preview seal alpha")?;
            bitmap_text(
                &self.document,
                &self.content,
                &TextSpec {
                    align: Align::Center,
                    tint: 0xe7cc71,
                    ..TextSpec::new(
                        Font::Medium,
                        &format!("KILLS: {}", snapshot.kills),
                        800.0,
                        337.0,
                    )
                },
            )?;
            bitmap_text(
                &self.document,
                &self.content,
                &TextSpec {
                    align: Align::Center,
                    tint: 0xe7cc71,
                    ..TextSpec::new(
                        Font::Medium,
                        &format!("AWESOMENESS: {}", snapshot.awesomeness),
                        800.0,
                        359.0,
                    )
                },
            )?;
        }
        let backpack_leather = self.ui.tiles(&self.document, 49, 1600.0, 310.0)?;
        place(&backpack_leather, HudRect::new(0.0, 490.0, 1600.0, 310.0))?;
        append(&self.content, &backpack_leather)?;
        for y in [470.0, 800.0] {
            let chain = self.ui.tiles(&self.document, 10, 1280.0, 19.0)?;
            chain
                .style()
                .set_property("transform", "scale(1.25)")
                .map_err(|_| "backpack chain scale")?;
            chain
                .style()
                .set_property("transform-origin", "top left")
                .map_err(|_| "backpack chain origin")?;
            place(&chain, HudRect::new(0.0, y, 1280.0, 19.0))?;
            append(&self.content, &chain)?;
        }
        for (x, y, sx, sy) in [
            (-63.5, 513.5, 1.0, 1.0),
            (1663.5, 513.5, -1.0, 1.0),
            (-63.5, 775.5, 1.0, -1.0),
            (1663.5, 775.5, -1.0, -1.0),
        ] {
            art(&self.document, &self.content, &self.inventory, 8, [x, y], [sx, sy], 0.0)?;
        }
        for (x, y) in [(21.0, 481.0), (1631.0, 481.0), (21.0, 809.0), (1631.0, 809.0)] {
            art(&self.document, &self.content, &self.ui, 71, [x, y], [1.0, 1.0], 0.0)?;
        }
        self.draw_sack_pages(snapshot)?;
        Ok(())
    }

    pub(super) fn section_header(
        &self,
        parent: &HtmlElement,
        label: &str,
        center_x: f64,
        top: f64,
        baseline: f64,
        sack_name: bool,
    ) -> Result<(), String> {
        let fallback = if sack_name && inventory_sack_name_needs_browser_font(label) {
            Some(crate::ui::sack_name::browser_sack_name(
                &self.document,
                label,
                0x808080,
                1000.0,
                false,
            )?)
        } else {
            None
        };
        let width = fallback
            .as_ref()
            .map_or_else(|| measure_native_ui_text(label, Font::Menu, 1.0), |name| name.size.x)
            + 40.0;
        let frame = self.ui.frame(&self.document, 4, width, 40.0, 5.0)?;
        place(&frame, HudRect::new(center_x - width / 2.0, top, width, 40.0))?;
        append(parent, &frame)?;
        if let Some(name) = fallback {
            crate::ui::add_style(
                &name.node,
                &format!(
                    "position:absolute;left:{}px;top:{}px;visibility:visible",
                    center_x - name.size.x / 2.0,
                    baseline - name.baseline_offset,
                ),
            )?;
            append(parent, &name.node)?;
        } else {
            bitmap_text(
                &self.document,
                parent,
                &TextSpec {
                    align: Align::Center,
                    tint: 0x808080,
                    ..TextSpec::new(Font::Menu, label, center_x, baseline)
                },
            )?;
        }
        Ok(())
    }

    fn draw_side_panel(&self, left: bool) -> Result<(), String> {
        let companion = matches!(self.panel, Some(PlayerPanel::Service(_)));
        let x = if companion {
            if left { 103.0 } else { 1177.0 }
        } else if left {
            50.0
        } else {
            1230.0
        };
        let shift = if companion {
            0.0
        } else if left {
            -53.0
        } else {
            53.0
        };
        let placements: &[(usize, f64, f64, f64, f64)] = if left {
            &[
                (30, 233.0, 429.0, 1.0, 1.0),
                (30, 24.0, 429.0, 1.0, 1.0),
                (30, 233.0, 39.0, 1.0, 1.0),
                (30, 24.0, 39.0, 1.0, 1.0),
                (33, 53.0, 249.0, -1.0, 1.0),
                (29, 303.0, 449.0, 1.0, -1.0),
                (32, 63.0, 439.0, 0.85, 0.85),
                (31, 343.0, 449.0, -0.75, 0.75),
                (31, 343.0, 39.0, -1.0, 1.0),
                (31, 73.0, 39.0, 1.0, 1.0),
                (20, 55.0, 119.0, 1.0, 1.0),
            ]
        } else {
            &[
                (30, 1367.0, 429.0, 1.0, 1.0),
                (30, 1576.0, 429.0, 1.0, 1.0),
                (30, 1367.0, 39.0, 1.0, 1.0),
                (30, 1576.0, 39.0, 1.0, 1.0),
                (33, 1547.0, 279.0, 1.0, 1.0),
                (29, 1297.0, 449.0, -1.0, -1.0),
                (31, 1527.0, 439.0, 0.85, 0.85),
                (32, 1257.0, 459.0, -0.75, 0.75),
                (31, 1257.0, 39.0, 1.0, 1.0),
                (31, 1527.0, 39.0, -1.0, 1.0),
                (20, 1549.0, 119.0, 1.0, 1.0),
            ]
        };
        for (record, cx, cy, sx, sy) in placements {
            art(
                &self.document,
                &self.content,
                &self.ui,
                *record,
                [cx + shift, *cy],
                [*sx, *sy],
                0.0,
            )?;
        }
        let leather = self.ui.tiles(&self.document, 49, 320.0, 320.0)?;
        place(&leather, HudRect::new(x, 89.0, 320.0, 320.0))?;
        append(&self.content, &leather)?;
        let frame = self.inventory.nine_slice(&self.document, 8, 320.0, 320.0)?;
        place(&frame, HudRect::new(x, 89.0, 320.0, 320.0))?;
        append(&self.content, &frame)?;
        for y in [77.0, 404.0] {
            let chain = self.ui.tiles(&self.document, 10, 320.0, 19.0)?;
            place(&chain, HudRect::new(x, y, 320.0, 19.0))?;
            append(&self.content, &chain)?;
        }
        for cx in [x - 10.0, x + 313.0] {
            let chain = self.ui.tiles(&self.document, 79, 21.0, 320.0)?;
            place(&chain, HudRect::new(cx, 89.0, 21.0, 320.0))?;
            append(&self.content, &chain)?;
        }
        self.section_header(
            &self.content,
            if left { "stats" } else { "equip" },
            x + 160.0,
            66.0,
            91.0,
            false,
        )?;
        for (record, cx, cy) in [
            (107, x + 25.0, 114.0),
            (108, x + 295.0, 114.0),
            (109, x + 25.0, 384.0),
            (110, x + 295.0, 384.0),
        ] {
            art(&self.document, &self.content, &self.ui, record, [cx, cy], [1.0, 1.0], 0.0)?;
        }
        Ok(())
    }

    pub(super) fn info_frame(
        &self,
        parent: &web_sys::HtmlElement,
        rect: HudRect,
    ) -> Result<(), String> {
        let frame = self.inventory.frame(&self.document, 10, rect.width, rect.height, 24.0)?;
        frame
            .style()
            .set_property("background", "#1a1a17")
            .map_err(|_| "information frame fill")?;
        place(&frame, rect)?;
        append(parent, &frame)
    }

    fn draw_stats(&self, snapshot: &PlayerPanelSnapshot<'_>) -> Result<(), String> {
        let pane = element(&self.document, "div", "panel-stats")?;
        place(
            &pane,
            HudRect::new(
                if matches!(self.panel, Some(PlayerPanel::Service(_))) { 103.0 } else { 50.0 },
                89.0,
                320.0,
                320.0,
            ),
        )?;
        append(&self.content, &pane)?;
        if self.stats_page == 0 {
            for (x, y) in [
                (16.0, 62.0),
                (206.0, 62.0),
                (61.0, 144.5),
                (66.0, 195.5),
                (216.0, 167.5),
                (16.0, 278.5),
            ] {
                art(&self.document, &pane, &self.inventory, 16, [x, y], [1.0, 1.0], 0.0)?;
            }
            for (x, y, w, h) in [
                (36.0, 22.0, 228.0, 32.0),
                (36.0, 50.0, 228.0, 50.0),
                (36.0, 118.0, 228.0, 27.0),
                (36.0, 141.0, 228.0, 80.0),
                (36.0, 240.0, 228.0, 27.0),
                (36.0, 263.0, 228.0, 32.0),
            ] {
                self.info_frame(&pane, HudRect::new(x, y, w, h))?;
            }
            bitmap_text(
                &self.document,
                &pane,
                &TextSpec {
                    align: Align::Left,
                    tint: 0xffffff,
                    ..TextSpec::new(Font::Menu, &snapshot.selection.name.to_uppercase(), 46.0, 47.0)
                },
            )?;
            bitmap_text(
                &self.document,
                &pane,
                &TextSpec {
                    align: Align::Left,
                    tint: 0xd9ba70,
                    ..TextSpec::new(
                        Font::Medium,
                        &format!(
                            "LEVEL {}\n{}",
                            snapshot.progression.level,
                            native_wizard_class_title(
                                snapshot.selection.element,
                                snapshot.selection.discipline
                            )
                        ),
                        46.0,
                        70.0,
                    )
                },
            )?;
            bitmap_text(
                &self.document,
                &pane,
                &TextSpec {
                    align: Align::Left,
                    tint: 0xd9ba70,
                    ..TextSpec::new(Font::Body, "PRIMARY SPELL", 45.0, 136.0)
                },
            )?;
            let descriptor = native_primary_spell_descriptor(
                snapshot.book.primary_skill_id,
                snapshot.book.weld_build_id,
            );
            let summary = native_primary_spell_summary_with_stat_book(
                snapshot.book,
                snapshot.stat_book,
                snapshot.offensive,
            );
            let damage = if descriptor.damage_range {
                format!(
                    "{} - {}",
                    inventory_stat_number(summary.damage_minimum)?,
                    inventory_stat_number(summary.damage_maximum)?
                )
            } else {
                inventory_stat_number(summary.damage_maximum)?
            };
            let damage_unit = match descriptor.damage_unit {
                NativePrimarySpellDamageUnit::Bolt => "bolt",
                NativePrimarySpellDamageUnit::Boulder => "boulder",
                NativePrimarySpellDamageUnit::Impact => "impact",
                NativePrimarySpellDamageUnit::Rock => "rock",
                NativePrimarySpellDamageUnit::Second => "sec",
            };
            let mana_unit = match descriptor.mana_unit {
                NativePrimarySpellManaUnit::Cast => "cast",
                NativePrimarySpellManaUnit::Second => "sec",
            };
            // Inventory uses the pale skill-picker palette, rather than the saturated VFX color.
            let root = solomon_sim::profile::native_skill_root(snapshot.book.primary_skill_id)
                .expect("selected primary root");
            let color =
                [0xffe5ff, 0xffcbcb, 0xe5ffff, 0xcbcbff, 0xcbffcb, 0xffe5cb, 0xcbd8ff, 0xe5e5e5]
                    [root];
            for (line, unit, baseline) in [
                (descriptor.name.to_string(), None, 162.0),
                (format!("damage: {damage}"), Some(format!(" / {damage_unit}")), 184.0),
                (
                    format!("mana cost: {}", inventory_stat_number(summary.mana_cost)?),
                    Some(format!(" / {mana_unit}")),
                    197.0,
                ),
                (
                    format!(
                        "mana heal: {}",
                        inventory_stat_number(snapshot.attributes.mana_recovery_per_second)?
                    ),
                    Some(" / sec".to_owned()),
                    210.0,
                ),
            ] {
                self.draw_stat_line(&pane, &line, unit.as_deref(), baseline, color)?;
            }
            art(&self.document, &pane, &self.inventory, 3, [269.0, 167.5], [1.0, 1.0], 0.0)?;
            bitmap_text(
                &self.document,
                &pane,
                &TextSpec {
                    align: Align::Left,
                    tint: 0xd9ba70,
                    ..TextSpec::new(Font::Body, "MELEE DAMAGE", 45.0, 258.0)
                },
            )?;
            let melee = format!(
                "{} - {}",
                inventory_stat_number(snapshot.attributes.melee_damage_minimum)?,
                inventory_stat_number(snapshot.attributes.melee_damage_maximum)?,
            );
            self.draw_stat_line(&pane, &melee, Some(" / whack"), 285.0, color)?;
        } else if self.stats_page == 1 {
            // `HUB_INVENTORY_ATTRIBUTES_PAGE` is offset by pageHeight=320 and
            // the stats viewport's top=89; labels and values use distinct native pens.
            for (x, y) in [(64.0, 66.0), (64.0, 133.0), (206.0, 96.0), (64.0, 231.0), (-9.0, 271.0)]
            {
                art(&self.document, &pane, &self.inventory, 16, [x, y], [1.0, 1.0], 0.0)?;
            }
            for rect in [
                HudRect::new(36.0, 42.0, 228.0, 32.0),
                HudRect::new(36.0, 70.0, 228.0, 80.0),
                HudRect::new(156.0, 70.0, 108.0, 80.0),
                HudRect::new(36.0, 190.0, 228.0, 32.0),
                HudRect::new(36.0, 218.0, 228.0, 60.0),
                HudRect::new(156.0, 218.0, 108.0, 60.0),
            ] {
                self.info_frame(&pane, rect)?;
            }
            for (label, baseline) in [("ATTRIBUTES", 63.0), ("RESISTANCES", 211.0)] {
                bitmap_text(
                    &self.document,
                    &pane,
                    &TextSpec {
                        tint: 0xd9ba70,
                        ..TextSpec::new(Font::Medium, label, 150.0, baseline)
                    },
                )?;
            }
            let attributes = &snapshot.attributes;
            for (label, value, tint, baseline) in [
                (
                    "HEALTH:",
                    format!(
                        "{}/{}",
                        native_rounded_stat(snapshot.resources.current_health),
                        native_rounded_stat(snapshot.resources.maximum_health)
                    ),
                    0xe9c9c9,
                    91.0,
                ),
                (
                    "MANA:",
                    format!(
                        "{}/{}",
                        native_rounded_stat(snapshot.resources.current_mana),
                        native_rounded_stat(snapshot.resources.maximum_mana)
                    ),
                    0xc2c2e2,
                    105.0,
                ),
                (
                    "CAST SPEED:",
                    format!("{}%", native_rounded_stat(attributes.cast_speed_percent)),
                    0xc9f9c9,
                    124.0,
                ),
                (
                    "WALK SPEED:",
                    format!("{}%", native_rounded_stat(attributes.walk_speed_percent)),
                    0xc9f9c9,
                    138.0,
                ),
                (
                    "PAIN:",
                    format!("{}%", native_rounded_stat(attributes.pain_resistance_percent)),
                    0xe9c9c9,
                    239.0,
                ),
                (
                    "MAGIC:",
                    format!("{}%", native_rounded_stat(attributes.magic_resistance_percent)),
                    0xc2c2e2,
                    253.0,
                ),
                (
                    "POISON:",
                    format!("{}%", native_rounded_stat(attributes.poison_resistance_percent)),
                    0xc9f9c9,
                    267.0,
                ),
            ] {
                bitmap_text(
                    &self.document,
                    &pane,
                    &TextSpec {
                        align: Align::Right,
                        tint,
                        ..TextSpec::new(Font::Body, label, 151.0, baseline)
                    },
                )?;
                bitmap_text(
                    &self.document,
                    &pane,
                    &TextSpec {
                        align: Align::Left,
                        tint,
                        ..TextSpec::new(Font::Medium, &value, 166.0, baseline)
                    },
                )?;
            }
        } else {
            self.draw_owned_perks(&pane, snapshot, -103.0, -89.0)?;
        }
        if self.stats_page > 0 {
            let arrow = button(
                &self.document,
                &self.content,
                "stats-up",
                "Previous player stats page",
                HudRect::new(
                    if matches!(self.panel, Some(PlayerPanel::Service(_))) { 373.0 } else { 320.0 },
                    101.0,
                    36.0,
                    36.0,
                ),
            )?;
            art(&self.document, &arrow, &self.inventory, 13, [18.0, 18.0], [1.0, -1.0], 0.0)?;
        }
        if self.stats_page < 2 {
            let arrow = button(
                &self.document,
                &self.content,
                "stats-down",
                "Next player stats page",
                HudRect::new(
                    if matches!(self.panel, Some(PlayerPanel::Service(_))) { 373.0 } else { 320.0 },
                    361.0,
                    36.0,
                    36.0,
                ),
            )?;
            art(&self.document, &arrow, &self.inventory, 13, [18.0, 18.0], [1.0, 1.0], 0.0)?;
        }
        Ok(())
    }

    fn draw_equipment(&self, snapshot: &PlayerPanelSnapshot<'_>) -> Result<(), String> {
        let shift = if matches!(self.panel, Some(PlayerPanel::Service(_))) { -53.0 } else { 0.0 };
        for (x, y) in
            [(1390.0, 224.0), (1390.0, 289.0), (1532.0, 192.0), (1532.0, 256.0), (1532.0, 321.0)]
        {
            art(
                &self.document,
                &self.content,
                &self.inventory,
                16,
                [x + shift, y],
                [1.0, 1.0],
                0.0,
            )?;
        }
        for slot in EquipmentSlot::ALL {
            if slot == EquipmentSlot::Ring2 && !snapshot.economy.owned_perk_selectors.contains(&19)
            {
                continue;
            }
            let item = snapshot.economy.equipment.at(slot).as_ref();
            for (alias, mut rect) in equipment_slot_rects(slot).into_iter().enumerate() {
                rect.x += shift;
                let sink = button(
                    &self.document,
                    &self.content,
                    &format!("equipment:{}", slot.as_str()),
                    &format!(
                        "{}: {}",
                        slot.as_str(),
                        item.map_or("empty", |item| item.name.as_str())
                    ),
                    rect,
                )?;
                sink.set_class_name("panel-action panel-equipment-sink");
                if rect.height == 46.0 || rect.height == 72.0 {
                    append(
                        &sink,
                        &self
                            .inventory
                            .raw_sprite(&self.document, if rect.height == 46.0 { 9 } else { 10 })?,
                    )?;
                }
                let displayed = item.filter(|item| {
                    !self.inventory_item_hidden(item.id)
                        && (slot != EquipmentSlot::Weapon
                            || alias == 0
                            || item.equipment_type != Some(EquipmentType::Wand))
                });
                if let Some(item) = displayed {
                    self.draw_item(
                        &sink,
                        item,
                        rect.width / 2.0,
                        rect.height / 2.0,
                        1,
                        snapshot.mod_assets,
                    )?;
                }
                if self.selected == Some(Target::Equipment(slot)) {
                    sink.set_attribute("data-selected", "true")
                        .map_err(|_| "equipment selection")?;
                }
            }
        }
        if self.panel == Some(PlayerPanel::Inventory) {
            self.draw_active_set_bonuses(snapshot)?;
        }
        Ok(())
    }

    fn draw_stat_line(
        &self,
        pane: &web_sys::HtmlElement,
        line: &str,
        unit: Option<&str>,
        baseline: f64,
        tint: u32,
    ) -> Result<(), String> {
        let mut runs =
            vec![NativeUiTextRun { advance_scale: Some(0.9), ..NativeUiTextRun::new(line) }];
        if let Some(unit) = unit {
            runs.push(NativeUiTextRun {
                scale: Some(0.7),
                advance_scale: Some(0.9 * 0.7),
                offset_y: 1.0,
                italic: true,
                ..NativeUiTextRun::new(unit)
            });
        }
        let layout = layout_native_ui_text_runs(Font::Medium, &runs, 45.0, baseline, tint);
        let node = element(&self.document, "span", "panel-bitmap-text")?;
        style(&node, "position:absolute;left:0;top:0;width:0;height:0;pointer-events:none")?;
        paint_text_layout(&self.document, &node, &layout)?;
        append(pane, &node)
    }

    /// Source `addEquipment` pins its contextual bonus card left of the equipment pane.
    fn draw_active_set_bonuses(&self, snapshot: &PlayerPanelSnapshot<'_>) -> Result<(), String> {
        let mut recipes = Vec::new();
        for slot in EquipmentSlot::ALL {
            if let Some(recipe) =
                snapshot.economy.equipment.at(slot).as_ref().and_then(|item| item.recipe_index)
            {
                recipes.push(recipe);
            }
        }
        let sets = native_equipment_completed_sets(&recipes);
        if sets.is_empty() {
            return Ok(());
        }
        let mut lines = vec![("Set Bonus Active:".to_owned(), 0xd9ba70)];
        for (index, set) in sets.iter().enumerate() {
            if index > 0 {
                lines.push((String::new(), 0xbfbfbf));
            }
            lines.push((set.name.to_owned(), 0xffbf80));
            for effect in set.effects {
                lines.push((super::skills::equipment_effect_text(*effect), 0xbfbfbf));
            }
        }
        let mut layouts = Vec::new();
        let mut content_width: f64 = 0.0;
        let mut cursor = 25.0;
        let line_height = native_ui_font(Font::Body).metrics[0];
        for (text, tint) in &lines {
            let layout = layout_native_ui_text(&TextSpec {
                align: Align::Left,
                max_width: Some(300.0),
                line_height: Some(line_height),
                tint: *tint,
                ..TextSpec::new(Font::Body, text, 25.0, cursor)
            });
            content_width = content_width.max(layout.width);
            cursor += layout.lines.len() as f64 * line_height + 10.0;
            layouts.push(layout);
        }
        let width = content_width + 50.0;
        let card = element(&self.document, "section", "panel-active-set-bonuses")?;
        card.set_attribute("aria-label", "Active set bonuses").map_err(|_| "active set label")?;
        card.set_attribute("role", "status").map_err(|_| "active set status")?;
        card.set_attribute("aria-live", "polite").map_err(|_| "active set live status")?;
        card.set_attribute("aria-atomic", "true").map_err(|_| "active set atomic status")?;
        // Graphics' centered one-pixel stroke contributes one pixel to Source card.width.
        place(&card, HudRect::new(1214.0 - width, 110.0, width, cursor + 15.0))?;
        for (text, _) in &lines {
            if text.is_empty() {
                continue;
            }
            let semantic = element(&self.document, "p", "native-ui-sr-only")?;
            semantic.set_text_content(Some(text));
            append(&card, &semantic)?;
        }
        for layout in layouts {
            let line = element(&self.document, "span", "panel-bitmap-text")?;
            style(&line, "position:absolute;inset:0;pointer-events:none")?;
            paint_text_layout(&self.document, &line, &layout)?;
            append(&card, &line)?;
        }
        append(&self.content, &card)
    }
}

// Source uses Number.toFixed(1), including binary half cases and large authored values.
fn inventory_stat_number(value: f64) -> Result<String, String> {
    js_sys::Number::from(value)
        .to_fixed(1)
        .map(String::from)
        .map_err(|_| "Inventory stat decimal formatting".to_owned())
}

// `nativeRoundedStat` uses JavaScript Math.round, whose half cases round toward +∞.
fn native_rounded_stat(value: f64) -> String {
    format!("{:.0}", (value + 0.5).floor())
}
