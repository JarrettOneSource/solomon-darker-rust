//! Stock Harden chips and breakup shards: their creation, flight and sprites.
//! Their presentation draws share the gameplay stream and precede armor reduction.

use crate::float;
use crate::float::fround;
use crate::profile::harden_break_angle_step;
use crate::spells::model::PrimarySpellTransientBase;
use crate::spells::presentation::{SpellBlend, SpellDrawQueue, SpellSprite, SpellSpritePlan};
use crate::{Rng, Vec2};

/// One Harden shard's flight: position, height, bounce, spin, remaining life and sprite
/// record.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HardenShardBody {
    pub bounce_velocity: f64,
    #[cfg_attr(feature = "persistence", serde(default = "legacy_enhanced_shadow"))]
    pub enhanced_shadow: bool,
    pub height: f64,
    pub life: f64,
    pub position: Vec2,
    pub record: u32,
    pub rotation_degrees: f64,
    pub rotation_step_degrees: f64,
    pub velocity: Vec2,
    pub vertical_velocity: f64,
}

/// The transient fields every Harden effect shares, and the tick that sets its age.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HardenEffectOwner {
    pub common: PrimarySpellTransientBase,
    pub birth_tick: u64,
}

impl HardenEffectOwner {
    #[must_use]
    pub fn new(id: u64, owner_id: &str, world_key: &str, birth_tick: u64) -> Self {
        Self {
            common: PrimarySpellTransientBase {
                age_ticks: 0,
                id,
                light_registration: None,
                owner_id: owner_id.into(),
                painter_registrations: Vec::new(),
                world_key: world_key.into(),
            },
            birth_tick,
        }
    }
}

/// A live Harden transient: a flying shard, or the fading burst where the coating broke.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum HardenEffect {
    Shard { owner: HardenEffectOwner, body: HardenShardBody },
    Burst { owner: HardenEffectOwner, alpha: f64, position: Vec2 },
}

impl HardenEffect {
    #[must_use]
    pub fn common(&self) -> &PrimarySpellTransientBase {
        match self {
            Self::Shard { owner, .. } | Self::Burst { owner, .. } => &owner.common,
        }
    }

    pub fn common_mut(&mut self) -> &mut PrimarySpellTransientBase {
        match self {
            Self::Shard { owner, .. } | Self::Burst { owner, .. } => &mut owner.common,
        }
    }
}

/// A shard a hit chips off the Harden coating, and the pitch of its shatter sound.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HardenChip {
    pub pitch: f64,
    pub shard: HardenShardBody,
}

/// What breaking a Harden coating needs: the coating it had, where and when it broke, its
/// owner and world, the first free effect ID and whether enhanced effects are on.
pub struct HardenBreakupInput<'a> {
    pub coating: f64,
    pub position: Vec2,
    pub owner_id: &'a str,
    pub world_key: &'a str,
    pub tick: u64,
    pub first_id: u64,
    pub enhanced_effects: bool,
}

/// The shards and burst a broken Harden coating spawns, the next free effect ID and the
/// pitch of its shatter sound.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct HardenBreakup {
    pub effects: Vec<HardenEffect>,
    pub next_id: u64,
    pub pitch: f64,
}

pub fn create_harden_chip(
    position: Vec2,
    rng: &mut Rng,
    enhanced_effects: bool,
) -> Option<HardenChip> {
    if rng.draw_integer(3) != 1 {
        return None;
    }
    let pitch = rng.draw_scaled_float(0.1_f32 as f64, false);
    let heading = rng.draw_scaled_float(360.0, false);
    let shard = create_shard_body(position, heading, rng, enhanced_effects);
    Some(HardenChip { pitch: fround(1.0 + pitch), shard })
}

pub fn create_harden_breakup(input: HardenBreakupInput<'_>, rng: &mut Rng) -> HardenBreakup {
    let pitch = rng.draw_scaled_float(0.1_f32 as f64, true);
    let mut next_id = input.first_id;
    let mut effects = Vec::new();
    if let Some(step) = harden_break_angle_step(input.coating) {
        let mut heading = 0.0;
        while heading < 360.0 {
            let body = create_shard_body(input.position, heading, rng, input.enhanced_effects);
            let owner =
                HardenEffectOwner::new(next_id, input.owner_id, input.world_key, input.tick);
            next_id += 1;
            effects.push(HardenEffect::Shard { owner, body });
            heading += step;
        }
        let owner = HardenEffectOwner::new(next_id, input.owner_id, input.world_key, input.tick);
        next_id += 1;
        effects.push(HardenEffect::Burst { owner, alpha: 1.0, position: input.position });
    }
    HardenBreakup { effects, next_id, pitch: fround(1.0 + pitch) }
}

#[cfg(feature = "persistence")]
const fn legacy_enhanced_shadow() -> bool {
    true
}

fn create_shard_body(
    origin: Vec2,
    heading: f64,
    rng: &mut Rng,
    enhanced_effects: bool,
) -> HardenShardBody {
    let bounce = rng.draw_scaled_float(3.0, false);
    let height = rng.draw_scaled_float(20.0, false);
    let rotation = rng.draw_scaled_float(360.0, false);
    let spin = rng.draw_scaled_float(10.0, false);
    let record = rng.draw_integer(5) as u32;
    let jitter = rng.draw_scaled_float(10.0, true);
    let speed = rng.draw_scaled_float(0.5, false);
    let lead = rng.draw_scaled_float(10.0, false);
    let radians = fround(heading + jitter) * std::f64::consts::PI / 180.0;
    let velocity = Vec2::new(
        fround(fround(fround(float::sin(radians)) * 1.5) * fround(1.0 + speed)),
        fround(fround(-float::cos(radians)) * fround(1.0 + speed)),
    );
    let distance = fround(10.0 + lead);
    let mut position = Vec2::new(
        fround(origin.x + fround(velocity.x * distance)),
        fround(origin.y + fround(velocity.y * distance)),
    );
    // The shipped factory applies its last two-velocity offset only to X.
    position.x = fround(position.x + velocity.x + velocity.x);
    let bounce = fround(-(2.0 + bounce));
    HardenShardBody {
        bounce_velocity: bounce,
        enhanced_shadow: enhanced_effects,
        height: fround(-height),
        life: if enhanced_effects { 10.0 } else { 2.0 },
        position,
        record: 446 + record,
        rotation_degrees: rotation,
        rotation_step_degrees: fround(1.0 + spin),
        velocity,
        vertical_velocity: bounce,
    }
}

/// Returns whether the effect remains alive. Birth ticks determine age even when
/// an airborne shard skips its physics update on every third simulation tick.
pub fn step_harden_effect(
    effect: &mut HardenEffect,
    tick: u64,
    rng: &mut Rng,
    can_land_at: impl Fn(Vec2) -> bool,
) -> bool {
    let (owner, body) = match effect {
        HardenEffect::Burst { owner, alpha, .. } => {
            owner.common.age_ticks = tick - owner.birth_tick;
            *alpha = fround(*alpha - (0.05_f32 as f64));
            return *alpha > 0.0;
        }
        HardenEffect::Shard { owner, body } => (owner, body),
    };
    owner.common.age_ticks = tick - owner.birth_tick;
    if body.height != 0.0 && tick.is_multiple_of(3) {
        return true;
    }
    body.life = fround(body.life - (0.015_f32 as f64));
    if body.life <= 0.0 {
        return false;
    }
    if body.height == 0.0 {
        return true;
    }
    body.position = Vec2::new(
        fround(body.position.x + body.velocity.x),
        fround(body.position.y + body.velocity.y),
    );
    body.height = fround(body.height + body.vertical_velocity);
    body.vertical_velocity = fround(body.vertical_velocity + (0.4_f32 as f64));
    let mut blocked = false;
    if body.height > 0.0 {
        blocked = !can_land_at(body.position);
        let spin = rng.draw_scaled_float(10.0, false);
        let damp = rng.draw_integer(2);
        body.rotation_step_degrees = fround(1.0 + spin);
        body.bounce_velocity = fround(body.bounce_velocity * (0.65_f32 as f64));
        body.vertical_velocity = body.bounce_velocity;
        if damp == 1 {
            body.velocity = Vec2::new(
                fround(body.velocity.x * (0.65_f32 as f64)),
                fround(body.velocity.y * (0.65_f32 as f64)),
            );
        }
        if body.vertical_velocity > -0.75 {
            body.vertical_velocity = 0.0;
            body.bounce_velocity = 0.0;
            body.rotation_step_degrees = 0.0;
            body.velocity = Vec2::ZERO;
        }
        body.height = body.vertical_velocity;
    }
    body.rotation_degrees = fround(body.rotation_degrees + body.rotation_step_degrees);
    !blocked
}

/// The exact stock sprites and painter queue for a Harden shard or burst.
#[must_use]
pub fn harden_effect_plan(effect: &HardenEffect) -> SpellSpritePlan {
    let (position, sort_bias, draw) = match effect {
        HardenEffect::Shard { body, .. } => (
            body.position,
            0.0,
            SpellSprite {
                alpha: body.life.min(1.0),
                blend: SpellBlend::Add,
                offset: Vec2::new(0.0, body.height),
                queue: SpellDrawQueue::PostWorld,
                rotation: body.rotation_degrees * std::f64::consts::PI / 180.0,
                ..SpellSprite::new(body.record as usize, 1.0)
            },
        ),
        HardenEffect::Burst { alpha, position, .. } => {
            (*position, 10.0, SpellSprite { alpha: *alpha, ..SpellSprite::new(15, 3.5) })
        }
    };
    let mut draws = Vec::new();
    if let HardenEffect::Shard { body, .. } = effect
        && body.enhanced_shadow
    {
        // AdditiveBouncer preserves its Add blend for the black inherited shadow;
        // drawing it as a normal shadow would incorrectly darken the floor.
        draws.push(SpellSprite {
            offset: Vec2::new(0.0, 2.0),
            scale: Vec2::new(1.0, 0.75),
            tint: 0,
            ..draw
        });
    }
    draws.push(draw);
    SpellSpritePlan { position, world_y: position.y, sort_bias, region_light_point: None, draws }
}
