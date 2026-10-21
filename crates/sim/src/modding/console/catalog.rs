//! Existing stock tables projected as `web-lua-developer-grants.ts` catalogs.

use crate::economy::{
    DOWSING_EQUIPMENT_RECIPES, FOMENTIUS_STOCK_DEFINITIONS, HubInventoryItem,
    NATIVE_SKILL_BOOK_DEFINITIONS, create_equipment_inventory_item,
    create_fomentius_inventory_item, create_native_skill_book_inventory_item,
};
use crate::enemies::EnemyToken;
use crate::profile::{NATIVE_SKILL_CATALOG, NATIVE_WELD_BUILDS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WebLuaStockEnemy {
    pub key: &'static str,
    pub token: EnemyToken,
}
pub const WEB_LUA_STOCK_ENEMIES: [WebLuaStockEnemy; 11] = [
    WebLuaStockEnemy { key: "coffin", token: EnemyToken::Coffin },
    WebLuaStockEnemy { key: "demon_skull", token: EnemyToken::DemonSkull },
    WebLuaStockEnemy { key: "demon", token: EnemyToken::Demon },
    WebLuaStockEnemy { key: "dire_faculty", token: EnemyToken::DireFaculty },
    WebLuaStockEnemy { key: "heartmonger", token: EnemyToken::Heartmonger },
    WebLuaStockEnemy { key: "imp", token: EnemyToken::Imp },
    WebLuaStockEnemy { key: "skeleton", token: EnemyToken::Skeleton },
    WebLuaStockEnemy { key: "skeleton_archer", token: EnemyToken::SkeletonArcher },
    WebLuaStockEnemy { key: "skeleton_mage", token: EnemyToken::SkeletonMage },
    WebLuaStockEnemy { key: "wraith", token: EnemyToken::Wraith },
    WebLuaStockEnemy { key: "zombie", token: EnemyToken::Zombie },
];

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub struct WebLuaDeveloperItemDescriptor {
    pub key: String,
    pub kind: &'static str,
    pub name: &'static str,
    pub native_subtype: Option<u32>,
    pub native_type_id: u32,
    pub recipe_index: Option<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub struct WebLuaDeveloperSkillDescriptor {
    pub family: &'static str,
    pub id: usize,
    pub maximum_rank: u32,
    pub name: &'static str,
    pub weld_only: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize))]
pub struct WebLuaDeveloperWeldDescriptor {
    pub component_skill_ids: [usize; 6],
    pub id: u32,
    pub name: &'static str,
}

#[must_use]
pub fn web_lua_developer_items() -> Vec<WebLuaDeveloperItemDescriptor> {
    let mut rows = Vec::new();
    for definition in FOMENTIUS_STOCK_DEFINITIONS {
        rows.push(WebLuaDeveloperItemDescriptor {
            key: definition.kind.as_str().into(),
            kind: definition.kind.as_str(),
            name: definition.name,
            native_subtype: Some(definition.native_subtype),
            native_type_id: definition.native_type_id,
            recipe_index: None,
        });
    }
    for definition in NATIVE_SKILL_BOOK_DEFINITIONS {
        rows.push(WebLuaDeveloperItemDescriptor {
            key: definition.key.into(),
            kind: "skill-book",
            name: definition.name,
            native_subtype: Some(definition.native_subtype),
            native_type_id: definition.native_type_id,
            recipe_index: None,
        });
    }
    for recipe in DOWSING_EQUIPMENT_RECIPES {
        rows.push(WebLuaDeveloperItemDescriptor {
            key: format!("equipment:{}", recipe.source_index),
            kind: "equipment",
            name: recipe.name,
            native_subtype: None,
            native_type_id: recipe.native_type_id,
            recipe_index: Some(recipe.source_index),
        });
    }
    rows
}
#[must_use]
pub fn web_lua_developer_skills() -> Vec<WebLuaDeveloperSkillDescriptor> {
    NATIVE_SKILL_CATALOG
        .iter()
        .filter(|skill| (8..=79).contains(&skill.id) && skill.maximum_level > 0)
        .map(|skill| WebLuaDeveloperSkillDescriptor {
            family: skill.family,
            id: skill.id,
            maximum_rank: skill.maximum_level,
            name: skill.name,
            weld_only: skill.id == 52,
        })
        .collect()
}
#[must_use]
pub fn web_lua_developer_welds() -> Vec<WebLuaDeveloperWeldDescriptor> {
    NATIVE_WELD_BUILDS
        .iter()
        .map(|weld| WebLuaDeveloperWeldDescriptor {
            component_skill_ids: weld.component_skill_ids,
            id: weld.id,
            name: weld.synthetic_name,
        })
        .collect()
}
#[must_use]
pub fn web_lua_developer_skill(id: u32) -> Option<WebLuaDeveloperSkillDescriptor> {
    web_lua_developer_skills().into_iter().find(|skill| skill.id == id as usize)
}
#[must_use]
pub fn web_lua_developer_weld(id: u32) -> Option<WebLuaDeveloperWeldDescriptor> {
    web_lua_developer_welds().into_iter().find(|weld| weld.id == id)
}
#[must_use]
pub fn web_lua_developer_item_exists(key: &str) -> bool {
    web_lua_developer_items().iter().any(|item| item.key == key)
}

#[must_use]
pub fn create_web_lua_developer_grant_items(
    key: &str,
    quantity: f64,
) -> Option<Vec<HubInventoryItem>> {
    if !quantity.is_finite() || quantity.fract() != 0.0 || !(1.0..=100.0).contains(&quantity) {
        return None;
    }
    let quantity = quantity as u32;
    if let Some(definition) =
        FOMENTIUS_STOCK_DEFINITIONS.iter().find(|item| item.kind.as_str() == key)
    {
        if definition.native_type_id == 7001 {
            return Some(vec![create_fomentius_inventory_item(definition, 0, quantity)]);
        }
        return Some(
            (0..quantity).map(|_| create_fomentius_inventory_item(definition, 0, 1)).collect(),
        );
    }
    if let Some(definition) = NATIVE_SKILL_BOOK_DEFINITIONS.iter().find(|item| item.key == key) {
        return Some(
            (0..quantity).map(|_| create_native_skill_book_inventory_item(definition, 0)).collect(),
        );
    }
    let recipe = DOWSING_EQUIPMENT_RECIPES
        .iter()
        .find(|recipe| format!("equipment:{}", recipe.source_index) == key)?;
    Some((0..quantity).map(|_| create_equipment_inventory_item(recipe, 0)).collect())
}
