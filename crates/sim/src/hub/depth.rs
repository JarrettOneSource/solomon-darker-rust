//! Fixed Hub drawing bands from `hub-depth.ts` and `renderer/hub-render-contract.ts`.
//!
//! `hub-depth-goldens.json` pins every band and the reference-relative actor depth function.
//! The renderer places the sorted world queue consecutively after `TeacherPreWorld`, as
//! `NativeHubPainterPlanner` does; rows decide order, never a band's numeric position.

use super::painter::painter_row;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubDepth {
    Courtyard,
    SealGlyphs,
    SealCore,
    UsefulThyngsShadow,
    Fountain,
    TeacherPreWorld,
    StatueAura,
    Statue,
    TeacherPostWorld,
    CourtyardForeground,
    CourtyardOnboarding,
    SouthernForeground,
    Astronomer,
    AstronomerTelescope,
    AstronomerFront,
}

/// Every `HUB_WORLD_DEPTH` entry in ascending drawing order. Statue entries are its initial
/// depths; the world painter planner keeps the aura directly under the ordered body.
pub const HUB_WORLD_DEPTH: [HubDepth; 15] = [
    HubDepth::Courtyard,
    HubDepth::SealGlyphs,
    HubDepth::SealCore,
    HubDepth::UsefulThyngsShadow,
    HubDepth::Fountain,
    HubDepth::TeacherPreWorld,
    HubDepth::StatueAura,
    HubDepth::Statue,
    HubDepth::TeacherPostWorld,
    HubDepth::CourtyardForeground,
    HubDepth::CourtyardOnboarding,
    HubDepth::SouthernForeground,
    HubDepth::Astronomer,
    HubDepth::AstronomerTelescope,
    HubDepth::AstronomerFront,
];

impl HubDepth {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Courtyard => "courtyard",
            Self::SealGlyphs => "sealGlyphs",
            Self::SealCore => "sealCore",
            Self::UsefulThyngsShadow => "usefulThyngsShadow",
            Self::Fountain => "fountain",
            Self::TeacherPreWorld => "teacherPreWorld",
            Self::StatueAura => "statueAura",
            Self::Statue => "statue",
            Self::TeacherPostWorld => "teacherPostWorld",
            Self::CourtyardForeground => "courtyardForeground",
            Self::CourtyardOnboarding => "courtyardOnboarding",
            Self::SouthernForeground => "southernForeground",
            Self::Astronomer => "astronomer",
            Self::AstronomerTelescope => "astronomerTelescope",
            Self::AstronomerFront => "astronomerFront",
        }
    }

    #[must_use]
    pub fn value(self) -> f64 {
        match self {
            Self::Courtyard => 0.0,
            Self::SealGlyphs => 10.0,
            Self::SealCore => 11.0,
            Self::UsefulThyngsShadow => 900.0,
            Self::Fountain => 980.0,
            Self::TeacherPreWorld => 999.0,
            Self::StatueAura => hub_actor_depth(834.0, 0.0) - 1.0,
            Self::Statue => hub_actor_depth(834.0, 0.0),
            Self::TeacherPostWorld => 2499.0,
            Self::CourtyardForeground => 2500.0,
            Self::CourtyardOnboarding => 2500.5,
            Self::SouthernForeground => 2501.0,
            Self::Astronomer => 2502.0,
            Self::AstronomerTelescope => 2503.0,
            Self::AstronomerFront => 2504.0,
        }
    }
}

/// Initial actor depth (`hubActorDepth`); the active painter queue later replaces it.
#[must_use]
pub fn hub_actor_depth(y: f64, reference_y: f64) -> f64 {
    1000.0 + painter_row(y, 0.0, reference_y) as f64
}
