//! Source `HubInventoryActions.tsx::dropInventorySource` presentation producer.
//! Plans preserve gesture order and delayed flybys; every inventory mutation is still a command.

use crate::Vec2;
use crate::audio::AudioCue;
use crate::belt::{
    NativeBeltCommand, native_belt_equipment_slots, native_inventory_item_can_bind_to_belt,
};
use crate::economy::{
    EquipmentSlot, HubEconomyState, HubInventoryAction, HubInventoryItem, HubTransferGesture,
    TransferDirection, inventory_items_at_sack_path, inventory_items_share_stack,
    project_inventory_items, project_inventory_root_slots,
};
use crate::hud::HudRect;

use super::flyby::{InventoryFlybyLane, InventoryFlybyRequest};
use super::{INVENTORY_CAPACITY, equipment_slot_rects, inventory_slot_rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InventoryDropSource {
    Backpack { item_id: u64 },
    Equipment { slot: EquipmentSlot, item_id: u64 },
}

pub struct InventoryDropContext<'a> {
    pub economy: &'a HubEconomyState,
    pub sack_path: &'a [u64],
    pub belt_rects: &'a [HudRect; 8],
    pub belt_start: usize,
    pub companion: bool,
    pub storage_drop_rect: Option<HudRect>,
    pub reference_drop_rect: Option<HudRect>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InventoryDropEffect {
    Action(HubInventoryAction),
    BindBelt(NativeBeltCommand),
    Flyby(InventoryFlybyRequest),
    Sound { cue: AudioCue, playback_rate: f64 },
    SelectReference(u64),
    ConfirmUnforge(u64),
    RequiredClothing(EquipmentSlot),
}

#[must_use]
pub fn drop_inventory_source(
    source: InventoryDropSource,
    point: Vec2,
    context: &InventoryDropContext<'_>,
) -> Vec<InventoryDropEffect> {
    match source {
        InventoryDropSource::Backpack { item_id } => drop_backpack(item_id, point, context),
        InventoryDropSource::Equipment { slot, item_id } => {
            drop_equipment(slot, item_id, point, context)
        }
    }
}

fn drop_backpack(
    item_id: u64,
    point: Vec2,
    context: &InventoryDropContext<'_>,
) -> Vec<InventoryDropEffect> {
    let projected = project_inventory_items(&context.economy.backpack);
    let Some(source) = projected.iter().find(|entry| entry.item.id == item_id) else {
        return Vec::new();
    };
    let item = source.item;
    let has_parent = !context.sack_path.is_empty();
    let source_center = inventory_center(source.slot + usize::from(has_parent));
    if point_in_rect(point, HudRect::new(1500.0, 800.0, 100.0, 100.0)) {
        if !item.can_unforge() {
            return restore(item, point, source_center);
        }
        if item.native_type_id == 7008 {
            return if item.contents.as_ref().is_none_or(Vec::is_empty) {
                vec![InventoryDropEffect::Action(HubInventoryAction::Unforge { item_id })]
            } else {
                restore(item, point, source_center)
            };
        }
        return vec![InventoryDropEffect::ConfirmUnforge(item_id)];
    }
    if let Some(binding) = belt_binding(item, point, context) {
        return vec![InventoryDropEffect::BindBelt(binding)];
    }
    if context.storage_drop_rect.is_some_and(|rect| point_in_rect(point, rect)) {
        return vec![InventoryDropEffect::Action(HubInventoryAction::Transfer {
            item_id,
            direction: TransferDirection::ToStorage,
            gesture: HubTransferGesture::Drag,
        })];
    }
    if context.reference_drop_rect.is_some_and(|rect| point_in_rect(point, rect)) {
        let mut effects = vec![InventoryDropEffect::SelectReference(item_id)];
        effects.extend(restore(item, point, source_center));
        return effects;
    }
    if let Some(slot) = equipment_sink(item, point, context) {
        return vec![InventoryDropEffect::Action(HubInventoryAction::Equip { item_id, slot })];
    }
    let Some(visible_slot) = visible_slot_at(point) else {
        return restore(item, point, source_center);
    };
    if has_parent && visible_slot == 0 {
        return vec![
            backpack_open(),
            InventoryDropEffect::Action(HubInventoryAction::Move {
                item_id,
                destination_sack_id: parent_sack(context.sack_path),
                destination_slot: None,
            }),
        ];
    }
    let destination_slot = visible_slot - usize::from(has_parent);
    let visible = visible_items(context);
    let resident =
        visible.iter().find(|entry| entry.slot == destination_slot).map(|entry| entry.item);
    if let Some(sack) = resident.filter(|item| item.native_type_id == 7008) {
        return vec![
            backpack_open(),
            InventoryDropEffect::Action(HubInventoryAction::Move {
                item_id,
                destination_sack_id: Some(sack.id),
                destination_slot: None,
            }),
        ];
    }
    let destination_sack_id = context.sack_path.last().copied();
    let action = HubInventoryAction::Move {
        item_id,
        destination_sack_id,
        destination_slot: Some(destination_slot),
    };
    if source.parent_sack_id == destination_sack_id && source.slot == destination_slot {
        return restore(item, point, source_center);
    }
    let Some(resident) = resident else { return vec![InventoryDropEffect::Action(action)] };
    let mut effects = vec![click()];
    if inventory_items_share_stack(resident, item) {
        effects.push(InventoryDropEffect::Action(action));
    } else {
        let destination_center = inventory_center(visible_slot);
        effects.push(InventoryDropEffect::Flyby(InventoryFlybyRequest {
            action: Some(action),
            lanes: vec![
                lane(item, source_center, destination_center),
                lane(resident, destination_center, source_center),
            ],
        }));
    }
    effects
}

fn drop_equipment(
    slot: EquipmentSlot,
    source_item_id: u64,
    point: Vec2,
    context: &InventoryDropContext<'_>,
) -> Vec<InventoryDropEffect> {
    let Some(item) = context.economy.equipment.at(slot).as_ref() else { return Vec::new() };
    if item.id == source_item_id
        && let Some(destination) = equipment_sink(item, point, context)
    {
        return if destination == slot {
            Vec::new()
        } else {
            vec![InventoryDropEffect::Action(HubInventoryAction::Equip {
                item_id: item.id,
                slot: destination,
            })]
        };
    }
    if context.reference_drop_rect.is_some_and(|rect| point_in_rect(point, rect)) {
        return vec![InventoryDropEffect::SelectReference(item.id)];
    }
    if let Some(binding) = belt_binding(item, point, context) {
        return vec![InventoryDropEffect::BindBelt(binding)];
    }
    let Some(visible_slot) = visible_slot_at(point) else { return Vec::new() };
    if matches!(slot, EquipmentSlot::Hat | EquipmentSlot::Robe) {
        return vec![InventoryDropEffect::RequiredClothing(slot)];
    }
    let has_parent = !context.sack_path.is_empty();
    if has_parent && visible_slot == 0 {
        return vec![
            backpack_open(),
            InventoryDropEffect::Action(HubInventoryAction::Unequip {
                slot,
                destination_sack_id: parent_sack(context.sack_path),
                destination_slot: None,
            }),
        ];
    }
    let destination_slot = visible_slot - usize::from(has_parent);
    let visible = visible_items(context);
    let resident =
        visible.iter().find(|entry| entry.slot == destination_slot).map(|entry| entry.item);
    if let Some(sack) = resident.filter(|item| item.native_type_id == 7008) {
        return vec![
            backpack_open(),
            InventoryDropEffect::Action(HubInventoryAction::Unequip {
                slot,
                destination_sack_id: Some(sack.id),
                destination_slot: None,
            }),
        ];
    }
    let action = HubInventoryAction::Unequip {
        slot,
        destination_sack_id: context.sack_path.last().copied(),
        destination_slot: Some(destination_slot),
    };
    let Some(resident) = resident else { return vec![InventoryDropEffect::Action(action)] };
    let capacity = INVENTORY_CAPACITY - usize::from(has_parent);
    let first_hole = (0..capacity).find(|slot| visible.iter().all(|entry| entry.slot != *slot));
    let Some(first_hole) = first_hole else { return vec![InventoryDropEffect::Action(action)] };
    let mut rect = equipment_slot_rects(slot)[0];
    if context.companion {
        rect.x -= 53.0;
    }
    let source_center = Vec2::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    let destination_center = inventory_center(visible_slot);
    vec![
        click(),
        InventoryDropEffect::Flyby(InventoryFlybyRequest {
            action: Some(action),
            lanes: vec![
                lane(item, source_center, destination_center),
                lane(
                    resident,
                    destination_center,
                    inventory_center(first_hole + usize::from(has_parent)),
                ),
            ],
        }),
    ]
}

fn visible_items<'a>(
    context: &InventoryDropContext<'a>,
) -> Vec<crate::economy::ProjectedInventoryRootSlot<'a>> {
    let source = inventory_items_at_sack_path(&context.economy.backpack, context.sack_path)
        .unwrap_or(&context.economy.backpack);
    project_inventory_root_slots(source)
        .into_iter()
        .filter(|entry| {
            entry.slot < INVENTORY_CAPACITY - usize::from(!context.sack_path.is_empty())
        })
        .collect()
}

fn equipment_sink(
    item: &HubInventoryItem,
    point: Vec2,
    context: &InventoryDropContext<'_>,
) -> Option<EquipmentSlot> {
    for slot in
        native_belt_equipment_slots(item, context.economy.owned_perk_selectors.contains(&19))
    {
        for mut rect in equipment_slot_rects(slot) {
            if context.companion {
                rect.x -= 53.0;
            }
            if point_in_rect(point, rect) {
                return Some(slot);
            }
        }
    }
    None
}

fn belt_binding(
    item: &HubInventoryItem,
    point: Vec2,
    context: &InventoryDropContext<'_>,
) -> Option<NativeBeltCommand> {
    if !native_inventory_item_can_bind_to_belt(item) {
        return None;
    }
    context.belt_rects.iter().position(|rect| point_in_rect(point, *rect)).map(|slot| {
        NativeBeltCommand::BindItem { item_id: item.id, slot: context.belt_start + slot }
    })
}

fn visible_slot_at(point: Vec2) -> Option<usize> {
    (0..INVENTORY_CAPACITY).find(|slot| point_in_rect(point, inventory_slot_rect(*slot)))
}

fn inventory_center(slot: usize) -> Vec2 {
    let rect = inventory_slot_rect(slot);
    Vec2::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
}

fn point_in_rect(point: Vec2, rect: HudRect) -> bool {
    point.x >= rect.x
        && point.x <= rect.x + rect.width
        && point.y >= rect.y
        && point.y <= rect.y + rect.height
}

fn parent_sack(path: &[u64]) -> Option<u64> {
    path.iter().rev().nth(1).copied()
}

fn restore(item: &HubInventoryItem, from: Vec2, to: Vec2) -> Vec<InventoryDropEffect> {
    vec![
        InventoryDropEffect::Sound { cue: AudioCue::BadAction, playback_rate: 1.0 },
        InventoryDropEffect::Flyby(InventoryFlybyRequest {
            action: None,
            lanes: vec![lane(item, from, to)],
        }),
    ]
}

fn lane(item: &HubInventoryItem, from: Vec2, to: Vec2) -> InventoryFlybyLane {
    InventoryFlybyLane { from, to, item: item.clone() }
}

fn backpack_open() -> InventoryDropEffect {
    InventoryDropEffect::Sound { cue: AudioCue::BackpackOpen, playback_rate: 1.25 }
}

fn click() -> InventoryDropEffect {
    InventoryDropEffect::Sound { cue: AudioCue::Click, playback_rate: 1.75 }
}
