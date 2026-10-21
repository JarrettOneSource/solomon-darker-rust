//! Shared College portraits from Website `core-kernels/hub-memorial.ts`.
//! The shared-world owner captures a completed run and supplies its one RNG draw.

use crate::appearance::equipment::PlayerLivingEquipmentAppearance;
use crate::profile::NewGameSelection;

pub const HUB_MEMORIAL_SLOT_COUNT: usize = 10;
pub const HUB_MEMORIAL_FIRST_EXTERNAL_PORTRAIT_ID: u32 = 100;
pub const HUB_MEMORIAL_LAST_EXTERNAL_PORTRAIT_ID: u32 = 109;
pub const HUB_MEMORIAL_INITIAL_SLOT_AGES: [u64; HUB_MEMORIAL_SLOT_COUNT] =
    [9, 1, 0, 2, 7, 4, 3, 8, 5, 6];
pub const HUB_MEMORIAL_INITIAL_MARKERS: [bool; HUB_MEMORIAL_SLOT_COUNT] =
    [false, true, true, true, false, true, true, false, false, true];
pub const HUB_MEMORIAL_INTERACTION_IDS: [&str; HUB_MEMORIAL_SLOT_COUNT] = [
    "painting-0",
    "painting-1",
    "painting-100",
    "painting-3",
    "painting-4",
    "painting-5",
    "painting-6",
    "painting-7",
    "painting-8",
    "painting-9",
];

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubMemorialPortrait {
    pub account_username: Option<String>,
    pub awesomeness: u64,
    pub awesomest_kill: Option<String>,
    // Source persistence accepts any finite nonnegative integer Number here,
    // including metadata beyond a native simulation clock's u64 range.
    pub captured_at_tick: f64,
    pub config: NewGameSelection,
    pub elapsed_ticks: u64,
    pub equipment: PlayerLivingEquipmentAppearance,
    pub heading_index: usize,
    pub level: u32,
    pub monsters_killed: u64,
    pub player_id: String,
    pub portrait_scale: f64,
    pub run_id: String,
    pub wave: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubMemorialPlayerProfile {
    pub account_username: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubMemorialSlot {
    pub age: u64,
    pub marker: bool,
    pub portrait: Option<HubMemorialPortrait>,
    pub portrait_id: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HubMemorialState {
    pub next_age: u64,
    pub next_portrait_id: u32,
    pub slots: [HubMemorialSlot; HUB_MEMORIAL_SLOT_COUNT],
}

#[must_use]
pub fn create_hub_memorial_state() -> HubMemorialState {
    HubMemorialState {
        next_age: 1001,
        next_portrait_id: HUB_MEMORIAL_FIRST_EXTERNAL_PORTRAIT_ID,
        slots: std::array::from_fn(|index| HubMemorialSlot {
            age: HUB_MEMORIAL_INITIAL_SLOT_AGES[index],
            marker: HUB_MEMORIAL_INITIAL_MARKERS[index],
            portrait: None,
            portrait_id: index as u32,
        }),
    }
}

/// Returns whether a new portrait replaced the oldest slot. Ties use the first
/// slot, and duplicate run/player identities do not advance either counter.
pub fn archive_hub_memorial_portrait(
    state: &mut HubMemorialState,
    portrait: &HubMemorialPortrait,
    marker_draw: u32,
) -> bool {
    assert!(marker_draw < 5, "Memorial marker draw must be an integer within 0..4");
    for slot in &state.slots {
        if let Some(current) = &slot.portrait
            && current.run_id == portrait.run_id
            && current.player_id == portrait.player_id
        {
            return false;
        }
    }

    let mut replacement_index = 0;
    for index in 1..state.slots.len() {
        if state.slots[index].age < state.slots[replacement_index].age {
            replacement_index = index;
        }
    }
    state.slots[replacement_index] = HubMemorialSlot {
        age: state.next_age,
        marker: marker_draw != 3,
        portrait: Some(portrait.clone()),
        portrait_id: state.next_portrait_id,
    };
    state.next_age += 1;
    state.next_portrait_id = if state.next_portrait_id == HUB_MEMORIAL_LAST_EXTERNAL_PORTRAIT_ID {
        HUB_MEMORIAL_FIRST_EXTERNAL_PORTRAIT_ID
    } else {
        state.next_portrait_id + 1
    };
    true
}

#[must_use]
pub fn hub_memorial_slot_index_for_interaction(interaction_id: &str) -> Option<usize> {
    HUB_MEMORIAL_INTERACTION_IDS.iter().position(|candidate| *candidate == interaction_id)
}
