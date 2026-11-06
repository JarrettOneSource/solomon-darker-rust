//! Secondary actor records and native painter/light enrollment.
//! Painters register before any extra light-only owners, and every registration keeps its
//! manager lane.

use std::collections::BTreeMap;

use super::golem::SecondaryGolemState;
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::{Rng, Vec2};

/// Every kind of secondary effect actor, from spell bodies to their particles and debris.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum SecondaryActorKind {
    Leviathan,
    LeviathanAppendage,
    LeviathanMote,
    EtherBolt,
    EtherFade,
    PhaseBurst,
    PlaneOrbShot,
    PlaneOrbParticle,
    MovingFire,
    Shockwave,
    FirePatch,
    FireBurn,
    FireBurnFlame,
    EtherBurn,
    EtherBurnFlare,
    StormCloud,
    StormDrop,
    StormStrike,
    PrismaticWave,
    FreezeWave,
    FreezeWaveVisual,
    IceBlast,
    FrostBurnFlare,
    Earthquake,
    EarthquakeSceneryWobble,
    EarthquakeQuake,
    EarthquakeDust,
    EarthquakeDebris,
    Golem,
    GolemDeath,
    GolemAssemblyDebris,
    GolemKnockback,
    StoneskinChip,
    PlanewalkerMote,
    TeleportBurst,
    MagicCircle,
    MagicCirclePlayerFlash,
    MagicTrap,
    MagicTrapShimmer,
    MagicTrapBurst,
    ElectricBurn,
    ElectricBurnFlare,
    ElectricBurnArc,
    FlashResponseFade,
    FlashResponseGrow,
    DampenWave,
    DampenedProjectile,
    DampenedSmoke,
    ShieldBreak,
    ShieldExplosion,
    AcidRain,
    AcidDrop,
    MindblastBurst,
    MindblastShockwave,
    RescueShockwave,
    RingFireExplosion,
    RingFireFragment,
    AcidSplash,
    EtherDrain,
    EtherDrainCloud,
    EtherDrainDebris,
    EtherDrainCaptureFlare,
    Comet,
    CometTrail,
    CometImpact,
    CometDebris,
    TurnUndead,
}

impl SecondaryActorKind {
    pub const ALL: [Self; 67] = [
        Self::Leviathan,
        Self::LeviathanAppendage,
        Self::LeviathanMote,
        Self::EtherBolt,
        Self::EtherFade,
        Self::PhaseBurst,
        Self::PlaneOrbShot,
        Self::PlaneOrbParticle,
        Self::MovingFire,
        Self::Shockwave,
        Self::FirePatch,
        Self::FireBurn,
        Self::FireBurnFlame,
        Self::EtherBurn,
        Self::EtherBurnFlare,
        Self::StormCloud,
        Self::StormDrop,
        Self::StormStrike,
        Self::PrismaticWave,
        Self::FreezeWave,
        Self::FreezeWaveVisual,
        Self::IceBlast,
        Self::FrostBurnFlare,
        Self::Earthquake,
        Self::EarthquakeSceneryWobble,
        Self::EarthquakeQuake,
        Self::EarthquakeDust,
        Self::EarthquakeDebris,
        Self::Golem,
        Self::GolemDeath,
        Self::GolemAssemblyDebris,
        Self::GolemKnockback,
        Self::StoneskinChip,
        Self::PlanewalkerMote,
        Self::TeleportBurst,
        Self::MagicCircle,
        Self::MagicCirclePlayerFlash,
        Self::MagicTrap,
        Self::MagicTrapShimmer,
        Self::MagicTrapBurst,
        Self::ElectricBurn,
        Self::ElectricBurnFlare,
        Self::ElectricBurnArc,
        Self::FlashResponseFade,
        Self::FlashResponseGrow,
        Self::DampenWave,
        Self::DampenedProjectile,
        Self::DampenedSmoke,
        Self::ShieldBreak,
        Self::ShieldExplosion,
        Self::AcidRain,
        Self::AcidDrop,
        Self::MindblastBurst,
        Self::MindblastShockwave,
        Self::RescueShockwave,
        Self::RingFireExplosion,
        Self::RingFireFragment,
        Self::AcidSplash,
        Self::EtherDrain,
        Self::EtherDrainCloud,
        Self::EtherDrainDebris,
        Self::EtherDrainCaptureFlare,
        Self::Comet,
        Self::CometTrail,
        Self::CometImpact,
        Self::CometDebris,
        Self::TurnUndead,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Leviathan => "leviathan",
            Self::LeviathanAppendage => "leviathan-appendage",
            Self::LeviathanMote => "leviathan-mote",
            Self::EtherBolt => "ether-bolt",
            Self::EtherFade => "ether-fade",
            Self::PhaseBurst => "phase-burst",
            Self::PlaneOrbShot => "plane-orb-shot",
            Self::PlaneOrbParticle => "plane-orb-particle",
            Self::MovingFire => "moving-fire",
            Self::Shockwave => "shockwave",
            Self::FirePatch => "fire-patch",
            Self::FireBurn => "fire-burn",
            Self::FireBurnFlame => "fire-burn-flame",
            Self::EtherBurn => "ether-burn",
            Self::EtherBurnFlare => "ether-burn-flare",
            Self::StormCloud => "storm-cloud",
            Self::StormDrop => "storm-drop",
            Self::StormStrike => "storm-strike",
            Self::PrismaticWave => "prismatic-wave",
            Self::FreezeWave => "freeze-wave",
            Self::FreezeWaveVisual => "freeze-wave-visual",
            Self::IceBlast => "ice-blast",
            Self::FrostBurnFlare => "frost-burn-flare",
            Self::Earthquake => "earthquake",
            Self::EarthquakeSceneryWobble => "earthquake-scenery-wobble",
            Self::EarthquakeQuake => "earthquake-quake",
            Self::EarthquakeDust => "earthquake-dust",
            Self::EarthquakeDebris => "earthquake-debris",
            Self::Golem => "golem",
            Self::GolemDeath => "golem-death",
            Self::GolemAssemblyDebris => "golem-assembly-debris",
            Self::GolemKnockback => "golem-knockback",
            Self::StoneskinChip => "stoneskin-chip",
            Self::PlanewalkerMote => "planewalker-mote",
            Self::TeleportBurst => "teleport-burst",
            Self::MagicCircle => "magic-circle",
            Self::MagicCirclePlayerFlash => "magic-circle-player-flash",
            Self::MagicTrap => "magic-trap",
            Self::MagicTrapShimmer => "magic-trap-shimmer",
            Self::MagicTrapBurst => "magic-trap-burst",
            Self::ElectricBurn => "electric-burn",
            Self::ElectricBurnFlare => "electric-burn-flare",
            Self::ElectricBurnArc => "electric-burn-arc",
            Self::FlashResponseFade => "flash-response-fade",
            Self::FlashResponseGrow => "flash-response-grow",
            Self::DampenWave => "dampen-wave",
            Self::DampenedProjectile => "dampened-projectile",
            Self::DampenedSmoke => "dampened-smoke",
            Self::ShieldBreak => "shield-break",
            Self::ShieldExplosion => "shield-explosion",
            Self::AcidRain => "acid-rain",
            Self::AcidDrop => "acid-drop",
            Self::MindblastBurst => "mindblast-burst",
            Self::MindblastShockwave => "mindblast-shockwave",
            Self::RescueShockwave => "rescue-shockwave",
            Self::RingFireExplosion => "ring-fire-explosion",
            Self::RingFireFragment => "ring-fire-fragment",
            Self::AcidSplash => "acid-splash",
            Self::EtherDrain => "ether-drain",
            Self::EtherDrainCloud => "ether-drain-cloud",
            Self::EtherDrainDebris => "ether-drain-debris",
            Self::EtherDrainCaptureFlare => "ether-drain-capture-flare",
            Self::Comet => "comet",
            Self::CometTrail => "comet-trail",
            Self::CometImpact => "comet-impact",
            Self::CometDebris => "comet-debris",
            Self::TurnUndead => "turn-undead",
        }
    }
}

/// One secondary effect actor. Fields follow the save record's order.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct SecondaryActorState {
    pub age_ticks: u64,
    pub alpha: f64,
    pub damage: f64,
    pub enhanced: bool,
    pub endpoint: Vec2,
    /// Ether Drain fills this on its first tick, when it discovers its targets.
    pub ether_drain: Option<super::ether_drain::EtherDrainState>,
    pub frame: f64,
    pub freeze_ticks: u64,
    /// Set on Golem summons.
    pub golem: Option<SecondaryGolemState>,
    /// Set on Golem deaths.
    pub golem_death: Option<crate::death_animations::GolemDeathAnimation>,
    /// Holds both enemy IDs and parent secondary actor IDs.
    pub hit_target_ids: Vec<u64>,
    pub id: u64,
    pub kind: SecondaryActorKind,
    pub lifetime_ticks: u64,
    pub light_registration: Option<PainterRegistration>,
    pub midpoint: Vec2,
    pub misc_light_append_ordinal: Option<usize>,
    pub owner_id: String,
    pub painter_registrations: Vec<PainterRegistration>,
    pub phase: f64,
    pub position: Vec2,
    pub presentation_rng: Option<Rng>,
    pub quantity: f64,
    pub radius: f64,
    pub rank: u32,
    pub rotation_radians: f64,
    pub scale: f64,
    /// Native effect actors can also carry non-ability rows 14, 22 and 53.
    pub skill_id: Option<usize>,
    pub slow_factor: f64,
    pub target_id: Option<usize>,
    pub variant: i32,
    pub velocity: Vec2,
    pub world_key: String,
}

impl SecondaryActorState {
    /// An actor with default fields; ability constructors overwrite their authored fields.
    #[must_use]
    pub fn new(
        id: u64,
        kind: SecondaryActorKind,
        owner_id: String,
        world_key: String,
        skill_id: Option<usize>,
    ) -> Self {
        Self {
            ether_drain: None,
            golem: None,
            golem_death: None,
            age_ticks: 0,
            alpha: 1.0,
            damage: 0.0,
            enhanced: false,
            endpoint: Vec2::ZERO,
            frame: 0.0,
            freeze_ticks: 0,
            hit_target_ids: Vec::new(),
            id,
            kind,
            lifetime_ticks: 60,
            light_registration: None,
            midpoint: Vec2::ZERO,
            misc_light_append_ordinal: None,
            owner_id,
            painter_registrations: Vec::new(),
            phase: 0.0,
            position: Vec2::ZERO,
            presentation_rng: None,
            quantity: 0.0,
            radius: 0.0,
            rank: 1,
            rotation_radians: 0.0,
            scale: 1.0,
            skill_id,
            slow_factor: 0.0,
            target_id: None,
            variant: 0,
            velocity: Vec2::ZERO,
            world_key,
        }
    }
}

/// A target secondary abilities can affect: its family and ID, light registration,
/// position, size and shield health.
#[derive(Clone, Debug, PartialEq)]
pub struct SecondaryTarget {
    pub family: String,
    pub id: usize,
    pub light_registration: PainterRegistration,
    pub native_flags: Option<u32>,
    pub registration_order: Option<usize>,
    pub position: Vec2,
    pub radius: f64,
    pub scale: f64,
    pub shield_health: f64,
}

/// The damage element of a secondary hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondaryDamageKind {
    Acid,
    Fire,
    Ice,
    Lightning,
    Magic,
    Physical,
}

/// One secondary damage hit on a target: amount, kind, owner, source actor and hit
/// reaction options.
#[derive(Clone, Debug, PartialEq)]
pub struct SecondaryDamageContact {
    pub amount: f64,
    pub ether_drain: bool,
    pub hit_strength: Option<f64>,
    pub kind: SecondaryDamageKind,
    pub owner_id: String,
    pub source_actor_id: u64,
    pub special_death_payload: Option<crate::spells::special_death::SpellSpecialDeathPayload>,
    pub suppress_hit_reaction: bool,
    pub suppress_hurt_sound: bool,
    pub target_id: usize,
}

/// One secondary knockback on a target: its displacement and source actor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SecondaryKnockbackContact {
    pub delta: Vec2,
    pub source_actor_id: u64,
    pub target_id: usize,
}

/// A secondary effect turning a target, to an absolute heading or by a relative amount.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SecondaryTargetHeadingChange {
    pub degrees: f64,
    pub mode: SecondaryHeadingMode,
    pub target_id: usize,
}

/// Whether a heading change sets the heading or adds to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondaryHeadingMode {
    Absolute,
    Relative,
}

/// How a secondary actor takes part in lighting: as an actor or transient light provider,
/// as a miscellaneous light, or not at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondaryLightDisposition {
    ActorProvider,
    Misc,
    None,
    TransientProvider,
}

#[must_use]
pub fn secondary_light_disposition(actor: &SecondaryActorState) -> SecondaryLightDisposition {
    use SecondaryActorKind::*;
    match actor.kind {
        Leviathan | EtherBolt | MovingFire | Shockwave | MindblastShockwave | RescueShockwave
        | FirePatch | RingFireFragment | StormCloud | FreezeWave | Golem | MagicTrap | AcidRain
        | EtherDrain | Comet => SecondaryLightDisposition::ActorProvider,
        RingFireExplosion | ElectricBurnFlare => SecondaryLightDisposition::TransientProvider,
        EtherFade if actor.variant == 1 => SecondaryLightDisposition::TransientProvider,
        MagicCircle | FireBurn | EtherBurn | ElectricBurn => SecondaryLightDisposition::Misc,
        _ => SecondaryLightDisposition::None,
    }
}

#[must_use]
pub fn secondary_painter_manager_lane(kind: SecondaryActorKind) -> WorldManagerLane {
    use SecondaryActorKind::*;
    match kind {
        Leviathan | LeviathanAppendage | MovingFire | FirePatch | StormCloud | StormStrike
        | Earthquake | Golem | GolemDeath | GolemAssemblyDebris | ElectricBurnArc | AcidRain
        | Comet => WorldManagerLane::Actor,
        _ => WorldManagerLane::Transient,
    }
}

pub fn enroll_secondary_painter_owners<'a>(
    actors: impl IntoIterator<Item = &'a mut SecondaryActorState>,
    order: &mut WorldPainterOrder,
) {
    for actor in actors {
        let lane = secondary_painter_manager_lane(actor.kind);
        let count = if actor.kind == SecondaryActorKind::GolemDeath {
            crate::death_animations::GOLEM_DEATH_PAINTER_COUNT
        } else {
            1
        };
        if actor.painter_registrations.is_empty() {
            actor.painter_registrations = order.register_many(lane, count);
        }
        assert_eq!(
            actor.painter_registrations.len(),
            count,
            "secondary preserves native painter membership"
        );
        assert!(
            actor
                .painter_registrations
                .iter()
                .all(|registration| registration.manager_lane == lane),
            "secondary preserves painter-manager ownership"
        );
    }
}

pub fn enroll_secondary_light_owners(
    actors: &mut [super::SecondaryActorState],
    order: Option<&mut WorldPainterOrder>,
    target_light: impl Fn(&str, usize) -> Option<PainterRegistration>,
) {
    let mut retained_order = retained_world_manager_order(actors);
    let order = order.unwrap_or(&mut retained_order);
    // Register every painter first, then allocate any extra light-only owners.
    enroll_secondary_painter_owners(actors.iter_mut(), order);
    let mut next_modifier_ordinal: BTreeMap<(String, usize), usize> = BTreeMap::new();
    for actor in actors.iter_mut() {
        let disposition = secondary_light_disposition(actor);
        if disposition == SecondaryLightDisposition::None {
            actor.light_registration = None;
            actor.misc_light_append_ordinal = None;
            continue;
        }
        if matches!(
            actor.kind,
            SecondaryActorKind::FireBurn
                | SecondaryActorKind::EtherBurn
                | SecondaryActorKind::ElectricBurn
        ) {
            let target_id = actor
                .target_id
                .expect("secondary burn retains its target-owned light registration");
            let registration = target_light(&actor.world_key, target_id)
                .or(actor.light_registration)
                .expect("secondary burn target is registered in the actor light manager");
            assert_eq!(registration.manager_lane, WorldManagerLane::Actor);
            let ordinal =
                next_modifier_ordinal.entry((actor.world_key.clone(), target_id)).or_default();
            actor.light_registration = Some(registration);
            actor.misc_light_append_ordinal = Some(*ordinal);
            *ordinal += 1;
            continue;
        }
        let expected_lane = if disposition == SecondaryLightDisposition::TransientProvider {
            WorldManagerLane::Transient
        } else {
            WorldManagerLane::Actor
        };
        let registration = match actor.light_registration {
            Some(registration) => registration,
            None if actor.painter_registrations[0].manager_lane == expected_lane => {
                actor.painter_registrations[0]
            }
            None => order.register(expected_lane),
        };
        assert_eq!(
            registration.manager_lane, expected_lane,
            "secondary preserves its light-manager lane"
        );
        actor.light_registration = Some(registration);
        actor.misc_light_append_ordinal =
            if disposition == SecondaryLightDisposition::Misc { Some(0) } else { None };
    }
}

fn retained_world_manager_order(actors: &[SecondaryActorState]) -> WorldPainterOrder {
    let mut order = WorldPainterOrder::default();
    for actor in actors {
        for registration in
            actor.painter_registrations.iter().copied().chain(actor.light_registration)
        {
            let next = match registration.manager_lane {
                WorldManagerLane::Actor => &mut order.next_actor_ordinal,
                WorldManagerLane::Transient => &mut order.next_transient_ordinal,
            };
            *next = (*next).max(registration.registration_ordinal + 1);
        }
    }
    order
}
