//! The Belt: three eight-slot hotbars of skills, potion aliases and items.
//! The local command boundary reports invalid bindings without changing an existing slot.

use crate::economy::{
    EquipmentSlot, EquipmentType, HubEconomyState, HubInventoryItem, HubItemKind,
    find_inventory_item, project_inventory_items,
};
use crate::profile::{SkillBookState, skill_category};

pub const BELT_SLOT_COUNT: usize = 8;
pub const PLAYER_HOTBAR_COUNT: usize = 3;
pub const PLAYER_BELT_SLOT_COUNT: usize = BELT_SLOT_COUNT * PLAYER_HOTBAR_COUNT;
pub const HEALTH_BELT_SLOT: usize = 3;
pub const MANA_BELT_SLOT: usize = 4;
pub const BELT_ITEM_TYPE_IDS: [u32; 10] =
    [7001, 7002, 7003, 7004, 7005, 7006, 7008, 7009, 7010, 7011];

/// What a Belt slot holds: a skill, the health or mana potion alias, or one owned item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum BeltEntry {
    Skill {
        skill_id: usize,
    },
    HealthPotion,
    ManaPotion,
    Item {
        item_id: u64,
        #[cfg_attr(feature = "persistence", serde(rename = "native_type_id"))]
        type_id: u32,
    },
}

/// A request to bind a skill (or clear the slot with `None`) or an owned item to a Belt slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum BeltCommand {
    BindSkill { skill_id: Option<usize>, slot: usize },
    BindItem { item_id: u64, slot: usize },
}

/// A player's Belt: its three eight-slot hotbars stored as one array, bank by bank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerBeltComponent {
    #[cfg_attr(
        feature = "persistence",
        serde(deserialize_with = "deserialize_player_belt_entries")
    )]
    pub entries: [Option<BeltEntry>; PLAYER_BELT_SLOT_COUNT],
}

impl PlayerBeltComponent {
    /// A fresh Belt: the first slot is the learned secondary, not the primary attack, and
    /// `HEALTH_BELT_SLOT` and `MANA_BELT_SLOT` hold the potion aliases.
    #[must_use]
    pub fn new(book: &SkillBookState) -> Self {
        let skill_id = book
            .learned_skill_order
            .iter()
            .find(|id| skill_category(**id) == Some(2))
            .copied()
            .expect("a fresh native belt has one learned secondary skill");
        let mut entries = [None; PLAYER_BELT_SLOT_COUNT];
        entries[0] = Some(BeltEntry::Skill { skill_id });
        entries[HEALTH_BELT_SLOT] = Some(BeltEntry::HealthPotion);
        entries[MANA_BELT_SLOT] = Some(BeltEntry::ManaPotion);
        Self { entries }
    }

    /// Retain current banks and pad an eight-slot legacy belt with empty banks.
    /// The accepted component always has three banks, including after old-save restore.
    #[must_use]
    pub fn from_entries(source: &[Option<BeltEntry>]) -> Option<Self> {
        if source.len() != BELT_SLOT_COUNT && source.len() != PLAYER_BELT_SLOT_COUNT {
            return None;
        }
        let mut entries = [None; PLAYER_BELT_SLOT_COUNT];
        entries[..source.len()].copy_from_slice(source);
        Some(Self { entries })
    }

    /// Converts a legacy eight-slot skill quickbar. Its empty potion cells acquire the health
    /// and mana potion aliases.
    #[must_use]
    pub fn from_skill_quickbar(quickbar: [Option<usize>; BELT_SLOT_COUNT]) -> Option<Self> {
        if quickbar.iter().flatten().any(|id| !is_belt_skill(*id)) {
            return None;
        }
        let mut entries =
            quickbar.map(|skill_id| skill_id.map(|skill_id| BeltEntry::Skill { skill_id }));
        entries[HEALTH_BELT_SLOT].get_or_insert(BeltEntry::HealthPotion);
        entries[MANA_BELT_SLOT].get_or_insert(BeltEntry::ManaPotion);
        Self::from_entries(&entries)
    }

    pub fn apply(
        &mut self,
        command: BeltCommand,
        book: &SkillBookState,
        economy: &HubEconomyState,
    ) -> bool {
        let (slot, entry) = match command {
            BeltCommand::BindSkill { skill_id, slot } => {
                if skill_id.is_some_and(|id| {
                    !is_belt_skill(id) || book.effective_ranks.get(id).is_none_or(|rank| *rank < 1)
                }) {
                    return false;
                }
                (slot, skill_id.map(|skill_id| BeltEntry::Skill { skill_id }))
            }
            BeltCommand::BindItem { item_id, slot } => {
                let Some(item) = belt_owned_item(economy, item_id) else {
                    return false;
                };
                if !inventory_item_can_bind_to_belt(item) {
                    return false;
                }
                let entry = match (item.type_id, item.subtype) {
                    (7001, Some(0)) => BeltEntry::HealthPotion,
                    (7001, Some(1)) => BeltEntry::ManaPotion,
                    _ => BeltEntry::Item { item_id: item.id, type_id: item.type_id },
                };
                (slot, Some(entry))
            }
        };
        let Some(destination) = self.entries.get_mut(slot) else {
            return false;
        };
        *destination = entry;
        true
    }

    pub fn autofill_newly_learned(&mut self, previous: &SkillBookState, current: &SkillBookState) {
        for skill_id in &current.learned_skill_order {
            if previous.learned_skill_order.contains(skill_id)
                || current.effective_ranks[*skill_id] < 1
                || !matches!(skill_category(*skill_id), Some(1 | 2))
            {
                continue;
            }
            let Some(slot) = self.entries.iter_mut().find(|entry| entry.is_none()) else {
                return;
            };
            *slot = Some(BeltEntry::Skill { skill_id: *skill_id });
        }
    }

    /// Potion aliases survive empty stacks; stale item and skill references disappear when
    /// ownership changes, including transfers into storage.
    pub fn refresh(&mut self, book: &SkillBookState, economy: &HubEconomyState) {
        for entry in &mut self.entries {
            let retained = match *entry {
                None | Some(BeltEntry::HealthPotion | BeltEntry::ManaPotion) => true,
                Some(BeltEntry::Skill { skill_id }) => {
                    is_belt_skill(skill_id)
                        && book.effective_ranks.get(skill_id).is_some_and(|rank| *rank > 0)
                }
                Some(BeltEntry::Item { item_id, type_id }) => belt_owned_item(economy, item_id)
                    .is_some_and(|item| {
                        item.type_id == type_id && inventory_item_can_bind_to_belt(item)
                    }),
            };
            if !retained {
                *entry = None;
            }
        }
    }

    #[must_use]
    pub fn skill_projection(&self) -> [Option<usize>; BELT_SLOT_COUNT] {
        // Retail export and the fixed eight-action bot consume only the original bank.
        let mut projection = [None; BELT_SLOT_COUNT];
        for (slot, entry) in self.entries[..BELT_SLOT_COUNT].iter().enumerate() {
            if let Some(BeltEntry::Skill { skill_id }) = entry {
                projection[slot] = Some(*skill_id);
            }
        }
        projection
    }
}

#[cfg(feature = "persistence")]
fn deserialize_player_belt_entries<'de, D>(
    deserializer: D,
) -> Result<[Option<BeltEntry>; PLAYER_BELT_SLOT_COUNT], D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;

    let entries = Vec::<Option<BeltEntry>>::deserialize(deserializer)?;
    let Some(belt) = PlayerBeltComponent::from_entries(&entries) else {
        return Err(serde::de::Error::custom(
            "player belt requires eight legacy slots or three eight-slot hotbars",
        ));
    };
    Ok(belt.entries)
}

/// The potion a Belt alias draws from: health or mana.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeltPotionKind {
    Health,
    Mana,
}

/// The backpack's total count of one potion kind and its first matching item, if any.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeltPotionProjection<'a> {
    pub count: u64,
    pub item: Option<&'a HubInventoryItem>,
}

#[must_use]
pub fn is_belt_skill(skill_id: usize) -> bool {
    matches!(skill_category(skill_id), Some(1..=3))
}

#[must_use]
pub fn inventory_item_can_bind_to_belt(item: &HubInventoryItem) -> bool {
    if matches!(item.kind, HubItemKind::ModItem | HubItemKind::ModPotion) {
        return false;
    }
    BELT_ITEM_TYPE_IDS.contains(&item.type_id)
        && (item.type_id != 7001 || item.subtype.is_some_and(|subtype| subtype <= 5))
}

#[must_use]
pub fn belt_owned_item(economy: &HubEconomyState, item_id: u64) -> Option<&HubInventoryItem> {
    if let Some(item) = find_inventory_item(&economy.backpack, item_id) {
        return Some(item);
    }
    [
        &economy.equipment.hat,
        &economy.equipment.robe,
        &economy.equipment.weapon,
        &economy.equipment.amulet,
        &economy.equipment.rings[0],
        &economy.equipment.rings[1],
        &economy.equipment.rings[2],
    ]
    .into_iter()
    .flatten()
    .find(|item| item.id == item_id)
}

#[must_use]
pub fn belt_entry_item(entry: BeltEntry, economy: &HubEconomyState) -> Option<&HubInventoryItem> {
    match entry {
        BeltEntry::Skill { .. } => None,
        BeltEntry::HealthPotion => {
            belt_potion_projection(&economy.backpack, BeltPotionKind::Health).item
        }
        BeltEntry::ManaPotion => {
            belt_potion_projection(&economy.backpack, BeltPotionKind::Mana).item
        }
        BeltEntry::Item { item_id, .. } => belt_owned_item(economy, item_id),
    }
}

#[must_use]
pub fn belt_potion_projection(
    backpack: &[HubInventoryItem],
    kind: BeltPotionKind,
) -> BeltPotionProjection<'_> {
    let subtype = match kind {
        BeltPotionKind::Health => 0,
        BeltPotionKind::Mana => 1,
    };
    let mut count = 0;
    let mut item = None;
    for entry in project_inventory_items(backpack) {
        if entry.item.type_id != 7001 || entry.item.subtype != Some(subtype) {
            continue;
        }
        count += u64::from(entry.item.quantity);
        if item.is_none() {
            item = Some(entry.item);
        }
    }
    BeltPotionProjection { count, item }
}

#[must_use]
pub fn belt_equipment_slots(
    item: &HubInventoryItem,
    third_ring_unlocked: bool,
) -> Vec<EquipmentSlot> {
    match item.equipment_type {
        Some(EquipmentType::Amulet) => vec![EquipmentSlot::Amulet],
        Some(EquipmentType::Hat) => vec![EquipmentSlot::Hat],
        Some(EquipmentType::Robe) => vec![EquipmentSlot::Robe],
        Some(EquipmentType::Staff | EquipmentType::Wand) => vec![EquipmentSlot::Weapon],
        Some(EquipmentType::Ring) => {
            let mut slots = vec![EquipmentSlot::Ring0, EquipmentSlot::Ring1];
            if third_ring_unlocked {
                slots.push(EquipmentSlot::Ring2);
            }
            slots
        }
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::{HubEconomyContext, HubInventoryAction};
    use crate::profile::{WizardDiscipline, WizardElement};

    #[test]
    fn the_potion_alias_remains_bound_when_its_last_owned_potion_is_drunk() {
        let book = SkillBookState::new(WizardElement::Ether, WizardDiscipline::Arcane);
        let mut economy = HubEconomyState::new(1, WizardElement::Ether);
        let mut belt = PlayerBeltComponent::new(&book);
        assert!(
            economy
                .apply(&HubInventoryAction::Consume { item_id: 1 }, &HubEconomyContext::default())
                .accepted
        );
        belt.refresh(&book, &economy);
        assert_eq!(belt.entries[HEALTH_BELT_SLOT], Some(BeltEntry::HealthPotion));
        assert_eq!(belt_potion_projection(&economy.backpack, BeltPotionKind::Health).count, 0);
    }

    #[cfg(feature = "persistence")]
    #[test]
    fn legacy_belts_restore_into_three_banks_without_losing_the_original_entries() {
        let legacy = [Some(BeltEntry::Skill { skill_id: 11 }); BELT_SLOT_COUNT];
        let saved = serde_json::json!({ "entries": legacy });
        let restored: PlayerBeltComponent = serde_json::from_value(saved).unwrap();
        assert_eq!(&restored.entries[..BELT_SLOT_COUNT], &legacy);
        assert!(restored.entries[BELT_SLOT_COUNT..].iter().all(Option::is_none));
        assert_eq!(
            serde_json::to_value(restored).unwrap()["entries"].as_array().unwrap().len(),
            PLAYER_BELT_SLOT_COUNT,
        );
        let malformed = serde_json::json!({ "entries": vec![None::<BeltEntry>; 9] });
        assert!(serde_json::from_value::<PlayerBeltComponent>(malformed).is_err());
    }
}
