//! Enemy projectile identities and persistent state from `enemies/model.ts`.
use super::model::EnemyPainterRegistration;
use crate::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyProjectileKind {
    Arrow,
    DemonBomb,
    Firebolt,
    GuidedMissile,
    PoisonPool,
}
impl EnemyProjectileKind {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Arrow => "arrow",
            Self::DemonBomb => "demon-bomb",
            Self::Firebolt => "firebolt",
            Self::GuidedMissile => "guided-missile",
            Self::PoisonPool => "poison-pool",
        }
    }
    #[must_use]
    pub const fn native_type_id(self) -> u16 {
        match self {
            Self::Arrow => 0x7da,
            Self::DemonBomb => 0x7f7,
            Self::Firebolt => 0x7eb,
            Self::GuidedMissile => 0x7ec,
            Self::PoisonPool => 0x806,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyProjectilePayload {
    Cold,
    Fire,
    None,
    Normal,
    Poison,
}
impl EnemyProjectilePayload {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Fire => "fire",
            Self::None => "none",
            Self::Normal => "normal",
            Self::Poison => "poison",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyProjectileState {
    pub age_ticks: u64,
    pub bounce_velocity: f64,
    pub chill_tumble_accumulator: f64,
    pub cold_slow_ticks: u64,
    pub contact_radius: f64,
    pub damage: f64,
    pub secondary_damage: f64,
    pub heading_deg: f64,
    pub hit_player_ids: Vec<String>,
    pub homing: bool,
    pub id: usize,
    pub last_step_tick: u64,
    pub light_registration: Option<EnemyPainterRegistration>,
    pub lifetime_ticks: u64,
    pub minimum_speed: f64,
    pub native_type_id: u16,
    pub native_cell_binding_order: usize,
    pub native_registration_order: usize,
    pub owner_actor_id: usize,
    pub painter_registration: EnemyPainterRegistration,
    pub payload: EnemyProjectilePayload,
    pub poison_damage: f64,
    pub poison_duration: f64,
    pub position: Vec2,
    pub speed: f64,
    pub settled_ticks_remaining: i64,
    pub spawn_tick: u64,
    pub target_player_id: Option<String>,
    pub turn_speed: f64,
    pub vertical_offset: f64,
    pub vertical_velocity: f64,
    pub visual_phase_deg: f64,
    pub visual_scale: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyProjectile {
    Arrow { state: EnemyProjectileState, velocity: Vec2 },
    DemonBomb(EnemyProjectileState),
    Firebolt(EnemyProjectileState),
    GuidedMissile(EnemyProjectileState),
    PoisonPool(EnemyProjectileState),
}
impl EnemyProjectile {
    #[must_use]
    pub const fn kind(&self) -> EnemyProjectileKind {
        match self {
            Self::Arrow { .. } => EnemyProjectileKind::Arrow,
            Self::DemonBomb(_) => EnemyProjectileKind::DemonBomb,
            Self::Firebolt(_) => EnemyProjectileKind::Firebolt,
            Self::GuidedMissile(_) => EnemyProjectileKind::GuidedMissile,
            Self::PoisonPool(_) => EnemyProjectileKind::PoisonPool,
        }
    }
    #[must_use]
    pub const fn state(&self) -> &EnemyProjectileState {
        match self {
            Self::Arrow { state, .. }
            | Self::DemonBomb(state)
            | Self::Firebolt(state)
            | Self::GuidedMissile(state)
            | Self::PoisonPool(state) => state,
        }
    }
    pub const fn state_mut(&mut self) -> &mut EnemyProjectileState {
        match self {
            Self::Arrow { state, .. }
            | Self::DemonBomb(state)
            | Self::Firebolt(state)
            | Self::GuidedMissile(state)
            | Self::PoisonPool(state) => state,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum MageLightningContact {
    World { position: Vec2 },
    TargetAttached { local_offset: Vec2, target_player_id: String },
}
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct MageLightningPulse {
    #[cfg_attr(feature = "persistence", serde(default = "super::model::default_enhanced_effects"))]
    pub enhanced_effects: bool,
    pub contact: MageLightningContact,
    pub endpoint: Vec2,
    pub id: usize,
    pub light_registration: EnemyPainterRegistration,
    pub midpoint: Vec2,
    pub owner_actor_id: usize,
    pub painter_registrations: Vec<EnemyPainterRegistration>,
    pub seed: u32,
    pub source: Vec2,
    pub tick: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyProjectileEffectState {
    pub age_ticks: u64,
    pub alpha: f64,
    pub alpha_loss_per_tick: f64,
    pub angular_velocity_deg: f64,
    pub atlas: super::model::EnemyEffectAtlas,
    pub blend_mode: super::model::EnemyEffectBlend,
    pub entry: usize,
    pub id: usize,
    pub last_step_tick: u64,
    pub light_registration: Option<EnemyPainterRegistration>,
    pub lifetime_ticks: u64,
    pub owner_actor_id: usize,
    pub owner_projectile_id: usize,
    pub painter_registration: EnemyPainterRegistration,
    pub phase_origin_ticks: u64,
    pub position: Vec2,
    pub rotation_deg: f64,
    pub scale: f64,
    pub spawn_tick: u64,
    pub tint: u32,
    pub velocity: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum EnemyProjectileEffectKind {
    ArrowTumble,
    DemonBombParticle,
    // Keep the large persistent Fire payload out of every small trail/impact row.
    DemonFire { fire: Box<crate::spells::model::PrimarySpellFirePatch> },
    DemonExplosionCore,
    DemonExplosionArray,
    DemonExplosionLitArray,
    PoisonBubble { growth_per_tick: f64, maximum_scale: f64 },
    FireBurst,
    GuidedImpact,
    FireboltTrail,
}
impl EnemyProjectileEffectKind {
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::ArrowTumble => "arrow-tumble",
            Self::DemonBombParticle => "demon-bomb-particle",
            Self::DemonFire { .. } => "demon-fire",
            Self::DemonExplosionCore => "demon-explosion-core",
            Self::DemonExplosionArray => "demon-explosion-array",
            Self::DemonExplosionLitArray => "demon-explosion-lit-array",
            Self::PoisonBubble { .. } => "poison-bubble",
            Self::FireBurst => "fire-burst",
            Self::GuidedImpact => "guided-impact",
            Self::FireboltTrail => "firebolt-trail",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyProjectileEffect {
    pub state: EnemyProjectileEffectState,
    pub kind: EnemyProjectileEffectKind,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct EnemyProjectileKnockback {
    pub actor_id: usize,
    pub event_id: usize,
    pub player_id: String,
    pub last_step_tick: u64,
    pub remaining_ticks: u64,
    pub delta: Vec2,
}

#[must_use]
pub fn native_poison_pool_alpha(age: u64) -> f64 {
    let mut alpha = 1.0_f32;
    for _ in 0..age.saturating_sub(2999).min(201) {
        alpha = (alpha - 0.005_f32).max(0.0);
    }
    f64::from(alpha)
}
#[must_use]
pub fn native_poison_pool_contact(origin: Vec2, target: Vec2) -> bool {
    let x = crate::float::fround(target.x - origin.x);
    let y = crate::float::fround(crate::float::fround(target.y - origin.y) / (0.8_f32 as f64));
    crate::float::fround(x * x + y * y) < 4900.0
}
