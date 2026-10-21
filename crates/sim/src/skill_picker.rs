//! Native skill-offer presentation from `skill-picker-render-contract.ts` and
//! `level-up-presentation.ts`. Offers and acceptance remain in `profile::progression`.

use crate::hud::HudRect;
use crate::profile::{
    NATIVE_SKILL_CATALOG, PlayerSkillOfferOption, SPELL_WELDING_SKILL_ID, native_skill_root,
    native_weld_build,
};

pub const SKILL_PICKER_ROOT_TINTS: [u32; 8] =
    [0xffe5ff, 0xffcbcb, 0xe5ffff, 0xcbcbff, 0xcbffcb, 0xffe5cb, 0xcbd8ff, 0xe5e5e5];
const ROOT_LABELS: [&str; 8] =
    [" ETHER", " FIRE", " AIR", " WATER", " EARTH", "BODY ", "MIND ", "ARCANE "];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkillPickerReveal {
    pub ambient_alpha: f64,
    pub curtain_alpha: f64,
    pub interactive: bool,
    pub panel_alpha: f64,
    pub reveal_alpha: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkillPickerCard {
    pub family_label: &'static str,
    pub frame_record: usize,
    pub glow_tints: Vec<u32>,
    pub icon_record: usize,
    pub name: String,
    pub quick_description: &'static str,
    pub root: usize,
    pub root_tint: u32,
}

#[must_use]
pub fn skill_picker_reveal(elapsed: f64) -> SkillPickerReveal {
    let alpha = (elapsed.max(0.0) / 10.0).floor().min(40.0) / 40.0;
    reveal_frame(alpha, alpha == 1.0)
}

#[must_use]
pub fn skill_picker_close(elapsed: f64, direction: f64) -> SkillPickerReveal {
    assert!(direction == -1.0 || direction == -0.75, "native picker close direction");
    let alpha = (1.0 + direction * 0.025 * (elapsed.max(0.0) / 10.0).floor()).max(0.0);
    reveal_frame(alpha, false)
}

fn reveal_frame(alpha: f64, interactive: bool) -> SkillPickerReveal {
    SkillPickerReveal {
        ambient_alpha: alpha * 0.1,
        curtain_alpha: alpha,
        interactive,
        panel_alpha: alpha.powi(3),
        reveal_alpha: alpha,
    }
}

#[must_use]
pub fn skill_picker_centers(count: usize) -> &'static [f64] {
    match count {
        3 => &[600.0, 800.0, 1000.0],
        4 => &[500.0, 700.0, 900.0, 1100.0],
        _ => panic!("native skill offer has three or four choices"),
    }
}

#[must_use]
pub fn skill_picker_panel(count: usize) -> HudRect {
    let _ = skill_picker_centers(count);
    let width = count as f64 * 200.0 + 60.0;
    HudRect::new((1600.0 - width) / 2.0, 272.5, width, 355.0)
}

#[must_use]
pub fn skill_picker_card(option: &PlayerSkillOfferOption) -> SkillPickerCard {
    let skill = &NATIVE_SKILL_CATALOG[option.skill_id];
    let weld = if option.skill_id == SPELL_WELDING_SKILL_ID {
        Some(
            native_weld_build(option.weld_build_id.expect("offered weld build"))
                .expect("native weld"),
        )
    } else {
        assert!(option.weld_build_id.is_none(), "ordinary choice has no weld build");
        None
    };
    let root = native_skill_root(option.skill_id).expect("offered skill root");
    let name = if let Some(weld) = weld {
        weld.synthetic_name.to_owned()
    } else if option.target_rank > 1 {
        format!("{} {}", skill.name, option.target_rank).to_uppercase()
    } else {
        skill.name.to_uppercase()
    };
    SkillPickerCard {
        family_label: ROOT_LABELS[root],
        frame_record: if weld.is_some() { 14 } else { 5 },
        glow_tints: weld.map_or_else(
            || vec![SKILL_PICKER_ROOT_TINTS[root]],
            |weld| weld.color_roots.map(|root| SKILL_PICKER_ROOT_TINTS[root]).to_vec(),
        ),
        icon_record: weld.map_or(skill.icon_record, |weld| weld.icon_record),
        name,
        quick_description: weld.map_or(skill.quick_description, |weld| weld.pair_description),
        root,
        root_tint: SKILL_PICKER_ROOT_TINTS[root],
    }
}
