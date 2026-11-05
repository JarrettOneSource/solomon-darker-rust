//! Source sprite transforms from `renderer/primary-spell-air-water-native.ts`.
use crate::Vec2;
use crate::cold_aura::{ColdAuraVisualInput, cold_aura_visual_state};
use crate::float::fround;
use crate::spells::model::{
    PrimarySpellAirHurricane, PrimarySpellWaterAura, PrimarySpellWaterHail,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HurricaneSpriteRole {
    Core,
    Lane,
    LaneCopy,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HurricaneSpritePlan {
    pub alpha: f64,
    pub additive: bool,
    pub position: Vec2,
    pub role: HurricaneSpriteRole,
    pub rotation_radians: f64,
    pub scale: Vec2,
    pub tint: u32,
}

#[must_use]
pub fn hurricane_visual_plan(state: &PrimarySpellAirHurricane) -> Vec<HurricaneSpritePlan> {
    let degrees = std::f64::consts::PI / 180.0;
    let mut plans = vec![HurricaneSpritePlan {
        alpha: fround(state.charge * 0.75),
        additive: false,
        position: Vec2::new(0.0, -15.0),
        role: HurricaneSpriteRole::Core,
        rotation_radians: state.phase_degrees * 1.5 * degrees,
        scale: Vec2::new(5.0, 4.0),
        tint: 0xf2ffff,
    }];
    for (index, lane) in state.lanes.iter().enumerate() {
        if !state.enhanced_effects && !index.is_multiple_of(2) {
            continue;
        }
        let plan = HurricaneSpritePlan {
            alpha: fround(state.charge * 0.4000000059604645),
            additive: true,
            position: Vec2::new(0.0, -(lane.vertical_offset + 15.0)),
            role: HurricaneSpriteRole::Lane,
            rotation_radians: lane.angle_degrees * degrees,
            scale: Vec2::new(lane.radius, fround(lane.radius * 0.800000011920929)),
            tint: 0xccffff,
        };
        plans.push(plan);
        if state.enhanced_effects {
            plans.push(HurricaneSpritePlan {
                role: HurricaneSpriteRole::LaneCopy,
                rotation_radians: lane.angle_degrees * 0.75 * degrees,
                ..plan
            });
        }
    }
    plans
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterAuraVisualPlan {
    pub alpha: f64,
    pub rotation_radians: f64,
    pub scale: f64,
    pub tint: u32,
}
#[must_use]
pub fn water_aura_visual_plan(state: &PrimarySpellWaterAura) -> WaterAuraVisualPlan {
    let native = cold_aura_visual_state(ColdAuraVisualInput {
        age_ticks: state.common.age_ticks as f64,
        alpha_decay: state.alpha_decay,
        initial_rotation_degrees: state.initial_rotation_degrees,
        rotation_step_degrees: state.rotation_step_degrees,
    });
    WaterAuraVisualPlan {
        alpha: native.alpha,
        rotation_radians: native.rotation_degrees * (std::f64::consts::PI / 180.0),
        scale: native.scale,
        tint: ((native.red * 255.0).round() as u32) << 16 | 0x00ffff,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HailVisualPlan {
    pub alpha: f64,
    pub offset_y: f64,
    pub rotation_radians: f64,
    pub scale: f64,
}
#[must_use]
pub fn hail_visual_plan(state: &PrimarySpellWaterHail) -> HailVisualPlan {
    HailVisualPlan {
        alpha: state.life.min(1.0),
        offset_y: state.height,
        rotation_radians: state.rotation_degrees * (std::f64::consts::PI / 180.0),
        scale: state.scale,
    }
}
