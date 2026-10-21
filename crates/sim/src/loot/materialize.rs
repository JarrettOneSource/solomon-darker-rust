//! Drop constructors and placement from Website `core-kernels/native-loot.ts`.

use crate::economy::{DOWSING_EQUIPMENT_RECIPES, HubInventoryItem};
use crate::float;
use crate::float::fround;
use crate::{NativeRng, Vec2};

use super::items::{equipment_recipe_item, misc_item, potion_item, select_enemy_item};
use super::{
    NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS, NATIVE_LOOT_PLACEMENT_VERTICAL_SCALE, NativeBonusKind,
    NativeLootCategory, NativeLootDropKind, NativeLootDropSource, NativeLootDropSpec,
    NativeLootItemIds, NativeLootMaterialization, NativeLootPlacement, NativeLootPolicy,
    NativeLootScriptAction, NativeLootSelection, NativeLootSelectionInput, NativeOrbKind,
    advance_native_key_drop_level, native_gold_tier, select_native_loot_candidate,
};

/// GMath's authored single at `0x007DE8A8`, rather than a platform double PI constant.
const NATIVE_LOOT_PLACEMENT_PI: f64 = 3.141_592_502_593_994;

#[must_use]
pub fn roll_native_enemy_loot(
    input: &NativeLootSelectionInput<'_>,
    shared_rng: &mut NativeRng,
    item_ids: &mut NativeLootItemIds,
) -> NativeLootSelection {
    if roll_emergency_potion_gate(shared_rng, input) {
        let can_create = input.world_badguy_count > 79
            && !input.inventory_has_health_potion
            && !input.world_has_health_potion_sack
            && input.nearby_mask_two_count > 49;
        let materialization = if can_create {
            materialize_potion(input, shared_rng, item_ids, 0, NativeLootDropSource::Enemy)
        } else {
            NativeLootMaterialization::empty(input)
        };
        return NativeLootSelection {
            materialization,
            emergency_potion_attempted: true,
            selected_category: if can_create { Some(NativeLootCategory::Potion) } else { None },
        };
    }
    let (selected_category, mut private_rng) = select_native_loot_candidate(input);
    let Some(category) = selected_category else {
        return NativeLootSelection {
            materialization: NativeLootMaterialization::empty(input),
            emergency_potion_attempted: false,
            selected_category: None,
        };
    };
    let mut materialization = if input.participant.slot == 0
        || matches!(category, NativeLootCategory::Key | NativeLootCategory::Orb)
    {
        materialize_selected(category, input, &mut private_rng, shared_rng, item_ids)
    } else {
        NativeLootMaterialization::empty(input)
    };
    if input.policies.gold == NativeLootPolicy::GoldScatter && !input.arena.special_suppression {
        // Source constructs a scatter factor even though Gold construction does not use it.
        shared_rng.draw_float_range(0.9_f32 as f64, 1.1_f32 as f64);
        let extra =
            materialize_gold(input, shared_rng, item_ids, Some(1000), NativeLootDropSource::Enemy);
        materialization.drops.extend(extra.drops);
    }
    NativeLootSelection { materialization, emergency_potion_attempted: false, selected_category }
}

#[must_use]
pub fn materialize_native_loot_script_action(
    input: &NativeLootSelectionInput<'_>,
    action: NativeLootScriptAction,
    shared_rng: &mut NativeRng,
    item_ids: &mut NativeLootItemIds,
) -> NativeLootMaterialization {
    match action {
        NativeLootScriptAction::DropGold { amount } => materialize_gold(
            input,
            shared_rng,
            item_ids,
            Some(amount),
            NativeLootDropSource::Script,
        ),
        NativeLootScriptAction::DropRandomGold { minimum, maximum } => {
            assert!(maximum >= minimum, "native random Gold bounds are ordered");
            let mut amount = minimum;
            if minimum != maximum {
                amount += shared_rng.draw_integer(maximum - minimum + 1);
            }
            materialize_gold(
                input,
                shared_rng,
                item_ids,
                Some(i64::from(amount)),
                NativeLootDropSource::Script,
            )
        }
        NativeLootScriptAction::DropPotion { subtype } => {
            assert!(subtype <= 5, "native script potion subtype is in0..6");
            let subtype = if input.scene_forces_health_potion { 0 } else { subtype };
            materialize_potion(input, shared_rng, item_ids, subtype, NativeLootDropSource::Script)
        }
        NativeLootScriptAction::DropKey => {
            let first = resolve_native_loot_placement(
                shared_rng,
                input.placement,
                input.source_position,
                NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS,
                &[],
            );
            let second = resolve_native_loot_placement(
                shared_rng,
                input.placement,
                first,
                NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS,
                &[],
            );
            let item = misc_item(item_ids, 1);
            let mut result = NativeLootMaterialization::empty(input);
            result.drops.push(sack_drop(item_ids, item, NativeLootDropSource::Script, second, 0));
            result
        }
        NativeLootScriptAction::DropItem { recipe_index } => {
            let mut result = NativeLootMaterialization::empty(input);
            let Some(recipe) =
                DOWSING_EQUIPMENT_RECIPES.iter().find(|recipe| recipe.source_index == recipe_index)
            else {
                return result;
            };
            let item = equipment_recipe_item(recipe, item_ids, shared_rng);
            let position = resolve_native_loot_placement(
                shared_rng,
                input.placement,
                input.source_position,
                NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS,
                &[],
            );
            result.drops.push(sack_drop(item_ids, item, NativeLootDropSource::Script, position, 0));
            result.last_successful_item_level = input.arena.level;
            result
        }
        NativeLootScriptAction::DropRandomItem { mode } => {
            assert!(mode <= 4, "native random Item mode is in0..5");
            let mut selected_input = *input;
            selected_input.policies.specific_item = mode;
            let item = select_enemy_item(shared_rng, item_ids, &selected_input, None);
            let mut result = NativeLootMaterialization::empty(input);
            if let Some(item) = item {
                let position = resolve_native_loot_placement(
                    shared_rng,
                    input.placement,
                    input.source_position,
                    NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS,
                    &[],
                );
                result.drops.push(sack_drop(
                    item_ids,
                    item,
                    NativeLootDropSource::Script,
                    position,
                    0,
                ));
            }
            // DROP RANDOM ITEM writes +0x9064 even when its virtual found no candidate.
            result.last_successful_item_level = input.arena.level;
            result
        }
    }
}

fn materialize_selected(
    category: NativeLootCategory,
    input: &NativeLootSelectionInput<'_>,
    private_rng: &mut NativeRng,
    shared_rng: &mut NativeRng,
    item_ids: &mut NativeLootItemIds,
) -> NativeLootMaterialization {
    let mut result = NativeLootMaterialization::empty(input);
    match category {
        NativeLootCategory::Key => {
            result.next_key_drop_level =
                advance_native_key_drop_level(shared_rng, input.key.current);
            let first = resolve_native_loot_placement(
                shared_rng,
                input.placement,
                input.source_position,
                NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS,
                &[],
            );
            let second = resolve_native_loot_placement(
                shared_rng,
                input.placement,
                first,
                NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS,
                &[],
            );
            let item = misc_item(item_ids, 1);
            result.drops.push(sack_drop(item_ids, item, NativeLootDropSource::Enemy, second, 0));
        }
        NativeLootCategory::Orb => {
            // Shared constructor defaults are consumed before the actor-private overwrite.
            shared_rng.draw_integer(3);
            shared_rng.draw_scaled_float(0.45_f32 as f64, false);
            shared_rng.draw_scaled_float(360.0, false);
            let kind = private_rng.draw_integer(3);
            let value = private_rng.draw_scaled_float(0.45_f32 as f64, false);
            let phase = private_rng.draw_scaled_float(360.0, false);
            let raw = fround(fround(value) + 0.25);
            let mut drop = NativeLootDropSpec::new(
                NativeLootDropKind::Orb,
                item_ids.allocate(),
                input.source_position,
                NativeLootDropSource::Enemy,
            );
            drop.orb_kind =
                Some(if kind == 1 { NativeOrbKind::Health } else { NativeOrbKind::Mana });
            drop.phase = phase;
            drop.value = Some(if input.participant.modifiers.orb_value_bonus {
                fround(raw * 1.25)
            } else {
                raw
            });
            result.drops.push(drop);
        }
        NativeLootCategory::Gold => {
            return materialize_gold(
                input,
                shared_rng,
                item_ids,
                input.explicit_gold_amount.map(|amount| amount as i64),
                NativeLootDropSource::Enemy,
            );
        }
        NativeLootCategory::Potion => {
            let subtype = if input.scene_forces_health_potion {
                0
            } else {
                shared_rng.draw_integer(2) as u32
            };
            return materialize_potion(
                input,
                shared_rng,
                item_ids,
                subtype,
                NativeLootDropSource::Enemy,
            );
        }
        NativeLootCategory::Powerup => {
            let first = shared_rng.draw_integer(3) as u32;
            let overwrite = shared_rng.draw_integer(2);
            let phase = shared_rng.draw_scaled_float(360.0, false);
            let mut drop = NativeLootDropSpec::new(
                NativeLootDropKind::Bonus,
                item_ids.allocate(),
                input.source_position,
                NativeLootDropSource::Enemy,
            );
            drop.bonus_kind = NativeBonusKind::from_id(if overwrite == 1 { 2 } else { first });
            drop.phase = phase;
            result.drops.push(drop);
        }
        NativeLootCategory::Item => {
            if let Some(item) =
                select_enemy_item(shared_rng, item_ids, input, input.item_recipe_pool)
            {
                let position = resolve_native_loot_placement(
                    shared_rng,
                    input.placement,
                    input.source_position,
                    NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS,
                    &[],
                );
                let delay =
                    (input.drop_delay_context as f64 * 1.100_000_023_841_858).trunc() as i64;
                result.drops.push(sack_drop(
                    item_ids,
                    item,
                    NativeLootDropSource::Enemy,
                    position,
                    delay,
                ));
                result.last_successful_item_level = input.arena.level;
            }
        }
    }
    result
}

fn materialize_gold(
    input: &NativeLootSelectionInput<'_>,
    rng: &mut NativeRng,
    item_ids: &mut NativeLootItemIds,
    explicit_amount: Option<i64>,
    source: NativeLootDropSource,
) -> NativeLootMaterialization {
    let total = match explicit_amount {
        None | Some(-1) => {
            let addend = 1.max(input.arena.level / 5);
            let mut raw = rng.draw_integer(input.arena.level / 2 + 6) + addend;
            if raw == 1 && rng.draw_integer(3) != 2 {
                raw = 2;
            }
            fround(input.participant.modifiers.gold_amount * f64::from(raw)).trunc() as i64
        }
        Some(amount) => {
            fround(input.participant.modifiers.gold_amount * amount as f64).trunc() as i64
        }
    };
    let mut remaining = total;
    let mut next_activation_delay = input.drop_delay_context;
    let mut constructed = Vec::new();
    while remaining > 0 {
        let identity = rng.draw_integer(100_000) as u32;
        let phase = rng.draw_scaled_float(360.0, false);
        let motion = rng.draw_scaled_float(20.0, true);
        let radius = rng.draw_scaled_float(3.0, false);
        let position = resolve_native_loot_placement(
            rng,
            input.placement,
            input.source_position,
            fround(radius + 1.0),
            &constructed,
        );
        let mut chunk = remaining.min(25);
        if explicit_amount.is_some_and(|amount| amount > 25) && rng.draw_integer(2) == 1 {
            chunk = i64::from(rng.draw_integer((chunk / 2) as i32)) + 1;
        }
        let mut drop = NativeLootDropSpec::new(
            NativeLootDropKind::Gold,
            item_ids.allocate(),
            position,
            source,
        );
        drop.activation_delay_ticks = next_activation_delay;
        drop.amount = Some(chunk as u64);
        drop.phase = phase;
        drop.rotation_deg = Some(motion);
        drop.scatter_seed = Some(identity);
        drop.tier = Some(native_gold_tier(chunk as u64));
        constructed.push(drop);
        remaining -= chunk;
        if constructed.len() > 5 {
            let delay = rng.draw_scaled_float(0.04_f32 as f64, false);
            next_activation_delay +=
                fround(fround(delay + (0.01_f32 as f64)) * 100.0).trunc() as i64;
        }
    }
    // Arena_CreateGold constructs one dummy stack to recover its sort-field offset.
    rng.draw_integer(100_000);
    rng.draw_scaled_float(360.0, false);
    rng.draw_scaled_float(20.0, true);
    constructed.sort_by(|left, right| {
        left.position.y.partial_cmp(&right.position.y).expect("loot positions are finite")
    });
    for drop in &mut constructed {
        if drop.activation_delay_ticks == 0 {
            let delay = rng.draw_scaled_float(0.25, false);
            drop.activation_delay_ticks = fround(delay * 100.0).trunc() as i64;
        }
    }
    let mut result = NativeLootMaterialization::empty(input);
    result.drops = constructed;
    result
}

fn materialize_potion(
    input: &NativeLootSelectionInput<'_>,
    rng: &mut NativeRng,
    item_ids: &mut NativeLootItemIds,
    subtype: u32,
    source: NativeLootDropSource,
) -> NativeLootMaterialization {
    let position = resolve_native_loot_placement(
        rng,
        input.placement,
        input.source_position,
        NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS,
        &[],
    );
    let item = potion_item(item_ids, subtype);
    let mut result = NativeLootMaterialization::empty(input);
    result.drops.push(sack_drop(item_ids, item, source, position, 0));
    result
}

fn sack_drop(
    item_ids: &mut NativeLootItemIds,
    item: HubInventoryItem,
    source: NativeLootDropSource,
    position: Vec2,
    activation_delay: i64,
) -> NativeLootDropSpec {
    let mut drop =
        NativeLootDropSpec::new(NativeLootDropKind::Sack, item_ids.allocate(), position, source);
    drop.activation_delay_ticks = activation_delay;
    drop.item = Some(item);
    drop.tier = Some(0);
    drop
}

/// The source expands an elliptical ring until real geometry accepts a carrier.
/// Pending Gold participates in the same overlap test before actors are allocated.
pub fn resolve_native_loot_placement(
    rng: &mut NativeRng,
    placement: &dyn NativeLootPlacement,
    source_position: Vec2,
    radius: f64,
    pending_gold: &[NativeLootDropSpec],
) -> Vec2 {
    assert!(
        radius.is_finite() && radius > 0.0,
        "native loot placement radius is positive and finite"
    );
    let origin = Vec2 { x: fround(source_position.x), y: fround(source_position.y) };
    let native_radius = fround(radius);
    let can_place = |position: Vec2| {
        if !placement.can_place(position, native_radius) {
            return false;
        }
        let horizontal = fround(native_radius + NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS);
        let vertical = fround(
            fround(native_radius * NATIVE_LOOT_PLACEMENT_VERTICAL_SCALE)
                + fround(
                    NATIVE_LOOT_CARRIER_PLACEMENT_RADIUS * NATIVE_LOOT_PLACEMENT_VERTICAL_SCALE,
                ),
        );
        for earlier in pending_gold {
            let dx = fround(fround(position.x - earlier.position.x) * fround(1.0 / horizontal));
            let dy = fround(fround(position.y - earlier.position.y) * fround(1.0 / vertical));
            if fround(dx * dx + dy * dy) < 1.0 {
                return false;
            }
        }
        true
    };
    if can_place(origin) {
        return origin;
    }
    let mut search_radius = native_radius;
    let mut growth = 1.0;
    loop {
        let sample_count =
            (fround(2.0 * NATIVE_LOOT_PLACEMENT_PI * (search_radius + native_radius))
                / (2.0 * search_radius))
                .trunc();
        let angle_step = fround(360.0 / sample_count);
        let start = rng.draw_scaled_float(360.0, false);
        let vertical_radius = fround(search_radius * NATIVE_LOOT_PLACEMENT_VERTICAL_SCALE);
        let mut offset = 0.0;
        while offset < 360.0 {
            let radians = fround(fround(start + offset) * NATIVE_LOOT_PLACEMENT_PI / 180.0);
            let candidate = Vec2 {
                x: fround(origin.x + fround(fround(float::sin(radians)) * search_radius)),
                y: fround(origin.y - fround(fround(float::cos(radians)) * vertical_radius)),
            };
            if can_place(candidate) {
                return candidate;
            }
            offset = fround(offset + angle_step);
        }
        search_radius = fround(search_radius + growth * native_radius);
        let multiplier = rng.draw_scaled_float(1.0, false);
        growth = fround((multiplier + 1.0) * growth);
    }
}

fn roll_emergency_potion_gate(rng: &mut NativeRng, input: &NativeLootSelectionInput<'_>) -> bool {
    let policies = input.policies;
    if [policies.orb, policies.gold, policies.item, policies.potion]
        .contains(&NativeLootPolicy::Forced)
        || policies.gold == NativeLootPolicy::GoldScatter
    {
        return false;
    }
    if rng.draw_integer(2) != 0 {
        return false;
    }
    rng.draw_integer(10) == 1
}
