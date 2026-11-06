//! Real WebAudio buffers and resident HTML media, including browser gesture admission.
use super::*;
use wasm_bindgen::{JsCast, JsValue, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    AudioBufferSourceNode, AudioContext, AudioContextState, Document, Event, GainNode,
    HtmlAudioElement,
};

pub(super) type AudioBuffer = web_sys::AudioBuffer;

#[derive(Clone)]
pub(super) struct AudioEngine(Rc<Engine>);
struct Engine {
    context: AudioContext,
    master: GainNode,
    sound_mix: GainNode,
    document: Document,
}

impl AudioEngine {
    pub fn new(surface: &crate::platform::Surface) -> Result<Self, String> {
        let document = surface.canvas().owner_document().ok_or("Audio document is unavailable")?;
        let context = AudioContext::new().map_err(|error| audio_error("create context", error))?;
        let master = context.create_gain().map_err(|error| audio_error("master gain", error))?;
        master
            .connect_with_audio_node(&context.destination())
            .map_err(|error| audio_error("master output", error))?;
        let sound_mix = context.create_gain().map_err(|error| audio_error("sound mix", error))?;
        sound_mix
            .connect_with_audio_node(&master)
            .map_err(|error| audio_error("sound mix output", error))?;
        Ok(Self(Rc::new(Engine { context, master, sound_mix, document })))
    }
    pub fn current_time(&self) -> f64 {
        self.0.context.current_time()
    }
    pub fn check(&self) -> Result<(), String> {
        if self.0.context.state() == AudioContextState::Closed {
            Err("The browser audio device closed".into())
        } else {
            Ok(())
        }
    }
    pub fn set_sound_volume(&self, value: f32) {
        self.0.master.gain().set_value(value);
    }
    pub fn set_narration_mix(&self, value: f32) {
        self.0.sound_mix.gain().set_value(value);
    }
    pub async fn decode(&self, bytes: Vec<u8>) -> Result<AudioBuffer, String> {
        let encoded = js_sys::Uint8Array::from(bytes.as_slice());
        let decoded = self
            .0
            .context
            .decode_audio_data(&encoded.buffer())
            .map_err(|error| audio_error("decode audio", error))?;
        JsFuture::from(decoded)
            .await
            .map_err(|error| audio_error("decode audio", error))?
            .dyn_into::<AudioBuffer>()
            .map_err(|_| "Decoded audio is not a buffer".into())
    }
    pub async fn load_music(&self, cue: AudioCue) -> Result<Music, String> {
        let raw = HtmlAudioElement::new().map_err(|error| audio_error("music channel", error))?;
        raw.set_preload("auto");
        raw.set_loop(true);
        raw.set_volume(0.0);
        raw.set_attribute("hidden", "").map_err(|_| "hide music channel")?;
        raw.set_attribute("data-audio-cue", cue.as_str()).map_err(|_| "music cue")?;
        self.0
            .document
            .body()
            .ok_or("audio document body")?
            .append_child(&raw)
            .map_err(|_| "music channel attach")?;
        let music = Music { raw };
        ready_music(&music.raw, cue.definition().path).await?;
        Ok(music)
    }
    pub fn asset_music(&self, url: &str, _bytes: std::sync::Arc<[u8]>) -> Result<Music, String> {
        let raw = HtmlAudioElement::new_with_src(url)
            .map_err(|error| audio_error("custom music", error))?;
        raw.set_loop(true);
        raw.set_preload("auto");
        Ok(Music { raw })
    }
    pub fn voice(
        &self,
        buffer: &AudioBuffer,
        options: AudioOptions,
        looping: bool,
        delay: f64,
        bus: AudioBus,
    ) -> Result<Voice, String> {
        let context = &self.0.context;
        let source =
            context.create_buffer_source().map_err(|error| audio_error("buffer source", error))?;
        source.set_buffer(Some(buffer));
        source.set_loop(looping);
        source.playback_rate().set_value(options.playback_rate as f32);
        let gain = context.create_gain().map_err(|error| audio_error("voice gain", error))?;
        gain.gain().set_value(options.volume.clamp(0.0, 1.0) as f32);
        source
            .connect_with_audio_node(&gain)
            .map_err(|error| audio_error("voice gain connection", error))?;
        let output = match bus {
            AudioBus::Effects => &self.0.sound_mix,
            AudioBus::Narration => &self.0.master,
        };
        gain.connect_with_audio_node(output).map_err(|error| audio_error("voice output", error))?;
        source
            .start_with_when_and_grain_offset(
                context.current_time() + delay,
                options.offset_seconds,
            )
            .map_err(|error| audio_error("voice start", error))?;
        Ok(Voice { source, gain })
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        match self.context.close() {
            Ok(promise) => spawn_local(async move {
                if let Err(error) = JsFuture::from(promise).await {
                    web_sys::console::error_1(&error);
                }
            }),
            Err(error) => web_sys::console::error_1(&error),
        }
    }
}

pub(super) struct Voice {
    source: AudioBufferSourceNode,
    gain: GainNode,
}
impl Voice {
    pub fn set_volume(&self, value: f32) {
        self.gain.gain().set_value(value);
    }
    pub fn disconnect(&self) -> Result<(), String> {
        self.source.disconnect().map_err(|error| audio_error("source disconnect", error))?;
        self.gain.disconnect().map_err(|error| audio_error("gain disconnect", error))
    }
    pub fn stop(&self) -> Result<(), String> {
        self.source
            .unchecked_ref::<web_sys::AudioScheduledSourceNode>()
            .stop()
            .map_err(|error| audio_error("source stop", error))?;
        self.disconnect()
    }
}

pub(super) struct Music {
    raw: HtmlAudioElement,
}
impl Music {
    pub fn pause(&self) -> Result<(), String> {
        self.raw.pause().map_err(|error| audio_error("pause music", error))
    }
    pub fn paused(&self) -> bool {
        self.raw.paused()
    }
    pub fn rewind(&self) {
        self.raw.set_current_time(0.0);
    }
    pub fn set_volume(&self, value: f64) {
        self.raw.set_volume(value);
    }
}
impl Drop for Music {
    fn drop(&mut self) {
        if let Err(error) = self.pause() {
            crate::platform::error(&error);
        }
        self.raw.remove();
    }
}

struct Activation {
    name: &'static str,
    callback: Closure<dyn FnMut(Event)>,
}
pub(super) struct ActivationListeners {
    window: web_sys::Window,
    callbacks: Vec<Activation>,
}
impl ActivationListeners {
    pub fn attach(
        surface: &crate::platform::Surface,
        state: &Rc<RefCell<AudioState>>,
    ) -> Result<Self, String> {
        let window = surface
            .canvas()
            .owner_document()
            .and_then(|document| document.default_view())
            .ok_or("Audio activation window unavailable")?;
        let mut owner = Self { window, callbacks: Vec::new() };
        for name in ["pointerdown", "keydown"] {
            let weak = Rc::downgrade(state);
            let callback = Closure::<dyn FnMut(Event)>::new(move |_| {
                if let Some(state) = weak.upgrade()
                    && let Err(error) = unlock(&state)
                {
                    crate::platform::error(&error);
                }
            });
            owner
                .window
                .add_event_listener_with_callback_and_bool(
                    name,
                    callback.as_ref().unchecked_ref(),
                    true,
                )
                .map_err(|_| "audio activation listener")?;
            owner.callbacks.push(Activation { name, callback });
        }
        Ok(owner)
    }
}
impl Drop for ActivationListeners {
    fn drop(&mut self) {
        for Activation { name, callback } in &self.callbacks {
            if let Err(error) = self.window.remove_event_listener_with_callback_and_bool(
                name,
                callback.as_ref().unchecked_ref(),
                true,
            ) {
                web_sys::console::error_1(&error);
            }
        }
    }
}

pub(super) fn play_stock_music(
    state: &Rc<RefCell<AudioState>>,
    index: usize,
    channel: &mut MusicChannel,
) -> Result<(), String> {
    play_stock_music_inner(state, index, channel, false)
}

fn audio_error(operation: &str, error: JsValue) -> String {
    format!("{operation}: {}", error.as_string().unwrap_or_else(|| format!("{error:?}")))
}
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
    // Install readiness handlers before starting the resource selection. Setting src
    // earlier and loading again in this async task aborts an already-started request.
    media.set_src(&format!("assets/stock/audio/{path}"));
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

fn unlock(state: &Rc<RefCell<AudioState>>) -> Result<(), String> {
    let mut inner = state.borrow_mut();
    inner.unlocked = true;
    if inner.context.0.context.state() != AudioContextState::Running {
        let resume =
            inner.context.0.context.resume().map_err(|error| audio_error("resume audio", error))?;
        spawn_local(async move {
            if let Err(error) = JsFuture::from(resume).await {
                web_sys::console::error_1(&error);
            }
        });
    }
    if !inner.music_paused {
        for index in [inner.incoming, inner.outgoing].into_iter().flatten() {
            if inner.music[index].media.paused() {
                play_stock_music_inner(state, index, &mut inner.music[index], false)?;
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
            channel.media.rewind();
            channel.media.raw.set_loop(true);
            channel.media.raw.set_muted(false);
            channel.media.set_volume(0.0);
            play_stock_music_inner(state, index, channel, true)?;
        }
        for channel in inner.asset_music.values() {
            if channel.media.paused() {
                play_music(&channel.media)?;
            }
        }
    }
    Ok(())
}
/// Source primes the resident songs in the trusted gesture. The native play call
/// stays synchronous; completion only touches this still-owned channel and checks
/// whether a scene/pause transition changed its role while the promise was pending.
fn play_stock_music_inner(
    state: &Rc<RefCell<AudioState>>,
    index: usize,
    channel: &mut MusicChannel,
    prime: bool,
) -> Result<(), String> {
    let promise = channel.media.raw.play().map_err(|error| audio_error("play music", error))?;
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
            channel.media.pause()
        } else {
            Ok(())
        };
        if let Err(error) = cleanup {
            web_sys::console::error_1(&JsValue::from_str(&error));
        }
        if let Err(error) = result {
            let name = js_sys::Reflect::get(&error, &JsValue::from_str("name"))
                .ok()
                .and_then(|name| name.as_string());
            // The actual pause/retirement may interrupt its pending play promise.
            // This does not excuse a media error/abort event or a resource reload.
            if name.as_deref() != Some("AbortError") || (owned && !paused) {
                report_music_rejection(&error);
            }
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

pub(super) fn play_music(media: &Music) -> Result<(), String> {
    let promise = media.raw.play().map_err(|error| audio_error("play music", error))?;
    spawn_local(async move {
        if let Err(error) = JsFuture::from(promise).await {
            report_music_rejection(&error);
        }
    });
    Ok(())
}
