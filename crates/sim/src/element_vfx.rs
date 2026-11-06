//! Animated element sprites for the five elements and selected welds, as per-tick draw plans.
//! These value plans serve Create, held spells and equipped staffs; no renderer or random state
//! is involved.

use crate::float;
use crate::profile::{WeldBuildId, WizardElement};

/// A sprite family used by element effects; `records` lists its frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementVfxSprite {
    Air,
    Aura,
    Core,
    Earth,
    Fire,
    Ray,
    Spark,
    Steam,
    Water,
}

impl ElementVfxSprite {
    /// Every entry belongs to the shared stock BadGuys atlas.
    #[must_use]
    pub const fn records(self) -> &'static [usize] {
        match self {
            Self::Air => &[1836, 1837, 1838, 1839],
            Self::Aura => &[15],
            Self::Core => &[110],
            Self::Earth => &[238, 239, 240, 241, 242, 243, 244, 245],
            Self::Fire => &[255, 256, 257, 258, 259, 260, 261, 262, 263, 264, 265, 266],
            Self::Ray => &[112],
            Self::Spark => &[111],
            Self::Steam => &[2002, 2003, 2004, 2005, 2006, 2007],
            Self::Water => &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 281, 282],
        }
    }

    /// Registered texture-cell dimensions; aura and steam use their raw atlas crop.
    #[must_use]
    pub const fn registered_size(self) -> Option<[f64; 2]> {
        match self {
            Self::Air => Some([55.0, 59.0]),
            Self::Core => Some([27.0, 26.0]),
            Self::Earth => Some([50.0, 50.0]),
            Self::Fire => Some([32.0, 54.0]),
            Self::Ray | Self::Spark => Some([40.0, 40.0]),
            Self::Water => Some([38.0, 36.0]),
            Self::Aura | Self::Steam => None,
        }
    }
}

/// How an element sprite is blended: normally or additively.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementVfxBlend {
    Normal,
    Add,
}

/// One sprite draw in an element effect plan: sprite and frame, offset, scale, rotation, tint,
/// alpha, blend and an optional anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElementVfxDraw {
    pub alpha: f64,
    pub anchor: Option<[f64; 2]>,
    pub blend: ElementVfxBlend,
    pub color: [f64; 3],
    pub frame: usize,
    pub rotation: f64,
    pub scale: f64,
    pub sprite: ElementVfxSprite,
    pub x: f64,
    pub y: f64,
}

impl ElementVfxDraw {
    const fn new(sprite: ElementVfxSprite, scale: f64) -> Self {
        Self {
            alpha: 1.0,
            anchor: None,
            blend: ElementVfxBlend::Normal,
            color: WHITE,
            frame: 0,
            rotation: 0.0,
            scale,
            sprite,
            x: 0.0,
            y: 0.0,
        }
    }
}

pub const ELEMENT_VFX_HELD_SCALE: f64 = 6.0;
pub const ELEMENT_VFX_PICKER_SCALE: f64 = 2.0;
pub const ELEMENT_VFX_STAFF_SCALE: f64 = 1.0;

const WHITE: [f64; 3] = [1.0, 1.0, 1.0];
const AIR: [f64; 3] = [0.5, 0.75, 0.75];
const EARTH_DARK: [f64; 3] = [0.5, 0.65, 0.5];
const EARTH_LIGHT: [f64; 3] = [0.75, 0.95, 0.75];
const ETHER: [f64; 3] = [1.0, 0.5, 1.0];
const FIRE: [f64; 3] = [1.0, 0.5, 0.0];
const WATER: [f64; 3] = [1.0, 0.75, 1.0];
const STEAM_ANCHORS: [[f64; 2]; 6] = [
    [16.0 / 34.0, 17.0 / 33.0],
    [18.0 / 37.0, 17.0 / 34.0],
    [17.0 / 35.0, 16.0 / 34.0],
    [16.0 / 34.0, 16.0 / 34.0],
    [16.0 / 35.0, 17.0 / 35.0],
    [16.0 / 34.0, 18.0 / 35.0],
];

#[must_use]
pub fn element_vfx_plan(element: WizardElement, tick: f64, scale: f64) -> Vec<ElementVfxDraw> {
    element_vfx_plan_at_phase(element, tick.floor(), scale)
}

#[must_use]
pub fn element_vfx_plan_at_phase(
    element: WizardElement,
    phase: f64,
    scale: f64,
) -> Vec<ElementVfxDraw> {
    match element {
        WizardElement::Air => air_plan(phase, scale),
        WizardElement::Earth => earth_plan(phase, scale),
        WizardElement::Ether => ether_plan(phase, scale),
        WizardElement::Fire => fire_plan(phase, scale),
        WizardElement::Water => water_plan(phase, scale),
    }
}

/// The plan for a selected primary: an elemental primary's skill ID or a Weld build ID.
#[must_use]
pub fn selected_primary_element_vfx_plan(
    skill_id: u32,
    tick: f64,
    scale: f64,
) -> Vec<ElementVfxDraw> {
    let tick = tick.floor();
    if let Ok(build) = WeldBuildId::try_from(skill_id) {
        return selected_weld_plan(build, tick, scale);
    }
    let (element, doubled) = match skill_id {
        8 => (WizardElement::Ether, false),
        16 => (WizardElement::Fire, false),
        24 => (WizardElement::Air, false),
        32 => (WizardElement::Water, false),
        40 => (WizardElement::Earth, false),
        // Codes 1010 to 1014 follow the ten Weld builds, one per element, and draw that
        // element twice. No build uses them.
        1010 => (WizardElement::Ether, true),
        1011 => (WizardElement::Fire, true),
        1012 => (WizardElement::Water, true),
        1013 => (WizardElement::Air, true),
        1014 => (WizardElement::Earth, true),
        _ => return Vec::new(),
    };
    let mut plan = element_vfx_plan_at_phase(element, tick, scale);
    if doubled {
        plan.extend(element_vfx_plan_at_phase(element, tick, scale));
    }
    plan
}

fn frame(tick: f64, count: usize, divisor: f64) -> usize {
    (tick / divisor).floor().rem_euclid(count as f64) as usize
}

fn pulse(tick: f64, degrees_per_tick: f64, base: f64) -> f64 {
    float::sin(tick * degrees_per_tick * std::f64::consts::PI / 180.0).abs() * 0.15 + base
}

fn js_i32(value: f64) -> i32 {
    value.trunc().rem_euclid(4_294_967_296.0) as u32 as i32
}

fn integer_hash(value: i32) -> u32 {
    let mut mixed = value.wrapping_shl(21) ^ value;
    mixed ^= ((mixed as u32) >> 11) as i32;
    mixed = (mixed.wrapping_shl(4) ^ mixed).wrapping_mul(0x0a67_cfcf);
    mixed.unsigned_abs()
}

fn visual_random(tick: f64, channel: u32) -> f64 {
    let salted = js_i32(tick + 1.0) ^ (channel.wrapping_add(1).wrapping_mul(0x045d_9f3b) as i32);
    f64::from(integer_hash(salted)) / 2_147_483_648.0
}

fn ether_plan(tick: f64, scale: f64) -> Vec<ElementVfxDraw> {
    let mut plan = Vec::new();
    let mut channel = 0;
    let mut random = || {
        let value = visual_random(tick, channel);
        channel += 1;
        value
    };
    for _ in 0..2 {
        plan.push(ElementVfxDraw {
            alpha: 0.2 + random() * 0.25,
            color: ETHER,
            ..ElementVfxDraw::new(ElementVfxSprite::Core, pulse(tick, 15.0, 2.5) * scale)
        });
        plan.push(ElementVfxDraw {
            alpha: 0.35 + random() * 0.55,
            color: ETHER,
            ..ElementVfxDraw::new(ElementVfxSprite::Core, pulse(tick, 5.0, 1.5) * scale)
        });
        plan.push(ElementVfxDraw {
            alpha: float::sin(tick * 5.0 * std::f64::consts::PI / 180.0).abs() * 0.35,
            blend: ElementVfxBlend::Add,
            rotation: float::sin(tick * std::f64::consts::PI / 180.0) * scale * 50.0,
            ..ElementVfxDraw::new(ElementVfxSprite::Spark, (1.0 + random() * 0.1) * scale)
        });
        let count = (random() * 10.0).floor() as usize + 2;
        for _ in 0..count {
            let radius = random() * 20.0 * scale;
            let angle = random() * std::f64::consts::PI * 2.0;
            let particle_scale = (0.25 + random() * 0.2) * scale;
            let alpha = random() * 0.75;
            let rotation = random() * 360.0;
            plan.push(ElementVfxDraw {
                alpha,
                blend: ElementVfxBlend::Add,
                rotation,
                x: float::sin(angle) * radius,
                y: -float::cos(angle) * radius,
                ..ElementVfxDraw::new(ElementVfxSprite::Spark, particle_scale)
            });
        }
        plan.push(ElementVfxDraw {
            alpha: float::sin(tick * 8.0 * std::f64::consts::PI / 180.0).abs() * 0.55,
            blend: ElementVfxBlend::Add,
            rotation: float::sin(tick * 0.5 * std::f64::consts::PI / 180.0) * scale * 50.0,
            ..ElementVfxDraw::new(ElementVfxSprite::Ray, (1.0 + random() * 0.3) * scale)
        });
    }
    plan
}

fn fire_plan(tick: f64, scale: f64) -> Vec<ElementVfxDraw> {
    let frame = frame(tick, 12, 5.0);
    vec![
        ElementVfxDraw {
            alpha: 0.2 + visual_random(tick, 0) * 0.25,
            color: FIRE,
            ..ElementVfxDraw::new(ElementVfxSprite::Core, pulse(tick, 15.0, 3.5) * scale)
        },
        ElementVfxDraw {
            frame,
            blend: ElementVfxBlend::Add,
            ..ElementVfxDraw::new(ElementVfxSprite::Fire, 2.0 * scale)
        },
        ElementVfxDraw {
            frame,
            alpha: 0.5,
            ..ElementVfxDraw::new(ElementVfxSprite::Fire, 2.0 * scale)
        },
    ]
}

fn air_plan(tick: f64, scale: f64) -> Vec<ElementVfxDraw> {
    let primary = pulse(tick, 15.0, 3.5);
    let secondary = pulse(tick, 1.0, 3.5);
    let stage = frame(tick, 8, 1.0);
    let mut hash = integer_hash(js_i32((tick / 8.0).trunc()));
    let angle = f64::from(hash % 36_000) / 360_000.0 * 360.0 * std::f64::consts::PI / 180.0;
    hash = integer_hash(hash as i32);
    let radius = f64::from(hash % 36_000) / 360_000.0 * 10.0;
    hash = integer_hash(hash as i32);
    let rotation = f64::from(hash % 36_000) / 360_000.0 * 360.0;
    hash = integer_hash(hash as i32);
    let particle_scale = (0.75 + f64::from(hash % 36_000) / 360_000.0 * 0.25) * scale;
    hash = integer_hash(hash as i32);
    let frame = (hash % 4) as usize;
    let alpha = float::sin(stage as f64 * std::f64::consts::PI / 8.0);
    vec![
        ElementVfxDraw {
            alpha: 0.2 + visual_random(tick, 0) * 0.25,
            blend: ElementVfxBlend::Add,
            color: AIR,
            ..ElementVfxDraw::new(ElementVfxSprite::Core, primary * scale)
        },
        ElementVfxDraw {
            alpha: 0.5,
            blend: ElementVfxBlend::Add,
            color: AIR,
            ..ElementVfxDraw::new(ElementVfxSprite::Core, secondary * 0.75 * scale)
        },
        ElementVfxDraw {
            alpha: 0.5,
            blend: ElementVfxBlend::Add,
            color: AIR,
            ..ElementVfxDraw::new(ElementVfxSprite::Core, secondary * 0.5 * scale)
        },
        ElementVfxDraw {
            alpha: 0.25,
            blend: ElementVfxBlend::Add,
            color: AIR,
            ..ElementVfxDraw::new(
                ElementVfxSprite::Core,
                secondary * (0.2 + visual_random(tick, 1) * 0.2) * scale,
            )
        },
        ElementVfxDraw {
            alpha,
            blend: ElementVfxBlend::Add,
            frame,
            rotation,
            x: float::sin(angle) * radius,
            y: -float::cos(angle) * radius,
            ..ElementVfxDraw::new(ElementVfxSprite::Air, particle_scale)
        },
        ElementVfxDraw {
            alpha: alpha * 0.25,
            blend: ElementVfxBlend::Add,
            frame: 3 - frame,
            rotation: rotation + 90.0,
            x: float::sin(angle) * radius,
            y: -float::cos(angle) * radius,
            ..ElementVfxDraw::new(ElementVfxSprite::Air, scale)
        },
    ]
}

fn water_plan(tick: f64, scale: f64) -> Vec<ElementVfxDraw> {
    let mut plan = vec![
        ElementVfxDraw {
            frame: frame(tick, 12, 8.0),
            ..ElementVfxDraw::new(ElementVfxSprite::Water, 1.8 * scale)
        },
        ElementVfxDraw {
            alpha: 0.2 + visual_random(tick, 0) * 0.25,
            blend: ElementVfxBlend::Add,
            color: WATER,
            ..ElementVfxDraw::new(ElementVfxSprite::Core, pulse(tick, 15.0, 3.5) * scale)
        },
    ];
    for pass in 0..2 {
        plan.push(ElementVfxDraw {
            alpha: float::sin(tick * 11.0 * std::f64::consts::PI / 180.0).abs() * 0.55,
            rotation: float::sin(tick * 0.5 * std::f64::consts::PI / 180.0) * scale * 50.0,
            ..ElementVfxDraw::new(
                ElementVfxSprite::Ray,
                (1.0 + visual_random(tick, pass + 1) * 0.3) * scale,
            )
        });
    }
    plan
}

fn earth_plan(tick: f64, scale: f64) -> Vec<ElementVfxDraw> {
    let frame = frame(tick, 8, 1.0);
    vec![
        ElementVfxDraw {
            color: EARTH_DARK,
            frame: 7 - frame,
            ..ElementVfxDraw::new(ElementVfxSprite::Earth, 1.5 * scale)
        },
        ElementVfxDraw {
            alpha: 0.1,
            blend: ElementVfxBlend::Add,
            frame,
            ..ElementVfxDraw::new(ElementVfxSprite::Earth, 1.8 * scale)
        },
        ElementVfxDraw {
            alpha: 0.2 + visual_random(tick, 0) * 0.25,
            blend: ElementVfxBlend::Add,
            color: EARTH_LIGHT,
            ..ElementVfxDraw::new(ElementVfxSprite::Core, pulse(tick, 15.0, 3.5) * scale)
        },
        ElementVfxDraw {
            alpha: 0.25,
            blend: ElementVfxBlend::Add,
            ..ElementVfxDraw::new(ElementVfxSprite::Core, pulse(tick, 1.0, 1.0) * scale)
        },
    ]
}

fn multiply_plan(
    mut plan: Vec<ElementVfxDraw>,
    color: [f64; 3],
    alpha: f64,
    blend: Option<ElementVfxBlend>,
) -> Vec<ElementVfxDraw> {
    for draw in &mut plan {
        draw.alpha *= alpha;
        if let Some(blend) = blend {
            draw.blend = blend;
        }
        for (channel, multiplier) in draw.color.iter_mut().zip(color) {
            *channel *= multiplier;
        }
    }
    plan
}

fn air_companion_plan(tick: f64, scale: f64) -> Vec<ElementVfxDraw> {
    air_plan(tick, scale).into_iter().skip(4).collect()
}

fn steam_plan(tick: f64, scale: f64) -> Vec<ElementVfxDraw> {
    let green = (0.5 + visual_random(tick, 0) * 0.5) * (0.5 + visual_random(tick, 1) * 0.5);
    let alpha = 0.2 + visual_random(tick, 2) * 0.25;
    let frame = frame(tick, 6, 8.0);
    vec![
        ElementVfxDraw {
            alpha,
            anchor: Some(STEAM_ANCHORS[frame]),
            color: [1.0, green, 0.0],
            frame,
            ..ElementVfxDraw::new(ElementVfxSprite::Steam, 1.8 * scale)
        },
        ElementVfxDraw {
            alpha,
            color: [1.0, green, 0.0],
            ..ElementVfxDraw::new(ElementVfxSprite::Core, pulse(tick, 15.0, 3.5) * scale)
        },
    ]
}

fn selected_weld_plan(build: WeldBuildId, tick: f64, scale: f64) -> Vec<ElementVfxDraw> {
    let mut plan = match build {
        WeldBuildId::BurningBolt => fire_plan(tick, scale),
        WeldBuildId::FrostMissile | WeldBuildId::BlizzardBeam => water_plan(tick, scale),
        WeldBuildId::BallLightning => air_plan(tick, scale),
        WeldBuildId::FlameLash => multiply_plan(earth_plan(tick, scale), FIRE, 1.0, None),
        WeldBuildId::SteamJet => steam_plan(tick, scale),
        WeldBuildId::EtherealBoulder => {
            multiply_plan(earth_plan(tick, scale * 0.75), ETHER, 1.0, None)
        }
        WeldBuildId::MeteorSwarm | WeldBuildId::Hailstones => earth_plan(tick, scale),
        WeldBuildId::CrawlingShock => vec![ElementVfxDraw {
            color: [0.5, 0.75, 0.5],
            rotation: tick * 8.0,
            ..ElementVfxDraw::new(
                ElementVfxSprite::Aura,
                (1.5 + float::sin(tick * std::f64::consts::PI / 180.0).abs() * 0.5) * scale,
            )
        }],
    };
    match build {
        WeldBuildId::BurningBolt | WeldBuildId::FrostMissile | WeldBuildId::BallLightning => {
            plan.extend(multiply_plan(ether_plan(tick, scale), WHITE, 0.25, None))
        }
        WeldBuildId::FlameLash => {
            plan.push(ElementVfxDraw {
                color: [1.0, 0.75, 0.0],
                ..ElementVfxDraw::new(ElementVfxSprite::Core, scale * 2.0)
            });
            if scale > 1.0 {
                plan.push(ElementVfxDraw {
                    alpha: (scale - 1.0).min(1.0),
                    blend: ElementVfxBlend::Add,
                    color: [1.0, 0.75, 0.0],
                    ..ElementVfxDraw::new(ElementVfxSprite::Core, scale * 2.0)
                });
            }
            plan.extend(air_companion_plan(tick, scale));
        }
        WeldBuildId::BlizzardBeam => plan.extend(air_companion_plan(tick, scale)),
        WeldBuildId::SteamJet => {}
        WeldBuildId::EtherealBoulder => plan.extend(multiply_plan(
            earth_plan(tick, scale),
            ETHER,
            1.0,
            Some(ElementVfxBlend::Add),
        )),
        WeldBuildId::MeteorSwarm => plan.extend(fire_plan(tick, scale)),
        WeldBuildId::Hailstones => plan.extend(water_plan(tick, scale)),
        WeldBuildId::CrawlingShock => {
            plan.extend(multiply_plan(air_plan(tick, scale * 1.25), WHITE, 0.5, None))
        }
    }
    plan
}
