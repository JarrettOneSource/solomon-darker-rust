//! Browser audio edge: decoded stock buffers, native WebAudio sources and owned HTML
//! music channels. Gameplay supplies cues/rates; this module never rolls random values.

mod custom;
pub use custom::GameAssetAudio;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use solomon_sim::audio::{
    AUDIO_CATALOG, AudioCue, AudioEvent, AudioKind, AudioOptions, MusicScene,
};
use wasm_bindgen::{JsCast, JsValue, prelude::*};
use wasm_bindgen_futures::{JsFuture, future_to_promise, spawn_local};
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState, Document, Event, GainNode,
    HtmlAudioElement,
};

use solomon_graphics::assets;

/// Source `game-audio-browser.ts::loadMusicChannel` admits a channel on loadeddata.
/// A failed resident music fetch is a startup error, just like a failed effect buffer.
async fn ready_music(media: &HtmlAudioElement, path: &str) -> Result<(), String> {
    if media.ready_state() >= 2 {
        return Ok(());
    }
    let mut settle = None;
    let ready = js_sys::Promise::new(&mut |resolve, reject| settle = Some((resolve, reject)));
    let (resolve, reject) = settle.expect("Promise installs its resolvers synchronously");
    let loaded = Closure::<dyn FnMut(Event)>::new(move |_| {
        let _ = resolve.call0(&JsValue::UNDEFINED);
    });
    let failed = Closure::<dyn FnMut(Event)>::new(move |_| {
        let _ = reject.call1(&JsValue::UNDEFINED, &JsValue::from_str("could not load game music"));
    });
    media
        .add_event_listener_with_callback("loadeddata", loaded.as_ref().unchecked_ref())
        .map_err(|error| audio_error("music readiness listener", error))?;
    if let Err(error) =
        media.add_event_listener_with_callback("error", failed.as_ref().unchecked_ref())
    {
        media
            .remove_event_listener_with_callback("loadeddata", loaded.as_ref().unchecked_ref())
            .map_err(|error| audio_error("music readiness cleanup", error))?;
        return Err(audio_error("music error listener", error));
    }
    media.load();
    let result = JsFuture::from(ready).await;
    media
        .remove_event_listener_with_callback("loadeddata", loaded.as_ref().unchecked_ref())
        .map_err(|error| audio_error("music readiness cleanup", error))?;
    media
        .remove_event_listener_with_callback("error", failed.as_ref().unchecked_ref())
        .map_err(|error| audio_error("music error cleanup", error))?;
    result.map(|_| ()).map_err(|error| audio_error(path, error))
}

struct PlayingVoice {
    cue: Option<AudioCue>,
    source: AudioBufferSourceNode,
    gain: GainNode,
    ends_at: Option<f64>,
}
impl PlayingVoice {
    fn disconnect(&self) -> Result<(), String> {
        self.source.disconnect().map_err(|error| audio_error("source disconnect", error))?;
        self.gain.disconnect().map_err(|error| audio_error("gain disconnect", error))
    }
    fn stop(&self) -> Result<(), String> {
        self.source
            .unchecked_ref::<web_sys::AudioScheduledSourceNode>()
            .stop()
            .map_err(|error| audio_error("source stop", error))?;
        self.disconnect()
    }
}
struct LoopRequest {
    cue: AudioCue,
    owners: Vec<(String, AudioOptions)>,
    current: Option<(AudioOptions, PlayingVoice)>,
}
struct MusicChannel {
    cue: AudioCue,
    media: HtmlAudioElement,
    envelope: f64,
    priming: bool,
    primed: bool,
}
struct MusicFade {
    started_at: f64,
    duration: f64,
    incoming_start: f64,
    outgoing_start: f64,
}
struct AudioState {
    context: AudioContext,
    master: GainNode,
    sound_mix: GainNode,
    buffers: BTreeMap<String, AudioBuffer>,
    asset_loads: BTreeMap<String, js_sys::Promise>,
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
    // The document owns these listeners for the game's lifetime. Resume is invoked directly
    // inside a trusted pointer/key event, before any asynchronous fetch or UI transition.
    _listeners: Vec<Closure<dyn FnMut(Event)>>,
}

impl GameAudio {
    pub(crate) fn startup_asset_count() -> usize {
        AUDIO_CATALOG
            .iter()
            .map(|definition| definition.path)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    }

    pub async fn load(document: &Document) -> Result<Self, String> {
        let context = AudioContext::new().map_err(|error| audio_error("create context", error))?;
        let master = context.create_gain().map_err(|error| audio_error("master gain", error))?;
        master
            .connect_with_audio_node(&context.destination())
            .map_err(|error| audio_error("master output", error))?;
        let sound_mix = context.create_gain().map_err(|error| audio_error("sound mix", error))?;
        sound_mix
            .connect_with_audio_node(&master)
            .map_err(|error| audio_error("sound mix output", error))?;
        let mut music = Vec::new();
        for definition in AUDIO_CATALOG {
            if definition.kind != AudioKind::Music {
                continue;
            }
            let media =
                HtmlAudioElement::new_with_src(&format!("assets/stock/audio/{}", definition.path))
                    .map_err(|error| audio_error("music channel", error))?;
            media.set_preload("auto");
            media.set_loop(true);
            media.set_volume(0.0);
            media.set_attribute("hidden", "").map_err(|_| "hide music channel")?;
            media.set_attribute("data-audio-cue", definition.id).map_err(|_| "music cue")?;
            document
                .body()
                .ok_or("audio document body")?
                .append_child(&media)
                .map_err(|_| "music channel attach")?;
            music.push(MusicChannel {
                cue: definition.cue,
                media,
                envelope: 0.0,
                priming: false,
                primed: false,
            });
        }
        let state = Rc::new(RefCell::new(AudioState {
            context: context.clone(),
            master,
            sound_mix,
            buffers: BTreeMap::new(),
            asset_loads: BTreeMap::new(),
            asset_loops: BTreeMap::new(),
            asset_music: BTreeMap::new(),
            one_shots: Vec::new(),
            streams: BTreeMap::new(),
            loops: Vec::new(),
            music,
            scene: None,
            incoming: None,
            outgoing: None,
            fade: None,
            sound_volume: 1.0,
            music_volume: 1.0,
            narration_mix: 1.0,
            unlocked: false,
            music_paused: false,
        }));
        let mut paths = Vec::new();
        for definition in AUDIO_CATALOG {
            if definition.kind != AudioKind::Music && !paths.contains(&definition.path) {
                paths.push(definition.path);
            }
        }
        let promises = js_sys::Array::new();
        for channel in &state.borrow().music {
            let media = channel.media.clone();
            let path = channel.cue.definition().path;
            promises.push(&future_to_promise(async move {
                ready_music(&media, path).await.map_err(JsValue::from)?;
                crate::runtime_ui::completed("audio", path, 1).map_err(JsValue::from)?;
                Ok(JsValue::UNDEFINED)
            }));
        }
        for path in paths {
            let state = state.clone();
            let context = context.clone();
            let promise = future_to_promise(async move {
                let bytes = assets::fetch_bytes(&format!("assets/stock/audio/{path}"))
                    .await
                    .map_err(|error| JsValue::from_str(&error))?;
                let encoded = js_sys::Uint8Array::from(bytes.as_slice());
                let decoded = context.decode_audio_data(&encoded.buffer())?;
                let buffer = JsFuture::from(decoded).await?.dyn_into::<AudioBuffer>()?;
                state.borrow_mut().buffers.insert(path.to_owned(), buffer);
                crate::runtime_ui::completed("audio", path, 1).map_err(JsValue::from)?;
                Ok(JsValue::UNDEFINED)
            });
            promises.push(&promise);
        }
        JsFuture::from(js_sys::Promise::all(&promises))
            .await
            .map_err(|error| audio_error("load stock audio", error))?;
        let mut listeners = Vec::new();
        let window = document.default_view().ok_or("audio activation window")?;
        for name in ["pointerdown", "keydown"] {
            let state = state.clone();
            let listener = Closure::<dyn FnMut(Event)>::new(move |_event: Event| {
                if let Err(error) = unlock(&state) {
                    web_sys::console::error_1(&JsValue::from_str(&error));
                }
            });
            // Source MainMenuScene captures before a focused canvas/control owns
            // the event and stops propagation. Resume must run in that gesture.
            window
                .add_event_listener_with_callback_and_bool(
                    name,
                    listener.as_ref().unchecked_ref(),
                    true,
                )
                .map_err(|_| "audio activation listener")?;
            listeners.push(listener);
        }
        Ok(Self { state, _listeners: listeners })
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
        state.master.gain().set_value(state.sound_volume as f32);
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
        state.sound_mix.gain().set_value(state.narration_mix as f32);
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
        state.music[index].media.set_current_time(0.0);
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
            play_stock_music(&self.state, index, &mut state.music[index], false)?;
        }
        Ok(())
    }

    /// Call once per presentation frame. AudioContext time stops while suspended, so voice
    /// admission and music fading cannot expire before an actual browser audio sample plays.
    pub fn update(&self) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
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
                channel.media.pause().map_err(|error| audio_error("pause music", error))?;
            }
        } else if state.unlocked {
            for index in [state.incoming, state.outgoing].into_iter().flatten() {
                play_stock_music(&self.state, index, &mut state.music[index], false)?;
            }
        }
        for channel in state.asset_music.values() {
            if paused {
                channel.media.pause().map_err(|error| audio_error("pause custom music", error))?;
            } else if state.unlocked {
                play_music(&channel.media);
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
    let output =
        if cue.definition().kind == AudioKind::Stream { &state.master } else { &state.sound_mix };
    make_buffer_voice(state, Some(cue), buffer, options, looping, delay_seconds, output)
}

fn make_buffer_voice(
    state: &AudioState,
    cue: Option<AudioCue>,
    buffer: &AudioBuffer,
    options: AudioOptions,
    looping: bool,
    delay_seconds: f64,
    output: &GainNode,
) -> Result<PlayingVoice, String> {
    let source = state
        .context
        .create_buffer_source()
        .map_err(|error| audio_error("buffer source", error))?;
    source.set_buffer(Some(buffer));
    source.set_loop(looping);
    source.playback_rate().set_value(options.playback_rate as f32);
    let gain = state.context.create_gain().map_err(|error| audio_error("voice gain", error))?;
    gain.gain().set_value(options.volume.clamp(0.0, 1.0) as f32);
    source
        .connect_with_audio_node(&gain)
        .map_err(|error| audio_error("voice gain connection", error))?;
    gain.connect_with_audio_node(output).map_err(|error| audio_error("voice output", error))?;
    source
        .start_with_when_and_grain_offset(
            state.context.current_time() + delay_seconds,
            options.offset_seconds,
        )
        .map_err(|error| audio_error("voice start", error))?;
    Ok(PlayingVoice {
        cue,
        source,
        gain,
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
        current.1.gain.gain().set_value(next.volume as f32);
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

fn unlock(state: &Rc<RefCell<AudioState>>) -> Result<(), String> {
    let mut inner = state.borrow_mut();
    inner.unlocked = true;
    if inner.context.state() != AudioContextState::Running {
        let resume = inner.context.resume().map_err(|error| audio_error("resume audio", error))?;
        spawn_local(async move {
            if let Err(error) = JsFuture::from(resume).await {
                web_sys::console::error_1(&error);
            }
        });
    }
    if !inner.music_paused {
        for index in [inner.incoming, inner.outgoing].into_iter().flatten() {
            if inner.music[index].media.paused() {
                play_stock_music(state, index, &mut inner.music[index], false)?;
            }
        }
        for index in 0..inner.music.len() {
            if inner.incoming == Some(index)
                || inner.outgoing == Some(index)
                || inner.music[index].primed
                || inner.music[index].priming
            {
                continue;
            }
            let channel = &mut inner.music[index];
            channel.media.set_current_time(0.0);
            channel.media.set_loop(true);
            channel.media.set_muted(false);
            channel.media.set_volume(0.0);
            play_stock_music(state, index, channel, true)?;
        }
        for channel in inner.asset_music.values() {
            if channel.media.paused() {
                play_music(&channel.media);
            }
        }
    }
    Ok(())
}
/// Source primes the resident songs in the trusted gesture. The native play call
/// stays synchronous; completion only touches this still-owned channel and checks
/// whether a scene/pause transition changed its role while the promise was pending.
fn play_stock_music(
    state: &Rc<RefCell<AudioState>>,
    index: usize,
    channel: &mut MusicChannel,
    prime: bool,
) -> Result<(), String> {
    let promise = channel.media.play().map_err(|error| audio_error("play music", error))?;
    if prime {
        channel.priming = true;
    }
    let state = Rc::downgrade(state);
    spawn_local(async move {
        let result = JsFuture::from(promise).await;
        let Some(state) = state.upgrade() else { return };
        let mut inner = state.borrow_mut();
        let owned = inner.incoming == Some(index) || inner.outgoing == Some(index);
        let paused = inner.music_paused;
        let channel = &mut inner.music[index];
        if prime {
            channel.priming = false;
        }
        channel.primed |= result.is_ok();
        let cleanup = if !owned {
            reset_music(channel)
        } else if paused {
            channel.media.pause().map_err(|error| audio_error("pause music", error))
        } else {
            Ok(())
        };
        if let Err(error) = cleanup {
            web_sys::console::error_1(&JsValue::from_str(&error));
        }
        if let Err(error) = result {
            report_music_rejection(&error);
        }
    });
    Ok(())
}

fn report_music_rejection(error: &JsValue) {
    // A fresh trusted gesture retries browser autoplay rejection. Other playback
    // errors are defects and remain visible to the browser acceptance observer.
    if js_sys::Reflect::get(error, &JsValue::from_str("name"))
        .ok()
        .and_then(|name| name.as_string())
        .as_deref()
        != Some("NotAllowedError")
    {
        web_sys::console::error_1(error);
    }
}

fn play_music(media: &HtmlAudioElement) {
    match media.play() {
        Ok(promise) => spawn_local(async move {
            if let Err(error) = JsFuture::from(promise).await {
                report_music_rejection(&error);
            }
        }),
        Err(error) => web_sys::console::error_1(&error),
    }
}
fn reset_music(channel: &mut MusicChannel) -> Result<(), String> {
    channel.media.pause().map_err(|error| audio_error("reset music", error))?;
    channel.media.set_current_time(0.0);
    channel.media.set_volume(0.0);
    channel.envelope = 0.0;
    Ok(())
}
fn validate_options(options: AudioOptions) -> Result<(), String> {
    if !options.playback_rate.is_finite()
        || options.playback_rate <= 0.0
        || !options.volume.is_finite()
        || !options.offset_seconds.is_finite()
        || options.offset_seconds < 0.0
    {
        return Err("audio rate/gain/offset are invalid".into());
    }
    Ok(())
}
fn audio_error(operation: &str, error: JsValue) -> String {
    format!("{operation}: {}", error.as_string().unwrap_or_else(|| format!("{error:?}")))
}
