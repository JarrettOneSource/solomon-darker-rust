//! Each Weld build's stock cast cue and its one-shot, variant and loop sounds.

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
    pub build_id: u32,
    pub cue: WeldCastCue,
    pub looping: bool,
    pub loop_ids: &'static [u32],
    pub sound_ids: &'static [u32],
    pub sound_variant_ids: &'static [u32],
}

#[must_use]
pub fn weld_audio_plan(build_id: u32) -> WeldAudioPlan {
    let (cue, loops, sounds, variants): (_, &'static [u32], &'static [u32], &'static [u32]) =
        match build_id {
            1000 => (WeldCastCue::BurningBolt, &[], &[57, 97], &[]),
            1001 => (WeldCastCue::FrostMissile, &[], &[38], &[]),
            1002 => (WeldCastCue::BallLightning, &[], &[], &[224, 225]),
            1003 => (WeldCastCue::FlameLashLoop, &[157], &[33], &[]),
            1004 => (WeldCastCue::BlizzardBeamLoop, &[160], &[44], &[]),
            1005 => (WeldCastCue::SteamJetLoop, &[172, 157], &[], &[]),
            1006 => (WeldCastCue::EtherealBoulderLoop, &[159], &[87], &[]),
            1007 => (WeldCastCue::MeteorSwarmLoop, &[165], &[], &[]),
            1008 => (WeldCastCue::HailstonesLoop, &[160, 159], &[87], &[]),
            1009 => (WeldCastCue::CrawlingShock, &[], &[], &[203, 204, 205]),
            _ => panic!("unknown native Weld build"),
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
