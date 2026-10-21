//! Stock cast/loop cue ownership from `native-weld-primary-runtime.ts`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeWeldCastCue {
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

impl NativeWeldCastCue {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeWeldAudioPlan {
    pub build_id: u32,
    pub cue: NativeWeldCastCue,
    pub looping: bool,
    pub native_loop_ids: &'static [u32],
    pub native_sound_ids: &'static [u32],
    pub native_sound_variant_ids: &'static [u32],
}

#[must_use]
pub fn native_weld_audio_plan(build_id: u32) -> NativeWeldAudioPlan {
    let (cue, loops, sounds, variants): (_, &'static [u32], &'static [u32], &'static [u32]) =
        match build_id {
            1000 => (NativeWeldCastCue::BurningBolt, &[], &[57, 97], &[]),
            1001 => (NativeWeldCastCue::FrostMissile, &[], &[38], &[]),
            1002 => (NativeWeldCastCue::BallLightning, &[], &[], &[224, 225]),
            1003 => (NativeWeldCastCue::FlameLashLoop, &[157], &[33], &[]),
            1004 => (NativeWeldCastCue::BlizzardBeamLoop, &[160], &[44], &[]),
            1005 => (NativeWeldCastCue::SteamJetLoop, &[172, 157], &[], &[]),
            1006 => (NativeWeldCastCue::EtherealBoulderLoop, &[159], &[87], &[]),
            1007 => (NativeWeldCastCue::MeteorSwarmLoop, &[165], &[], &[]),
            1008 => (NativeWeldCastCue::HailstonesLoop, &[160, 159], &[87], &[]),
            1009 => (NativeWeldCastCue::CrawlingShock, &[], &[], &[203, 204, 205]),
            _ => panic!("unknown native Weld build"),
        };
    NativeWeldAudioPlan {
        build_id,
        cue,
        looping: !loops.is_empty(),
        native_loop_ids: loops,
        native_sound_ids: sounds,
        native_sound_variant_ids: variants,
    }
}
