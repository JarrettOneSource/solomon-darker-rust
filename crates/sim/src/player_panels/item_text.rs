//! Current Website renderer/hub-inventory-item-text.ts and the contextual hover box
//! in renderer/hub-inventory/items.ts. Item ownership stays in economy; these are
//! borrowed display rows and stock glyph geometry.

mod hagatha;

use crate::Vec2;
use crate::boneyard::tutorial::{
    NATIVE_TUTORIAL_AMULET_DESCRIPTION, native_tutorial_amulet_identity_matches,
};
use crate::economy::{
    DOWSING_EQUIPMENT_RECIPES, HAGATHA_PERKS, HubEconomyContext, HubEconomyState, HubInventoryItem,
    HubItemKind, ItemRarity, native_equipment_meets_level_requirement,
    native_equipment_required_level,
};
use crate::equipment::{
    NATIVE_EQUIPMENT_SETS, NativeEquipmentEffect, native_equipment_recipe_description,
    native_equipment_recipe_effects,
};
use crate::hud::HudRect;
use crate::native_ui_text::{
    NativeUiFontName, measure_native_ui_text, native_ui_font, wrap_native_ui_text,
};
use crate::profile::NATIVE_SKILL_CATALOG;

use super::native_tooltip_fixed;
use super::sack_rename::inventory_sack_name_needs_browser_font;

pub const HUB_TOOLTIP_CONTENT_MARGIN: f64 = 25.0;
pub const HUB_TOOLTIP_CONTENT_MAX_WIDTH: f64 = 300.0;
pub const HUB_TOOLTIP_LINE_GAP: f64 = 10.0;

const BODY: u32 = 0xbfbfbf;
const COMPLETE_SET: u32 = 0x80ff80;
const GOLD: u32 = 0xd9ba70;
const SET_NAME: u32 = 0xffbf80;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HubTooltipLine {
    pub font: NativeUiFontName,
    pub text: String,
    pub tint: u32,
    pub sack_name_browser_font: bool,
}

impl HubTooltipLine {
    fn body(text: impl Into<String>) -> Self {
        Self {
            font: NativeUiFontName::Body,
            text: text.into(),
            tint: BODY,
            sack_name_browser_font: false,
        }
    }

    fn gold(text: impl Into<String>) -> Self {
        Self { tint: GOLD, ..Self::body(text) }
    }

    fn title(text: impl Into<String>) -> Self {
        Self { font: NativeUiFontName::Menu, tint: 0xffffff, ..Self::body(text) }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct HubTooltipOptions<'a> {
    pub creativity_rank: u32,
    pub owned_recipe_indices: &'a [usize],
    pub player_level: Option<u32>,
    pub price: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
pub struct HagathaTooltipOptions<'a> {
    pub bundle_selectors: &'a [u32],
    pub cheat_death_charges: Option<u32>,
    pub first_mixed: bool,
    pub price: Option<u64>,
    pub selector: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HubInventoryItemInfoText {
    pub description: Option<String>,
    pub instruction: Option<String>,
    pub title: String,
}

/// Source service inspection includes nested belongings, storage and worn items.
#[must_use]
pub fn economy_recipe_indices(economy: &HubEconomyState) -> Vec<usize> {
    fn visit(items: &[HubInventoryItem], recipes: &mut Vec<usize>) {
        for item in items {
            if let Some(recipe) = item.recipe_index {
                recipes.push(recipe);
            }
            if let Some(contents) = &item.contents {
                visit(contents, recipes);
            }
        }
    }
    let mut recipes = Vec::new();
    visit(&economy.backpack, &mut recipes);
    visit(&economy.storage, &mut recipes);
    for item in [
        &economy.equipment.hat,
        &economy.equipment.robe,
        &economy.equipment.amulet,
        &economy.equipment.weapon,
        &economy.equipment.rings[0],
        &economy.equipment.rings[1],
        &economy.equipment.rings[2],
    ]
    .into_iter()
    .flatten()
    {
        visit(std::slice::from_ref(item), &mut recipes);
    }
    recipes
}

#[must_use]
pub fn hub_inventory_item_info_text(item: &HubInventoryItem) -> HubInventoryItemInfoText {
    let description = match item.kind {
        HubItemKind::HealthPotion => "Restores your health to maximum",
        HubItemKind::ManaPotion => "Restores your mana to maximum",
        HubItemKind::WizardChug => "Quadruples the damage of all attacks for 60 seconds",
        HubItemKind::Antidote => "Cures poisoning and grants immunity to poison for 10 seconds",
        HubItemKind::MindChug => "Grants concentration of all skills (at once) for 60 seconds",
        HubItemKind::RejuvenationPotion => "Restores your health and mana to maximum",
        HubItemKind::ModPotion => {
            &item.mod_content.as_ref().expect("mod potion content identity").description
        }
        HubItemKind::ModItem => {
            return HubInventoryItemInfoText {
                description: Some(
                    item.mod_item_content
                        .as_ref()
                        .expect("mod item content identity")
                        .description
                        .clone(),
                ),
                instruction: None,
                title: item.name.clone(),
            };
        }
        HubItemKind::Dye => {
            return HubInventoryItemInfoText {
                description: Some("Double click to dye an article of clothing".into()),
                instruction: None,
                title: "Fabric Dye Kit".into(),
            };
        }
        HubItemKind::Key => {
            return HubInventoryItemInfoText {
                description: Some("Bursts non-magical locks".into()),
                instruction: None,
                title: "Wizard Key".into(),
            };
        }
        HubItemKind::Sack => {
            let count = item.contents.as_ref().map_or(0, Vec::len);
            return HubInventoryItemInfoText {
                description: Some(match count {
                    0 => "Currently empty".into(),
                    1 => "Contains 1 item".into(),
                    _ => format!("Contains {count} items"),
                }),
                instruction: None,
                title: item.name.clone(),
            };
        }
        HubItemKind::SkillBook => {
            return HubInventoryItemInfoText {
                description: Some(if item.native_subtype == Some(2) {
                    "Double click to learn a new skill".into()
                } else {
                    "Double click to improve a learned skill".into()
                }),
                instruction: None,
                title: item.name.clone(),
            };
        }
        HubItemKind::Equipment => {
            return HubInventoryItemInfoText {
                description: item.mod_affixes.as_ref().map(|affixes| {
                    affixes.iter().map(|affix| affix.name.as_str()).collect::<Vec<_>>().join(" · ")
                }),
                instruction: None,
                title: item.name.clone(),
            };
        }
    };
    HubInventoryItemInfoText {
        description: Some(description.to_owned()),
        instruction: Some("Double-click to drink".into()),
        title: item.name.clone(),
    }
}

#[must_use]
pub fn hub_hagatha_tooltip_lines(options: &HagathaTooltipOptions<'_>) -> Vec<HubTooltipLine> {
    assert!((-1..28).contains(&options.selector), "native Hagatha tooltip selector");
    let mut lines = Vec::new();
    if options.selector == -1 {
        lines.push(HubTooltipLine::title("BARGAIN BUNDLE"));
        lines.push(HubTooltipLine::body("Get everything the last wizard got."));
        for selector in options.bundle_selectors {
            let perk = HAGATHA_PERKS.get(*selector as usize).expect("native bundle member");
            lines.push(HubTooltipLine::body(format!("        {}", perk.name)));
        }
    } else {
        let selector = options.selector as usize;
        lines.push(HubTooltipLine::title(HAGATHA_PERKS[selector].name));
        for text in hagatha::HAGATHA_NATIVE_TOOLTIP_LINES[selector] {
            lines.push(HubTooltipLine::body(*text));
        }
        if selector == 7
            && let Some(charges) = options.cheat_death_charges
        {
            lines.push(HubTooltipLine::body(if charges > 0 {
                format!("   Cheats remaining: {charges}")
            } else {
                "   Used up!".into()
            }));
        }
    }
    if let Some(price) = options.price {
        append_price(&mut lines, Some(price));
        if options.selector == -1 {
            lines.push(HubTooltipLine::gold("    Bulk discount: 50%"));
        } else if !options.first_mixed {
            lines.push(HubTooltipLine::gold("    High price due to first mixing."));
        }
    }
    lines
}

#[must_use]
pub fn hub_item_tooltip_lines(
    item: &HubInventoryItem,
    options: &HubTooltipOptions<'_>,
) -> Vec<HubTooltipLine> {
    let info = hub_inventory_item_info_text(item);
    let set = item.recipe_index.and_then(|recipe| {
        NATIVE_EQUIPMENT_SETS.iter().find(|set| set.member_recipe_indices.contains(&recipe))
    });
    let complete = set.is_some_and(|set| {
        set.member_recipe_indices.iter().all(|index| options.owned_recipe_indices.contains(index))
    });
    let tint = if complete {
        COMPLETE_SET
    } else {
        match item.rarity {
            Some(ItemRarity::Epic) => 0xffbf80,
            Some(ItemRarity::Rare) => 0xffff80,
            None => 0xffffff,
        }
    };
    let sack_name_browser_font = item.kind == HubItemKind::Sack
        && item.native_type_id == 7008
        && inventory_sack_name_needs_browser_font(&info.title);
    let mut lines =
        vec![HubTooltipLine { tint, sack_name_browser_font, ..HubTooltipLine::title(info.title) }];
    if item.kind != HubItemKind::Equipment {
        for text in [info.description, info.instruction].into_iter().flatten() {
            if !text.is_empty() {
                lines.push(HubTooltipLine::body(text));
            }
        }
        append_price(&mut lines, options.price);
        return lines;
    }
    let description = if let Some(content) = &item.mod_item_content {
        &content.description
    } else if let Some(recipe) = item.recipe_index {
        native_equipment_recipe_description(recipe)
    } else if native_tutorial_amulet_identity_matches(item) {
        NATIVE_TUTORIAL_AMULET_DESCRIPTION
    } else {
        ""
    };
    if !description.is_empty() {
        lines.push(HubTooltipLine::body(description));
    }
    if let Some(affixes) = info.description
        && !affixes.is_empty()
    {
        lines.push(HubTooltipLine::body(affixes));
    }
    if let Some(level) = options.player_level {
        let context = HubEconomyContext {
            player_level: level,
            creativity_rank: options.creativity_rank,
            ..HubEconomyContext::default()
        };
        if !native_equipment_meets_level_requirement(item, &context) {
            lines.push(HubTooltipLine {
                tint: 0xff8080,
                ..HubTooltipLine::body(format!(
                    "Requires Player Level {}",
                    native_equipment_required_level(item)
                ))
            });
        }
    }
    let effects = item
        .native_effects
        .as_deref()
        .unwrap_or_else(|| item.recipe_index.map_or(&[], native_equipment_recipe_effects));
    for effect in effects {
        lines.push(HubTooltipLine::body(hub_native_equipment_effect_text(*effect)));
    }
    if let Some(set) = set {
        lines.push(HubTooltipLine::body(""));
        lines.push(HubTooltipLine::gold("Item Set:"));
        lines.push(HubTooltipLine { tint: SET_NAME, ..HubTooltipLine::body(set.name) });
        for index in set.member_recipe_indices {
            lines.push(HubTooltipLine {
                tint: if options.owned_recipe_indices.contains(index) {
                    COMPLETE_SET
                } else {
                    BODY
                },
                ..HubTooltipLine::body(format!("  {}", DOWSING_EQUIPMENT_RECIPES[*index].name))
            });
        }
        lines.push(HubTooltipLine::body(""));
        lines.push(HubTooltipLine::gold("Complete Set Bonus:"));
        for effect in set.effects {
            lines.push(HubTooltipLine::body(hub_native_equipment_effect_text(*effect)));
        }
    }
    append_price(&mut lines, options.price);
    lines
}

fn append_price(lines: &mut Vec<HubTooltipLine>, price: Option<u64>) {
    if let Some(price) = price {
        assert!(price <= 9_007_199_254_740_991, "native tooltip price is a safe integer");
        lines.push(HubTooltipLine::body(""));
        lines.push(HubTooltipLine::gold(format!("    Price: {price}")));
    }
}

#[must_use]
pub fn hub_native_equipment_effect_text(effect: NativeEquipmentEffect) -> String {
    assert!(
        (1..=39).contains(&effect.kind) && effect.magnitude.is_finite() && effect.operator <= 2,
        "native equipment tooltip effect"
    );
    // Math.round ties toward positive infinity; toFixed(0) instead rounds away from
    // zero. Negative halves on generated/authored effects must preserve that distinction.
    let floor = effect.magnitude.floor();
    let rounded_value = if effect.magnitude - floor >= 0.5 { floor + 1.0 } else { floor };
    let rounded = ryu_js::Buffer::new().format(rounded_value).to_owned();
    let skill =
        || NATIVE_SKILL_CATALOG.get(effect.target as usize).expect("native tooltip skill").name;
    let class = || {
        ["Ether", "Fire", "Air", "Water", "Earth", "Body", "Mind", "Arcane"]
            .get(effect.target as usize)
            .copied()
            .expect("native tooltip class")
    };
    let value = || {
        if effect.operator == 1 {
            format!("x{}", native_tooltip_fixed(effect.magnitude, 1))
        } else {
            let suffix = if effect.operator == 2 { "%" } else { "" };
            let sign = if effect.magnitude <= 0.0 { "-" } else { "+" };
            let digits = if effect.operator == 2 && effect.magnitude <= 0.0 { 0 } else { 1 };
            format!("{sign}{}{suffix}", native_tooltip_fixed(effect.magnitude.abs(), digits))
        }
    };
    match effect.kind {
        4 if effect.target == 0 => "Grant Skill".into(),
        4 if effect.magnitude <= 1.0 => format!("Grant {}", skill()),
        4 => format!("Grant level {rounded} {}", skill()),
        5 if effect.target == 0 => "Boost Skill".into(),
        5 => format!("Boost {} + {rounded}", skill()),
        6 => format!("Boost {} + {rounded}", class()),
        7 if effect.target == 0 => "Add Skill".into(),
        7 => format!("{} + {rounded}", skill()),
        8 => format!("All Skills + {rounded}"),
        14 if effect.magnitude < 0.0 => {
            format!("Find {}% less gold", ryu_js::Buffer::new().format(rounded_value.abs()))
        }
        14 => format!("Find {rounded}% more gold"),
        15 if effect.operator == 1 => {
            format!("Pull orbs from {}x further away", native_tooltip_fixed(effect.magnitude, 1))
        }
        15 => format!("Pull orbs from {rounded}% further away"),
        18 => format!("Resist Pain +{rounded}%"),
        19 => format!("Resist Magic +{rounded}%"),
        20 => format!("Resist Poison +{rounded}%"),
        25 => format!("{} Damage {}", skill(), value()),
        26 => "Always summon max Leviathan tentacles".into(),
        27 => "Double the duration of magic storms".into(),
        28 => "Ring of fire explodes enemies".into(),
        29 => "Allows control of two golems".into(),
        30 => "Ring of Ice does frostburn damage".into(),
        31 => "Doubles the lifetime of imps".into(),
        32 => "Increases the disintegration threshold to 30%".into(),
        33 => "Doubles the rate of Ether Charge accumulation".into(),
        34 => "Harden shell retaliates with projectiles when struck".into(),
        35 => "Rock Surge causes a strong knockback when invoked".into(),
        36 => "Emits a mindblast on levelup".into(),
        37 => "Weld +unlearned components".into(),
        38 => format!("Enhance weld effects {}", value()),
        39 => "+Bias for welding skill picks".into(),
        kind => {
            let label = match kind {
                1 => "Spell Damage".into(),
                2 => format!("{} Damage", class()),
                3 => "Melee Damage".into(),
                9 => "Mana Recovery".into(),
                10 => "Mana Cost".into(),
                11 => format!("{} Mana Cost", class()),
                12 => "Cast Speed".into(),
                13 => format!("{} Cast Speed", class()),
                16 => "Health Recovery".into(),
                17 => "Walk Speed".into(),
                21 => "Spell Recharge".into(),
                22 => format!("{} Spell Recharge", class()),
                23 => "Max Health".into(),
                24 => "Max Mana".into(),
                _ => unreachable!("scalar tooltip kind"),
            };
            format!("{label} {}", value())
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HubTooltipRenderedLine {
    pub sources: Vec<String>,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HubContextualHoverLayout {
    pub rect: HudRect,
    pub rendered: Vec<HubTooltipRenderedLine>,
}

/// Browser Sack names supply actual measured metrics for their one unsupported row.
/// All stock rows are measured by the canonical atlas, exactly as the source.
#[must_use]
pub fn hub_contextual_hover_layout(
    lines: &[HubTooltipLine],
    source: Vec2,
    source_gap: f64,
    browser_metrics: Option<Vec2>,
) -> HubContextualHoverLayout {
    let mut rendered = Vec::new();
    let mut content_width: f64 = 0.0;
    let mut content_height = 0.0;
    for (index, line) in lines.iter().enumerate() {
        let row = if line.sack_name_browser_font {
            let measured = browser_metrics.expect("actual custom Sack browser metrics");
            HubTooltipRenderedLine {
                sources: vec![line.text.clone()],
                width: measured.x,
                height: measured.y,
            }
        } else {
            let sources = wrap_native_ui_text(
                &line.text,
                line.font,
                Some(HUB_TOOLTIP_CONTENT_MAX_WIDTH),
                1.0,
            );
            let mut width: f64 = 0.0;
            for text in &sources {
                width = width.max(measure_native_ui_text(text, line.font, 1.0));
            }
            let height = sources.len() as f64 * native_ui_font(line.font).metrics[0];
            HubTooltipRenderedLine { sources, width, height }
        };
        content_width = content_width.max(row.width);
        content_height += row.height;
        if index + 1 < lines.len() {
            content_height += HUB_TOOLTIP_LINE_GAP;
        }
        rendered.push(row);
    }
    let width = content_width + HUB_TOOLTIP_CONTENT_MARGIN * 2.0;
    let height = content_height + HUB_TOOLTIP_CONTENT_MARGIN * 2.0;
    HubContextualHoverLayout {
        rect: hub_contextual_hover_rect(Vec2::new(width, height), source, source_gap),
        rendered,
    }
}

#[must_use]
pub fn hub_contextual_hover_rect(size: Vec2, source: Vec2, source_gap: f64) -> HudRect {
    let width = size.x;
    let height = size.y;
    let mut x = source.x + source_gap;
    if x + width > 1575.0 {
        x = source.x - source_gap - width;
    }
    x = x.min(1575.0 - width).max(25.0);
    let y = (source.y - height / 2.0).min(875.0 - height).max(25.0);
    HudRect::new(x, y, width, height)
}
