//! Source `ml-bot-policy/inventory.ts`: addressed potions, equipment and inventory totals.

use crate::economy::{
    HubEquipmentState, HubInventoryItem, HubItemKind, ItemRarity, project_inventory_items,
};
use crate::equipment::{
    NATIVE_EQUIPMENT_SETS, NativeEquipmentEffectSource, NativeEquipmentModifiers,
    NativeEquipmentScalarTransform, native_equipment_recipe_effects,
    resolve_native_equipment_effects,
};
use crate::profile::NATIVE_SKILL_ROW_COUNT;
use crate::session::GameSession;

use super::numbers::{count_scaled, signed, unsigned, write};
use super::spec::scales;

#[derive(Clone, Copy, Debug)]
pub struct MlBotPolicyPotionRow {
    pub item_id: u64,
    pub legal: bool,
}

#[derive(Clone, Debug)]
pub struct MlBotPolicyInventoryObservation {
    pub block_o: Vec<f32>,
    pub block_p: Vec<f32>,
    pub block_q: Vec<f32>,
    pub potions: Vec<MlBotPolicyPotionRow>,
}

pub fn observe_ml_bot_policy_inventory(
    session: &GameSession,
    player_id: &str,
    has_consumable: Option<&dyn Fn(&str) -> bool>,
) -> Result<MlBotPolicyInventoryObservation, String> {
    let participant = session
        .participant(player_id)
        .ok_or_else(|| format!("game simulation has no player {player_id}"))?;
    let player = &participant.player;
    let economy = &player.economy;
    let items = project_inventory_items(&economy.backpack);
    let mut potions = items
        .iter()
        .map(|row| row.item)
        .filter(|item| {
            item.native_type_id == 7001
                && matches!(
                    item.kind,
                    HubItemKind::Antidote
                        | HubItemKind::HealthPotion
                        | HubItemKind::ManaPotion
                        | HubItemKind::MindChug
                        | HubItemKind::ModPotion
                        | HubItemKind::RejuvenationPotion
                        | HubItemKind::WizardChug
                )
        })
        .collect::<Vec<_>>();
    // These are the seven fixed lowercase ASCII kind names; localeCompare and
    // byte ordering agree within this closed Source set.
    potions.sort_by(|left, right| {
        right
            .quantity
            .cmp(&left.quantity)
            .then(left.kind.as_str().cmp(right.kind.as_str()))
            .then(left.id.cmp(&right.id))
    });
    let mut block_o = vec![0.0; 12 * 19 + 2];
    let mut potion_rows = Vec::new();
    let mut potion_total = 0.0;
    for item in &potions {
        potion_total += f64::from(item.quantity);
    }
    for (slot, item) in potions.iter().take(12).enumerate() {
        let stock = if item.kind == HubItemKind::ModPotion { None } else { item.native_subtype };
        let identity = if item.kind == HubItemKind::ModPotion {
            item.mod_content.as_ref().map_or_else(
                || item.kind.as_str().to_owned(),
                |content| format!("{}:{}", content.mod_id, content.content_id),
            )
        } else {
            item.kind.as_str().to_owned()
        };
        let (hash_a, hash_b) = identity_hashes(&identity);
        // Source's potion durations are the same canonical progression programs:
        // Wizard Chug / Mind Chug 6000 ticks, antidote immunity 1000 ticks.
        let duration = match stock {
            Some(2 | 4) => duration_scaled(6000.0),
            Some(3) => duration_scaled(1000.0),
            _ => item.mod_content.as_ref().map_or(0.0, |content| {
                unsigned(content.duration_ms / 1000.0, scales::STATUS_DURATION_SECONDS)
            }),
        };
        write(
            &mut block_o,
            slot * 19,
            &[
                f64::from(item.quantity > 0),
                count_scaled(f64::from(item.quantity)),
                f64::from(stock == Some(0)),
                f64::from(stock == Some(1)),
                f64::from(stock == Some(2)),
                f64::from(stock == Some(3)),
                f64::from(stock == Some(4)),
                f64::from(stock == Some(5)),
                f64::from(item.kind == HubItemKind::ModPotion),
                f64::from(matches!(stock, Some(0 | 5))),
                f64::from(matches!(stock, Some(1 | 5))),
                if stock == Some(2) { 4.0 / scales::MULTIPLIER } else { 0.0 },
                f64::from(stock == Some(3)),
                if stock == Some(3) { duration_scaled(1000.0) } else { 0.0 },
                f64::from(stock == Some(4)),
                duration,
                0.0,
                hash_a,
                hash_b,
            ],
        );
        let legal = if item.quantity == 0
            || item.native_subtype.is_none()
            || !player.combat.can_accept_input()
            || session.level_up_barrier.is_some()
        {
            false
        } else if item.kind == HubItemKind::ModPotion {
            item.mod_content
                .as_ref()
                .is_some_and(|content| has_consumable.is_some_and(|has| has(&content.content_id)))
        } else {
            match item.native_subtype {
                Some(0) => player.combat.current_health < player.combat.maximum_health,
                Some(1) => player.combat.current_mana < player.combat.maximum_mana,
                Some(2) => player.progression.damage_x4_ticks_remaining < 6000,
                Some(3) => {
                    player.combat.poison_ticks_remaining > 0
                        || player.progression.poison_immunity_ticks_remaining < 1000
                }
                Some(4) => player.progression.mind_chug_ticks_remaining < 6000,
                Some(5) => {
                    player.combat.current_health < player.combat.maximum_health
                        || player.combat.current_mana < player.combat.maximum_mana
                }
                _ => false,
            }
        };
        potion_rows.push(MlBotPolicyPotionRow { item_id: item.id, legal });
    }
    block_o[228] = count_scaled(potions.len() as f64) as f32;
    block_o[229] = count_scaled(potion_total) as f32;

    let block_p = observe_equipment(&economy.equipment, &player.book.permanent_ranks);
    let count = |predicate: &dyn Fn(&HubInventoryItem) -> bool| {
        let mut total = 0.0;
        for row in &items {
            if predicate(row.item) {
                total += f64::from(row.item.quantity);
            }
        }
        count_scaled(total)
    };
    let block_q = [
        count(&|_| true),
        count(&|item| item.native_type_id == 7001),
        count(&|item| item.kind == HubItemKind::Equipment),
        count(&|item| item.kind == HubItemKind::Sack),
        count(&|item| item.kind == HubItemKind::Dye),
        count_scaled(economy.owned_perk_selectors.len() as f64),
        count(&|item| item.kind == HubItemKind::ModPotion && item.mod_content.is_some()),
        count(&|item| item.kind == HubItemKind::Key),
        f64::from(economy.has_wizard_key()),
    ]
    .map(|value| value as f32)
    .to_vec();
    Ok(MlBotPolicyInventoryObservation { block_o, block_p, block_q, potions: potion_rows })
}

fn observe_equipment(
    equipment: &HubEquipmentState,
    ranks: &[u32; NATIVE_SKILL_ROW_COUNT],
) -> Vec<f32> {
    let mut block = vec![0.0; 7 * 15];
    let slots = [
        &equipment.hat,
        &equipment.robe,
        &equipment.weapon,
        &equipment.rings[0],
        &equipment.rings[1],
        &equipment.rings[2],
        &equipment.amulet,
    ];
    let equipped_recipes =
        slots.iter().filter_map(|item| item.as_ref()?.recipe_index).collect::<Vec<_>>();
    for (slot, item) in slots.into_iter().enumerate() {
        let Some(item) = item else {
            continue;
        };
        let identity = item.recipe_index.map_or_else(
            || format!("{}:{}", item.native_type_id, item.name),
            |recipe| format!("{}:{recipe}", item.native_type_id),
        );
        let (hash_a, hash_b) = identity_hashes(&identity);
        let effects = item
            .native_effects
            .as_deref()
            .unwrap_or_else(|| item.recipe_index.map_or(&[][..], native_equipment_recipe_effects));
        let resolution = resolve_native_equipment_effects(
            ranks,
            &[NativeEquipmentEffectSource { effects: effects.to_vec(), recipe_index: None }],
            &[],
        );
        let tooltip_set = item.recipe_index.and_then(|recipe| {
            NATIVE_EQUIPMENT_SETS.iter().find(|set| set.member_recipe_indices.contains(&recipe))
        });
        let set_complete = tooltip_set.is_some_and(|set| {
            set.member_recipe_indices.iter().all(|id| equipped_recipes.contains(id))
        });
        let totals = family_totals(&resolution.modifiers);
        let targeted = effects.iter().find(|effect| effect.target != 0);
        write(
            &mut block,
            slot * 15,
            &[
                1.0,
                f64::from(item.recipe_index.is_some()),
                hash_a,
                hash_b,
                match item.rarity {
                    Some(ItemRarity::Epic) => 1.0,
                    Some(ItemRarity::Rare) => 0.5,
                    None => 0.0,
                },
                unsigned(f64::from(item.generated_level.unwrap_or(0)), scales::LEVEL),
                f64::from(set_complete),
                signed(totals.offense, scales::EQUIPMENT_EFFECT),
                signed(totals.resource, scales::EQUIPMENT_EFFECT),
                signed(totals.mobility, scales::EQUIPMENT_EFFECT),
                signed(totals.defense, scales::EQUIPMENT_EFFECT),
                f64::from(targeted.is_some()),
                targeted.map_or(0.0, |effect| {
                    unsigned(f64::from(effect.target), scales::EQUIPMENT_TARGET_KIND)
                }),
                targeted.map_or(0.0, |effect| signed(effect.magnitude, scales::EQUIPMENT_EFFECT)),
                f64::from(resolution.modifiers.feature_bits != 0),
            ],
        );
    }
    block
}

#[derive(Default)]
struct FamilyTotals {
    defense: f64,
    mobility: f64,
    offense: f64,
    resource: f64,
}
enum ModifierFamily {
    Defense,
    Mobility,
    Offense,
    Resource,
}

fn family_totals(modifiers: &NativeEquipmentModifiers) -> FamilyTotals {
    use ModifierFamily::*;
    let mut totals = FamilyTotals::default();
    // Retain closed-unions.ts Object.entries order and each array's own subtotal.
    // Reassociating the per-family sum changes the deployed Float32 observation.
    let contributions = [
        (Mobility, modifiers.cast_speed_flat),
        (Mobility, modifiers.cast_speed_multiplier - 1.0),
        (Mobility, sum(&modifiers.class_cast_speed_flat, false)),
        (Mobility, sum(&modifiers.class_cast_speed_multiplier, true)),
        (Offense, sum(&modifiers.class_damage_flat, false)),
        (Offense, sum(&modifiers.class_damage_multiplier, true)),
        (Resource, sum(&modifiers.class_mana_cost_flat, false)),
        (Resource, sum(&modifiers.class_mana_cost_multiplier, true)),
        (Resource, sum_transforms(&modifiers.class_recharge)),
        (Defense, modifiers.damage_resistance),
        (Offense, modifiers.global_damage_flat),
        (Offense, modifiers.global_damage_multiplier - 1.0),
        (Resource, modifiers.global_mana_cost_flat),
        (Resource, modifiers.global_mana_cost_multiplier - 1.0),
        (Mobility, modifiers.gold_multiplier - 1.0),
        (Resource, transform(modifiers.health_recovery)),
        (Defense, modifiers.magic_resistance),
        (Resource, transform(modifiers.mana_recovery)),
        (Defense, transform(modifiers.maximum_health)),
        (Resource, transform(modifiers.maximum_mana)),
        (Offense, modifiers.melee_damage_flat),
        (Offense, modifiers.melee_damage_multiplier - 1.0),
        (Mobility, modifiers.orb_pull_multiplier - 1.0),
        (Defense, modifiers.poison_resistance),
        (Resource, transform(modifiers.recharge)),
        (Offense, sum(&modifiers.skill_damage_flat, false)),
        (Offense, sum(&modifiers.skill_damage_multiplier, true)),
        (Mobility, transform(modifiers.walk_speed)),
        (Offense, modifiers.weld_effect - 1.0),
    ];
    for (family, value) in contributions {
        match family {
            Defense => totals.defense += value,
            Mobility => totals.mobility += value,
            Offense => totals.offense += value,
            Resource => totals.resource += value,
        }
    }
    totals
}

fn sum(values: &[f64], multiplier: bool) -> f64 {
    let mut total = 0.0;
    for value in values {
        total += if multiplier { value - 1.0 } else { *value };
    }
    total
}

fn transform(value: NativeEquipmentScalarTransform) -> f64 {
    value.offset + (value.scale - 1.0)
}

fn sum_transforms(values: &[NativeEquipmentScalarTransform]) -> f64 {
    let mut total = 0.0;
    for value in values {
        total += transform(*value);
    }
    total
}

fn identity_hashes(value: &str) -> (f64, f64) {
    if value.is_empty() {
        return (0.0, 0.0);
    }
    let mut first = 216_613_u64;
    let mut second = 104_729_u64;
    for (index, byte) in value.bytes().enumerate() {
        let ordinal = index as u64 + 1;
        first = (first * 167 + u64::from(byte) + ordinal) % 1_000_003;
        second = (second * 257 + u64::from(byte) * 3 + ordinal) % 1_000_033;
    }
    (first as f64 / 1_000_003.0, second as f64 / 1_000_033.0)
}

fn duration_scaled(ticks: f64) -> f64 {
    unsigned(ticks / scales::TICK_RATE, scales::STATUS_DURATION_SECONDS)
}
