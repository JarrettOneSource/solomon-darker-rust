//! HUD primary/concentration choices from Website `hud-skill-selector.ts`.

use crate::hud::HudRect;
use crate::profile::{NATIVE_SKILL_CATALOG, NativeSkillBookState, native_skill_category};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHudSkillSelectorTarget {
    Primary,
    ConcentrationA,
    ConcentrationB,
}

impl NativeHudSkillSelectorTarget {
    #[must_use]
    pub const fn from_binding(binding: usize) -> Option<Self> {
        match binding {
            12 => Some(Self::Primary),
            16 => Some(Self::ConcentrationA),
            20 => Some(Self::ConcentrationB),
            _ => None,
        }
    }

    #[must_use]
    pub const fn title(self) -> &'static str {
        if matches!(self, Self::Primary) { "Select Primary Attack" } else { "Select Concentration" }
    }

    #[must_use]
    pub const fn concentration_slot(self) -> Option<usize> {
        match self {
            Self::Primary => None,
            Self::ConcentrationA => Some(0),
            Self::ConcentrationB => Some(1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeHudSkillSelectorOption {
    pub icon_record: usize,
    pub name: &'static str,
    pub skill_id: usize,
}

#[must_use]
pub fn native_hud_skill_selector_options(
    book: &NativeSkillBookState,
    concentrations: &[Option<usize>; 2],
    target: NativeHudSkillSelectorTarget,
) -> Vec<NativeHudSkillSelectorOption> {
    let category = if target == NativeHudSkillSelectorTarget::Primary { 1 } else { 3 };
    let excluded = target.concentration_slot().and_then(|slot| concentrations[1 - slot]);
    let mut options = Vec::new();
    for &skill_id in &book.learned_skill_order {
        if book.effective_ranks[skill_id] == 0
            || Some(skill_id) == excluded
            || native_skill_category(skill_id) != Some(category)
            || (skill_id == 52 && book.weld_build_id.is_none())
        {
            continue;
        }
        options.push(NativeHudSkillSelectorOption {
            icon_record: book.skill_icon_record(skill_id),
            name: NATIVE_SKILL_CATALOG[skill_id].name,
            skill_id,
        });
    }
    options.sort_by_key(|option| option.skill_id);
    options
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHudSkillSelectorLayout {
    pub panel: HudRect,
    pub option_left: f64,
    pub option_top: f64,
    pub strip_width: f64,
    pub title_y: f64,
}

#[must_use]
pub fn native_hud_skill_selector_layout(
    option_count: usize,
    title_width: f64,
) -> NativeHudSkillSelectorLayout {
    let strip_width = option_count as f64 * 52.0;
    let width = strip_width.max(title_width) + 10.0;
    NativeHudSkillSelectorLayout {
        panel: HudRect::new(800.0 - width / 2.0, 52.0, width, 79.0),
        option_left: 800.0 - strip_width / 2.0,
        option_top: 74.0,
        strip_width,
        title_y: 69.0,
    }
}
