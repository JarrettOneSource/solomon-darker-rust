//! Private eligibility and biased category choice, retail `0x0047C070`.
//! Reference: Website `core-kernels/native-loot-selection.ts`.

use crate::NativeRng;
use crate::float::fround;

use super::{NativeLootCategory, NativeLootPolicy, NativeLootSelectionInput};

#[must_use]
pub fn select_native_loot_candidate(
    input: &NativeLootSelectionInput<'_>,
) -> (Option<NativeLootCategory>, NativeRng) {
    let mut rng = NativeRng::new(i64::from(input.actor_seed));
    let mut candidates = Vec::with_capacity(6);
    if input.arena.disable_mask & (1 << 4) == 0
        && input.key.current <= input.key.level
        && input.key.remaining > 0
    {
        let key_bound = (((input.arena.level - 20) / 5).max(0) + 10) * 100;
        if rng.draw_integer(key_bound) == 2 {
            candidates.push(NativeLootCategory::Key);
        }
    }
    let policies = input.policies;
    let modifiers = input.participant.modifiers;
    append(
        &mut candidates,
        NativeLootCategory::Orb,
        policies.orb,
        input.arena.disable_mask & (1 << 3) != 0,
        Some(policy_bound(policies.orb, 8.0, 16.0, 4.0, modifiers.orb_chance)),
        &mut rng,
    );
    append(
        &mut candidates,
        NativeLootCategory::Gold,
        policies.gold,
        input.arena.disable_mask & 1 != 0
            || input.arena.special_suppression
            || policies.gold == NativeLootPolicy::GoldScatter,
        Some(policy_bound(policies.gold, 22.0, 44.0, 11.0, modifiers.gold_chance)),
        &mut rng,
    );
    let mut item_bound = policy_bound(policies.item, 360.0, 720.0, 180.0, 1.0);
    if input.arena.level < 5 {
        item_bound = fround(item_bound * 200.0);
    }
    item_bound = fround(item_bound * 2.0);
    if input.arena.level != input.arena.last_successful_item_level {
        item_bound = fround(item_bound * 2.0);
    }
    item_bound = fround(item_bound * modifiers.item_chance);
    append(
        &mut candidates,
        NativeLootCategory::Item,
        policies.item,
        input.arena.disable_mask & (1 << 5) != 0 || input.arena.special_suppression,
        Some(item_bound),
        &mut rng,
    );
    if policies.potion == NativeLootPolicy::Forced
        && input.scene_forces_health_potion
        && input.arena.disable_mask & (1 << 1) == 0
    {
        candidates.clear();
    }
    append(
        &mut candidates,
        NativeLootCategory::Potion,
        policies.potion,
        input.arena.disable_mask & (1 << 1) != 0,
        Some(policy_bound(policies.potion, 400.0, 800.0, 200.0, 1.0)),
        &mut rng,
    );
    let powerup_bound = if input.participant.level <= 1 || input.participant.level.is_multiple_of(5)
    {
        None
    } else {
        let bases = [75.0, 75.0, 77.0, 82.0, 92.0, 102.0, 117.0, 137.0];
        let base = bases[(input.participant.level as usize / 5).min(bases.len() - 1)];
        let scale = match policies.powerup {
            NativeLootPolicy::Reduced => 2.0,
            NativeLootPolicy::Increased => 0.5,
            _ => 1.0,
        };
        Some(fround(fround(fround(base * scale) * 9.0) * modifiers.powerup_chance))
    };
    append(
        &mut candidates,
        NativeLootCategory::Powerup,
        policies.powerup,
        input.arena.disable_mask & (1 << 2) != 0,
        powerup_bound,
        &mut rng,
    );
    if candidates.is_empty() {
        return (None, rng);
    }
    let category = candidates[rng.draw_integer(candidates.len() as i32) as usize];
    (Some(category), rng)
}

fn append(
    candidates: &mut Vec<NativeLootCategory>,
    category: NativeLootCategory,
    policy: NativeLootPolicy,
    disabled: bool,
    bound: Option<f64>,
    rng: &mut NativeRng,
) {
    if disabled || policy == NativeLootPolicy::Disabled {
        return;
    }
    if policy == NativeLootPolicy::Forced || bound.is_some_and(|bound| bound.trunc() <= 0.0) {
        candidates.push(category);
        return;
    }
    if let Some(bound) = bound
        && rng.draw_integer(bound.trunc() as i32) == 1
    {
        candidates.push(category);
    }
}

fn policy_bound(
    policy: NativeLootPolicy,
    ordinary: f64,
    reduced: f64,
    increased: f64,
    modifier: f64,
) -> f64 {
    let base = match policy {
        NativeLootPolicy::Reduced => reduced,
        NativeLootPolicy::Increased => increased,
        _ => ordinary,
    };
    fround(fround(base) * fround(modifier))
}
