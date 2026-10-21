//! Loot item construction from Website `core-kernels/native-loot-items.ts`.
//! All drops use the same inventory item model as College services and saved profiles.

use crate::NativeRng;
use crate::appearance::{clamp_unit, rgb_tint};
use crate::boneyard::waves::native_survival_programs;
use crate::boneyard::waves::programs::BossKind;
use crate::economy::{
    DOWSING_EQUIPMENT_RECIPES, EquipmentRecipe, EquipmentType, FOMENTIUS_STOCK_DEFINITIONS,
    HubInventoryItem, HubItemKind, ItemRarity, NATIVE_SKILL_BOOK_DEFINITIONS, base_item,
    create_equipment_inventory_item, create_fomentius_inventory_item,
    create_native_skill_book_inventory_item,
};
use crate::enemies::EnemyToken;
use crate::equipment::NATIVE_EQUIPMENT_SETS;
use crate::equipment::random::generate_native_random_equipment_effects;
use crate::float::fround;

use super::{NativeLootItemIds, NativeLootSelectionInput};

#[derive(Clone, Debug, PartialEq)]
pub struct NativeGoodieContents {
    pub gold: u64,
    pub items: Vec<HubInventoryItem>,
}

#[must_use]
pub fn resolve_native_goodie_contents(
    rng: &mut NativeRng,
    item_ids: &mut NativeLootItemIds,
    selector: u32,
    player_level: u32,
    advanced_unlocks: &[bool; 8],
    owned_recipe_indexes: &[usize],
    ground_recipe_indexes: &[usize],
) -> NativeGoodieContents {
    assert!(selector <= 17, "native Goodie selector is in0..18");
    let mut contents = NativeGoodieContents { gold: 0, items: Vec::new() };
    match selector {
        0..=3 => {
            for _ in 0..5 {
                insert_goodie_item(&mut contents.items, potion_item(item_ids, 0));
            }
        }
        4..=7 => {
            for _ in 0..6 {
                insert_goodie_item(&mut contents.items, potion_item(item_ids, 1));
            }
        }
        8..=9 => {
            let count = rng.draw_integer(2) + 2;
            for _ in 0..count {
                let level_offset = rng.draw_integer(5) as u32;
                let item = random_equipment(
                    rng,
                    item_ids,
                    (player_level + level_offset) as i32,
                    advanced_unlocks,
                );
                insert_goodie_item(&mut contents.items, item);
            }
        }
        10 => {
            let candidates: Vec<_> = DOWSING_EQUIPMENT_RECIPES
                .iter()
                .filter(|recipe| {
                    recipe.level <= 100
                        && !owned_recipe_indexes.contains(&recipe.source_index)
                        && !ground_recipe_indexes.contains(&recipe.source_index)
                })
                .collect();
            if !candidates.is_empty() {
                let recipe = candidates[rng.draw_integer(candidates.len() as i32) as usize];
                let item = equipment_recipe_item(recipe, item_ids, rng);
                insert_goodie_item(&mut contents.items, item);
            }
        }
        11..=12 => {
            for _ in 0..3 {
                let subtype = rng.draw_integer(2) as u32 + 2;
                insert_goodie_item(&mut contents.items, misc_item(item_ids, subtype));
            }
        }
        13..=16 => {
            contents.gold = rng.draw_integer(3) as u64 * 300 + 500;
        }
        17 => {
            // The stock constructor consumes three addressed items before this overwrite.
            for _ in 0..3 {
                item_ids.allocate();
            }
            for subtype in [5, 0, 1, 4, 2, 2] {
                insert_goodie_item(&mut contents.items, potion_item(item_ids, subtype));
            }
        }
        _ => unreachable!("Goodie selector validated above"),
    }
    contents
}

fn insert_goodie_item(items: &mut Vec<HubInventoryItem>, item: HubInventoryItem) {
    if item.native_type_id == 7001
        && let Some(stack) = items.iter_mut().find(|candidate| {
            candidate.native_type_id == 7001 && candidate.native_subtype == item.native_subtype
        })
    {
        stack.quantity += item.quantity;
        return;
    }
    items.push(item);
}

#[must_use]
pub fn select_enemy_item(
    rng: &mut NativeRng,
    item_ids: &mut NativeLootItemIds,
    input: &NativeLootSelectionInput<'_>,
    item_recipe_pool: Option<&[usize]>,
) -> Option<HubInventoryItem> {
    if let Some(indexes) = item_recipe_pool {
        let mut pool = Vec::with_capacity(indexes.len());
        for index in indexes {
            let recipe = DOWSING_EQUIPMENT_RECIPES
                .get(*index)
                .expect("enemy Item pool requires valid named recipes");
            pool.push(recipe);
        }
        assert!(!pool.is_empty(), "enemy Item pool must not be empty");
        let mut missing = Vec::new();
        for recipe in &pool {
            if !input.participant.owned_recipe_indexes.contains(&recipe.source_index) {
                missing.push(*recipe);
            }
        }
        // The approved override ignores arena bounds and items on the ground.
        // If every pool member is owned, Source draws from the original pool.
        let candidates = if missing.is_empty() { &pool } else { &missing };
        let recipe = candidates[rng.draw_integer(candidates.len() as i32) as usize];
        return Some(equipment_recipe_item(recipe, item_ids, rng));
    }
    let recipes: Vec<_> = DOWSING_EQUIPMENT_RECIPES
        .iter()
        .filter(|recipe| {
            recipe.level as i32 >= input.arena.item_level_minimum
                && recipe.level as i32 <= input.arena.item_level_maximum
                && !input.participant.owned_recipe_indexes.contains(&recipe.source_index)
                && !input.ground_recipe_indexes.contains(&recipe.source_index)
        })
        .collect();
    let mut candidates = Vec::new();
    let mode = input.policies.specific_item;
    let mut append_rarity = |rarity| {
        for recipe in &recipes {
            if recipe.rarity == rarity {
                candidates.push(Some(*recipe));
            }
        }
    };
    match mode {
        0 => {
            if rng.draw_integer(15) == 1 {
                append_rarity(ItemRarity::Rare);
            }
            if rng.draw_integer(20) == 1 {
                append_rarity(ItemRarity::Epic);
            }
        }
        2 => append_rarity(ItemRarity::Rare),
        3 => append_rarity(ItemRarity::Epic),
        4 => candidates.extend(recipes.iter().map(|recipe| Some(*recipe))),
        _ => {}
    }
    if candidates.is_empty() && input.arena.mode == 1 {
        for rarity in [ItemRarity::Rare, ItemRarity::Epic] {
            if (mode == 3 && rarity == ItemRarity::Rare)
                || (mode == 2 && rarity == ItemRarity::Epic)
            {
                continue;
            }
            for recipe in &recipes {
                if recipe.rarity == rarity {
                    candidates.push(Some(*recipe));
                }
            }
        }
    }
    if input.arena.mode != 1 && (mode == 0 || mode == 1) {
        candidates.extend([None; 110]);
    }
    if candidates.is_empty() {
        return None;
    }
    let recipe = candidates[rng.draw_integer(candidates.len() as i32) as usize];
    Some(match recipe {
        Some(recipe) => equipment_recipe_item(recipe, item_ids, rng),
        None => {
            random_equipment(rng, item_ids, input.arena.level, input.participant.advanced_unlocks)
        }
    })
}

/// `direBossItemRecipePool` from current Website `core-kernels/dire-boss-loot.ts`.
/// Source identity and original recipe UID admit only the retained three faculty bosses.
#[must_use]
pub fn dire_boss_item_recipe_pool(
    source_sha256: Option<&str>,
    enemy_token: EnemyToken,
    recipe_uid: Option<usize>,
) -> Option<Vec<usize>> {
    if enemy_token != EnemyToken::DireFaculty {
        return None;
    }
    let recipe_uid = recipe_uid?;
    let programs = native_survival_programs(source_sha256?)?;
    let faculty = programs.bosses.iter().find(|definition| definition.kind == BossKind::Faculty)?;
    let recipe = faculty.recipes.iter().find(|recipe| recipe.uid == recipe_uid)?;
    let set_member = match recipe.name.as_str() {
        "Dire Sirmin" => 11,
        "Dire Lucritius" => 20,
        "Dire Aliss" => 16,
        _ => return None,
    };
    let set = NATIVE_EQUIPMENT_SETS
        .iter()
        .find(|set| set.member_recipe_indices.contains(&set_member))
        .expect("each approved faculty Item pool names a retained equipment set");
    let mut pool = set.member_recipe_indices.to_vec();
    if recipe.name == "Dire Lucritius" {
        pool.push(3);
    }
    Some(pool)
}

fn random_equipment(
    rng: &mut NativeRng,
    item_ids: &mut NativeLootItemIds,
    level: i32,
    advanced_unlocks: &[bool; 8],
) -> HubInventoryItem {
    let types = [
        EquipmentType::Hat,
        EquipmentType::Robe,
        EquipmentType::Staff,
        EquipmentType::Wand,
        EquipmentType::Ring,
        EquipmentType::Amulet,
    ];
    let equipment_type = types[rng.draw_integer(6) as usize];
    let selector_count = match equipment_type {
        EquipmentType::Amulet | EquipmentType::Ring => 12,
        EquipmentType::Hat => 4,
        EquipmentType::Robe => 3,
        EquipmentType::Staff | EquipmentType::Wand => 6,
    };
    let selector = rng.draw_integer(selector_count) as u32;
    let icon_tints = if matches!(equipment_type, EquipmentType::Hat | EquipmentType::Robe) {
        Some(random_wearable_colors(rng).map(Some))
    } else {
        None
    };
    let generated =
        generate_native_random_equipment_effects(rng, equipment_type, level, advanced_unlocks);
    let native_type_id = match equipment_type {
        EquipmentType::Amulet => 7003,
        EquipmentType::Hat => 7005,
        EquipmentType::Ring => 7002,
        EquipmentType::Robe => 7006,
        EquipmentType::Staff => 7004,
        EquipmentType::Wand => 7011,
    };
    let mut item = base_item(
        item_ids.allocate(),
        HubItemKind::Equipment,
        &generated.name,
        native_type_id,
        None,
        &equipment_icon_records(equipment_type, selector as usize),
    );
    item.equipment_type = Some(equipment_type);
    item.generated_level = Some(generated.item_level);
    item.icon_tints = icon_tints;
    item.native_effects = Some(generated.effects);
    item.native_selector = Some(selector);
    item
}

fn random_wearable_colors(rng: &mut NativeRng) -> [u32; 2] {
    let mut color = random_clothing_color(rng).map(clamp_unit);
    if rng.draw_integer(4) == 1 {
        color = color.map(|channel| clamp_unit(channel * 1.85));
    }
    let luminance = fround(
        fround(color[0] * (0.3086_f32 as f64))
            + fround(color[1] * (0.6094_f32 as f64))
            + fround(color[2] * (0.082_f32 as f64)),
    );
    let primary = color.map(|channel| {
        clamp_unit(fround(
            fround(luminance * (0.8_f32 as f64)) + fround(channel * ((1.0_f32 - 0.8_f32) as f64)),
        ))
    });
    [rgb_tint(primary), 0xffffff]
}

/// Retail `0x004630E0`: preserve out-of-range jitter until the caller's colour transform.
fn random_clothing_color(rng: &mut NativeRng) -> [f64; 3] {
    let palette = [
        [1.0, 0.0, 0.0],
        [1.0, 0.5, 0.0],
        [1.0, 1.0, 0.0],
        [0.25, 1.0, 0.25],
        [0.25, 1.0, 1.0],
        [0.25, 0.25, 1.0],
        [1.0, 0.25, 1.0],
        [0.4, 0.4, 0.4],
        [0.8, 0.8, 0.8],
    ];
    let mut color = palette[rng.draw_integer(palette.len() as i32) as usize].map(fround);
    if rng.draw_integer(2) == 1 {
        for channel in &mut color {
            *channel = fround(*channel + rng.draw_scaled_float(0.1_f32 as f64, true));
        }
    }
    color
}

fn equipment_icon_records(equipment_type: EquipmentType, selector: usize) -> Vec<usize> {
    match equipment_type {
        EquipmentType::Hat => vec![34 + selector, 38 + selector],
        EquipmentType::Robe => vec![64 + selector, 67 + selector],
        EquipmentType::Staff => vec![72 + selector],
        EquipmentType::Wand => vec![78 + selector],
        EquipmentType::Ring => vec![52 + selector],
        EquipmentType::Amulet => vec![30 + selector / 6, 18 + selector],
    }
}

#[must_use]
pub fn equipment_recipe_item(
    recipe: &EquipmentRecipe,
    item_ids: &mut NativeLootItemIds,
    rng: &mut NativeRng,
) -> HubInventoryItem {
    let mut item = create_equipment_inventory_item(recipe, item_ids.allocate());
    if matches!(recipe.equipment_type, EquipmentType::Hat | EquipmentType::Robe) {
        let primary = match recipe.icon_tints[0] {
            Some(tint) => tint_color(tint),
            None => random_clothing_color(rng),
        };
        let secondary = tint_color(recipe.icon_tints[1].unwrap_or(0xffffff));
        item.icon_tints =
            Some([Some(desaturate_named_color(primary)), Some(desaturate_named_color(secondary))]);
    }
    item.native_selector = Some(match recipe.equipment_type {
        EquipmentType::Hat => recipe.icon_records[0] - 34,
        EquipmentType::Robe => recipe.icon_records[0] - 64,
        EquipmentType::Staff => recipe.icon_records[0] - 72,
        EquipmentType::Wand => recipe.icon_records[0] - 78,
        EquipmentType::Ring => recipe.icon_records[0] - 52,
        EquipmentType::Amulet => recipe.icon_records[1] - 18,
    } as u32);
    item
}

fn tint_color(tint: u32) -> [f64; 3] {
    [
        fround(f64::from((tint >> 16) & 0xff) / 255.0),
        fround(f64::from((tint >> 8) & 0xff) / 255.0),
        fround(f64::from(tint & 0xff) / 255.0),
    ]
}

fn desaturate_named_color(color: [f64; 3]) -> u32 {
    // Named clones use native-color.ts: one luminance rounding, unlike random clothing.
    let luminance = fround(
        color[0] * (0.3086_f32 as f64)
            + color[1] * (0.6094_f32 as f64)
            + color[2] * (0.082_f32 as f64),
    );
    let amount = 0.8_f32 as f64;
    rgb_tint(
        color.map(|channel| fround(luminance * amount + channel * (1.0 - amount)).clamp(0.0, 1.0)),
    )
}

#[must_use]
pub fn potion_item(item_ids: &mut NativeLootItemIds, subtype: u32) -> HubInventoryItem {
    assert!(subtype <= 5, "native potion subtype is in0..6");
    let definition = FOMENTIUS_STOCK_DEFINITIONS
        .iter()
        .find(|row| row.native_type_id == 7001 && row.native_subtype == subtype)
        .expect("all six native potion definitions are retained");
    let mut item = create_fomentius_inventory_item(definition, item_ids.allocate(), 1);
    item.native_selector = Some(subtype);
    item
}

#[must_use]
pub fn misc_item(item_ids: &mut NativeLootItemIds, subtype: u32) -> HubInventoryItem {
    let mut item = match subtype {
        0 => {
            base_item(item_ids.allocate(), HubItemKind::Dye, "Fabric Dye Kit", 7012, Some(0), &[42])
        }
        1 => base_item(item_ids.allocate(), HubItemKind::Key, "Wizard Key", 7012, Some(1), &[43]),
        2 | 3 => create_native_skill_book_inventory_item(
            &NATIVE_SKILL_BOOK_DEFINITIONS[(subtype - 2) as usize],
            item_ids.allocate(),
        ),
        _ => panic!("native miscellaneous subtype is in0..4"),
    };
    item.native_selector = Some(subtype);
    item
}
