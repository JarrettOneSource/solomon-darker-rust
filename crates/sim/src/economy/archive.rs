//! Completed-run and Last Word retention from Website `core-kernels/hub-economy.ts`.
//! The run owner supplies its carried-item policy; this module owns ids, packing and restock.

use crate::appearance::roll_native_starter_equipment_appearance;
use crate::profile::WizardElement;

use super::inventory::{
    base_item, identify_tree, insert, project_inventory_root_slots, starter_loadout,
};
use super::{
    BoastState, EquipmentSlot, HUB_SACK_CHILD_REPLICATION_LIMIT, HUB_SACK_REPLICATION_DEPTH_LIMIT,
    HUB_STORAGE_SLOT_CAPACITY, HubEconomyState, HubInventoryItem, HubItemKind,
};

pub const NATIVE_RETAINED_SACK_SUFFIXES: [&str; 5] =
    ["Earthly Possessions", "Stuff", "Dead Stuff", "Bag", "Loot"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompletedRunEconomyArchive<'a> {
    pub display_name: &'a str,
    pub ground_gold: u64,
    pub ground_items: &'a [HubInventoryItem],
    pub starter_element: WizardElement,
    pub transfer_carried_items: bool,
}

impl HubEconomyState {
    pub fn archive_completed_run(&mut self, archive: CompletedRunEconomyArchive<'_>) {
        let mut ground_items = Vec::with_capacity(archive.ground_items.len());
        for item in archive.ground_items {
            ground_items.push(identify_tree(item.clone(), &mut self.next_item_id));
        }
        let mut contents = Vec::new();
        if archive.transfer_carried_items {
            for slot in EquipmentSlot::ALL {
                if let Some(item) = self.equipment.at(slot) {
                    contents.push(item.clone());
                }
            }
            for entry in project_inventory_root_slots(&self.backpack) {
                contents.push(entry.item.clone());
            }
        }
        contents.extend(ground_items);
        let contents = repack_inventory_root(contents);
        if !contents.is_empty() {
            let suffix = NATIVE_RETAINED_SACK_SUFFIXES
                [self.rng.draw_integer(NATIVE_RETAINED_SACK_SUFFIXES.len() as i32) as usize];
            let contents = pack_sack_contents(contents, &mut self.next_item_id);
            let retained = archive_sack(
                self.next_item_id,
                format!("{}'s {suffix}", archive.display_name),
                contents,
            );
            self.next_item_id += 1;
            if let Some(storage) =
                insert(&self.storage, retained.clone(), HUB_STORAGE_SLOT_CAPACITY)
            {
                self.storage = storage;
            } else {
                let mut items: Vec<_> = project_inventory_root_slots(&self.storage)
                    .into_iter()
                    .map(|entry| entry.item.clone())
                    .collect();
                items.push(retained);
                let contents = pack_sack_contents(items, &mut self.next_item_id);
                let mut consolidated = archive_sack(
                    self.next_item_id,
                    format!("{}'s Stored Possessions", archive.display_name),
                    contents,
                );
                consolidated.inventory_slot = Some(0);
                self.storage = vec![consolidated];
                self.next_item_id += 1;
            }
        }
        // Website draws the appearance from this state without feeding its returned RNG into
        // restock. Cloning preserves that separate draw lane instead of advancing shop draws.
        let mut appearance_rng = self.rng.clone();
        let appearance = roll_native_starter_equipment_appearance(
            &mut appearance_rng,
            archive.starter_element.into(),
        );
        let (backpack, equipment, next_item_id) = starter_loadout(self.next_item_id, appearance);
        self.backpack = backpack;
        self.equipment = equipment;
        self.next_item_id = next_item_id;
        self.roll_fomentius_stock();
        self.dowsing_offers.clear();
        self.dowsing_rolled = false;
        self.action_feedback = None;
        self.gold = self
            .gold
            .checked_add(archive.ground_gold)
            .expect("completed-run Gold remains within its integer range");
        self.npc.boast = BoastState::default();
        self.revision += 1;
        assert!(self.inventory_is_valid(), "completed-run archive fits the native inventory tree");
    }

    /// Native Last Word has a distinct bounded continuation chain and never consolidates
    /// full storage; failure leaves the source inventory and id counter unchanged.
    pub fn archive_hagatha_last_word_items(
        &mut self,
        items: &[HubInventoryItem],
        sack_name: &str,
    ) -> bool {
        if items.is_empty() {
            return true;
        }
        if self.storage.len() >= HUB_STORAGE_SLOT_CAPACITY {
            return false;
        }
        assert!(!sack_name.is_empty(), "a Last Word Sack has a name");
        let Some(retained) = retained_last_word_sack(items, sack_name, 0) else {
            return false;
        };
        let mut next_item_id = self.next_item_id;
        let identified = identify_tree(retained, &mut next_item_id);
        let Some(storage) = insert(&self.storage, identified, HUB_STORAGE_SLOT_CAPACITY) else {
            return false;
        };
        let mut candidate = self.clone();
        candidate.storage = storage;
        candidate.next_item_id = next_item_id;
        candidate.revision += 1;
        if !candidate.inventory_is_valid() {
            return false;
        }
        *self = candidate;
        true
    }
}

fn archive_sack(id: u64, name: String, contents: Vec<HubInventoryItem>) -> HubInventoryItem {
    let mut sack = base_item(id, HubItemKind::Sack, &name, 7008, Some(0), &[70]);
    sack.contents = Some(repack_inventory_root(contents));
    sack
}

fn repack_inventory_root(mut source: Vec<HubInventoryItem>) -> Vec<HubInventoryItem> {
    for (slot, item) in source.iter_mut().enumerate() {
        item.inventory_slot = Some(slot);
    }
    source
}

fn pack_sack_contents(
    source: Vec<HubInventoryItem>,
    next_item_id: &mut u64,
) -> Vec<HubInventoryItem> {
    let mut contents = Vec::new();
    for item in source {
        archive_safe_roots(item, HUB_SACK_REPLICATION_DEPTH_LIMIT - 2, &mut contents);
    }
    while contents.len() > HUB_SACK_CHILD_REPLICATION_LIMIT {
        let mut parents = Vec::new();
        for chunk in contents.chunks(HUB_SACK_CHILD_REPLICATION_LIMIT) {
            parents.push(archive_sack(*next_item_id, "Sack".to_owned(), chunk.to_vec()));
            *next_item_id += 1;
        }
        contents = parents;
    }
    repack_inventory_root(contents)
}

fn archive_safe_roots(
    item: HubInventoryItem,
    maximum_depth: usize,
    output: &mut Vec<HubInventoryItem>,
) {
    if inventory_tree_depth(&item) <= maximum_depth {
        output.push(item);
        return;
    }
    if let Some(contents) = item.contents {
        for item in contents {
            archive_safe_roots(item, maximum_depth, output);
        }
    }
}

fn inventory_tree_depth(item: &HubInventoryItem) -> usize {
    match &item.contents {
        Some(contents) if !contents.is_empty() => {
            1 + contents
                .iter()
                .map(inventory_tree_depth)
                .max()
                .expect("a non-empty Sack has a child depth")
        }
        _ => 0,
    }
}

fn retained_last_word_sack(
    source: &[HubInventoryItem],
    name: &str,
    depth: usize,
) -> Option<HubInventoryItem> {
    if depth >= HUB_SACK_REPLICATION_DEPTH_LIMIT {
        return None;
    }
    let contents = if source.len() > HUB_SACK_CHILD_REPLICATION_LIMIT {
        let continuation = retained_last_word_sack(
            &source[HUB_SACK_CHILD_REPLICATION_LIMIT - 1..],
            name,
            depth + 1,
        )?;
        let mut contents = source[..HUB_SACK_CHILD_REPLICATION_LIMIT - 1].to_vec();
        contents.push(continuation);
        contents
    } else {
        source.to_vec()
    };
    Some(archive_sack(1, name.to_owned(), contents))
}
