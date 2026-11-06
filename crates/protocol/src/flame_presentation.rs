//! Flame Lash presentation births sent with snapshots, and their connection history.
//! These records replay presentation births; they never advance gameplay or RNG.

use serde::{Deserialize, Serialize};
use solomon_sim::Vec2;
use solomon_sim::painter::PainterRegistration;
use solomon_sim::spells::model::{PrimarySpellTransient, SpellState};
use solomon_sim::spells::weld::{
    FlameChannelVisual, FlameFadeVisual, WeldActorBirth, WeldChannelActorState,
    WeldFlameLashFadeState, WeldFlameLashFadeVariant, WeldWorldActor, flame_fade_alpha,
};

mod client;
mod history;
mod validation;
mod wire;
pub use client::RetainedFlamePresentation;
pub use history::{
    FlamePresentationBirthBytes, FlamePresentationHistory, FlamePresentationHistoryOptions,
    FlamePresentationReplay,
};

pub const MAX_FLAME_PRESENTATION_BIRTHS: usize = 16_384;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// Whether a presentation birth is a Flame Lash ribbon or a fade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlamePresentationKind {
    Ribbon,
    Fade,
}

/// The visual parameters a client needs to redraw one Flame Lash ribbon or fade.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum FlamePresentationRecipe {
    Ribbon {
        #[serde(with = "wire::ChannelVisual")]
        birth: FlameChannelVisual,
        enhanced_effects: bool,
        weak: bool,
        #[serde(with = "wire::Registration")]
        painter_registration: PainterRegistration,
    },
    Fade {
        #[serde(with = "wire::FadeVisual")]
        birth: FlameFadeVisual,
        #[serde(with = "wire::Point")]
        position: Vec2,
        alpha_step: f64,
        base_scale: f64,
        wrapper_scalar: f64,
        #[serde(with = "wire::FadeVariant")]
        variant: WeldFlameLashFadeVariant,
        #[serde(with = "wire::Registration")]
        painter_registration: PainterRegistration,
    },
}

impl FlamePresentationRecipe {
    #[must_use]
    pub const fn kind(&self) -> FlamePresentationKind {
        match self {
            Self::Ribbon { .. } => FlamePresentationKind::Ribbon,
            Self::Fade { .. } => FlamePresentationKind::Fade,
        }
    }
}

/// One Flame Lash presentation birth: its identity, owner, world, tick interval and recipe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlamePresentationBirth {
    pub id: u64,
    pub kind: FlamePresentationKind,
    pub owner_id: String,
    pub world_key: String,
    pub birth_tick: u64,
    pub end_tick_exclusive: u64,
    pub recipe: FlamePresentationRecipe,
}

impl FlamePresentationBirth {
    fn same_identity(&self, other: &Self) -> bool {
        self.world_key == other.world_key
            && self.owner_id == other.owner_id
            && self.kind == other.kind
            && self.id == other.id
            && self.birth_tick == other.birth_tick
    }
    fn same_recipe(&self, other: &Self) -> bool {
        self.same_identity(other) && self.recipe == other.recipe
    }
    fn maximum_end_tick(&self) -> u64 {
        self.birth_tick + if self.kind == FlamePresentationKind::Fade { 5 } else { 1 }
    }
    fn validate(&self, world_key: &str) -> Result<(), String> {
        require_tick(self.birth_tick)?;
        require_tick(self.end_tick_exclusive)?;
        if self.id == 0
            || self.id > MAX_SAFE_INTEGER
            || !(self.world_key == world_key
                || (world_key == "hub" && self.world_key.starts_with("hub:")))
            || self.owner_id.is_empty()
            || self.kind != self.recipe.kind()
            || self.end_tick_exclusive < self.birth_tick
            || self.end_tick_exclusive > self.maximum_end_tick()
        {
            return Err("Invalid Flame presentation birth interval or identity".into());
        }
        Ok(())
    }
}

/// The history epoch and world tick a client has acknowledged; the next frame carries
/// only births after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlamePresentationBaseline {
    pub epoch: u64,
    pub world_tick: u64,
}

/// The Flame births sent with a snapshot: a reset that starts a new epoch, or every birth
/// changed after the client's acknowledged tick.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum FlamePresentationFrame {
    Reset {
        epoch: u64,
        world_key: String,
        through_tick: u64,
        births: [FlamePresentationBirth; 0],
    },
    Continuous {
        epoch: u64,
        world_key: String,
        through_tick: u64,
        after_tick: u64,
        births: Vec<FlamePresentationBirth>,
    },
}

impl FlamePresentationFrame {
    #[must_use]
    pub fn reset(epoch: u64, world_key: String, through_tick: u64) -> Self {
        Self::Reset { epoch, world_key, through_tick, births: [] }
    }
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        match self {
            Self::Reset { epoch, .. } | Self::Continuous { epoch, .. } => *epoch,
        }
    }
    #[must_use]
    pub const fn through_tick(&self) -> u64 {
        match self {
            Self::Reset { through_tick, .. } | Self::Continuous { through_tick, .. } => {
                *through_tick
            }
        }
    }
    #[must_use]
    pub fn world_key(&self) -> &str {
        match self {
            Self::Reset { world_key, .. } | Self::Continuous { world_key, .. } => world_key,
        }
    }
}

/// Complete actors are the fallback after admission/reset; missing legacy
/// samples are deliberately excluded rather than reconstructed from new RNG.
pub fn flame_presentation_birth(
    actor: &PrimarySpellTransient,
) -> Result<Option<FlamePresentationBirth>, String> {
    let PrimarySpellTransient::Weld(actor) = actor else { return Ok(None) };
    let recipe = match actor {
        WeldWorldActor::Channel(channel) if channel.common.build_id == 1003 => {
            let Some(FlameChannelVisual::Birth(_)) = &channel.flame_visual else {
                return Ok(None);
            };
            FlamePresentationRecipe::Ribbon {
                birth: channel.flame_visual.as_ref().unwrap().clone(),
                enhanced_effects: channel.enhanced_effects,
                weak: channel.underpowered,
                painter_registration: root(actor)?,
            }
        }
        WeldWorldActor::FlameLashFade(fade) => {
            let FlameFadeVisual::Birth(_) = fade.flame_visual else { return Ok(None) };
            FlamePresentationRecipe::Fade {
                birth: fade.flame_visual,
                position: fade.position,
                alpha_step: fade.alpha_step,
                base_scale: fade.base_scale,
                wrapper_scalar: fade.wrapper_scalar,
                variant: fade.variant,
                painter_registration: root(actor)?,
            }
        }
        _ => return Ok(None),
    };
    let common = actor.common();
    let mut lifetime = 1;
    if let FlamePresentationRecipe::Fade {
        birth: FlameFadeVisual::Birth(birth), alpha_step, ..
    } = recipe
    {
        lifetime = 0;
        while flame_fade_alpha(birth.birth_alpha, alpha_step, lifetime) > 0.0 {
            lifetime += 1;
        }
    }
    Ok(Some(FlamePresentationBirth {
        id: common.owner.id,
        kind: recipe.kind(),
        owner_id: common.owner.owner_id.clone(),
        world_key: common.owner.world_key.clone(),
        birth_tick: common.birth_tick,
        end_tick_exclusive: common.birth_tick + lifetime,
        recipe,
    }))
}

/// Compact before/after host capture. Session/catalog geometry is never cloned.
pub fn capture_flame_presentation_births(
    spells: &SpellState,
) -> Result<Vec<FlamePresentationBirth>, String> {
    let mut births = Vec::new();
    for actor in spells.transients.iter() {
        if let Some(birth) = flame_presentation_birth(actor)? {
            births.push(birth);
        }
    }
    Ok(births)
}

/// Display-only projection. Callers insert this only into their sampled clone.
#[must_use]
pub fn materialize_flame_presentation(
    birth: &FlamePresentationBirth,
    tick: f64,
) -> Option<PrimarySpellTransient> {
    if tick < birth.birth_tick as f64 || tick >= birth.end_tick_exclusive as f64 {
        return None;
    }
    let common = WeldActorBirth {
        build_id: 1003,
        direction: Vec2::new(0.0, -1.0),
        id: birth.id,
        origin: Vec2::ZERO,
        owner_id: &birth.owner_id,
        tick: birth.birth_tick,
        vector: &[0.0; 8],
        world_key: &birth.world_key,
    };
    let mut actor = match &birth.recipe {
        FlamePresentationRecipe::Ribbon {
            birth: visual,
            enhanced_effects,
            weak,
            painter_registration,
        } => {
            let FlameChannelVisual::Birth(visual_birth) = visual else { return None };
            let mut channel = WeldChannelActorState {
                common: common.actor(),
                enhanced_effects: *enhanced_effects,
                flame_visual: Some(visual.clone()),
                endpoint: Some(visual_birth.end),
                midpoint: Some(visual_birth.control),
                target_id: None,
                underpowered: *weak,
                variant: (birth.id % 4) as u32,
            };
            channel.common.origin = visual_birth.start;
            channel.common.owner.painter_registrations.push(*painter_registration);
            WeldWorldActor::Channel(channel)
        }
        FlamePresentationRecipe::Fade {
            birth: visual,
            position,
            alpha_step,
            base_scale,
            wrapper_scalar,
            variant,
            painter_registration,
        } => {
            let FlameFadeVisual::Birth(visual_birth) = visual else { return None };
            let mut fade = WeldFlameLashFadeState {
                common: common.actor(),
                alpha: flame_fade_alpha(
                    visual_birth.birth_alpha,
                    *alpha_step,
                    tick.floor() as u64 - birth.birth_tick,
                ),
                alpha_step: *alpha_step,
                base_scale: *base_scale,
                color_green: if *variant == WeldFlameLashFadeVariant::Source { 1.0 } else { 0.75 },
                flame_visual: *visual,
                position: *position,
                record: 35,
                rotation_degrees: 0.0,
                variant: *variant,
                wrapper_scalar: *wrapper_scalar,
            };
            fade.common.origin = *position;
            fade.common.owner.painter_registrations.push(*painter_registration);
            if *variant != WeldFlameLashFadeVariant::Source {
                fade.common.owner.light_registration = Some(*painter_registration);
            }
            WeldWorldActor::FlameLashFade(fade)
        }
    };
    actor.common_mut().owner.age_ticks = tick.floor() as u64 - birth.birth_tick;
    Some(PrimarySpellTransient::Weld(actor))
}

fn root(actor: &WeldWorldActor) -> Result<PainterRegistration, String> {
    actor
        .common()
        .owner
        .painter_registrations
        .first()
        .copied()
        .ok_or_else(|| "Flame presentation lost its native painter root".into())
}
fn require_tick(tick: u64) -> Result<(), String> {
    if tick > MAX_SAFE_INTEGER {
        return Err("Flame world tick must be a nonnegative safe integer".into());
    }
    Ok(())
}
