//! Source `hub-npc-dialogue.ts` selectors with the accepted ModContent metadata.
//! Stock rows and responses delegate to the same authored native tables.

use super::{
    HubNpcChatContent, HubNpcChatNext, HubNpcListSelector, NATIVE_BOAST_INSTRUCTION, NATIVE_BOASTS,
    hub_npc_selector_action, hub_npc_selector_response, hub_npc_selector_rows,
};
use crate::economy::{BoastSelection, BoastState, HubInventoryAction, NativeHubNpcState};
use crate::modding::content::{ModBoastSelection, PreparedModBoastIcon};
use crate::modding::projection::ModContentProjection;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(untagged))]
pub enum HubNpcSelectorId {
    Native(u32),
    ModBoast(ModBoastSelection),
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "persistence", serde(rename_all = "camelCase"))]
pub struct HubNpcSelectorPresentation {
    pub id: HubNpcSelectorId,
    pub label: String,
    pub detail: String,
    pub price: Option<u64>,
    pub boast_icon: Option<PreparedModBoastIcon>,
}

#[must_use]
pub fn hub_npc_selector_rows_with_mods(
    selector: HubNpcListSelector,
    npc: &NativeHubNpcState,
    advanced_unlocks: &[bool; 8],
    mods: Option<&ModContentProjection>,
) -> Vec<HubNpcSelectorPresentation> {
    let mut rows = Vec::new();
    for row in hub_npc_selector_rows(selector, npc, advanced_unlocks) {
        rows.push(HubNpcSelectorPresentation {
            id: HubNpcSelectorId::Native(row.id),
            label: row.label.to_owned(),
            detail: row.detail,
            price: row.price,
            boast_icon: row
                .boast_icon_record
                .map(|record| PreparedModBoastIcon::Stock { record, style: record - 90 }),
        });
    }
    if selector == HubNpcListSelector::Boast
        && let Some(mods) = mods
    {
        for boast in &mods.boasts {
            rows.push(HubNpcSelectorPresentation {
                id: HubNpcSelectorId::ModBoast(ModBoastSelection {
                    content_id: boast.content_id.clone(),
                    mod_id: boast.mod_id.clone(),
                }),
                label: boast.name.clone(),
                detail: boast.statement.clone(),
                price: None,
                boast_icon: Some(boast.icon.clone()),
            });
        }
    }
    rows
}

#[must_use]
pub fn hub_npc_selector_action_with_mods(
    selector: HubNpcListSelector,
    id: &HubNpcSelectorId,
) -> Option<HubInventoryAction> {
    match id {
        HubNpcSelectorId::Native(id) => Some(hub_npc_selector_action(selector, *id)),
        HubNpcSelectorId::ModBoast(selection) if selector == HubNpcListSelector::Boast => {
            Some(HubInventoryAction::SelectBoast {
                boast_id: BoastSelection::Mod(selection.clone()),
            })
        }
        HubNpcSelectorId::ModBoast(_) => None,
    }
}

#[must_use]
pub fn hub_npc_selector_response_with_mods(
    selector: HubNpcListSelector,
    id: &HubNpcSelectorId,
    mods: Option<&ModContentProjection>,
) -> Option<HubNpcChatContent> {
    match id {
        HubNpcSelectorId::Native(id) => hub_npc_selector_response(selector, *id),
        HubNpcSelectorId::ModBoast(selection) if selector == HubNpcListSelector::Boast => {
            let boast = mods?.boast(selection)?;
            Some(HubNpcChatContent::Speech {
                key: format!("MOD_BOAST_{}", boast.content_id),
                lines: vec![boast.response.clone()],
                next: HubNpcChatNext::Close,
            })
        }
        HubNpcSelectorId::ModBoast(_) => None,
    }
}

#[must_use]
pub fn hub_boast_instruction<'a>(
    selection: Option<&BoastSelection>,
    mods: Option<&'a ModContentProjection>,
) -> Option<&'a str> {
    match selection? {
        BoastSelection::Native(_) => Some(NATIVE_BOAST_INSTRUCTION),
        BoastSelection::Mod(selection) => Some(&mods?.boast(selection)?.instruction),
    }
}

#[must_use]
pub fn hub_boast_failure_text(
    state: &BoastState,
    mods: Option<&ModContentProjection>,
) -> Option<String> {
    if !state.failed {
        return None;
    }
    let label = match state.selected.as_ref()? {
        BoastSelection::Native(id) => NATIVE_BOASTS.iter().find(|boast| boast.id == *id)?.label,
        BoastSelection::Mod(selection) => &mods?.boast(selection)?.name,
    };
    if label.is_empty() {
        return None;
    }
    Some(format!("FAILED \"{label}\""))
}
