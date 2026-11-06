//! Sprite layers and painter layers for stock ground loot and Goodies. Gold scatter seeds
//! belong to the view; planning never advances the loot store's RNG.

use super::{BoneyardGoodiePhase, BoneyardGoodieState, BoneyardLootActor};
use crate::Vec2;
use crate::float;
use crate::hub::painter::{PainterLane, PainterLayer};
use crate::loot::{BonusKind, LootDropKind, OrbKind};
use crate::presentation::puppet_hit_tint;
use crate::spells::presentation::{SpellAtlas, SpellBlend};

/// One sprite layer of a ground loot or Goodie drawing: atlas entry, offset, scale,
/// rotation, tint, alpha and blend.
#[derive(Clone, Debug, PartialEq)]
pub struct LootVisualLayer {
    pub alpha: f64,
    pub atlas: SpellAtlas,
    pub blend: SpellBlend,
    /// Native diffuse RGB with sampled texture alpha, used for Goodie hit copies.
    pub diffuse_color: bool,
    pub entry: usize,
    pub offset: Vec2,
    pub role: String,
    pub rotation_radians: f64,
    pub scale: Vec2,
    pub tint: u32,
}

impl LootVisualLayer {
    fn new(role: &str, atlas: SpellAtlas, entry: usize) -> Self {
        Self {
            alpha: 1.0,
            atlas,
            blend: SpellBlend::Normal,
            diffuse_color: false,
            entry,
            offset: Vec2::ZERO,
            role: role.into(),
            rotation_radians: 0.0,
            scale: Vec2::new(1.0, 1.0),
            tint: 0xffffff,
        }
    }
}

/// The sprite layers for one ground loot actor and the gold scatter seed to use next.
#[derive(Clone, Debug, PartialEq)]
pub struct LootPresentationPlan {
    pub layers: Vec<LootVisualLayer>,
    pub next_scatter_seed: u32,
}

#[must_use]
pub fn loot_presentation_plan(
    actor: &BoneyardLootActor,
    scatter_seed: u32,
) -> LootPresentationPlan {
    let mut next_scatter_seed = scatter_seed;
    let layers = match actor.kind {
        LootDropKind::Orb => orb_layers(actor),
        LootDropKind::Gold => gold_layers(actor, &mut next_scatter_seed),
        LootDropKind::Sack => sack_layers(actor),
        LootDropKind::Bonus => bonus_layers(actor),
    };
    LootPresentationPlan { layers, next_scatter_seed }
}

#[must_use]
pub fn goodie_presentation_plan(
    goodie: &BoneyardGoodieState,
    tick: f64,
    hit_alpha: f64,
    complex_lighting: bool,
) -> Vec<LootVisualLayer> {
    let phase = match goodie.phase {
        BoneyardGoodiePhase::Intact => 0,
        BoneyardGoodiePhase::Opening => 1,
        BoneyardGoodiePhase::Broken => 2,
    };
    let mut layers = vec![LootVisualLayer::new("goodie-body", SpellAtlas::DeadHawg, 145 + phase)];
    if goodie.active && goodie.timer < 100 && !((tick / 10.0).trunc() as u64).is_multiple_of(2) {
        let mut indicator =
            LootVisualLayer::new("goodie-active-indicator", SpellAtlas::BadGuys, 33);
        indicator.offset.y = -40.0;
        layers.push(indicator);
    }
    if hit_alpha > 0.0 {
        let base_count = layers.len();
        for index in 0..base_count {
            let mut draw = layers[index].clone();
            draw.alpha *= hit_alpha;
            draw.diffuse_color = true;
            draw.role = format!("hit:{}", draw.role);
            draw.tint = puppet_hit_tint(complex_lighting, draw.tint);
            layers.push(draw);
        }
    }
    layers
}

#[must_use]
pub fn loot_painter_layer(actor: &BoneyardLootActor) -> PainterLayer {
    actor.painter_registration.layer(actor.position.y, 0.0)
}

#[must_use]
pub fn goodie_painter_layer(goodie: &BoneyardGoodieState) -> PainterLayer {
    PainterLayer {
        visible: true,
        is_insertion: false,
        manager_lane: PainterLane::Scenery,
        registration_ordinal: goodie.scenery_registration_ordinal,
        sort_bias: 0.0,
        world_y: goodie.position.y,
    }
}

#[must_use]
pub fn next_gold_scatter_seed(source: u32) -> u32 {
    let mut value = source;
    value ^= value << 21;
    value ^= value >> 11;
    value = (value ^ (value << 4)).wrapping_mul(0x0a67_cfcf);
    if value as i32 >= 0 {
        return value;
    }
    0x8000_0000_u32.wrapping_sub(value & 0x7fff_ffff)
}

fn radians(degrees: f64) -> f64 {
    degrees * std::f64::consts::PI / 180.0
}

fn orb_layers(actor: &BoneyardLootActor) -> Vec<LootVisualLayer> {
    let angle = radians(actor.animation_phase);
    let entry = if actor.orb_kind == Some(OrbKind::Health) { 434 } else { 435 };
    if actor.alpha < 1.0 {
        let scale = actor.orb_value * actor.alpha * 2.0;
        let mut draw = LootVisualLayer::new("orb-fade-in", SpellAtlas::BadGuys, entry);
        draw.offset.y = float::sin(angle) * 3.0 - 5.0 + float::cos(angle) * 15.0;
        draw.scale = Vec2::new(scale, scale);
        return vec![draw];
    }
    let scale = actor.orb_value * actor.alpha * (float::cos(angle) * 0.2 + 2.0);
    let mut core = LootVisualLayer::new("orb-core", SpellAtlas::BadGuys, entry);
    core.offset.y = float::sin(angle) * 3.0 - 5.0;
    core.scale = Vec2::new(scale, scale);
    let mut shimmer = core.clone();
    shimmer.role = "orb-white-additive".into();
    shimmer.alpha = float::sin(angle).powi(2) * 0.75;
    shimmer.blend = SpellBlend::Add;
    vec![core, shimmer]
}

fn gold_layers(actor: &BoneyardLootActor, seed: &mut u32) -> Vec<LootVisualLayer> {
    if !actor.scatter_active {
        let mut gold =
            LootVisualLayer::new("gold-settled", SpellAtlas::BadGuys, 198 + actor.tier as usize);
        gold.rotation_radians = radians(actor.rotation_deg);
        let mut layers = vec![gold];
        let pulse = float::sin(radians(actor.animation_phase));
        if pulse > 0.0 {
            let mut draw = LootVisualLayer::new("gold-pulse", SpellAtlas::BadGuys, 73);
            draw.blend = SpellBlend::Add;
            draw.offset = Vec2::new(-5.0, -5.0);
            draw.rotation_radians = radians(actor.animation_phase);
            draw.scale = Vec2::new(pulse * 1.25, pulse * 1.25);
            layers.push(draw);
        }
        return layers;
    }
    let frame = actor.scatter_progress.trunc().clamp(0.0, 9.0) as usize;
    let mut layers =
        vec![LootVisualLayer::new("gold-scatter-base", SpellAtlas::BadGuys, 188 + frame)];
    let copies = match actor.tier {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 4,
    };
    for index in 0..copies {
        *seed = next_gold_scatter_seed(*seed);
        let angle = radians(f64::from(*seed % 360));
        let mut draw = LootVisualLayer::new(
            &format!("gold-scatter-copy-{index}"),
            SpellAtlas::BadGuys,
            188 + actor.tier as usize,
        );
        draw.offset = Vec2::new(float::sin(angle) * 8.0, -float::cos(angle) * 8.0);
        layers.push(draw);
    }
    layers
}

fn sack_layers(actor: &BoneyardLootActor) -> Vec<LootVisualLayer> {
    let item = actor.item.as_ref();
    let mut layers = match item.map(|item| item.type_id) {
        Some(7001) => vec![LootVisualLayer::new(
            "potion-sack",
            SpellAtlas::BadGuys,
            436 + item.and_then(|item| item.subtype).unwrap_or(0) as usize,
        )],
        Some(7012) => vec![LootVisualLayer::new("misc-sack", SpellAtlas::BadGuys, 33)],
        _ => vec![
            LootVisualLayer::new("equipment-sack-support", SpellAtlas::BadGuys, 67),
            LootVisualLayer::new(
                "equipment-sack-shell",
                SpellAtlas::BadGuys,
                442 + actor.tier as usize,
            ),
        ],
    };
    for draw in &mut layers {
        draw.offset.y = actor.bounce_height;
    }
    layers
}

fn bonus_layers(actor: &BoneyardLootActor) -> Vec<LootVisualLayer> {
    let frame = actor.frame_phase.trunc().clamp(0.0, 17.0) as usize;
    let (tint, entry) = match actor.bonus_kind {
        Some(BonusKind::SkillChoice) => (0xffbfbf, 140 + frame),
        Some(BonusKind::SkillRank) => (0xbfffff, 122 + frame),
        _ => (0xd9ba70, 61),
    };
    let mut support = LootVisualLayer::new("bonus-support-clockwise", SpellAtlas::BadGuys, 7);
    support.alpha = actor.alpha * 0.5;
    support.blend = SpellBlend::Add;
    support.offset.y = -5.0;
    support.rotation_radians = radians(actor.rotation_deg);
    support.scale = Vec2::new(2.5, 2.5);
    support.tint = tint;
    let mut counter = support.clone();
    counter.role = "bonus-support-counterclockwise".into();
    counter.alpha = actor.alpha * 0.25;
    counter.rotation_radians = radians(-actor.rotation_deg * 0.5);
    counter.scale = Vec2::new(2.25, 2.25);
    counter.tint = 0xffffff;
    let mut body = LootVisualLayer::new("bonus-body", SpellAtlas::BadGuys, entry);
    body.alpha = actor.alpha;
    vec![support, counter, body]
}
