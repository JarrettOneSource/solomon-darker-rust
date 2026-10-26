//! Shared audio director: cue admission, owned loops, narration mixing and music fades.
//! Browser WebAudio and native device playback supply the samples and playback clock.

mod custom;
pub use custom::GameAssetAudio;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use futures_util::future::{LocalBoxFuture, Shared};
use solomon_graphics::assets;
use solomon_sim::audio::{
    AUDIO_CATALOG, AudioCue, AudioEvent, AudioKind, AudioOptions, MusicScene,
};

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
use browser as platform;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
use native as platform;
use platform::{AudioBuffer, AudioEngine, Music, Voice, play_music, play_stock_music};

type AssetDecode = Shared<LocalBoxFuture<'static, Result<AudioBuffer, String>>>;

#[derive(Clone, Copy)]
enum AudioBus {
    Effects,
    Narration,
}

struct PlayingVoice {
    cue: Option<AudioCue>,
    handle: Voice,
    ends_at: Option<f64>,
}
impl PlayingVoice {
    fn disconnect(&self) -> Result<(), String> {
        self.handle.disconnect()
    }
    fn stop(&self) -> Result<(), String> {
        self.handle.stop()
    }
}
struct LoopRequest {
    cue: AudioCue,
    owners: Vec<(String, AudioOptions)>,
    current: Option<(AudioOptions, PlayingVoice)>,
}
struct MusicChannel {
    cue: AudioCue,
    media: Music,
    envelope: f64,
    #[cfg(target_arch = "wasm32")]
    priming: bool,
    #[cfg(target_arch = "wasm32")]
    primed: bool,
}
struct MusicFade {
    started_at: f64,
    duration: f64,
    incoming_start: f64,
    outgoing_start: f64,
}
struct AudioState {
    context: AudioEngine,
    buffers: BTreeMap<String, AudioBuffer>,
    asset_loads: BTreeMap<String, AssetDecode>,
    asset_loops: BTreeMap<String, PlayingVoice>,
    asset_music: BTreeMap<String, custom::AssetMusicChannel>,
    one_shots: Vec<PlayingVoice>,
    streams: BTreeMap<AudioCue, PlayingVoice>,
    loops: Vec<LoopRequest>,
    music: Vec<MusicChannel>,
    scene: Option<MusicScene>,
    incoming: Option<usize>,
    outgoing: Option<usize>,
    fade: Option<MusicFade>,
    sound_volume: f64,
    music_volume: f64,
    narration_mix: f64,
    unlocked: bool,
    music_paused: bool,
}

pub struct GameAudio {
    state: Rc<RefCell<AudioState>>,
    #[cfg(target_arch = "wasm32")]
    _activation: platform::ActivationListeners,
}

impl GameAudio {
    /// Playback diagnostics report the real director and device clock.
    pub fn snapshot(&self) -> serde_json::Value {
        let state = self.state.borrow();
        serde_json::json!({
            "clockSeconds": state.context.current_time(),
            "buffers": state.buffers.len(),
            "oneShots": state.one_shots.len(),
            "streams": state.streams.keys().map(|cue| cue.as_str()).collect::<Vec<_>>(),
            "loops": state.loops.iter().filter(|request| request.current.is_some()).map(|request| request.cue.as_str()).collect::<Vec<_>>(),
            "soundVolume": state.sound_volume, "musicVolume": state.music_volume,
            "narrationMix": state.narration_mix, "musicPaused": state.music_paused,
            "music": state.music.iter().map(|channel| serde_json::json!({
                "cue": channel.cue.as_str(), "envelope": channel.envelope, "paused": channel.media.paused(),
            })).collect::<Vec<_>>(),
        })
    }

    pub(crate) fn startup_resources() -> Vec<crate::runtime_ui::StartupResource> {
        AUDIO_CATALOG
            .iter()
            .map(|definition| {
                crate::runtime_ui::StartupResource::new(
                    "audio",
                    format!("assets/stock/audio/{}", definition.path),
                )
            })
            .collect()
    }

    pub async fn load(surface: &crate::platform::Surface) -> Result<Self, String> {
        crate::runtime_ui::enter_phase("audio")?;
        let context = AudioEngine::new(surface)?;
        let state = Rc::new(RefCell::new(AudioState {
            context: context.clone(),
            buffers: BTreeMap::new(),
            asset_loads: BTreeMap::new(),
            asset_loops: BTreeMap::new(),
            asset_music: BTreeMap::new(),
            one_shots: Vec::new(),
            streams: BTreeMap::new(),
            loops: Vec::new(),
            music: Vec::new(),
            scene: None,
            incoming: None,
            outgoing: None,
            fade: None,
            sound_volume: 1.0,
            music_volume: 1.0,
            narration_mix: 1.0,
            unlocked: cfg!(not(target_arch = "wasm32")),
            music_paused: false,
        }));
        let mut paths = std::collections::BTreeSet::new();
        let mut work = Vec::new();
        for definition in AUDIO_CATALOG {
            if !paths.insert(definition.path) {
                continue;
            }
            let state = state.clone();
            let context = context.clone();
            work.push(async move {
                if definition.kind == AudioKind::Music {
                    let media = context.load_music(definition.cue).await?;
                    state.borrow_mut().music.push(MusicChannel {
                        cue: definition.cue,
                        media,
                        envelope: 0.0,
                        #[cfg(target_arch = "wasm32")]
                        priming: false,
                        #[cfg(target_arch = "wasm32")]
                        primed: false,
                    });
                } else {
                    let bytes =
                        assets::fetch_bytes(&format!("assets/stock/audio/{}", definition.path))
                            .await?;
                    let buffer = context.decode(bytes).await?;
                    state.borrow_mut().buffers.insert(definition.path.to_owned(), buffer);
                }
                crate::runtime_ui::completed(&format!("assets/stock/audio/{}", definition.path))
            });
        }
        futures_util::future::try_join_all(work).await?;
        #[cfg(target_arch = "wasm32")]
        let activation = platform::ActivationListeners::attach(surface, &state)?;
        Ok(Self {
            state,
            #[cfg(target_arch = "wasm32")]
            _activation: activation,
        })
    }

    /// Custom content shares the one context, mixer and buffer cache. The document's
    /// trusted-gesture listeners remain owned by GameAudio.
    #[must_use]
    pub fn assets(&self) -> GameAssetAudio {
        GameAssetAudio { state: self.state.clone() }
    }

    pub fn set_volumes(&self, sound_percent: u16, music_percent: u16) {
        let mut state = self.state.borrow_mut();
        state.sound_volume = f64::from(sound_percent.min(100)) / 100.0;
        state.music_volume = f64::from(music_percent.min(100)) / 100.0;
        state.context.set_sound_volume(state.sound_volume as f32);
        for channel in &state.music {
            channel.media.set_volume(channel.envelope * state.music_volume * state.narration_mix);
        }
        custom::update_music_volume(&state);
    }

    /// Boss speech ducks music and sound effects. Narration streams still use the
    /// player's sound volume but bypass this mix, matching `setSoundMixVolume`.
    pub fn set_narration_mix(&self, mix: f64) {
        let mut state = self.state.borrow_mut();
        state.narration_mix = solomon_sim::hud::clamp_unit(mix);
        state.context.set_narration_mix(state.narration_mix as f32);
        for channel in &state.music {
            channel.media.set_volume(channel.envelope * state.music_volume * state.narration_mix);
        }
        custom::update_music_volume(&state);
    }

    pub fn set_scene(&self, scene: MusicScene) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        if state.scene == Some(scene) {
            return Ok(());
        }
        if let Some(index) = state.outgoing.take() {
            reset_music(&mut state.music[index])?;
        }
        state.outgoing = state.incoming.take();
        let index = state
            .music
            .iter()
            .position(|channel| channel.cue == scene.cue())
            .expect("stock scene music");
        state.incoming = Some(index);
        state.scene = Some(scene);
        state.music[index].media.rewind();
        state.music[index].envelope = 0.0;
        state.music[index].media.set_volume(0.0);
        let outgoing_start = state.outgoing.map_or(0.0, |outgoing| state.music[outgoing].envelope);
        state.fade = Some(MusicFade {
            started_at: state.context.current_time(),
            duration: f64::from(scene.transition_ticks()) * 0.01,
            incoming_start: 0.0,
            outgoing_start,
        });
        if state.unlocked && !state.music_paused {
            play_stock_music(&self.state, index, &mut state.music[index])?;
        }
        Ok(())
    }

    /// Call once per presentation frame. AudioContext time stops while suspended, so voice
    /// admission and music fading cannot expire before an actual browser audio sample plays.
    pub fn update(&self) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        state.context.check()?;
        let now = state.context.current_time();
        let mut index = 0;
        while index < state.one_shots.len() {
            if state.one_shots[index].ends_at.is_some_and(|end| end <= now) {
                state.one_shots.remove(index).disconnect()?;
            } else {
                index += 1;
            }
        }
        let ended: Vec<_> = state
            .streams
            .iter()
            .filter(|(_, voice)| voice.ends_at.is_some_and(|end| end <= now))
            .map(|(cue, _)| *cue)
            .collect();
        for cue in ended {
            state.streams.remove(&cue).expect("ended stream").disconnect()?;
        }
        let Some(fade) = &state.fade else {
            return Ok(());
        };
        let progress = if fade.duration == 0.0 {
            1.0
        } else {
            ((now - fade.started_at) / fade.duration).clamp(0.0, 1.0)
        };
        let incoming = fade.incoming_start + (1.0 - fade.incoming_start) * progress;
        let outgoing = fade.outgoing_start * (1.0 - progress);
        if let Some(index) = state.incoming {
            state.music[index].envelope = incoming;
            state.music[index]
                .media
                .set_volume(incoming * state.music_volume * state.narration_mix);
        }
        if let Some(index) = state.outgoing {
            state.music[index].envelope = outgoing;
            state.music[index]
                .media
                .set_volume(outgoing * state.music_volume * state.narration_mix);
        }
        if progress == 1.0 {
            if let Some(index) = state.outgoing.take() {
                reset_music(&mut state.music[index])?;
            }
            state.fade = None;
        }
        Ok(())
    }

    pub fn apply(&self, event: AudioEvent) -> Result<(), String> {
        match event {
            AudioEvent::Play(cue, options) => {
                self.play(cue, options)?;
            }
            AudioEvent::PlayAfter { cue, options, delay_ms } => {
                self.play_after(cue, options, delay_ms)?;
            }
            AudioEvent::StopStream(cue) => self.stop_stream(cue)?,
        }
        Ok(())
    }

    pub fn apply_combat(
        &self,
        output: solomon_sim::audio::combat::CombatAudioOutput,
    ) -> Result<(), String> {
        use solomon_sim::audio::combat::{
            BONEYARD_FACULTY_DIALOGUE_CUES, BONEYARD_SOLOMON_DIALOGUE_CUES, CombatAudioLoopCommand,
        };
        if output.stop_encounter_dialogue {
            for cue in
                BONEYARD_FACULTY_DIALOGUE_CUES.into_iter().chain(BONEYARD_SOLOMON_DIALOGUE_CUES)
            {
                self.stop_stream(cue)?;
            }
            for cue in solomon_sim::boneyard::tutorial::NativeTutorialCue::ALL {
                self.stop_stream(AudioCue::from_id(cue.id()).expect("stock Tutorial cue"))?;
            }
        }
        if let Some(mix) = output.narration_mix {
            self.set_narration_mix(mix);
        }
        for event in output.events {
            self.apply(event)?;
        }
        for command in output.loops {
            match command {
                CombatAudioLoopCommand::Start(request) => {
                    self.start_loop(request.cue, &request.owner, request.options)?;
                }
                CombatAudioLoopCommand::Stop { cue, owner } => self.stop_loop(cue, &owner)?,
            }
        }
        Ok(())
    }

    pub fn play(&self, cue: AudioCue, options: AudioOptions) -> Result<bool, String> {
        self.play_after(cue, options, 0.0)
    }

    pub fn play_after(
        &self,
        cue: AudioCue,
        options: AudioOptions,
        delay_ms: f64,
    ) -> Result<bool, String> {
        if !delay_ms.is_finite() || delay_ms < 0.0 {
            return Err("audio delay is invalid".into());
        }
        validate_options(options)?;
        let mut state = self.state.borrow_mut();
        match cue.definition().kind {
            AudioKind::Sound => {
                if let Some(maximum) = cue.maximum_voices() {
                    let count = state
                        .one_shots
                        .iter()
                        .filter(|voice| {
                            voice.cue.is_some_and(|voice_cue| {
                                voice_cue.definition().path == cue.definition().path
                            }) && voice.ends_at.is_none_or(|end| end > state.context.current_time())
                        })
                        .count();
                    if count >= maximum {
                        return Ok(false);
                    }
                }
                let voice = make_voice(&state, cue, options, false, delay_ms / 1000.0)?;
                state.one_shots.push(voice);
            }
            AudioKind::Stream => {
                if let Some(current) = state.streams.remove(&cue) {
                    current.stop()?;
                }
                let voice = make_voice(&state, cue, options, false, delay_ms / 1000.0)?;
                state.streams.insert(cue, voice);
            }
            AudioKind::Loop | AudioKind::Music => {
                return Err(format!("{} requires an owned loop or music scene", cue.as_str()));
            }
        }
        Ok(true)
    }

    pub fn stop_stream(&self, cue: AudioCue) -> Result<(), String> {
        if let Some(voice) = self.state.borrow_mut().streams.remove(&cue) {
            voice.stop()?;
        }
        Ok(())
    }

    pub fn start_loop(
        &self,
        cue: AudioCue,
        owner: &str,
        options: AudioOptions,
    ) -> Result<(), String> {
        validate_options(options)?;
        if cue.definition().kind != AudioKind::Loop {
            return Err(format!("{} is not a stock loop", cue.as_str()));
        }
        let mut state = self.state.borrow_mut();
        let index = if let Some(index) = state.loops.iter().position(|request| request.cue == cue) {
            index
        } else {
            state.loops.push(LoopRequest { cue, owners: Vec::new(), current: None });
            state.loops.len() - 1
        };
        let request = &mut state.loops[index];
        if let Some((_, current)) = request.owners.iter_mut().find(|(current, _)| current == owner)
        {
            *current = options;
        } else {
            request.owners.push((owner.to_owned(), options));
        }
        sync_loop(&mut state, index)
    }

    pub fn stop_loop(&self, cue: AudioCue, owner: &str) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        let Some(index) = state.loops.iter().position(|request| request.cue == cue) else {
            return Ok(());
        };
        state.loops[index].owners.retain(|(current, _)| current != owner);
        sync_loop(&mut state, index)
    }

    pub fn stop_loops_for_owner(&self, owner: &str) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        for index in 0..state.loops.len() {
            state.loops[index].owners.retain(|(current, _)| current != owner);
            sync_loop(&mut state, index)?;
        }
        Ok(())
    }

    pub fn stop_effects(&self) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        for voice in state.one_shots.drain(..) {
            voice.stop()?;
        }
        for (_, voice) in std::mem::take(&mut state.streams) {
            voice.stop()?;
        }
        for request in state.loops.drain(..) {
            if let Some((_, voice)) = request.current {
                voice.stop()?;
            }
        }
        Ok(())
    }

    pub fn set_music_paused(&self, paused: bool) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        state.music_paused = paused;
        if paused {
            for channel in &state.music {
                channel.media.pause()?;
            }
        } else if state.unlocked {
            for index in [state.incoming, state.outgoing].into_iter().flatten() {
                play_stock_music(&self.state, index, &mut state.music[index])?;
            }
        }
        for channel in state.asset_music.values() {
            if paused {
                channel.media.pause()?;
            } else if state.unlocked {
                play_music(&channel.media)?;
            }
        }
        Ok(())
    }
}

fn make_voice(
    state: &AudioState,
    cue: AudioCue,
    options: AudioOptions,
    looping: bool,
    delay_seconds: f64,
) -> Result<PlayingVoice, String> {
    let buffer = state
        .buffers
        .get(cue.definition().path)
        .ok_or_else(|| format!("{} buffer was not loaded", cue.as_str()))?;
    let output = if cue.definition().kind == AudioKind::Stream {
        AudioBus::Narration
    } else {
        AudioBus::Effects
    };
    make_buffer_voice(state, Some(cue), buffer, options, looping, delay_seconds, output)
}

fn make_buffer_voice(
    state: &AudioState,
    cue: Option<AudioCue>,
    buffer: &AudioBuffer,
    options: AudioOptions,
    looping: bool,
    delay_seconds: f64,
    bus: AudioBus,
) -> Result<PlayingVoice, String> {
    let handle = state.context.voice(buffer, options, looping, delay_seconds, bus)?;
    Ok(PlayingVoice {
        cue,
        handle,
        ends_at: if looping {
            None
        } else {
            Some(
                state.context.current_time()
                    + delay_seconds
                    + (buffer.duration() - options.offset_seconds).max(0.0) / options.playback_rate,
            )
        },
    })
}

fn sync_loop(state: &mut AudioState, index: usize) -> Result<(), String> {
    let request = &mut state.loops[index];
    let mut loudest = None;
    for (_, options) in &request.owners {
        if loudest.is_none_or(|current: AudioOptions| options.volume > current.volume) {
            loudest = Some(*options);
        }
    }
    let next = loudest.filter(|options| options.volume > 0.0);
    let previous = request.current.as_ref().map(|(options, _)| *options);
    if previous == next {
        return Ok(());
    }
    if let (Some(previous), Some(next)) = (previous, next)
        && previous.playback_rate == next.playback_rate
    {
        let current = request.current.as_mut().expect("retained loop");
        current.0 = next;
        current.1.handle.set_volume(next.volume as f32);
        return Ok(());
    }
    if let Some((_, voice)) = request.current.take() {
        voice.stop()?;
    }
    if let Some(next) = next {
        let cue = request.cue;
        let voice = make_voice(state, cue, next, true, 0.0)?;
        state.loops[index].current = Some((next, voice));
    }
    Ok(())
}

fn reset_music(channel: &mut MusicChannel) -> Result<(), String> {
    channel.media.pause()?;
    channel.media.rewind();
    channel.media.set_volume(0.0);
    channel.envelope = 0.0;
    Ok(())
}
fn validate_options(options: AudioOptions) -> Result<(), String> {
    if !options.playback_rate.is_finite()
        || !(options.playback_rate as f32).is_finite()
        || options.playback_rate as f32 <= 0.0
        || !options.volume.is_finite()
        || !options.offset_seconds.is_finite()
        || options.offset_seconds < 0.0
    {
        return Err("audio rate/gain/offset are invalid".into());
    }
    Ok(())
}
