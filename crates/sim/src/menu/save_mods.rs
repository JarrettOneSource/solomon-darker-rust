//! Website `save/game-save-mods.ts`: compare the actual saved and enabled
//! content identities before the host admits a changed-content continuation.

use std::collections::BTreeMap;

use crate::modding::identity::GameContentIdentity;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedSaveMod {
    pub active: GameContentIdentity,
    pub saved: GameContentIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSaveModMismatch {
    pub added: Vec<GameContentIdentity>,
    pub changed: Vec<ChangedSaveMod>,
    pub removed: Vec<GameContentIdentity>,
}

#[must_use]
pub fn game_save_mod_mismatch(
    saved: &[GameContentIdentity],
    active: &[GameContentIdentity],
) -> Option<GameSaveModMismatch> {
    // JavaScript Map keeps the final duplicate identity. Output still follows
    // the original arrays' insertion order, including retained duplicate rows.
    let saved_by_id: BTreeMap<_, _> =
        saved.iter().map(|entry| (entry.id.to_lowercase(), entry)).collect();
    let active_by_id: BTreeMap<_, _> =
        active.iter().map(|entry| (entry.id.to_lowercase(), entry)).collect();
    let mut result =
        GameSaveModMismatch { added: Vec::new(), changed: Vec::new(), removed: Vec::new() };
    for entry in active {
        if !saved_by_id.contains_key(&entry.id.to_lowercase()) {
            result.added.push(entry.clone());
        }
    }
    for entry in saved {
        match active_by_id.get(&entry.id.to_lowercase()) {
            None => result.removed.push(entry.clone()),
            Some(active)
                if active.version != entry.version
                    || active.content_sha256.to_lowercase()
                        != entry.content_sha256.to_lowercase() =>
            {
                result
                    .changed
                    .push(ChangedSaveMod { active: (*active).clone(), saved: entry.clone() });
            }
            Some(_) => {}
        }
    }
    if result.added.is_empty() && result.changed.is_empty() && result.removed.is_empty() {
        None
    } else {
        Some(result)
    }
}
