//! Purchases, dowsing, dye and Unforge from Website `core-kernels/hub-economy.ts`.

use crate::float::fround;
use crate::hub::interaction::{NATIVE_LIBRARIAN_BOOKS, NATIVE_TEACHER_SPELLS};

use super::inventory::{
    consume_tree, create_equipment_inventory_item, create_fomentius_inventory_item,
    find_inventory_item, insert, project_inventory_items, remove_tree, replace_tree,
};
use super::{
    DOWSING_EQUIPMENT_RECIPES, DowsingOffer, FOMENTIUS_STOCK_DEFINITIONS, HAGATHA_PERKS,
    HUB_INVENTORY_SLOT_CAPACITY, HagathaOffer, HubEconomyContext, HubEconomyRejection as Rejection,
    HubEconomyResult, HubEconomyState, HubInventoryAction, HubInventoryItem, HubItemKind,
    HubShopItem, NativeDyeLayer, NativeInventoryDyeSession, NativeUnforgeOutcome,
    NativeUnforgeOutcomeKind as UnforgeKind,
};

/// Native DyeClothing swatches preserve fractional channel values before the final tint.
pub const NATIVE_DYE_SWATCH_COLORS: [[f64; 3]; 18] = [
    [1.0, 1.0, 1.0],
    [1.0, 0.0, 0.0],
    [1.0, 0.5, 0.0],
    [1.0, 1.0, 0.0],
    [0.5, 1.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 1.0, 0.5],
    [0.0, 1.0, 1.0],
    [0.0, 0.5, 1.0],
    [0.0, 0.0, 1.0],
    [0.5, 0.0, 1.0],
    [1.0, 0.5, 1.0],
    [1.0, 0.0, 1.0],
    [1.0, 0.0, 0.5],
    [0.75, 0.75, 0.75],
    [0.5, 0.5, 0.5],
    [0.25, 0.25, 0.25],
    [0.1, 0.1, 0.1],
];

#[must_use]
pub fn native_dye_mixed_tint(rows: &[usize]) -> Option<u32> {
    mixed_color(rows).map(rgb_tint)
}

#[must_use]
pub fn native_dye_committed_tint(rows: &[usize]) -> Option<u32> {
    let mixed = mixed_color(rows)?;
    let luminance = fround(
        fround(mixed[0] * f64::from(0.3086_f32))
            + fround(mixed[1] * f64::from(0.6094_f32))
            + fround(mixed[2] * f64::from(0.082_f32)),
    );
    Some(rgb_tint(mixed.map(|channel| {
        fround(fround(luminance * fround(0.75)) + fround(channel * fround(0.25))).clamp(0.0, 1.0)
    })))
}

fn valid_dye_rows(rows: &[usize]) -> bool {
    !rows.is_empty()
        && rows.len() <= 256
        && rows.iter().all(|row| *row < NATIVE_DYE_SWATCH_COLORS.len())
}

fn mixed_color(rows: &[usize]) -> Option<[f64; 3]> {
    if rows.is_empty() {
        return None;
    }
    assert!(
        valid_dye_rows(rows),
        "native dye swatch rows must exist and fit the mixing transaction"
    );
    let mut mixed = NATIVE_DYE_SWATCH_COLORS[rows[0]].map(fround);
    for row in &rows[1..] {
        let incoming = NATIVE_DYE_SWATCH_COLORS[*row].map(fround);
        for index in 0..3 {
            mixed[index] = fround(
                fround(mixed[index] * fround(0.875)) + fround(incoming[index] * fround(0.125)),
            );
        }
    }
    Some(mixed)
}

fn rgb_tint(color: [f64; 3]) -> u32 {
    let [red, green, blue] =
        color.map(|channel| (fround(channel).clamp(0.0, 1.0) * 255.0).round() as u32);
    (red << 16) | (green << 8) | blue
}

impl HubInventoryItem {
    /// Stock recipe tints fill only absent channels; equipped dyed colours retain ownership.
    #[must_use]
    pub fn clothing_tints(&self) -> Option<[u32; 2]> {
        let recipe = self.recipe_index.and_then(|index| DOWSING_EQUIPMENT_RECIPES.get(index));
        let recipe_tints = recipe.map_or([None, None], |recipe| recipe.icon_tints);
        let item_tints = self.icon_tints.unwrap_or([None, None]);
        Some([item_tints[0].or(recipe_tints[0])?, item_tints[1].or(recipe_tints[1])?])
    }

    #[must_use]
    pub fn can_unforge(&self) -> bool {
        [7002, 7003, 7004, 7005, 7006, 7008, 7011].contains(&self.native_type_id)
    }
}

impl HubEconomyState {
    pub(super) fn apply_command(
        &mut self,
        action: &HubInventoryAction,
        context: &HubEconomyContext,
        extensions: Option<&dyn crate::modding::extensions::GameSimulationExtensions>,
        dye_session: Option<&NativeInventoryDyeSession>,
    ) -> HubEconomyResult {
        let mut result = HubEconomyResult::accepted();
        let operation = match action {
            HubInventoryAction::AcknowledgeNpcHint { interaction_id } => {
                if self.npc.acknowledge_hint(*interaction_id) {
                    self.revision += 1;
                }
                return result;
            }
            HubInventoryAction::BuyFomentius { item_id } => self.buy_fomentius(*item_id),
            HubInventoryAction::BuyHagatha { selector } => self.buy_hagatha(*selector),
            HubInventoryAction::CloseHagatha => {
                if self.hagatha_bundle_selectors != self.owned_perk_selectors {
                    self.hagatha_bundle_selectors = self.owned_perk_selectors.clone();
                    self.revision += 1;
                }
                return result;
            }
            HubInventoryAction::RemoveHagatha { selector } => {
                match self.owned_perk_selectors.iter().position(|owned| owned == selector) {
                    Some(index) if *selector <= 26 => {
                        self.owned_perk_selectors.remove(index);
                        Ok(())
                    }
                    _ => Err(Rejection::InvalidOffer),
                }
            }
            HubInventoryAction::Dowse { reference_item_id } => {
                self.dowse(*reference_item_id).map(|pitch| result.dowsing_pitch = Some(pitch))
            }
            HubInventoryAction::BuyDowsing { offer_id } => {
                self.buy_dowsing(*offer_id).map(|pitch| result.dowsing_pitch = Some(pitch))
            }
            HubInventoryAction::CloseDowsing => {
                if self.dowsing_rolled {
                    self.dowsing_offers.clear();
                    self.dowsing_rolled = false;
                    self.revision += 1;
                }
                return result;
            }
            HubInventoryAction::Move { item_id, destination_sack_id, destination_slot } => {
                self.move_item(*item_id, *destination_sack_id, *destination_slot)
            }
            HubInventoryAction::RenameSack { item_id, name } => self.rename_sack(*item_id, name),
            HubInventoryAction::Transfer { item_id, direction, .. } => {
                self.transfer_item(*item_id, *direction)
            }
            HubInventoryAction::Equip { item_id, slot } => {
                self.equip_item(*item_id, *slot, context)
            }
            HubInventoryAction::Unequip { slot, destination_sack_id, destination_slot } => {
                self.unequip(*slot, *destination_sack_id, *destination_slot)
            }
            HubInventoryAction::EquipSackContents { item_id } => {
                self.equip_sack_contents(*item_id, context)
            }
            HubInventoryAction::Consume { item_id } => self.consume_item(*item_id),
            HubInventoryAction::ReadSkillBook { item_id } => self.read_skill_book(*item_id),
            HubInventoryAction::Discard { item_id } => {
                self.require_valid_inventory().and_then(|()| {
                    self.backpack = remove_tree(&self.backpack, *item_id, None)
                        .ok_or(Rejection::ItemNotFound)?
                        .items;
                    Ok(())
                })
            }
            HubInventoryAction::OpenDye { dye_item_id, session_id } => self
                .begin_dye_session(*dye_item_id, session_id)
                .map(|session| result.dye_session = Some(session)),
            // The Session removes only the matching active ID. Closing changes
            // no inventory; its caller still records the accepted action receipt.
            HubInventoryAction::CloseDye { .. } => return result,
            HubInventoryAction::Dye { session_id, target_item_id, layer, swatch_rows } => {
                let Some(session) = dye_session.filter(|session| session.id == *session_id) else {
                    return HubEconomyResult::rejected(Rejection::InvalidTarget);
                };
                result.dye_session = Some(session.clone());
                self.dye_clothing(session, *target_item_id, *layer, swatch_rows).map(|()| {
                    let mut painted = session.clone();
                    painted.kit_consumed = true;
                    result.dye_session = Some(painted);
                })
            }
            HubInventoryAction::Unforge { item_id } => self
                .unforge(*item_id, context)
                .map(|outcome| result.unforge_outcome = Some(outcome)),
            HubInventoryAction::SelectBoast { boast_id } => {
                if crate::modding::extensions::game_boast_rules(boast_id, extensions).is_none() {
                    Err(Rejection::InvalidOffer)
                } else {
                    self.npc.boast.selected = Some(boast_id.clone());
                    self.npc.boast.succeeded = false;
                    Ok(())
                }
            }
            HubInventoryAction::ReadLibrarianBook { book_id } => {
                match NATIVE_LIBRARIAN_BOOKS.iter().find(|book| book.id == *book_id) {
                    None => Err(Rejection::InvalidOffer),
                    Some(book) if book.one_shot && self.npc.librarian_lace_read => {
                        Err(Rejection::InvalidOffer)
                    }
                    Some(book) => {
                        if book.one_shot {
                            self.npc.librarian_lace_read = true;
                        }
                        Ok(())
                    }
                }
            }
            HubInventoryAction::BuyTeacherSpell { skill_id } => {
                // Prices follow the authored teacherSpells order, not a numeric-id formula.
                let price = NATIVE_TEACHER_SPELLS
                    .iter()
                    .find(|spell| spell.skill_id == *skill_id)
                    .map(|spell| spell.price);
                match price {
                    None => Err(Rejection::InvalidOffer),
                    Some(_) if context.advanced_unlocks[*skill_id - 72] => {
                        Err(Rejection::InvalidOffer)
                    }
                    Some(price) if self.gold < price => Err(Rejection::InsufficientGold),
                    Some(price) => {
                        self.gold -= price;
                        Ok(())
                    }
                }
            }
        };
        match operation {
            Ok(()) => {
                self.revision += 1;
                result
            }
            Err(reason) => {
                let mut rejected = HubEconomyResult::rejected(reason);
                rejected.dye_session = result.dye_session;
                rejected
            }
        }
    }

    pub(super) fn roll_fomentius_stock(&mut self) {
        self.fomentius_stock.clear();
        for definition in &FOMENTIUS_STOCK_DEFINITIONS {
            let draw = self.rng.draw_integer(definition.roll_bound as i32) as u32;
            let quantity = match definition.gate_value {
                Some(gate) => u32::from(draw == gate),
                None => draw + definition.quantity_offset,
            };
            for _ in 0..quantity {
                self.fomentius_stock.push(HubShopItem {
                    item: create_fomentius_inventory_item(definition, self.next_item_id, 1),
                    price: definition.price,
                });
                self.next_item_id += 1;
            }
        }
    }

    pub fn restock_fomentius(&mut self) {
        self.roll_fomentius_stock();
        self.revision += 1;
    }

    fn buy_fomentius(&mut self, item_id: u64) -> Result<(), Rejection> {
        let index = self
            .fomentius_stock
            .iter()
            .position(|stock| stock.item.id == item_id)
            .ok_or(Rejection::InvalidOffer)?;
        let stock = &self.fomentius_stock[index];
        if self.gold < stock.price {
            return Err(Rejection::InsufficientGold);
        }
        let backpack = insert(&self.backpack, stock.item.clone(), HUB_INVENTORY_SLOT_CAPACITY)
            .ok_or(Rejection::CapacityFull)?;
        self.gold -= stock.price;
        self.backpack = backpack;
        self.fomentius_stock.remove(index);
        Ok(())
    }

    #[must_use]
    pub fn hagatha_offers(&self) -> Vec<HagathaOffer> {
        let mut offers = Vec::new();
        for perk in &HAGATHA_PERKS {
            if perk.selector == 8
                || if perk.selector == 27 {
                    self.tonic_purchases >= 2
                } else {
                    self.owned_perk_selectors.contains(&perk.selector)
                }
            {
                continue;
            }
            offers.push(HagathaOffer {
                selector: perk.selector as i32,
                name: perk.name.to_owned(),
                description: perk.description.to_owned(),
                members: vec![perk.selector],
                price: self.individual_perk_price(perk.selector),
            });
        }
        if !self.hagatha_bundle_selectors.is_empty() {
            let sum: u64 = self
                .hagatha_bundle_selectors
                .iter()
                .map(|selector| self.individual_perk_price(*selector))
                .sum();
            offers.push(HagathaOffer {
                selector: -1,
                name: "BARGAIN BUNDLE".to_owned(),
                description: "A bargain bundle mixed from the selected charms and curses."
                    .to_owned(),
                members: self.hagatha_bundle_selectors.clone(),
                price: sum.div_ceil(2),
            });
        }
        offers
    }

    fn individual_perk_price(&self, selector: u32) -> u64 {
        let perk = &HAGATHA_PERKS[selector as usize];
        if self.first_mixed_selectors.contains(&selector) {
            perk.base_price
        } else {
            perk.base_price * 3
        }
    }

    fn buy_hagatha(&mut self, selector: i32) -> Result<(), Rejection> {
        let offer = self
            .hagatha_offers()
            .into_iter()
            .find(|offer| offer.selector == selector)
            .ok_or(Rejection::InvalidOffer)?;
        if self.gold < offer.price {
            return Err(Rejection::InsufficientGold);
        }
        if !self.perks_fit_capacity(&offer.members) {
            return Err(Rejection::PerkCapacityFull);
        }
        for member in offer.members {
            if !self.first_mixed_selectors.contains(&member) {
                self.first_mixed_selectors.push(member);
            }
            if member == 27 {
                if self.tonic_purchases < 2 {
                    self.tonic_purchases += 1;
                    self.charm_capacity = 9.min(self.charm_capacity + 3);
                    self.owned_perk_selectors.push(member);
                }
            } else if !self.owned_perk_selectors.contains(&member) {
                self.owned_perk_selectors.push(member);
            }
        }
        self.first_mixed_selectors.sort_unstable();
        self.gold -= offer.price;
        if selector == -1 {
            self.hagatha_bundle_selectors.clear();
        }
        Ok(())
    }

    fn perks_fit_capacity(&self, selectors: &[u32]) -> bool {
        if !native_hagatha_outcome_state_is_valid(
            &self.owned_perk_selectors,
            self.tonic_purchases,
            self.charm_capacity,
        ) || self.owned_perk_selectors.len() >= self.charm_capacity as usize
        {
            return false;
        }
        let mut outcomes = self.owned_perk_selectors.clone();
        let mut tonic_purchases = self.tonic_purchases;
        let mut capacity = self.charm_capacity;
        for selector in selectors {
            if *selector == 27 {
                if tonic_purchases >= 2 {
                    return false;
                }
                tonic_purchases += 1;
                capacity = 9.min(capacity + 3);
                outcomes.push(*selector);
            } else if !outcomes.contains(selector) {
                outcomes.push(*selector);
            }
        }
        native_hagatha_outcome_state_is_valid(&outcomes, tonic_purchases, capacity)
    }

    fn dowse(&mut self, reference_item_id: Option<u64>) -> Result<f64, Rejection> {
        if self.dowsing_rolled {
            return Err(Rejection::OffersActive);
        }
        let reference = if let Some(id) = reference_item_id {
            let item = find_inventory_item(&self.backpack, id)
                .or_else(|| {
                    super::EquipmentSlot::ALL.iter().find_map(|slot| {
                        self.equipment.at(*slot).as_ref().filter(|item| item.id == id)
                    })
                })
                .ok_or(Rejection::ItemNotFound)?;
            Some((item.native_type_id, item.recipe_index))
        } else {
            None
        };
        if self.gold < self.dowsing_fee {
            return Err(Rejection::InsufficientGold);
        }
        let mut owned_recipes = std::collections::BTreeSet::new();
        for entry in project_inventory_items(&self.backpack)
            .into_iter()
            .chain(project_inventory_items(&self.storage))
        {
            if let Some(index) = entry.item.recipe_index {
                owned_recipes.insert(index);
            }
        }
        for slot in super::EquipmentSlot::ALL {
            if let Some(index) = self.equipment.at(slot).as_ref().and_then(|item| item.recipe_index)
            {
                owned_recipes.insert(index);
            }
        }
        let pitch = self.rng.draw_scaled_float(0.1, false);
        let requested_count = self.rng.draw_integer(2) as usize + 3;
        let mut selected = Vec::new();
        if let Some((native_type_id, recipe_index)) = reference {
            let set_name = recipe_index.and_then(|index| DOWSING_EQUIPMENT_RECIPES[index].set_name);
            for slot in 0..requested_count + 2 {
                let mut candidates = Vec::new();
                for recipe in &DOWSING_EQUIPMENT_RECIPES {
                    if owned_recipes.contains(&recipe.source_index)
                        || selected.contains(&recipe.source_index)
                    {
                        continue;
                    }
                    let matches_reference = if slot < 2 {
                        set_name.is_some() && recipe.set_name == set_name
                    } else {
                        recipe.native_type_id == native_type_id
                    };
                    if matches_reference {
                        candidates.push(recipe.source_index);
                    }
                }
                if !candidates.is_empty() {
                    let choice = self.rng.draw_integer(candidates.len() as i32) as usize;
                    selected.push(candidates[choice]);
                }
            }
        } else {
            for _ in 0..requested_count {
                for attempt in 1..=100 {
                    let index =
                        self.rng.draw_integer(DOWSING_EQUIPMENT_RECIPES.len() as i32) as usize;
                    if owned_recipes.contains(&index) || selected.contains(&index) {
                        continue;
                    }
                    // Retail rejects a successful hundredth draw as well. Every
                    // recipe is selected before any offer consumes its price draw.
                    if attempt < 100 {
                        selected.push(index);
                    }
                    break;
                }
            }
        }
        let mut offers = Vec::new();
        for recipe_index in selected {
            let price = (self.rng.draw_integer(15) as u64 + 100) * 50;
            offers.push(DowsingOffer { id: self.next_offer_id, price, recipe_index });
            self.next_offer_id += 1;
        }
        self.dowsing_offers = offers;
        self.dowsing_rolled = true;
        self.gold -= self.dowsing_fee;
        Ok(fround(0.8 + pitch))
    }

    fn buy_dowsing(&mut self, offer_id: u64) -> Result<f64, Rejection> {
        let offer = self
            .dowsing_offers
            .iter()
            .find(|offer| offer.id == offer_id)
            .ok_or(Rejection::InvalidOffer)?;
        if self.gold < offer.price {
            return Err(Rejection::InsufficientGold);
        }
        let recipe =
            DOWSING_EQUIPMENT_RECIPES.get(offer.recipe_index).ok_or(Rejection::InvalidOffer)?;
        let item = create_equipment_inventory_item(recipe, self.next_item_id);
        self.backpack = insert(&self.backpack, item, HUB_INVENTORY_SLOT_CAPACITY)
            .ok_or(Rejection::CapacityFull)?;
        self.gold -= offer.price;
        self.dowsing_fee = (self.rng.draw_integer(10) as u64 + 10) * 50;
        let pitch = self.rng.draw_scaled_float(0.1, false);
        self.dowsing_offers.clear();
        self.next_item_id += 1;
        Ok(fround(1.0 + pitch))
    }

    fn consume_item(&mut self, item_id: u64) -> Result<(), Rejection> {
        self.require_valid_inventory()?;
        let item = find_inventory_item(&self.backpack, item_id).ok_or(Rejection::ItemNotFound)?;
        if item.native_type_id != 7001 || item.native_subtype.is_none() {
            return Err(Rejection::IneligibleItem);
        }
        self.backpack = consume_tree(&self.backpack, item_id).ok_or(Rejection::ItemNotFound)?;
        Ok(())
    }

    fn read_skill_book(&mut self, item_id: u64) -> Result<(), Rejection> {
        self.require_valid_inventory()?;
        let item = find_inventory_item(&self.backpack, item_id).ok_or(Rejection::ItemNotFound)?;
        if item.kind != HubItemKind::SkillBook
            || item.native_type_id != 7012
            || !matches!(item.native_subtype, Some(2 | 3))
        {
            return Err(Rejection::IneligibleItem);
        }
        self.backpack = consume_tree(&self.backpack, item_id).ok_or(Rejection::ItemNotFound)?;
        Ok(())
    }

    /// `beginInventoryDyeSession`: opening validates ownership without consuming a kit.
    pub fn begin_inventory_dye_session(
        &mut self,
        dye_item_id: u64,
        session_id: &str,
    ) -> HubEconomyResult {
        self.apply(
            &HubInventoryAction::OpenDye { dye_item_id, session_id: session_id.into() },
            &HubEconomyContext::default(),
        )
    }

    /// `dyeInventoryClothing`: the first committed paint consumes one initiating kit.
    pub fn dye_inventory_clothing(
        &mut self,
        session: &NativeInventoryDyeSession,
        target_item_id: u64,
        layer: NativeDyeLayer,
        swatch_rows: &[usize],
    ) -> HubEconomyResult {
        self.apply_with_dye_session(
            &HubInventoryAction::Dye {
                session_id: session.id.clone(),
                target_item_id,
                layer,
                swatch_rows: swatch_rows.to_vec(),
            },
            &HubEconomyContext::default(),
            None,
            Some(session),
        )
    }

    fn begin_dye_session(
        &self,
        dye_item_id: u64,
        session_id: &str,
    ) -> Result<NativeInventoryDyeSession, Rejection> {
        // Website length is UTF-16 code units, not UTF-8 bytes or Unicode scalars.
        let length = session_id.encode_utf16().count();
        if length == 0 || length > 64 {
            return Err(Rejection::InvalidTarget);
        }
        self.require_valid_inventory()?;
        let dye =
            find_inventory_item(&self.backpack, dye_item_id).ok_or(Rejection::ItemNotFound)?;
        if dye.native_type_id != 7012
            || dye.native_subtype != Some(0)
            || dye.kind != HubItemKind::Dye
        {
            return Err(Rejection::IneligibleItem);
        }
        Ok(NativeInventoryDyeSession { id: session_id.into(), dye_item_id, kit_consumed: false })
    }

    fn dye_clothing(
        &mut self,
        session: &NativeInventoryDyeSession,
        target_item_id: u64,
        layer: NativeDyeLayer,
        rows: &[usize],
    ) -> Result<(), Rejection> {
        self.require_valid_inventory()?;
        if !valid_dye_rows(rows) {
            return Err(Rejection::InvalidTarget);
        }
        if !session.kit_consumed {
            let dye = find_inventory_item(&self.backpack, session.dye_item_id)
                .ok_or(Rejection::ItemNotFound)?;
            if dye.native_type_id != 7012
                || dye.native_subtype != Some(0)
                || dye.kind != HubItemKind::Dye
            {
                return Err(Rejection::IneligibleItem);
            }
        }
        let mut target = find_inventory_item(&self.backpack, target_item_id)
            .ok_or(Rejection::ItemNotFound)?
            .clone();
        if !target.is_dyeable_clothing() {
            return Err(Rejection::InvalidTarget);
        }
        let mut tints = target.clothing_tints().ok_or(Rejection::InvalidTarget)?;
        let committed = native_dye_committed_tint(rows).ok_or(Rejection::InvalidTarget)?;
        tints[if layer == NativeDyeLayer::Cloth { 0 } else { 1 }] = committed;
        target.icon_tints = Some(tints.map(Some));
        self.backpack =
            replace_tree(&self.backpack, target_item_id, target).ok_or(Rejection::InvalidTarget)?;
        if !session.kit_consumed {
            self.backpack =
                consume_tree(&self.backpack, session.dye_item_id).ok_or(Rejection::ItemNotFound)?;
        }
        Ok(())
    }

    fn unforge(
        &mut self,
        item_id: u64,
        context: &HubEconomyContext,
    ) -> Result<NativeUnforgeOutcome, Rejection> {
        self.require_valid_inventory()?;
        let item =
            find_inventory_item(&self.backpack, item_id).ok_or(Rejection::ItemNotFound)?.clone();
        if !item.can_unforge()
            || (item.native_type_id == 7008
                && item.contents.as_ref().is_some_and(|contents| !contents.is_empty()))
        {
            return Err(Rejection::IneligibleItem);
        }
        let recipe_backed = item.recipe_index.is_some() || item.native_effects.is_some();
        let (kind, amount) = if item.native_type_id == 7008 || !recipe_backed {
            let amount = self.rng.draw_integer(4) as u64 + 2;
            self.gold += amount;
            (UnforgeKind::Gold, Some(amount as f64))
        } else {
            loop {
                self.unforge_bonuses.recipe_attempt_count += 1;
                let attempts = self.unforge_bonuses.recipe_attempt_count;
                let bound = if attempts < 5 { 7 } else { attempts + 3 };
                let mut selector = self.rng.draw_integer(bound as i32);
                if selector > 7 {
                    if self.rng.draw_integer(6) != 3 {
                        break (UnforgeKind::Fizzle, None);
                    }
                    selector = 7;
                }
                match selector {
                    0 => {
                        let needs_rejuvenation = context.vitals.current_health
                            < context.vitals.maximum_health
                            || context.vitals.current_mana < context.vitals.maximum_mana;
                        if attempts <= 5 && !needs_rejuvenation {
                            continue;
                        }
                        break (UnforgeKind::FullRejuvenation, None);
                    }
                    1 | 2 => {
                        let amount =
                            if attempts < 5 && self.rng.draw_integer(3) == 1 { 2.0 } else { 1.0 };
                        if selector == 1 {
                            self.unforge_bonuses.offensive_damage += amount;
                            break (UnforgeKind::OffensiveDamage, Some(amount));
                        }
                        self.unforge_bonuses.mana_cost_reduction += amount;
                        break (UnforgeKind::ManaCost, Some(amount));
                    }
                    3 => {
                        if self.rng.draw_integer(100) != 25 {
                            continue;
                        }
                        break (UnforgeKind::MindDredge, Some(1.0));
                    }
                    4 | 5 => {
                        let maximum = if selector == 4 { 10.0 } else { 20.0 };
                        let amount = if attempts >= 5 && self.rng.draw_integer(4) != 1 {
                            maximum / 2.0
                        } else {
                            maximum
                        };
                        if selector == 4 {
                            self.unforge_bonuses.maximum_health += amount;
                            break (UnforgeKind::MaximumHealth, Some(amount));
                        }
                        self.unforge_bonuses.maximum_mana += amount;
                        break (UnforgeKind::MaximumMana, Some(amount));
                    }
                    6 => {
                        let draw = self.rng.draw_integer(2);
                        let amount = if attempts <= 4 {
                            if draw == 1 { 10.0 } else { 5.0 }
                        } else if draw == 1 {
                            2.0
                        } else {
                            1.0
                        };
                        self.unforge_bonuses.experience =
                            fround(self.unforge_bonuses.experience + amount / 100.0);
                        break (UnforgeKind::Experience, Some(amount));
                    }
                    _ => {
                        let amount = (self.rng.draw_integer(6) as u64 + 1) * 10;
                        self.gold += amount;
                        break (UnforgeKind::Gold, Some(amount as f64));
                    }
                }
            }
        };
        self.backpack =
            remove_tree(&self.backpack, item_id, None).ok_or(Rejection::ItemNotFound)?.items;
        Ok(NativeUnforgeOutcome { amount, item_name: item.name, kind })
    }
}

/// The ordered native perk list, repeated Tonic purchases and capacity must agree.
#[must_use]
pub fn native_hagatha_outcome_state_is_valid(
    outcomes: &[u32],
    tonic_purchases: u32,
    capacity: u32,
) -> bool {
    if tonic_purchases > 2
        || capacity != 3 + tonic_purchases * 3
        || outcomes.len() > capacity as usize
    {
        return false;
    }
    let mut ordinary = Vec::new();
    let mut tonics = 0;
    for selector in outcomes {
        if *selector > 27 || *selector == 8 {
            return false;
        }
        if *selector == 27 {
            tonics += 1;
        } else if ordinary.contains(selector) {
            return false;
        } else {
            ordinary.push(*selector);
        }
    }
    tonics == tonic_purchases
}

/// A disk bundle carries its Tonic count implicitly in its ordered selectors.
#[must_use]
pub fn native_hagatha_bundle_state_is_valid(outcomes: &[u32]) -> bool {
    let tonics = outcomes.iter().filter(|selector| **selector == 27).count();
    if tonics > 2 {
        return false;
    }
    let tonics = tonics as u32;
    native_hagatha_outcome_state_is_valid(outcomes, tonics, 3 + tonics * 3)
}
