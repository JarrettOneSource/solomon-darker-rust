//! Each Weld build's stock cast cue and its one-shot, variant and loop sounds.

use crate::profile::WeldBuildId;

/// The named cast cue of each Weld build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeldCastCue {
    BallLightning,
    BlizzardBeamLoop,
    BurningBolt,
    CrawlingShock,
    EtherealBoulderLoop,
    FlameLashLoop,
    FrostMissile,
    HailstonesLoop,
    MeteorSwarmLoop,
    SteamJetLoop,
}

impl WeldCastCue {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BallLightning => "ball-lightning",
            Self::BlizzardBeamLoop => "blizzard-beam-loop",
            Self::BurningBolt => "burning-bolt",
            Self::CrawlingShock => "crawling-shock",
            Self::EtherealBoulderLoop => "ethereal-boulder-loop",
            Self::FlameLashLoop => "flame-lash-loop",
            Self::FrostMissile => "frost-missile",
            Self::HailstonesLoop => "hailstones-loop",
            Self::MeteorSwarmLoop => "meteor-swarm-loop",
            Self::SteamJetLoop => "steam-jet-loop",
        }
    }
}

/// A Weld build's sounds: its cue, loop sounds, one-shot sounds and random variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeldAudioPlan {
    pub build_id: WeldBuildId,
    pub cue: WeldCastCue,
    pub looping: bool,
    pub loop_ids: &'static [u32],
    pub sound_ids: &'static [u32],
    pub sound_variant_ids: &'static [u32],
}

#[must_use]
pub fn weld_audio_plan(build_id: WeldBuildId) -> WeldAudioPlan {
    let (cue, loops, sounds, variants): (_, &'static [u32], &'static [u32], &'static [u32]) =
        match build_id {
            WeldBuildId::BurningBolt => (WeldCastCue::BurningBolt, &[], &[57, 97], &[]),
            WeldBuildId::FrostMissile => (WeldCastCue::FrostMissile, &[], &[38], &[]),
            WeldBuildId::BallLightning => (WeldCastCue::BallLightning, &[], &[], &[224, 225]),
            WeldBuildId::FlameLash => (WeldCastCue::FlameLashLoop, &[157], &[33], &[]),
            WeldBuildId::BlizzardBeam => (WeldCastCue::BlizzardBeamLoop, &[160], &[44], &[]),
            WeldBuildId::SteamJet => (WeldCastCue::SteamJetLoop, &[172, 157], &[], &[]),
            WeldBuildId::EtherealBoulder => (WeldCastCue::EtherealBoulderLoop, &[159], &[87], &[]),
            WeldBuildId::MeteorSwarm => (WeldCastCue::MeteorSwarmLoop, &[165], &[], &[]),
            WeldBuildId::Hailstones => (WeldCastCue::HailstonesLoop, &[160, 159], &[87], &[]),
            WeldBuildId::CrawlingShock => (WeldCastCue::CrawlingShock, &[], &[], &[203, 204, 205]),
        };
    WeldAudioPlan {
        build_id,
        cue,
        looping: !loops.is_empty(),
        loop_ids: loops,
        sound_ids: sounds,
        sound_variant_ids: variants,
    }
}
