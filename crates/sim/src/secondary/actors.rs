//! Secondary actor records and native painter/light enrollment.
//! Field meanings and registration order mirror native-secondary-abilities.ts.

use std::collections::BTreeMap;

use super::golem::NativeSecondaryGolemState;
use crate::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use crate::{NativeRng, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum NativeSecondaryActorKind {
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

impl NativeSecondaryActorKind {
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

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeSecondaryActorState {
    pub age_ticks: u64,
    pub alpha: f64,
    pub damage: f64,
    pub enhanced: bool,
    pub endpoint: Vec2,
    pub ether_drain: Option<super::ether_drain::NativeEtherDrainState>,
    pub frame: f64,
    pub freeze_ticks: u64,
    pub golem: Option<NativeSecondaryGolemState>,
    pub golem_death: Option<crate::death_animations::NativeGolemDeathAnimation>,
    /// This source field holds both enemy IDs and parent secondary actor IDs.
    pub hit_target_ids: Vec<u64>,
    pub id: u64,
    pub kind: NativeSecondaryActorKind,
    pub lifetime_ticks: u64,
    pub light_registration: Option<PainterRegistration>,
    pub midpoint: Vec2,
    pub misc_light_append_ordinal: Option<usize>,
    pub owner_id: String,
    pub painter_registrations: Vec<PainterRegistration>,
    pub phase: f64,
    pub position: Vec2,
    pub presentation_rng: Option<NativeRng>,
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

impl NativeSecondaryActorState {
    /// The exact `actorSeed` defaults; ability constructors overwrite their authored fields.
    #[must_use]
    pub fn new(
        id: u64,
        kind: NativeSecondaryActorKind,
        owner_id: String,
        world_key: String,
        skill_id: Option<usize>,
    ) -> Self {
        Self {
            age_ticks: 0,
            alpha: 1.0,
            damage: 0.0,
            enhanced: false,
            endpoint: Vec2::ZERO,
            ether_drain: None,
            frame: 0.0,
            freeze_ticks: 0,
            golem: None,
            golem_death: None,
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

#[derive(Clone, Debug, PartialEq)]
pub struct NativeSecondaryTarget {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSecondaryDamageKind {
    Acid,
    Fire,
    Ice,
    Lightning,
    Magic,
    Physical,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeSecondaryDamageContact {
    pub amount: f64,
    pub ether_drain: bool,
    pub hit_strength: Option<f64>,
    pub kind: NativeSecondaryDamageKind,
    pub owner_id: String,
    pub source_actor_id: u64,
    pub special_death_payload: Option<crate::spells::special_death::NativeSpellSpecialDeathPayload>,
    pub suppress_hit_reaction: bool,
    pub suppress_hurt_sound: bool,
    pub target_id: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSecondaryKnockbackContact {
    pub delta: Vec2,
    pub source_actor_id: u64,
    pub target_id: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSecondaryTargetHeadingChange {
    pub degrees: f64,
    pub mode: NativeSecondaryHeadingMode,
    pub target_id: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSecondaryHeadingMode {
    Absolute,
    Relative,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSecondaryLightDisposition {
    ActorProvider,
    Misc,
    None,
    TransientProvider,
}

#[must_use]
pub fn native_secondary_light_disposition(
    actor: &NativeSecondaryActorState,
) -> NativeSecondaryLightDisposition {
    use NativeSecondaryActorKind::*;
    match actor.kind {
        Leviathan | EtherBolt | MovingFire | Shockwave | MindblastShockwave | RescueShockwave
        | FirePatch | RingFireFragment | StormCloud | FreezeWave | Golem | MagicTrap | AcidRain
        | EtherDrain | Comet => NativeSecondaryLightDisposition::ActorProvider,
        RingFireExplosion | ElectricBurnFlare => NativeSecondaryLightDisposition::TransientProvider,
        EtherFade if actor.variant == 1 => NativeSecondaryLightDisposition::TransientProvider,
        MagicCircle | FireBurn | EtherBurn | ElectricBurn => NativeSecondaryLightDisposition::Misc,
        _ => NativeSecondaryLightDisposition::None,
    }
}

#[must_use]
pub fn native_secondary_painter_manager_lane(kind: NativeSecondaryActorKind) -> WorldManagerLane {
    use NativeSecondaryActorKind::*;
    match kind {
        Leviathan | LeviathanAppendage | MovingFire | FirePatch | StormCloud | StormStrike
        | Earthquake | Golem | GolemDeath | GolemAssemblyDebris | ElectricBurnArc | AcidRain
        | Comet => WorldManagerLane::Actor,
        _ => WorldManagerLane::Transient,
    }
}

pub fn enroll_native_secondary_painter_owners(
    actors: &mut [NativeSecondaryActorState],
    order: &mut WorldPainterOrder,
) {
    for actor in actors {
        let lane = native_secondary_painter_manager_lane(actor.kind);
        let count = if actor.kind == NativeSecondaryActorKind::GolemDeath {
            crate::death_animations::NATIVE_GOLEM_DEATH_PAINTER_COUNT
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

pub fn enroll_native_secondary_light_owners(
    actors: &mut [NativeSecondaryActorState],
    order: Option<&mut WorldPainterOrder>,
    target_light: impl Fn(&str, usize) -> Option<PainterRegistration>,
) {
    let mut retained_order = retained_world_manager_order(actors);
    let order = order.unwrap_or(&mut retained_order);
    // Source registers every painter first, then allocates any extra light-only owners.
    enroll_native_secondary_painter_owners(actors, order);
    let mut next_modifier_ordinal: BTreeMap<(String, usize), usize> = BTreeMap::new();
    for actor in actors {
        let disposition = native_secondary_light_disposition(actor);
        if disposition == NativeSecondaryLightDisposition::None {
            actor.light_registration = None;
            actor.misc_light_append_ordinal = None;
            continue;
        }
        if matches!(
            actor.kind,
            NativeSecondaryActorKind::FireBurn
                | NativeSecondaryActorKind::EtherBurn
                | NativeSecondaryActorKind::ElectricBurn
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
        let expected_lane = if disposition == NativeSecondaryLightDisposition::TransientProvider {
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
            if disposition == NativeSecondaryLightDisposition::Misc { Some(0) } else { None };
    }
}

fn retained_world_manager_order(actors: &[NativeSecondaryActorState]) -> WorldPainterOrder {
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
