//! Inventory and learned Skillbook geometry from `hub-inventory-render-contract.ts` and
//! `skill-book-model.ts`. The view reads canonical ownership; it never grants items or skills.

pub mod dialogue;
pub mod dye;
pub mod flyby;
pub mod inventory_drop;
pub mod item_text;
pub mod sack_pages;
pub mod sack_rename;
pub mod skill_hover;
pub mod skill_text;

use crate::economy::{EquipmentSlot, HubEconomyRejection, HubInventoryAction};
use crate::hud::HudRect;
use crate::profile::{
    NativeSkillBookState, NativeSkillDefinition, native_skill_category, native_skill_rule,
};

pub const INVENTORY_CAPACITY: usize = 88;
pub const INVENTORY_CELL_SIZE: f64 = 72.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InventoryRejectionNotice {
    pub title: &'static str,
    pub body: &'static str,
}

/// `HubInventorySurface.tsx` uses bad-action feedback for ordinary rejections. Only the
/// authored Hagatha, Shlorio and required-clothing cases open a message box; machine
/// reason codes are never product copy. The panel fixture retains the source strings.
#[must_use]
pub fn native_inventory_rejection_notice(
    action: &HubInventoryAction,
    reason: HubEconomyRejection,
) -> Option<InventoryRejectionNotice> {
    match (action, reason) {
        (HubInventoryAction::BuyHagatha { selector }, HubEconomyRejection::PerkCapacityFull) => {
            Some(InventoryRejectionNotice {
                title: "YOUR MIND IS FULL!",
                body: if *selector == 27 {
                    "Because the divinatorial phlogiston of your neurologic peridium is already at full capacity, drinking Hagatha's tonic would cause your head to explode!"
                } else {
                    "The Thaumic Covalence Meridian of your cortex is full and cannot hold more charms!\n\nDrinking Hagatha's tonic can sublimate the memetic sensorial pathways to allow more charms, but only if you're not already overloaded."
                },
            })
        }
        (HubInventoryAction::Dowse { .. }, HubEconomyRejection::InsufficientGold) => {
            Some(InventoryRejectionNotice {
                title: "NOT ENOUGH GOLD!",
                body: "Peering into the mirror at the endless, swirling, impossible colors of the ether is debilitating.  It is unthinkable that anyone would do so without just compensation, plus a little extra.",
            })
        }
        (
            HubInventoryAction::Unequip { slot: EquipmentSlot::Hat, .. },
            HubEconomyRejection::RequiredClothing,
        ) => Some(InventoryRejectionNotice {
            title: "A WIZARD WOULD NEVER REMOVE HIS HAT!",
            body: "A wizard might switch hats.  A wizard might even wear his hat at a jaunty angle.  But a wizard would never, under any circumstances, remove his hat altogether.\n\nAfter all, if you're not wearing a wizard hat, how would people know to be awed by the presence of a wizard?",
        }),
        (
            HubInventoryAction::Unequip { slot: EquipmentSlot::Robe, .. },
            HubEconomyRejection::RequiredClothing,
        ) => Some(InventoryRejectionNotice {
            title: "A WIZARD WOULD NEVER REMOVE HIS ROBE!",
            body: "A long, intimidating flowing robe looks debonaire on both a gluttonously fat slob and a pathetically wasted weakling.\n\nStrip away the robe and people might make comments about the kind of physique you get from years in wizarding school.  And then you'd have a completely avoidable disintegration on your conscience.",
        }),
        _ => None,
    }
}

/// The native backpack fills down each four-cell column before starting the next column.
#[must_use]
pub fn inventory_slot_rect(slot: usize) -> HudRect {
    assert!(slot < INVENTORY_CAPACITY, "backpack slot is visible");
    HudRect::new(24.0 + (slot / 4) as f64 * 75.0, 496.0 + (slot % 4) as f64 * 75.0, 72.0, 72.0)
}

#[must_use]
pub fn equipment_slot_rects(slot: EquipmentSlot) -> Vec<HudRect> {
    match slot {
        EquipmentSlot::Amulet => vec![HudRect::new(1300.0, 169.0, 46.0, 46.0)],
        EquipmentSlot::Hat => vec![HudRect::new(1354.0, 143.0, 72.0, 72.0)],
        EquipmentSlot::Weapon => {
            vec![HudRect::new(1274.0, 223.0, 72.0, 72.0), HudRect::new(1434.0, 223.0, 72.0, 72.0)]
        }
        EquipmentSlot::Robe => vec![HudRect::new(1354.0, 223.0, 72.0, 108.0)],
        EquipmentSlot::Ring0 => vec![HudRect::new(1300.0, 303.0, 46.0, 46.0)],
        EquipmentSlot::Ring1 => vec![HudRect::new(1434.0, 303.0, 46.0, 46.0)],
        EquipmentSlot::Ring2 => vec![HudRect::new(1434.0, 350.0, 46.0, 46.0)],
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkillBookPagePlacement {
    pub root_skill_id: usize,
    pub skills: Vec<usize>,
    pub width: f64,
    pub x: f64,
    pub y: f64,
}

/// `SkillScreen_BuildPages`: learned roots own pages and their transitive learned
/// dependents keep acquisition order. Rows wrap at 1590 units and share the widest origin.
#[must_use]
pub fn native_skill_book_page_placements(
    book: &NativeSkillBookState,
) -> Vec<SkillBookPagePlacement> {
    let mut learned = Vec::new();
    for id in &book.learned_skill_order {
        if book.permanent_ranks[*id] > 0 && native_skill_category(*id).is_some() {
            learned.push(*id);
        }
    }
    let mut pages = Vec::new();
    for root in &learned {
        let rule = native_skill_rule(*root).expect("learned skill has a rule");
        if !rule.all.is_empty() || !rule.any.is_empty() {
            continue;
        }
        let mut skills = vec![*root];
        for id in &learned {
            if id != root && depends_on(*id, *root, &learned, &mut [false; 83]) {
                skills.push(*id);
            }
        }
        pages.push(SkillBookPagePlacement {
            root_skill_id: *root,
            width: 200.0 + 160.0 * (skills.len() - 1) as f64,
            skills,
            x: 0.0,
            y: 0.0,
        });
    }
    let mut row_widths = vec![0.0_f64];
    let mut page_rows = Vec::new();
    for page in &pages {
        let row = row_widths.len() - 1;
        if row_widths[row] > 0.0 && row_widths[row] + page.width > 1590.0 {
            row_widths.push(page.width);
        } else {
            row_widths[row] += page.width;
        }
        page_rows.push(row_widths.len() - 1);
    }
    let widest = row_widths.iter().copied().fold(0.0_f64, f64::max);
    let origin_x = 800.0 - widest / 2.0;
    let origin_y = if row_widths.len() == 1 { 280.0 } else { 72.0 };
    let mut row_cursors = vec![origin_x; row_widths.len()];
    for (page, row) in pages.iter_mut().zip(page_rows) {
        page.x = row_cursors[row];
        page.y = origin_y + row as f64 * 300.0;
        row_cursors[row] += page.width;
    }
    pages
}

fn depends_on(id: usize, root: usize, learned: &[usize], seen: &mut [bool; 83]) -> bool {
    if seen[id] || !learned.contains(&id) {
        return false;
    }
    seen[id] = true;
    let rule = native_skill_rule(id).expect("learned skill has a rule");
    for dependency in rule.all.iter().chain(rule.any) {
        if *dependency == root || depends_on(*dependency, root, learned, seen) {
            return true;
        }
    }
    false
}

/// The authored tooltip grammar accepts lower-case format codes and a missing final `%`.
/// Numeric values come from the existing stat book, including rank-table saturation.
#[must_use]
pub fn format_native_skill_tooltip_line(
    source: &str,
    definition: &NativeSkillDefinition,
    rank: u32,
) -> String {
    let mut result = String::new();
    let mut index = 0;
    while index < source.len() {
        let rest = &source[index..];
        if rest.starts_with("%%") {
            result.push('%');
            index += 2;
            continue;
        }
        let bytes = rest.as_bytes();
        let format = bytes.get(1).copied().map(|byte| byte.to_ascii_uppercase());
        if bytes.first() == Some(&b'%')
            && bytes.get(2) == Some(&b':')
            && matches!(format, Some(b'D' | b'F' | b'N' | b'X'))
        {
            let mut end = index + 3;
            while let Some(byte) = source.as_bytes().get(end) {
                if !byte.is_ascii_alphanumeric() && *byte != b'_' {
                    break;
                }
                end += 1;
            }
            let value = definition.stat(&source[index + 3..end], rank).unwrap_or(0.0);
            let digits = match format.expect("format was checked") {
                b'D' => 0,
                b'F' => 1,
                b'X' => 2,
                _ => {
                    if value.fract() == 0.0 {
                        0
                    } else {
                        1
                    }
                }
            };
            result.push_str(&native_tooltip_fixed(value, digits));
            index = end + usize::from(source.as_bytes().get(end) == Some(&b'%'));
        } else {
            let character = rest.chars().next().expect("remaining source is non-empty");
            result.push(character);
            index += character.len_utf8();
        }
    }
    result.replace("[CR]", "\n").replace("\"\r\n\t\"", "\n").replace('"', "")
}

/// JavaScript `toFixed` rounds exact half values away from zero; Rust's formatter uses
/// ties-to-even. Work from the binary significand so 2.5 becomes 3 while 2.55 remains 2.5
/// at one decimal. Multiplying the float first would lose that second distinction.
#[must_use]
pub fn native_tooltip_fixed(value: f64, digits: u32) -> String {
    assert!(value.is_finite() && digits <= 2, "native tooltip stat range");
    if value.abs() >= 1e21 {
        return ryu_js::Buffer::new().format(value).to_owned();
    }
    let bits = value.abs().to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023 - 52;
    let significand = (bits & ((1_u64 << 52) - 1)) | (1_u64 << 52);
    let numerator = u128::from(significand) * 10_u128.pow(digits);
    let rounded = if value == 0.0 || exponent <= -128 {
        0
    } else if exponent >= 0 {
        numerator << exponent as u32
    } else {
        let denominator = 1_u128 << -exponent as u32;
        numerator / denominator + u128::from(numerator % denominator >= denominator / 2)
    };
    let sign = if value < 0.0 { "-" } else { "" };
    if digits == 0 {
        return format!("{sign}{rounded}");
    }
    let decimal_scale = 10_u128.pow(digits);
    let whole = rounded / decimal_scale;
    let fraction = rounded % decimal_scale;
    format!("{sign}{whole}.{fraction:0width$}", width = digits as usize)
}
