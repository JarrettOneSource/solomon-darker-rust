//! Native College shops. Selection and dragging are local; every purchase, transfer,
//! perk change and dowsing roll is a canonical economy command.

use solomon_sim::audio::AudioCue;
use solomon_sim::economy::{
    DOWSING_EQUIPMENT_RECIPES, HAGATHA_PERKS, HubInventoryAction, HubInventoryItem, HubTraderId,
    TransferDirection, create_equipment_inventory_item, project_inventory_root_slots,
};
use solomon_sim::hub::interaction::HubInteractionId;
use solomon_sim::hud::HudRect;
use solomon_sim::native_ui_text::{
    NativeUiFontName, NativeUiTextAlign, NativeUiTextSpec, measure_native_ui_text,
};

use super::chrome::PanelCanvas;
use super::{PlayerPanel, PlayerPanelAction, PlayerPanelSnapshot, PlayerPanels, Target};
pub(super) fn service_title(trader: HubTraderId) -> &'static str {
    interaction_for_trader(trader).definition(false).service_title.expect("stock trader title")
}

fn interaction_for_trader(trader: HubTraderId) -> HubInteractionId {
    match trader {
        HubTraderId::Fomentius => HubInteractionId::Fomentius,
        HubTraderId::Hagatha => HubInteractionId::Hagatha,
        HubTraderId::Luthacus => HubInteractionId::Luthacus,
        HubTraderId::Shlorio => HubInteractionId::Shlorio,
    }
}

fn shop_rect(index: usize, row_first: bool, dowsing: bool) -> HudRect {
    if dowsing {
        return HudRect::new(
            689.0 + (index % 3) as f64 * 75.0,
            94.0 + (index / 3) as f64 * 75.0,
            72.0,
            72.0,
        );
    }
    let (column, row) = if row_first { (index % 7, index / 7) } else { (index / 4, index % 4) };
    HudRect::new(539.0 + column as f64 * 75.0, 56.5 + row as f64 * 75.0, 72.0, 72.0)
}

impl PlayerPanels {
    pub fn show_service(&mut self, trader: HubTraderId, now: f64) -> Result<(), String> {
        self.show(PlayerPanel::Service(trader), now)?;
        self.actions.push(PlayerPanelAction::Inventory(HubInventoryAction::AcknowledgeNpcHint {
            interaction_id: interaction_for_trader(trader),
        }));
        Ok(())
    }
    pub(super) fn finish_service(&mut self) {
        let action = match self.panel {
            Some(PlayerPanel::Service(HubTraderId::Hagatha)) => {
                Some(HubInventoryAction::CloseHagatha)
            }
            Some(PlayerPanel::Service(HubTraderId::Shlorio)) if self.dowsing_rolled => {
                Some(HubInventoryAction::CloseDowsing)
            }
            _ => None,
        };
        if let Some(action) = action {
            self.actions.push(PlayerPanelAction::Inventory(action));
        }
    }
    pub(super) fn activate_service(
        &mut self,
        target: Target,
        snapshot: &PlayerPanelSnapshot<'_>,
        now: f64,
    ) {
        let Some(PlayerPanel::Service(trader)) = self.panel else {
            return;
        };
        let selected = self.selected == Some(target);
        match target {
            Target::Shop(item_id) if selected => {
                self.sound(AudioCue::Click);
                let action = if trader == HubTraderId::Fomentius {
                    HubInventoryAction::BuyFomentius { item_id }
                } else if trader == HubTraderId::Shlorio {
                    HubInventoryAction::BuyDowsing { offer_id: item_id }
                } else {
                    return;
                };
                self.actions.push(PlayerPanelAction::Inventory(action));
            }
            Target::Hagatha(selector) if selected => {
                self.sound(AudioCue::Click);
                self.actions.push(PlayerPanelAction::Inventory(HubInventoryAction::BuyHagatha {
                    selector,
                }));
            }
            Target::Storage(item_id) => {
                let doubled = self
                    .last_activation
                    .is_some_and(|(previous, at)| previous == target && now - at <= 500.0);
                self.last_activation = Some((target, now));
                self.selected = Some(target);
                self.selected_at = now;
                if doubled {
                    self.transfer_double_activation = true;
                    self.last_activation = None;
                    self.actions.push(PlayerPanelAction::Inventory(HubInventoryAction::Transfer {
                        item_id,
                        direction: TransferDirection::ToBackpack,
                        gesture: solomon_sim::economy::HubTransferGesture::DoubleActivation,
                    }));
                }
            }
            Target::Shop(_) | Target::Hagatha(_) => {
                self.sound(AudioCue::Click);
                self.selected = Some(target);
                self.selected_at = now;
            }
            Target::ShopCell => self.selected = None,
            Target::Dowse => {
                if snapshot.economy.dowsing_rolled {
                    return;
                }
                let action =
                    HubInventoryAction::Dowse { reference_item_id: self.dowsing_reference_item };
                if snapshot.economy.gold < snapshot.economy.dowsing_fee {
                    self.show_inventory_rejection(
                        &action,
                        solomon_sim::economy::HubEconomyRejection::InsufficientGold,
                    );
                } else {
                    self.actions.push(PlayerPanelAction::Inventory(action));
                }
            }
            _ => {}
        }
    }
    pub(super) fn service_item(
        &self,
        snapshot: &PlayerPanelSnapshot<'_>,
        target: Target,
    ) -> Option<(HubInventoryItem, Option<u64>)> {
        match target {
            Target::Storage(id) => {
                solomon_sim::economy::find_inventory_item(&snapshot.economy.storage, id)
                    .cloned()
                    .map(|item| (item, None))
            }
            Target::Shop(id) => {
                if self.panel == Some(PlayerPanel::Service(HubTraderId::Fomentius)) {
                    snapshot
                        .economy
                        .fomentius_stock
                        .iter()
                        .find(|stock| stock.item.id == id)
                        .map(|stock| (stock.item.clone(), Some(stock.price)))
                } else {
                    snapshot.economy.dowsing_offers.iter().find(|offer| offer.id == id).map(
                        |offer| {
                            (
                                create_equipment_inventory_item(
                                    &DOWSING_EQUIPMENT_RECIPES[offer.recipe_index],
                                    offer.id,
                                ),
                                Some(offer.price),
                            )
                        },
                    )
                }
            }
            _ => None,
        }
    }
    fn draw_shop_chrome(&self, c: &PanelCanvas<'_, '_>, purple: bool) -> Result<(), String> {
        c.tiles(
            &self.ui,
            49,
            HudRect::new(498.0, -20.0, 604.0, 400.0),
            1.0,
            if purple { 0xc899ff } else { 0xd9ffd9 },
        )?;
        c.additive().tiles(
            &self.ui,
            49,
            HudRect::new(498.0, -20.0, 604.0, 400.0),
            1.0,
            if purple { 0xc899ff } else { 0xd9ffd9 },
        )?;
        for (record, x, y, sx, sy) in [
            (8, 557.5, 16.5, 1.0, 1.0),
            (8, 1041.5, 16.5, -1.0, 1.0),
            (8, 557.5, 333.5, 1.0, -1.0),
            (8, 1041.5, 333.5, -1.0, -1.0),
        ] {
            c.art(&self.inventory, record, [x, y], [sx, sy], 0.0, 0xffffff)?;
        }
        for (atlas, record, x, y, sx, sy) in [
            (&self.skills, 4, 600.0, 25.0, 1.0, 1.0),
            (&self.skills, 4, 1000.0, 25.0, -1.0, 1.0),
            (&self.ui, 4, 588.5, 13.0, 1.0, 1.0),
            (&self.ui, 4, 1011.5, 13.0, -1.0, 1.0),
            (&self.ui, 4, 588.5, 33.0, 1.0, -1.0),
            (&self.ui, 4, 1011.5, 33.0, -1.0, -1.0),
        ] {
            c.art(atlas, record, [x, y], [sx, sy], 0.0, 0xffffff)?;
        }
        for index in 0..5 {
            c.art(&self.ui, 74, [570.5 + index as f64 * 129.0, -35.0], [1.0, -1.0], 0.0, 0xffffff)?;
        }
        for index in 0..10 {
            c.art(&self.ui, 74, [506.0, -13.0 + index as f64 * 44.0], [1.0, 1.0], 90.0, 0xffffff)?;
            c.art(
                &self.ui,
                74,
                [1093.0, -13.0 + index as f64 * 44.0],
                [1.0, -1.0],
                90.0,
                0xffffff,
            )?;
        }
        for (x, y, sx, sy) in [
            (1063.0, 355.0, 1.0, 1.0),
            (536.0, 355.0, -1.0, 1.0),
            (1063.0, -5.0, 1.0, -1.0),
            (536.0, -5.0, -1.0, -1.0),
        ] {
            c.art(&self.ui, 73, [x, y], [sx, sy], 0.0, 0xffffff)?;
        }
        Ok(())
    }
    pub(super) fn draw_service(
        &self,
        c: &PanelCanvas<'_, '_>,
        trader: HubTraderId,
        s: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        let dowsing = trader == HubTraderId::Shlorio;
        self.draw_shop_chrome(c, dowsing && s.economy.dowsing_rolled)?;
        let title = service_title(trader);
        let font = if measure_native_ui_text(title, NativeUiFontName::Menu, 1.0) > 549.0 {
            NativeUiFontName::Medium
        } else {
            NativeUiFontName::Menu
        };
        c.text(&NativeUiTextSpec {
            tint: 0xd9ba70,
            ..NativeUiTextSpec::new(font, title, 800.0, 32.0)
        });
        c.semantic("panel:service", title, "dialog", HudRect::new(498.0, 0.0, 604.0, 420.0));
        if dowsing && !s.economy.dowsing_rolled {
            c.art(&self.ui, 15, [800.0, 75.0], [1.0, 1.0], 0.0, 0xffffff)?;
            let mirror = c.at(750.0, 101.0).clip(HudRect::new(0.0, 0.0, 100.0, 149.0));
            mirror.fill(HudRect::new(0.0, 0.0, 100.0, 149.0), 0, 1.0);
            if let Some(item) = self
                .dowsing_reference_item
                .and_then(|id| solomon_sim::belt::native_belt_owned_item(s.economy, id))
            {
                self.draw_item(&mirror, item, 50.0, 74.5, 1)?;
            }
            let dowse = c.native_button(
                &self.ui,
                "dowse",
                "DOWSE",
                HudRect::new(675.0, 265.5, 250.0, 69.0),
            )?;
            dowse.text(&NativeUiTextSpec {
                tint: 0xd9ba70,
                ..NativeUiTextSpec::new(
                    NativeUiFontName::Medium,
                    &format!("{} GOLD", s.economy.dowsing_fee),
                    125.0,
                    57.0,
                )
            });
        } else if trader == HubTraderId::Hagatha {
            let offers = s.economy.hagatha_offers();
            for index in 0..28 {
                let r = shop_rect(index, true, false);
                let offer = offers.get(index);
                let action = offer.map_or_else(
                    || "shop-cell".into(),
                    |offer| format!("hagatha:{}", offer.selector),
                );
                let label = offer.map_or_else(
                    || "Empty shop slot".into(),
                    |offer| format!("Buy {} for {} gold", offer.name, offer.price),
                );
                let cell = self.shop_slot(c, &action, &label, r)?;
                if let Some(offer) = offer {
                    if self.selected == Some(Target::Hagatha(offer.selector)) {
                        self.draw_shop_selection(&cell, offer.price > s.economy.gold, false)?;
                    } else {
                        cell.art(
                            if offer.selector == -1 { &self.inventory } else { &self.skills },
                            if offer.selector == -1 { 5 } else { 127 + offer.selector as usize },
                            [36.0, 36.0],
                            if offer.selector == -1 { [0.6, 0.6] } else { [1.0, 1.0] },
                            0.0,
                            0xffffff,
                        )?;
                    }
                    self.shop_price(&cell, offer.price, offer.price > s.economy.gold);
                }
            }
        } else {
            let projected = project_inventory_root_slots(&s.economy.storage);
            for index in 0..if dowsing { 9 } else { 28 } {
                let generated;
                let (item, price) = match trader {
                    HubTraderId::Fomentius => s
                        .economy
                        .fomentius_stock
                        .get(index)
                        .map_or((None, 0), |stock| (Some(&stock.item), stock.price)),
                    HubTraderId::Luthacus => {
                        (projected.iter().find(|item| item.slot == index).map(|item| item.item), 0)
                    }
                    HubTraderId::Shlorio => {
                        if let Some(offer) = s.economy.dowsing_offers.get(index) {
                            generated = create_equipment_inventory_item(
                                &DOWSING_EQUIPMENT_RECIPES[offer.recipe_index],
                                offer.id,
                            );
                            (Some(&generated), offer.price)
                        } else {
                            (None, 0)
                        }
                    }
                    HubTraderId::Hagatha => unreachable!(),
                };
                let storage = trader == HubTraderId::Luthacus;
                let action = item.map_or_else(
                    || "shop-cell".into(),
                    |item| format!("{}:{}", if storage { "storage" } else { "shop" }, item.id),
                );
                let label = item.map_or_else(
                    || "Empty shop slot".into(),
                    |item| {
                        if storage {
                            format!("{}, quantity {}", item.name, item.quantity)
                        } else {
                            format!("Buy {} for {price} gold", item.name)
                        }
                    },
                );
                let cell = self.shop_slot(c, &action, &label, shop_rect(index, false, dowsing))?;
                if let Some(item) = item {
                    let target =
                        if storage { Target::Storage(item.id) } else { Target::Shop(item.id) };
                    if self.selected == Some(target) {
                        self.draw_shop_selection(
                            &cell,
                            !storage && price > s.economy.gold,
                            storage,
                        )?;
                    } else {
                        self.draw_item(&cell, item, 36.0, 36.0, 1)?;
                    }
                    if !storage {
                        self.shop_price(&cell, price, price > s.economy.gold);
                    } else if item.quantity > 1 {
                        self.shop_price(&cell, item.quantity as u64, false);
                    }
                }
            }
        }
        c.art(&self.ui, 72, [800.0, 387.0], [1.0, 1.0], 0.0, 0xffffff)?;
        c.faded(0.85).art(&self.ui, 12, [800.0, 385.0], [1.0, 1.0], 0.0, 0xffffff)?;
        c.art(&self.ui, 86, [800.0, 385.0], [1.0, 1.0], 0.0, 0xbfffbf)?;
        let done = c.named_button(
            "close",
            if self.compact { "panel:service:done" } else { "panel:close" },
            "Done",
            HudRect::new(714.5, 358.0, 171.0, 58.0),
        );
        done.text(&NativeUiTextSpec::new(NativeUiFontName::Menu, "DONE", 85.5, 34.0));
        Ok(())
    }
    fn shop_slot<'a, 'gpu>(
        &self,
        c: &PanelCanvas<'a, 'gpu>,
        action: &str,
        label: &str,
        r: HudRect,
    ) -> Result<PanelCanvas<'a, 'gpu>, String> {
        let cell = c.button(action, label, r);
        cell.faded(0.6).raw(
            &self.inventory,
            10,
            HudRect::new(0.0, 0.0, r.width, r.height),
            0xffffff,
        )?;
        Ok(cell)
    }
    fn shop_price(&self, c: &PanelCanvas<'_, '_>, amount: u64, unaffordable: bool) {
        c.text(&NativeUiTextSpec {
            align: NativeUiTextAlign::Right,
            tint: if unaffordable { 0xff8080 } else { 0xd9ba70 },
            ..NativeUiTextSpec::new(NativeUiFontName::Body, &amount.to_string(), 67.0, 67.0)
        });
    }
    fn draw_shop_selection(
        &self,
        c: &PanelCanvas<'_, '_>,
        unaffordable: bool,
        storage: bool,
    ) -> Result<(), String> {
        let record = if storage {
            111
        } else if unaffordable {
            46
        } else {
            84
        };
        let r = self.ui.record(record)?;
        c.raw(
            &self.ui,
            record,
            HudRect::new(
                if unaffordable { -0.5 } else { 3.0 },
                if unaffordable { 5.5 } else { 11.0 },
                r.width as f64,
                r.height as f64,
            ),
            0xffffff,
        )
    }
    pub(super) fn draw_hagatha_perks(
        &self,
        c: &PanelCanvas<'_, '_>,
        s: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        self.draw_owned_perks(c, s, 0.0, 0.0)
    }
    pub(super) fn draw_owned_perks(
        &self,
        c: &PanelCanvas<'_, '_>,
        s: &PlayerPanelSnapshot<'_>,
        offset_x: f64,
        offset_y: f64,
    ) -> Result<(), String> {
        let c = c.at(offset_x, offset_y);
        for (x, y) in
            [(323.0, 227.0), (166.0, 247.0), (166.0, 312.0), (166.0, 182.0), (111.0, 125.0)]
        {
            c.art(&self.inventory, 16, [x, y], [1.0, 1.0], 0.0, 0xffffff)?;
        }
        let gem = self.inventory.record(3)?;
        c.art(
            &self.inventory,
            3,
            [362.0 + gem.logical_width as f64 / 2.0, 218.0 + gem.logical_height as f64 / 2.0],
            [1.0, 1.0],
            0.0,
            0xffffff,
        )?;
        self.info_frame(&c, HudRect::new(138.0, 127.0, 230.0, 230.0))?;
        c.text(&NativeUiTextSpec {
            tint: 0xd9ba70,
            ..NativeUiTextSpec::new(NativeUiFontName::Medium, "CHARMS/CURSES", 253.0, 152.0)
        });
        for index in 0..9 {
            let x = 193.0 + (index % 3) as f64 * 60.0;
            let y = 198.0 + (index / 3) as f64 * 60.0;
            c.faded(if index >= s.economy.charm_capacity as usize { 0.5 } else { 1.0 }).art(
                &self.inventory,
                10,
                [x, y],
                [0.8, 0.8],
                0.0,
                0xffffff,
            )?;
            if let Some(selector) = s.economy.owned_perk_selectors.get(index) {
                let perk = HAGATHA_PERKS[*selector as usize];
                let removable = s.perk_removal_enabled && *selector != 27;
                let node = c.button(
                    &format!("owned-perk:{selector}"),
                    &format!("{} {}", if removable { "Remove" } else { "Inspect" }, perk.name),
                    HudRect::new(x - 28.8, y - 28.8, 57.6, 57.6),
                );
                node.art(
                    &self.skills,
                    127 + *selector as usize,
                    [28.8, 28.8],
                    [0.8, 0.8],
                    0.0,
                    0xffffff,
                )?;
            }
        }
        if s.economy.charm_capacity < 8 {
            c.art(
                &self.inventory,
                5,
                [253.0, if s.economy.charm_capacity < 4 { 288.0 } else { 318.0 }],
                [1.0, 1.0],
                0.0,
                0xffffff,
            )?;
        }
        Ok(())
    }
}
