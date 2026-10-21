//! Addressed inventory roots and nested sacks from Website `core-kernels/hub-economy.ts`.

use std::collections::HashSet;

use crate::appearance::StarterEquipmentAppearance;
use crate::modding::inventory::mod_item_inventory_identity_is_valid;

use super::{
    BoastSelection, BoastState, DOWSING_EQUIPMENT_RECIPES, EquipmentRecipe, EquipmentSlot,
    EquipmentType, FomentiusStockDefinition, HUB_INVENTORY_SLOT_CAPACITY,
    HUB_SACK_CHILD_REPLICATION_LIMIT, HUB_SACK_REPLICATION_DEPTH_LIMIT, HUB_STORAGE_SLOT_CAPACITY,
    HubEconomyContext, HubEconomyRejection as Rejection, HubEconomyResult, HubEconomyState,
    HubEquipmentState, HubInventoryItem, HubItemKind, NATIVE_LOOT_BACKPACK_REPLICATION_LIMIT,
    NativeSkillBookDefinition, TransferDirection,
};

/// Website Sack naming limit, measured in UTF-16 code units. Existing names are retained.
pub const INVENTORY_SACK_NAME_MAX_LENGTH: usize = 32;

/// `normalizeInventorySackName`: reject controls before applying JavaScript trim.
#[must_use]
pub fn normalize_inventory_sack_name(value: &str) -> Option<&str> {
    if value.chars().any(char::is_control) {
        return None;
    }
    let name = value.trim_matches(crate::social::javascript_whitespace);
    if name.is_empty() || name.encode_utf16().count() > INVENTORY_SACK_NAME_MAX_LENGTH {
        return None;
    }
    if name.chars().all(|character| character.is_whitespace() || unicode_format(character)) {
        return None;
    }
    Some(name)
}

/// Source's Unicode 16 `\p{Cf}` gate rejects names made entirely of invisible formatting.
/// The oracle retains every member of that category, including supplementary characters.
fn unicode_format(character: char) -> bool {
    matches!(
        character,
        '\u{00ad}'
            | '\u{0600}'..='\u{0605}'
            | '\u{061c}'
            | '\u{06dd}'
            | '\u{070f}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08e2}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{110bd}'
            | '\u{110cd}'
            | '\u{13430}'..='\u{1343f}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            | '\u{e0001}'
            | '\u{e0020}'..='\u{e007f}'
    )
}

impl HubInventoryItem {
    #[must_use]
    pub fn is_dyeable_clothing(&self) -> bool {
        matches!(
            (self.native_type_id, self.equipment_type),
            (7005, Some(EquipmentType::Hat)) | (7006, Some(EquipmentType::Robe))
        ) || self
            .mod_item_content
            .as_ref()
            .and_then(|content| content.wearable.as_ref())
            .is_some_and(|wearable| wearable.dyeable)
            && matches!(self.equipment_type, Some(EquipmentType::Hat | EquipmentType::Robe))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProjectedInventoryRootSlot<'a> {
    pub item: &'a HubInventoryItem,
    pub slot: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct ProjectedInventoryItem<'a> {
    pub depth: usize,
    pub item: &'a HubInventoryItem,
    pub parent_sack_id: Option<u64>,
    pub slot: usize,
}

/// `projectInventoryRootSlots`: old unslotted saves occupy the first unreserved cells.
#[must_use]
pub fn project_inventory_root_slots(
    source: &[HubInventoryItem],
) -> Vec<ProjectedInventoryRootSlot<'_>> {
    let explicitly_occupied: HashSet<_> =
        source.iter().filter_map(|item| item.inventory_slot).collect();
    let mut assigned = HashSet::new();
    let mut next_legacy_slot = 0;
    let mut projected = Vec::with_capacity(source.len());
    for item in source {
        let slot = match item.inventory_slot {
            Some(slot) => slot,
            None => {
                while explicitly_occupied.contains(&next_legacy_slot)
                    || assigned.contains(&next_legacy_slot)
                {
                    next_legacy_slot += 1;
                }
                let slot = next_legacy_slot;
                next_legacy_slot += 1;
                slot
            }
        };
        assigned.insert(slot);
        projected.push(ProjectedInventoryRootSlot { item, slot });
    }
    projected.sort_by_key(|entry| entry.slot);
    projected
}

#[must_use]
pub fn project_inventory_items(source: &[HubInventoryItem]) -> Vec<ProjectedInventoryItem<'_>> {
    fn visit<'a>(
        source: &'a [HubInventoryItem],
        depth: usize,
        parent_sack_id: Option<u64>,
        seen: &mut HashSet<u64>,
        output: &mut Vec<ProjectedInventoryItem<'a>>,
    ) {
        for entry in project_inventory_root_slots(source) {
            if !seen.insert(entry.item.id) {
                continue;
            }
            output.push(ProjectedInventoryItem {
                item: entry.item,
                slot: entry.slot,
                depth,
                parent_sack_id,
            });
            if entry.item.native_type_id != 7008 {
                continue;
            }
            let Some(contents) = &entry.item.contents else {
                continue;
            };
            visit(contents, depth + 1, Some(entry.item.id), seen, output);
        }
    }
    let mut output = Vec::new();
    visit(source, 0, None, &mut HashSet::new(), &mut output);
    output
}

#[must_use]
pub fn find_inventory_item(source: &[HubInventoryItem], item_id: u64) -> Option<&HubInventoryItem> {
    project_inventory_items(source)
        .into_iter()
        .find(|entry| entry.item.id == item_id)
        .map(|entry| entry.item)
}

#[must_use]
pub fn inventory_items_at_sack_path<'a>(
    source: &'a [HubInventoryItem],
    sack_path: &[u64],
) -> Option<&'a [HubInventoryItem]> {
    let mut current = source;
    for sack_id in sack_path {
        let sack = current.iter().find(|item| {
            item.id == *sack_id && item.kind == HubItemKind::Sack && item.native_type_id == 7008
        })?;
        current = sack.contents.as_deref().unwrap_or(&[]);
    }
    Some(current)
}

#[must_use]
pub fn reconcile_inventory_sack_path(source: &[HubInventoryItem], sack_path: &[u64]) -> Vec<u64> {
    let mut current = source;
    let mut reconciled = Vec::new();
    for sack_id in sack_path {
        let Some(sack) = current.iter().find(|item| {
            item.id == *sack_id && item.kind == HubItemKind::Sack && item.native_type_id == 7008
        }) else {
            break;
        };
        reconciled.push(*sack_id);
        current = sack.contents.as_deref().unwrap_or(&[]);
    }
    reconciled
}

pub(crate) fn base_item(
    id: u64,
    kind: HubItemKind,
    name: &str,
    native_type_id: u32,
    native_subtype: Option<u32>,
    icons: &[usize],
) -> HubInventoryItem {
    HubInventoryItem {
        contents: None,
        equipment_type: None,
        generated_level: None,
        icon_records: icons.to_vec(),
        icon_tints: None,
        id,
        inventory_slot: None,
        kind,
        mod_content: None,
        mod_item_content: None,
        mod_affixes: None,
        name: name.to_owned(),
        native_subtype,
        native_selector: None,
        native_effects: None,
        native_type_id,
        quantity: 1,
        rarity: None,
        recipe_index: None,
    }
}

#[must_use]
pub fn create_equipment_inventory_item(recipe: &EquipmentRecipe, id: u64) -> HubInventoryItem {
    let mut item = base_item(
        id,
        HubItemKind::Equipment,
        recipe.name,
        recipe.native_type_id,
        None,
        recipe.icon_records,
    );
    item.equipment_type = Some(recipe.equipment_type);
    item.rarity = Some(recipe.rarity);
    item.recipe_index = Some(recipe.source_index);
    item
}

#[must_use]
pub fn create_fomentius_inventory_item(
    definition: &FomentiusStockDefinition,
    id: u64,
    quantity: u32,
) -> HubInventoryItem {
    assert!(quantity > 0, "a stock inventory item has positive quantity");
    let mut item = base_item(
        id,
        definition.kind,
        definition.name,
        definition.native_type_id,
        Some(definition.native_subtype),
        definition.icon_records,
    );
    item.quantity = quantity;
    item
}

#[must_use]
pub fn create_native_skill_book_inventory_item(
    definition: &NativeSkillBookDefinition,
    id: u64,
) -> HubInventoryItem {
    base_item(
        id,
        HubItemKind::SkillBook,
        definition.name,
        definition.native_type_id,
        Some(definition.native_subtype),
        definition.icon_records,
    )
}

pub(super) fn starter_loadout(
    first_id: u64,
    appearance: StarterEquipmentAppearance,
) -> (Vec<HubInventoryItem>, HubEquipmentState, u64) {
    let mut health =
        base_item(first_id, HubItemKind::HealthPotion, "Health Potion", 7001, Some(0), &[46]);
    health.inventory_slot = Some(0);
    let mut mana =
        base_item(first_id + 1, HubItemKind::ManaPotion, "Mana Potion", 7001, Some(1), &[47]);
    mana.inventory_slot = Some(1);
    let mut equipment = HubEquipmentState::default();
    for (offset, slot, equipment_type, name, native_type_id, icons) in [
        (2, EquipmentSlot::Hat, EquipmentType::Hat, "Hat", 7005, &[34, 38][..]),
        (3, EquipmentSlot::Robe, EquipmentType::Robe, "Robe", 7006, &[64, 67][..]),
        (4, EquipmentSlot::Weapon, EquipmentType::Staff, "Staff", 7004, &[72][..]),
    ] {
        let mut item =
            base_item(first_id + offset, HubItemKind::Equipment, name, native_type_id, None, icons);
        item.equipment_type = Some(equipment_type);
        if equipment_type != EquipmentType::Staff {
            item.icon_tints =
                Some([Some(appearance.primary_tint), Some(appearance.secondary_tint)]);
        }
        equipment.set(slot, Some(item));
    }
    (vec![health, mana], equipment, first_id + 5)
}

pub(super) fn materialize(source: &[HubInventoryItem]) -> Vec<HubInventoryItem> {
    let mut items = Vec::with_capacity(source.len());
    for entry in project_inventory_root_slots(source) {
        let mut item = entry.item.clone();
        item.inventory_slot = Some(entry.slot);
        item.contents = item.contents.as_ref().map(|contents| materialize(contents));
        items.push(item);
    }
    items
}

/// `inventoryItemsShareStack`: the same predicate used by authoritative insert/move.
#[must_use]
pub fn inventory_items_share_stack(left: &HubInventoryItem, right: &HubInventoryItem) -> bool {
    if left.native_type_id != right.native_type_id {
        return false;
    }
    if let Some(content) = &right.mod_item_content {
        return left
            .mod_item_content
            .as_ref()
            .is_some_and(|left| left.content_id == content.content_id);
    }
    if let Some(content) = &right.mod_content {
        return left.mod_content.as_ref().is_some_and(|left| left.content_id == content.content_id);
    }
    (right.native_type_id == 7001 || right.kind == HubItemKind::ModItem)
        && left.native_subtype == right.native_subtype
}

fn stack_maximum(item: &HubInventoryItem) -> u32 {
    item.mod_item_content.as_ref().map_or(9_999, |content| content.stack_maximum)
}

pub(super) fn insert(
    source: &[HubInventoryItem],
    mut item: HubInventoryItem,
    capacity: usize,
) -> Option<Vec<HubInventoryItem>> {
    let mut destination = materialize(source);
    if (item.native_type_id == 7001 || item.kind == HubItemKind::ModItem)
        && let Some(stack) =
            destination.iter_mut().find(|entry| inventory_items_share_stack(entry, &item))
    {
        let quantity = stack.quantity.checked_add(item.quantity)?;
        if quantity > stack_maximum(&item) {
            return None;
        }
        stack.quantity = quantity;
        return Some(destination);
    }
    let occupied: HashSet<_> =
        destination.iter().filter_map(|entry| entry.inventory_slot).collect();
    let slot = (0..capacity).find(|slot| !occupied.contains(slot))?;
    item.inventory_slot = Some(slot);
    destination.push(item);
    Some(destination)
}

fn insert_at(
    source: &[HubInventoryItem],
    mut item: HubInventoryItem,
    slot: usize,
    capacity: usize,
) -> Option<Vec<HubInventoryItem>> {
    if slot >= capacity {
        return None;
    }
    let mut destination = materialize(source);
    if let Some(resident) = destination.iter_mut().find(|entry| entry.inventory_slot == Some(slot))
    {
        if !inventory_items_share_stack(resident, &item) {
            return None;
        }
        let quantity = resident.quantity.checked_add(item.quantity)?;
        if quantity > stack_maximum(&item) {
            return None;
        }
        resident.quantity = quantity;
    } else {
        item.inventory_slot = Some(slot);
        destination.push(item);
    }
    Some(destination)
}

/// `relocateInventoryRootItem`: an occupied cell swaps addresses unless both items stack.
fn relocate_root_item(
    source: &[HubInventoryItem],
    item_id: u64,
    destination_slot: usize,
) -> Result<Vec<HubInventoryItem>, Rejection> {
    let mut items = materialize(source);
    let moving =
        items.iter().find(|item| item.id == item_id).ok_or(Rejection::InvalidTarget)?.clone();
    if moving.inventory_slot == Some(destination_slot) {
        return Err(Rejection::InvalidTarget);
    }
    let resident = items.iter().find(|item| item.inventory_slot == Some(destination_slot)).cloned();
    if let Some(resident) =
        resident.as_ref().filter(|resident| inventory_items_share_stack(resident, &moving))
    {
        let quantity =
            resident.quantity.checked_add(moving.quantity).ok_or(Rejection::CapacityFull)?;
        if quantity > stack_maximum(&moving) {
            return Err(Rejection::CapacityFull);
        }
        items.retain(|item| item.id != moving.id);
        items
            .iter_mut()
            .find(|item| item.id == resident.id)
            .expect("the resident stack")
            .quantity = quantity;
    } else {
        for item in &mut items {
            if item.id == moving.id {
                item.inventory_slot = Some(destination_slot);
            } else if resident.as_ref().is_some_and(|resident| resident.id == item.id) {
                item.inventory_slot = moving.inventory_slot;
            }
        }
    }
    Ok(items)
}

pub(super) struct RemovedItem {
    pub item: HubInventoryItem,
    pub items: Vec<HubInventoryItem>,
    pub parent_sack_id: Option<u64>,
}

pub(super) fn remove_tree(
    source: &[HubInventoryItem],
    item_id: u64,
    parent_sack_id: Option<u64>,
) -> Option<RemovedItem> {
    let mut items = materialize(source);
    for index in 0..items.len() {
        if items[index].id == item_id {
            let item = items.remove(index);
            return Some(RemovedItem { item, items, parent_sack_id });
        }
        if items[index].native_type_id != 7008 {
            continue;
        }
        let Some(contents) = &items[index].contents else {
            continue;
        };
        if let Some(nested) = remove_tree(contents, item_id, Some(items[index].id)) {
            items[index].contents = Some(nested.items);
            return Some(RemovedItem {
                item: nested.item,
                items,
                parent_sack_id: nested.parent_sack_id,
            });
        }
    }
    None
}

pub(super) fn replace_tree(
    source: &[HubInventoryItem],
    item_id: u64,
    replacement: HubInventoryItem,
) -> Option<Vec<HubInventoryItem>> {
    let mut items = materialize(source);
    for index in 0..items.len() {
        if items[index].id == item_id {
            let slot = items[index].inventory_slot;
            items[index] = replacement;
            items[index].inventory_slot = slot;
            return Some(items);
        }
        if items[index].native_type_id != 7008 {
            continue;
        }
        let Some(contents) = &items[index].contents else {
            continue;
        };
        if let Some(contents) = replace_tree(contents, item_id, replacement.clone()) {
            items[index].contents = Some(contents);
            return Some(items);
        }
    }
    None
}

pub(super) fn consume_tree(
    source: &[HubInventoryItem],
    item_id: u64,
) -> Option<Vec<HubInventoryItem>> {
    let mut item = find_inventory_item(source, item_id)?.clone();
    if item.quantity > 1 {
        item.quantity -= 1;
        return replace_tree(source, item_id, item);
    }
    Some(remove_tree(source, item_id, None)?.items)
}

impl HubEconomyState {
    /// `grantPlayerEntityInventoryItems`: the complete addressed batch commits
    /// only if insertion and the existing replication limits admit every item.
    pub fn grant_inventory_items(&mut self, items: &[HubInventoryItem]) -> bool {
        if items.is_empty() {
            return false;
        }
        let mut candidate = self.clone();
        for item in items {
            if !candidate.insert_loot_item(item.clone()) {
                return false;
            }
        }
        if project_inventory_items(&candidate.backpack).len()
            > NATIVE_LOOT_BACKPACK_REPLICATION_LIMIT
            || !candidate.inventory_is_valid()
        {
            return false;
        }
        *self = candidate;
        true
    }

    /// Source package reconciliation removes only content whose owner is absent,
    /// including nested sacks, affixes and an unavailable authored Boast. It
    /// preserves economy revision and both native random streams.
    pub fn reconcile_mod_packages(&mut self, available_mod_ids: &[String]) -> bool {
        let available: HashSet<_> = available_mod_ids.iter().map(|id| id.to_lowercase()).collect();
        fn reconcile(item: &mut HubInventoryItem, available: &HashSet<String>) -> bool {
            let owner = item
                .mod_item_content
                .as_ref()
                .map(|content| &content.mod_id)
                .or_else(|| item.mod_content.as_ref().map(|content| &content.mod_id));
            if owner
                .filter(|owner| !owner.is_empty())
                .is_some_and(|owner| !available.contains(&owner.to_lowercase()))
            {
                return false;
            }
            if let Some(contents) = &mut item.contents {
                contents.retain_mut(|item| reconcile(item, available));
            }
            if let Some(affixes) = &mut item.mod_affixes {
                let previous_count = affixes.len();
                affixes.retain(|affix| available.contains(&affix.mod_id.to_lowercase()));
                if affixes.len() != previous_count && affixes.is_empty() {
                    item.mod_affixes = None;
                }
            }
            true
        }
        let previous = self.clone();
        self.backpack.retain_mut(|item| reconcile(item, &available));
        self.storage.retain_mut(|item| reconcile(item, &available));
        self.fomentius_stock.retain_mut(|entry| reconcile(&mut entry.item, &available));
        for slot in EquipmentSlot::ALL {
            let mut item = self.equipment.at(slot).clone();
            if item.as_mut().is_some_and(|item| !reconcile(item, &available)) {
                item = None;
            }
            self.equipment.set(slot, item);
        }
        if let Some(BoastSelection::Mod(selection)) = &self.npc.boast.selected
            && !available.contains(&selection.mod_id.to_lowercase())
        {
            self.npc.boast = BoastState::default();
        }
        *self != previous
    }

    pub fn reforge_mod_equipment(
        &mut self,
        item_id: u64,
        affixes: &[crate::modding::inventory::ModEquipmentAffix],
    ) -> HubEconomyResult {
        if !self.inventory_is_valid() || affixes.is_empty() || affixes.len() > 8 {
            return HubEconomyResult::rejected(Rejection::InvalidTarget);
        }
        let ids: HashSet<_> = affixes.iter().map(|affix| &affix.content_id).collect();
        let Some(mut item) = find_inventory_item(&self.backpack, item_id).cloned().filter(|item| {
            item.kind == HubItemKind::Equipment
                && item.equipment_type.is_some()
                && ids.len() == affixes.len()
        }) else {
            return HubEconomyResult::rejected(Rejection::InvalidTarget);
        };
        item.mod_affixes = Some(affixes.to_vec());
        let Some(backpack) = replace_tree(&self.backpack, item_id, item) else {
            return HubEconomyResult::rejected(Rejection::InvalidTarget);
        };
        self.backpack = backpack;
        self.revision += 1;
        HubEconomyResult::accepted()
    }

    /// Native College/Tutorial/loadout changes recolor only unmodified starter wearables.
    /// This does not advance inventory revision; the caller owns its transition revision.
    pub fn apply_starter_appearance(
        &mut self,
        appearance: crate::appearance::StarterEquipmentAppearance,
    ) -> bool {
        let mut changed = false;
        for (item, kind) in [
            (&mut self.equipment.hat, EquipmentType::Hat),
            (&mut self.equipment.robe, EquipmentType::Robe),
        ] {
            if let Some(item) = item
                && item.equipment_type == Some(kind)
                && item.generated_level.is_none()
                && item.kind == HubItemKind::Equipment
                && item.native_effects.is_none()
                && item.native_selector.is_none()
                && item.recipe_index.is_none()
            {
                let tints = Some([Some(appearance.primary_tint), Some(appearance.secondary_tint)]);
                if item.icon_tints != tints {
                    item.icon_tints = tints;
                    changed = true;
                }
            }
        }
        changed
    }

    #[must_use]
    pub fn inventory_is_valid(&self) -> bool {
        fn visit(
            items: &[HubInventoryItem],
            depth: usize,
            capacity: Option<usize>,
            seen: &mut HashSet<u64>,
        ) -> bool {
            if depth > HUB_SACK_REPLICATION_DEPTH_LIMIT {
                return false;
            }
            if let Some(capacity) = capacity {
                let mut slots = HashSet::new();
                for entry in project_inventory_root_slots(items) {
                    if entry.slot >= capacity || !slots.insert(entry.slot) {
                        return false;
                    }
                }
            }
            for item in items {
                if item.id == 0 || item.quantity == 0 || !seen.insert(item.id) {
                    return false;
                }
                if let Some(affixes) = &item.mod_affixes {
                    let ids: HashSet<_> = affixes.iter().map(|affix| &affix.content_id).collect();
                    if item.kind != HubItemKind::Equipment
                        || affixes.is_empty()
                        || affixes.len() > 8
                        || ids.len() != affixes.len()
                    {
                        return false;
                    }
                }
                if (item.kind == HubItemKind::ModPotion) != item.mod_content.is_some()
                    || !mod_item_inventory_identity_is_valid(item)
                {
                    return false;
                }
                let sack = item.native_type_id == 7008 && item.kind == HubItemKind::Sack;
                if (item.native_type_id == 7008) != (item.kind == HubItemKind::Sack) {
                    return false;
                }
                let Some(contents) = &item.contents else {
                    continue;
                };
                if !sack
                    || contents.len() > HUB_SACK_CHILD_REPLICATION_LIMIT
                    || !visit(contents, depth + 1, Some(HUB_SACK_CHILD_REPLICATION_LIMIT), seen)
                {
                    return false;
                }
            }
            true
        }
        let mut seen = HashSet::new();
        if !visit(&self.backpack, 0, Some(NATIVE_LOOT_BACKPACK_REPLICATION_LIMIT), &mut seen)
            || !visit(&self.storage, 0, Some(HUB_STORAGE_SLOT_CAPACITY), &mut seen)
        {
            return false;
        }
        for slot in EquipmentSlot::ALL {
            let Some(item) = self.equipment.at(slot) else {
                continue;
            };
            if !visit(std::slice::from_ref(item), 0, None, &mut seen) {
                return false;
            }
        }
        self.fomentius_stock
            .iter()
            .all(|stock| visit(std::slice::from_ref(&stock.item), 0, None, &mut seen))
    }

    pub(super) fn require_valid_inventory(&self) -> Result<(), Rejection> {
        if self.inventory_is_valid() { Ok(()) } else { Err(Rejection::InvalidInventory) }
    }

    pub(super) fn rename_sack(&mut self, item_id: u64, name: &str) -> Result<(), Rejection> {
        self.require_valid_inventory()?;
        let name = normalize_inventory_sack_name(name).ok_or(Rejection::InvalidName)?;
        let in_backpack = find_inventory_item(&self.backpack, item_id).is_some();
        let source = if in_backpack { &self.backpack } else { &self.storage };
        let mut item = find_inventory_item(source, item_id).ok_or(Rejection::ItemNotFound)?.clone();
        if item.kind != HubItemKind::Sack || item.native_type_id != 7008 {
            return Err(Rejection::IneligibleItem);
        }
        item.name = name.to_owned();
        let items = replace_tree(source, item_id, item).ok_or(Rejection::ItemNotFound)?;
        if in_backpack {
            self.backpack = items;
        } else {
            self.storage = items;
        }
        Ok(())
    }

    pub(super) fn move_item(
        &mut self,
        item_id: u64,
        destination_sack_id: Option<u64>,
        destination_slot: Option<usize>,
    ) -> Result<(), Rejection> {
        self.require_valid_inventory()?;
        if destination_slot.is_some_and(|slot| slot >= HUB_INVENTORY_SLOT_CAPACITY) {
            return Err(Rejection::InvalidTarget);
        }
        let projected = project_inventory_items(&self.backpack);
        let entry = projected
            .iter()
            .find(|entry| entry.item.id == item_id)
            .ok_or(Rejection::ItemNotFound)?;
        if entry.parent_sack_id == destination_sack_id {
            let slot = destination_slot.ok_or(Rejection::InvalidTarget)?;
            if entry.slot == slot {
                return Err(Rejection::InvalidTarget);
            }
            let root = match destination_sack_id {
                Some(parent) => find_inventory_item(&self.backpack, parent)
                    .and_then(|item| item.contents.as_deref())
                    .ok_or(Rejection::InvalidTarget)?,
                None => &self.backpack,
            };
            let root = relocate_root_item(root, item_id, slot)?;
            self.replace_root(destination_sack_id, root)?;
            return Ok(());
        }
        if destination_sack_id == Some(item_id) {
            return Err(Rejection::InvalidTarget);
        }
        if let Some(destination) = destination_sack_id {
            let valid_destination = find_inventory_item(&self.backpack, destination)
                .is_some_and(|item| item.native_type_id == 7008);
            let creates_cycle =
                find_inventory_item(entry.item.contents.as_deref().unwrap_or(&[]), destination)
                    .is_some();
            if !valid_destination || creates_cycle {
                return Err(Rejection::InvalidTarget);
            }
        }
        let removed = remove_tree(&self.backpack, item_id, None).ok_or(Rejection::ItemNotFound)?;
        self.backpack = removed.items;
        let root = match destination_sack_id {
            Some(id) => find_inventory_item(&self.backpack, id)
                .and_then(|item| item.contents.as_deref())
                .unwrap_or(&[]),
            None => &self.backpack,
        };
        let capacity = if destination_sack_id.is_some() {
            HUB_SACK_CHILD_REPLICATION_LIMIT
        } else {
            HUB_INVENTORY_SLOT_CAPACITY
        };
        let contents = match destination_slot {
            Some(slot) => insert_at(root, removed.item, slot, capacity),
            None => insert(root, removed.item, capacity),
        }
        .ok_or(Rejection::CapacityFull)?;
        self.replace_root(destination_sack_id, contents)?;
        if !self.inventory_is_valid() {
            return Err(Rejection::InvalidTarget);
        }
        Ok(())
    }

    fn replace_root(
        &mut self,
        parent: Option<u64>,
        contents: Vec<HubInventoryItem>,
    ) -> Result<(), Rejection> {
        if let Some(id) = parent {
            let mut sack =
                find_inventory_item(&self.backpack, id).ok_or(Rejection::InvalidTarget)?.clone();
            if sack.native_type_id != 7008 {
                return Err(Rejection::InvalidTarget);
            }
            sack.contents = Some(contents);
            self.backpack =
                replace_tree(&self.backpack, id, sack).ok_or(Rejection::InvalidTarget)?;
        } else {
            self.backpack = contents;
        }
        Ok(())
    }

    pub(super) fn transfer_item(
        &mut self,
        item_id: u64,
        direction: TransferDirection,
    ) -> Result<(), Rejection> {
        self.require_valid_inventory()?;
        let (from, to, capacity) = match direction {
            TransferDirection::ToStorage => {
                (&self.backpack, &self.storage, HUB_STORAGE_SLOT_CAPACITY)
            }
            TransferDirection::ToBackpack => {
                (&self.storage, &self.backpack, HUB_INVENTORY_SLOT_CAPACITY)
            }
        };
        let removed = remove_tree(from, item_id, None).ok_or(Rejection::ItemNotFound)?;
        let inserted = insert(to, removed.item, capacity).ok_or(Rejection::CapacityFull)?;
        match direction {
            TransferDirection::ToStorage => {
                self.backpack = removed.items;
                self.storage = inserted;
            }
            TransferDirection::ToBackpack => {
                self.storage = removed.items;
                self.backpack = inserted;
            }
        }
        Ok(())
    }

    pub(super) fn equip_item(
        &mut self,
        item_id: u64,
        slot: EquipmentSlot,
        context: &HubEconomyContext,
    ) -> Result<(), Rejection> {
        self.require_valid_inventory()?;
        if context.player_level == 0 {
            return Err(Rejection::IneligibleItem);
        }
        let source_slot = EquipmentSlot::ALL
            .into_iter()
            .find(|slot| self.equipment.at(*slot).as_ref().is_some_and(|item| item.id == item_id));
        let item = match source_slot {
            Some(slot) => self.equipment.at(slot).as_ref().expect("the equipped source item"),
            None => find_inventory_item(&self.backpack, item_id).ok_or(Rejection::ItemNotFound)?,
        }
        .clone();
        let equipment_type = item.equipment_type.ok_or(Rejection::IneligibleItem)?;
        if !native_equipment_meets_level_requirement(&item, context) {
            return Err(Rejection::IneligibleItem);
        }
        if !slot.accepts(equipment_type) {
            return Err(Rejection::InvalidSlot);
        }
        if slot == EquipmentSlot::Ring2 && !self.owned_perk_selectors.contains(&19) {
            return Err(Rejection::SlotLocked);
        }
        if let Some(source_slot) = source_slot {
            if source_slot != slot {
                let previous = self.equipment.at(slot).clone();
                self.equipment.set(source_slot, previous);
                self.equipment.set(slot, Some(item));
            }
            return Ok(());
        }
        let removed = remove_tree(&self.backpack, item_id, None).ok_or(Rejection::ItemNotFound)?;
        self.backpack = removed.items;
        if let Some(previous) = self.equipment.at(slot).clone() {
            if let Some(parent) = removed.parent_sack_id {
                let mut sack = find_inventory_item(&self.backpack, parent)
                    .ok_or(Rejection::InvalidInventory)?
                    .clone();
                let contents = insert(
                    sack.contents.as_deref().unwrap_or(&[]),
                    previous,
                    HUB_SACK_CHILD_REPLICATION_LIMIT,
                )
                .ok_or(Rejection::CapacityFull)?;
                sack.contents = Some(contents);
                self.backpack = replace_tree(&self.backpack, parent, sack)
                    .ok_or(Rejection::InvalidInventory)?;
            } else {
                self.backpack = insert(&self.backpack, previous, HUB_INVENTORY_SLOT_CAPACITY)
                    .ok_or(Rejection::CapacityFull)?;
            }
        }
        self.equipment.set(slot, Some(item));
        Ok(())
    }

    pub(super) fn unequip(
        &mut self,
        slot: EquipmentSlot,
        destination_sack_id: Option<u64>,
        destination_slot: Option<usize>,
    ) -> Result<(), Rejection> {
        self.require_valid_inventory()?;
        if slot == EquipmentSlot::Ring2 && !self.owned_perk_selectors.contains(&19) {
            return Err(Rejection::SlotLocked);
        }
        if matches!(slot, EquipmentSlot::Hat | EquipmentSlot::Robe) {
            return Err(Rejection::RequiredClothing);
        }
        let item = self.equipment.at(slot).clone().ok_or(Rejection::SlotEmpty)?;
        let root = match destination_sack_id {
            Some(id) => {
                let sack = find_inventory_item(&self.backpack, id)
                    .filter(|item| item.native_type_id == 7008)
                    .ok_or(Rejection::InvalidTarget)?;
                sack.contents.as_deref().unwrap_or(&[])
            }
            None => &self.backpack,
        };
        let capacity = if destination_sack_id.is_some() {
            HUB_SACK_CHILD_REPLICATION_LIMIT
        } else {
            HUB_INVENTORY_SLOT_CAPACITY
        };
        let visible_capacity = if destination_sack_id.is_some() {
            HUB_INVENTORY_SLOT_CAPACITY - 1
        } else {
            HUB_INVENTORY_SLOT_CAPACITY
        };
        if destination_slot.is_some_and(|slot| slot >= visible_capacity) {
            return Err(Rejection::InvalidTarget);
        }
        let placed = match destination_slot {
            Some(destination_slot) => {
                match insert_at(root, item.clone(), destination_slot, capacity) {
                    Some(placed) => placed,
                    None => {
                        // A swap needs a free visible cell to receive its resident. Hidden
                        // replicated Sack cells are available only to ordinary removal.
                        let inserted = insert(root, item.clone(), visible_capacity)
                            .ok_or(Rejection::CapacityFull)?;
                        relocate_root_item(&inserted, item.id, destination_slot)?
                    }
                }
            }
            None => insert(root, item, capacity).ok_or(Rejection::CapacityFull)?,
        };
        self.replace_root(destination_sack_id, placed)?;
        self.equipment.set(slot, None);
        self.require_valid_inventory()
    }

    pub(super) fn equip_sack_contents(
        &mut self,
        item_id: u64,
        context: &HubEconomyContext,
    ) -> Result<(), Rejection> {
        self.require_valid_inventory()?;
        if context.player_level == 0 {
            return Err(Rejection::IneligibleItem);
        }
        if find_inventory_item(&self.backpack, item_id)
            .is_none_or(|item| item.native_type_id != 7008)
        {
            return Err(Rejection::ItemNotFound);
        }
        let mut weapon_equipped = false;
        for (native_type_id, slot) in [
            (7005, EquipmentSlot::Hat),
            (7006, EquipmentSlot::Robe),
            (7004, EquipmentSlot::Weapon),
            (7011, EquipmentSlot::Weapon),
            (7003, EquipmentSlot::Amulet),
        ] {
            // A Staff swap may put the previous Wand back into the Sack. The
            // current Source admits only the first successful weapon replacement.
            if slot == EquipmentSlot::Weapon && weapon_equipped {
                continue;
            }
            let mut candidate = None;
            let contents = find_inventory_item(&self.backpack, item_id)
                .and_then(|sack| sack.contents.as_deref())
                .unwrap_or(&[]);
            for entry in project_inventory_root_slots(contents) {
                if entry.item.native_type_id == native_type_id
                    && native_equipment_meets_level_requirement(entry.item, context)
                {
                    candidate = Some(entry.item.id);
                    break;
                }
            }
            if let Some(id) = candidate {
                self.equip_direct_sack_child(item_id, id, slot)?;
                if slot == EquipmentSlot::Weapon {
                    weapon_equipped = true;
                }
            }
        }
        let contents = find_inventory_item(&self.backpack, item_id)
            .and_then(|sack| sack.contents.as_deref())
            .unwrap_or(&[]);
        let mut rings = Vec::new();
        for entry in project_inventory_root_slots(contents) {
            if entry.item.native_type_id == 7002
                && native_equipment_meets_level_requirement(entry.item, context)
            {
                rings.push(entry.item.id);
            }
        }
        let mut slots = vec![EquipmentSlot::Ring0, EquipmentSlot::Ring1];
        if self.owned_perk_selectors.contains(&19) {
            slots.push(EquipmentSlot::Ring2);
        }
        for ring in rings.into_iter().take(slots.len()) {
            let index =
                slots.iter().position(|slot| self.equipment.at(*slot).is_none()).unwrap_or(0);
            let slot = slots.remove(index);
            self.equip_direct_sack_child(item_id, ring, slot)?;
        }
        self.require_valid_inventory()
    }

    /// Current `equipDirectSackChild` assigns legacy cells before removing the child,
    /// so equipment replacement preserves the addresses of its siblings.
    fn equip_direct_sack_child(
        &mut self,
        sack_id: u64,
        item_id: u64,
        slot: EquipmentSlot,
    ) -> Result<(), Rejection> {
        let mut sack = find_inventory_item(&self.backpack, sack_id)
            .ok_or(Rejection::InvalidInventory)?
            .clone();
        let mut contents = Vec::new();
        for entry in project_inventory_root_slots(sack.contents.as_deref().unwrap_or(&[])) {
            let mut item = entry.item.clone();
            item.inventory_slot = Some(entry.slot);
            contents.push(item);
        }
        let index = contents
            .iter()
            .position(|item| item.id == item_id)
            .ok_or(Rejection::InvalidInventory)?;
        let item = contents.remove(index);
        if item.equipment_type.is_none_or(|equipment_type| !slot.accepts(equipment_type)) {
            return Err(Rejection::InvalidInventory);
        }
        if let Some(previous) = self.equipment.at(slot).clone() {
            contents = insert(&contents, previous, HUB_SACK_CHILD_REPLICATION_LIMIT)
                .ok_or(Rejection::InvalidInventory)?;
        }
        sack.contents = Some(contents);
        self.backpack =
            replace_tree(&self.backpack, sack_id, sack).ok_or(Rejection::InvalidInventory)?;
        self.equipment.set(slot, Some(item));
        Ok(())
    }

    /// `creditLootGold`: gold ownership is local simulation state, independent of the HUD.
    pub fn credit_loot_gold(&mut self, amount: u64) {
        assert!(amount > 0, "loot Gold credit must be positive");
        self.gold =
            self.gold.checked_add(amount).expect("loot Gold credit within its integer range");
        self.revision += 1;
    }

    pub fn insert_loot_item(&mut self, item: HubInventoryItem) -> bool {
        assert!(item.quantity > 0, "loot item quantity must be positive");
        if (item.native_type_id == 7001 || item.kind == HubItemKind::ModItem)
            && self.backpack.iter().any(|entry| inventory_items_share_stack(entry, &item))
        {
            let Some(backpack) =
                insert(&self.backpack, item, NATIVE_LOOT_BACKPACK_REPLICATION_LIMIT)
            else {
                return false;
            };
            self.backpack = backpack;
            self.revision += 1;
            return true;
        }
        let mut next_id = self.next_item_id;
        let item = identify_tree(item, &mut next_id);
        let Some(backpack) = insert(&self.backpack, item, NATIVE_LOOT_BACKPACK_REPLICATION_LIMIT)
        else {
            return false;
        };
        self.backpack = backpack;
        self.next_item_id = next_id;
        self.revision += 1;
        true
    }

    #[must_use]
    pub fn has_wizard_key(&self) -> bool {
        project_inventory_items(&self.backpack)
            .iter()
            .any(|entry| entry.item.native_type_id == 7012 && entry.item.native_subtype == Some(1))
    }

    pub fn consume_wizard_key(&mut self) -> bool {
        let id = project_inventory_items(&self.backpack)
            .iter()
            .find(|entry| entry.item.native_type_id == 7012 && entry.item.native_subtype == Some(1))
            .map(|entry| entry.item.id);
        let Some(id) = id else {
            return false;
        };
        self.backpack = consume_tree(&self.backpack, id).expect("the owned wizard key");
        self.revision += 1;
        true
    }
}

#[must_use]
pub fn native_equipment_required_level(item: &HubInventoryItem) -> u32 {
    let recipe_level = item
        .recipe_index
        .and_then(|index| DOWSING_EQUIPMENT_RECIPES.get(index))
        .map_or(0, |recipe| recipe.level);
    item.generated_level.unwrap_or(recipe_level)
}

#[must_use]
pub fn native_equipment_meets_level_requirement(
    item: &HubInventoryItem,
    context: &HubEconomyContext,
) -> bool {
    let reduction = if context.creativity_rank > 0 { 2 } else { 0 };
    native_equipment_required_level(item).saturating_sub(reduction) <= context.player_level
}

pub(super) fn identify_tree(mut item: HubInventoryItem, next_id: &mut u64) -> HubInventoryItem {
    item.id = *next_id;
    *next_id += 1;
    if let Some(contents) = &mut item.contents {
        for child in contents {
            *child = identify_tree(child.clone(), next_id);
        }
    }
    item
}
