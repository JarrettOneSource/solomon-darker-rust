//! Native secondary semantic events and screen flashes.

use crate::Vec2;
use crate::float::fround;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeSecondaryAudioCue {
    LeviathanRoar,
    PlanewalkerOn,
    PlanewalkerOff,
    Phase,
    BigFire,
    Nuke,
    Ignite,
    MagicStorm,
    LightningStart,
    Thunder,
    PrismaticShock,
    RingOfIce,
    QuakeCracks,
    QuakeCrackSmall,
    GolemProvoke,
    KnockbackGolem,
    StoneStep,
    GolemDie,
    StoneBreak,
    FlameLashStart,
    FlashSpell,
    RockHit,
    CrunchDrain,
    StoneskinOn,
    Stoneskin,
    Teleport,
    MagicCircle,
    SetTrap,
    Trap,
    MagicMissile,
    ThrowFire,
    IceStart,
    StartBoulder,
    Harden,
    IceShatter,
    Flash,
    Dampen,
    MagicShieldUp,
    HitShield,
    PopShield,
    MagicShieldExplode,
    AcidSizzle,
    FireballHit,
    DistortReality,
    CometWhistle,
    ExplodeSteam,
    LevelUp,
    Mindstar,
    Fizzle,
    PlaneCrossLoop,
    LowFireLoop,
    RainfallLoop,
    SteadyWindLoop,
    EarthquakeLoop,
    CometLoop,
    ElectricLoop,
}

impl NativeSecondaryAudioCue {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LeviathanRoar => "leviathan-roar",
            Self::PlanewalkerOn => "planewalker-on",
            Self::PlanewalkerOff => "planewalker-off",
            Self::Phase => "phase",
            Self::BigFire => "big-fire",
            Self::Nuke => "nuke",
            Self::Ignite => "ignite",
            Self::MagicStorm => "magic-storm",
            Self::LightningStart => "lightning-start",
            Self::Thunder => "thunder",
            Self::PrismaticShock => "prismatic-shock",
            Self::RingOfIce => "ring-of-ice",
            Self::QuakeCracks => "quake-cracks",
            Self::QuakeCrackSmall => "quake-crack-small",
            Self::GolemProvoke => "golem-provoke",
            Self::KnockbackGolem => "knockback-golem",
            Self::StoneStep => "stone-step",
            Self::GolemDie => "golem-die",
            Self::StoneBreak => "stone-break",
            Self::FlameLashStart => "flame-lash-start",
            Self::FlashSpell => "flash-spell",
            Self::RockHit => "rock-hit",
            Self::CrunchDrain => "crunch-drain",
            Self::StoneskinOn => "stoneskin-on",
            Self::Stoneskin => "stoneskin",
            Self::Teleport => "teleport",
            Self::MagicCircle => "magic-circle",
            Self::SetTrap => "set-trap",
            Self::Trap => "trap",
            Self::MagicMissile => "magic-missile",
            Self::ThrowFire => "throw-fire",
            Self::IceStart => "ice-start",
            Self::StartBoulder => "start-boulder",
            Self::Harden => "harden",
            Self::IceShatter => "ice-shatter",
            Self::Flash => "flash",
            Self::Dampen => "dampen",
            Self::MagicShieldUp => "magic-shield-up",
            Self::HitShield => "hit-shield",
            Self::PopShield => "pop-shield",
            Self::MagicShieldExplode => "magic-shield-explode",
            Self::AcidSizzle => "acid-sizzle",
            Self::FireballHit => "fireball-hit",
            Self::DistortReality => "distort-reality",
            Self::CometWhistle => "comet-whistle",
            Self::ExplodeSteam => "explode-steam",
            Self::LevelUp => "level-up",
            Self::Mindstar => "mindstar",
            Self::Fizzle => "fizzle",
            Self::PlaneCrossLoop => "plane-cross-loop",
            Self::LowFireLoop => "low-fire-loop",
            Self::RainfallLoop => "rainfall-loop",
            Self::SteadyWindLoop => "steady-wind-loop",
            Self::EarthquakeLoop => "earthquake-loop",
            Self::CometLoop => "comet-loop",
            Self::ElectricLoop => "electric-loop",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeSecondaryEventKind {
    Cast,
    Fizzle,
    Impact,
    LoopStart,
    LoopStop,
    Overload,
    Pulse,
    ShieldBreak,
    ShieldHit,
    ToggleOff,
    ToggleOn,
    Whistle,
}

impl NativeSecondaryEventKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cast => "cast",
            Self::Fizzle => "fizzle",
            Self::Impact => "impact",
            Self::LoopStart => "loop-start",
            Self::LoopStop => "loop-stop",
            Self::Overload => "overload",
            Self::Pulse => "pulse",
            Self::ShieldBreak => "shield-break",
            Self::ShieldHit => "shield-hit",
            Self::ToggleOff => "toggle-off",
            Self::ToggleOn => "toggle-on",
            Self::Whistle => "whistle",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeSecondaryScreenFlashState {
    pub alpha: f64,
    pub blue: f64,
    pub decay_per_tick: f64,
    pub green: f64,
    pub point_attenuated: bool,
    pub red: f64,
}

impl NativeSecondaryScreenFlashState {
    #[must_use]
    pub fn new(
        red: f64,
        green: f64,
        blue: f64,
        decay_per_tick: f64,
        point_attenuated: bool,
        alpha: f64,
    ) -> Self {
        Self {
            alpha: fround(alpha),
            blue: fround(blue),
            decay_per_tick: fround(decay_per_tick),
            green: fround(green),
            point_attenuated,
            red: fround(red),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeSecondaryEventState {
    pub actor_id: Option<u64>,
    pub camera_displacement: Option<Vec2>,
    pub camera_magnitude: f64,
    pub cue: Option<NativeSecondaryAudioCue>,
    pub event_id: u64,
    pub gain: f64,
    pub kind: NativeSecondaryEventKind,
    pub owner_id: String,
    pub pitch: f64,
    pub position: Vec2,
    pub screen_flash: Option<NativeSecondaryScreenFlashState>,
    pub skill_id: Option<usize>,
    pub tick: u64,
    pub world_key: String,
}

impl NativeSecondaryEventState {
    #[must_use]
    pub fn new(
        kind: NativeSecondaryEventKind,
        owner_id: String,
        world_key: String,
        tick: u64,
    ) -> Self {
        Self {
            actor_id: None,
            camera_displacement: None,
            camera_magnitude: 0.0,
            cue: None,
            event_id: 0,
            gain: 1.0,
            kind,
            owner_id,
            pitch: 1.0,
            position: Vec2::ZERO,
            screen_flash: None,
            skill_id: None,
            tick,
            world_key,
        }
    }
}
