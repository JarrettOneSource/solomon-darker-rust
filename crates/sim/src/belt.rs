//! Three native eight-slot hotbars from Website `core-kernels/native-belt.ts`.
//! The local command boundary reports invalid bindings without changing an existing slot.

use crate::economy::{
    EquipmentSlot, EquipmentType, HubEconomyState, HubInventoryItem, HubItemKind,
    find_inventory_item, project_inventory_items,
};
use crate::profile::{NativeSkillBookState, native_skill_category};

pub const NATIVE_BELT_SLOT_COUNT: usize = 8;
pub const PLAYER_HOTBAR_COUNT: usize = 3;
pub const PLAYER_BELT_SLOT_COUNT: usize = NATIVE_BELT_SLOT_COUNT * PLAYER_HOTBAR_COUNT;
pub const NATIVE_HEALTH_BELT_SLOT: usize = 3;
pub const NATIVE_MANA_BELT_SLOT: usize = 4;
pub const NATIVE_BELT_ITEM_TYPE_IDS: [u32; 10] =
    [7001, 7002, 7003, 7004, 7005, 7006, 7008, 7009, 7010, 7011];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeBeltEntry {
    Skill { skill_id: usize },
    HealthPotion,
    ManaPotion,
    Item { item_id: u64, native_type_id: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeBeltCommand {
    BindSkill { skill_id: Option<usize>, slot: usize },
    BindItem { item_id: u64, slot: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerBeltComponent {
    #[cfg_attr(
        feature = "persistence",
        serde(deserialize_with = "deserialize_player_belt_entries")
    )]
    pub entries: [Option<NativeBeltEntry>; PLAYER_BELT_SLOT_COUNT],
}

impl PlayerBeltComponent {
    /// `createNativePlayerBelt`: the first slot is the learned secondary, not the primary attack.
    #[must_use]
    pub fn new(book: &NativeSkillBookState) -> Self {
        let skill_id = book
            .learned_skill_order
            .iter()
            .find(|id| native_skill_category(**id) == Some(2))
            .copied()
            .expect("a fresh native belt has one learned secondary skill");
        let mut entries = [None; PLAYER_BELT_SLOT_COUNT];
        entries[0] = Some(NativeBeltEntry::Skill { skill_id });
        entries[NATIVE_HEALTH_BELT_SLOT] = Some(NativeBeltEntry::HealthPotion);
        entries[NATIVE_MANA_BELT_SLOT] = Some(NativeBeltEntry::ManaPotion);
        Self { entries }
    }

    /// `freezeNativeBelt`: retain current banks and pad an eight-slot legacy belt.
    /// The accepted component always has three banks, including after old-save restore.
    #[must_use]
    pub fn from_entries(source: &[Option<NativeBeltEntry>]) -> Option<Self> {
        if source.len() != NATIVE_BELT_SLOT_COUNT && source.len() != PLAYER_BELT_SLOT_COUNT {
            return None;
        }
        let mut entries = [None; PLAYER_BELT_SLOT_COUNT];
        entries[..source.len()].copy_from_slice(source);
        Some(Self { entries })
    }

    /// `migrateSkillQuickbarToNativeBelt`: legacy empty potion cells acquire native aliases.
    #[must_use]
    pub fn from_skill_quickbar(quickbar: [Option<usize>; NATIVE_BELT_SLOT_COUNT]) -> Option<Self> {
        if quickbar.iter().flatten().any(|id| !is_native_belt_skill(*id)) {
            return None;
        }
        let mut entries =
            quickbar.map(|skill_id| skill_id.map(|skill_id| NativeBeltEntry::Skill { skill_id }));
        entries[NATIVE_HEALTH_BELT_SLOT].get_or_insert(NativeBeltEntry::HealthPotion);
        entries[NATIVE_MANA_BELT_SLOT].get_or_insert(NativeBeltEntry::ManaPotion);
        Self::from_entries(&entries)
    }

    pub fn apply(
        &mut self,
        command: NativeBeltCommand,
        book: &NativeSkillBookState,
        economy: &HubEconomyState,
    ) -> bool {
        let (slot, entry) = match command {
            NativeBeltCommand::BindSkill { skill_id, slot } => {
                if skill_id.is_some_and(|id| {
                    !is_native_belt_skill(id)
                        || book.effective_ranks.get(id).is_none_or(|rank| *rank < 1)
                }) {
                    return false;
                }
                (slot, skill_id.map(|skill_id| NativeBeltEntry::Skill { skill_id }))
            }
            NativeBeltCommand::BindItem { item_id, slot } => {
                let Some(item) = native_belt_owned_item(economy, item_id) else {
                    return false;
                };
                if !native_inventory_item_can_bind_to_belt(item) {
                    return false;
                }
                let entry = match (item.native_type_id, item.native_subtype) {
                    (7001, Some(0)) => NativeBeltEntry::HealthPotion,
                    (7001, Some(1)) => NativeBeltEntry::ManaPotion,
                    _ => NativeBeltEntry::Item {
                        item_id: item.id,
                        native_type_id: item.native_type_id,
                    },
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

    pub fn autofill_newly_learned(
        &mut self,
        previous: &NativeSkillBookState,
        current: &NativeSkillBookState,
    ) {
        for skill_id in &current.learned_skill_order {
            if previous.learned_skill_order.contains(skill_id)
                || current.effective_ranks[*skill_id] < 1
                || !matches!(native_skill_category(*skill_id), Some(1 | 2))
            {
                continue;
            }
            let Some(slot) = self.entries.iter_mut().find(|entry| entry.is_none()) else {
                return;
            };
            *slot = Some(NativeBeltEntry::Skill { skill_id: *skill_id });
        }
    }

    /// `refreshNativePlayerBelt`: potion aliases survive empty stacks; stale item and skill
    /// references disappear when ownership changes, including transfers into storage.
    pub fn refresh(&mut self, book: &NativeSkillBookState, economy: &HubEconomyState) {
        for entry in &mut self.entries {
            let retained = match *entry {
                None | Some(NativeBeltEntry::HealthPotion | NativeBeltEntry::ManaPotion) => true,
                Some(NativeBeltEntry::Skill { skill_id }) => {
                    is_native_belt_skill(skill_id)
                        && book.effective_ranks.get(skill_id).is_some_and(|rank| *rank > 0)
                }
                Some(NativeBeltEntry::Item { item_id, native_type_id }) => {
                    native_belt_owned_item(economy, item_id).is_some_and(|item| {
                        item.native_type_id == native_type_id
                            && native_inventory_item_can_bind_to_belt(item)
                    })
                }
            };
            if !retained {
                *entry = None;
            }
        }
    }

    #[must_use]
    pub fn skill_projection(&self) -> [Option<usize>; NATIVE_BELT_SLOT_COUNT] {
        // Retail export and the fixed eight-action bot consume only the original bank.
        let mut projection = [None; NATIVE_BELT_SLOT_COUNT];
        for (slot, entry) in self.entries[..NATIVE_BELT_SLOT_COUNT].iter().enumerate() {
            if let Some(NativeBeltEntry::Skill { skill_id }) = entry {
                projection[slot] = Some(*skill_id);
            }
        }
        projection
    }
}

#[cfg(feature = "persistence")]
fn deserialize_player_belt_entries<'de, D>(
    deserializer: D,
) -> Result<[Option<NativeBeltEntry>; PLAYER_BELT_SLOT_COUNT], D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;

    let entries = Vec::<Option<NativeBeltEntry>>::deserialize(deserializer)?;
    let Some(belt) = PlayerBeltComponent::from_entries(&entries) else {
        return Err(serde::de::Error::custom(
            "player belt requires eight legacy slots or three eight-slot hotbars",
        ));
    };
    Ok(belt.entries)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeBeltPotionKind {
    Health,
    Mana,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeBeltPotionProjection<'a> {
    pub count: u64,
    pub item: Option<&'a HubInventoryItem>,
}

#[must_use]
pub fn is_native_belt_skill(skill_id: usize) -> bool {
    matches!(native_skill_category(skill_id), Some(1..=3))
}

#[must_use]
pub fn native_inventory_item_can_bind_to_belt(item: &HubInventoryItem) -> bool {
    if matches!(item.kind, HubItemKind::ModItem | HubItemKind::ModPotion) {
        return false;
    }
    NATIVE_BELT_ITEM_TYPE_IDS.contains(&item.native_type_id)
        && (item.native_type_id != 7001 || item.native_subtype.is_some_and(|subtype| subtype <= 5))
}

#[must_use]
pub fn native_belt_owned_item(
    economy: &HubEconomyState,
    item_id: u64,
) -> Option<&HubInventoryItem> {
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
pub fn native_belt_entry_item(
    entry: NativeBeltEntry,
    economy: &HubEconomyState,
) -> Option<&HubInventoryItem> {
    match entry {
        NativeBeltEntry::Skill { .. } => None,
        NativeBeltEntry::HealthPotion => {
            native_belt_potion_projection(&economy.backpack, NativeBeltPotionKind::Health).item
        }
        NativeBeltEntry::ManaPotion => {
            native_belt_potion_projection(&economy.backpack, NativeBeltPotionKind::Mana).item
        }
        NativeBeltEntry::Item { item_id, .. } => native_belt_owned_item(economy, item_id),
    }
}

#[must_use]
pub fn native_belt_potion_projection(
    backpack: &[HubInventoryItem],
    kind: NativeBeltPotionKind,
) -> NativeBeltPotionProjection<'_> {
    let subtype = match kind {
        NativeBeltPotionKind::Health => 0,
        NativeBeltPotionKind::Mana => 1,
    };
    let mut count = 0;
    let mut item = None;
    for entry in project_inventory_items(backpack) {
        if entry.item.native_type_id != 7001 || entry.item.native_subtype != Some(subtype) {
            continue;
        }
        count += u64::from(entry.item.quantity);
        if item.is_none() {
            item = Some(entry.item);
        }
    }
    NativeBeltPotionProjection { count, item }
}

#[must_use]
pub fn native_belt_equipment_slots(
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
        let book = NativeSkillBookState::new(WizardElement::Ether, WizardDiscipline::Arcane);
        let mut economy = HubEconomyState::new(1, WizardElement::Ether);
        let mut belt = PlayerBeltComponent::new(&book);
        assert!(
            economy
                .apply(&HubInventoryAction::Consume { item_id: 1 }, &HubEconomyContext::default())
                .accepted
        );
        belt.refresh(&book, &economy);
        assert_eq!(belt.entries[NATIVE_HEALTH_BELT_SLOT], Some(NativeBeltEntry::HealthPotion));
        assert_eq!(
            native_belt_potion_projection(&economy.backpack, NativeBeltPotionKind::Health).count,
            0
        );
    }

    #[cfg(feature = "persistence")]
    #[test]
    fn legacy_belts_restore_into_three_banks_without_losing_the_original_entries() {
        let legacy = [Some(NativeBeltEntry::Skill { skill_id: 11 }); NATIVE_BELT_SLOT_COUNT];
        let saved = serde_json::json!({ "entries": legacy });
        let restored: PlayerBeltComponent = serde_json::from_value(saved).unwrap();
        assert_eq!(&restored.entries[..NATIVE_BELT_SLOT_COUNT], &legacy);
        assert!(restored.entries[NATIVE_BELT_SLOT_COUNT..].iter().all(Option::is_none));
        assert_eq!(
            serde_json::to_value(restored).unwrap()["entries"].as_array().unwrap().len(),
            PLAYER_BELT_SLOT_COUNT,
        );
        let malformed = serde_json::json!({ "entries": vec![None::<NativeBeltEntry>; 9] });
        assert!(serde_json::from_value::<PlayerBeltComponent>(malformed).is_err());
    }
}
