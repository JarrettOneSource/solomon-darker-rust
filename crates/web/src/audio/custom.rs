//! Source GameAudioDirector `playAsset`, `startAssetLoop` and `startAssetMusic`.
//! This borrowed browser handle retains the existing context, not another audio engine.

use std::cell::RefCell;
use std::rc::Rc;

use solomon_sim::audio::AudioOptions;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, future_to_promise};
use web_sys::{AudioBuffer, HtmlAudioElement};

use super::{AudioState, audio_error, make_buffer_voice, play_music, validate_options};

pub(super) struct AssetMusicChannel {
    pub media: HtmlAudioElement,
    volume: f64,
}

#[derive(Clone)]
pub struct GameAssetAudio {
    pub(super) state: Rc<RefCell<AudioState>>,
}

impl GameAssetAudio {
    /// Source caches both fulfilled and rejected decode promises by immutable content URL.
    /// Decode the verified content bytes directly: fetching a Blob URL would add a
    /// connection outside the Website's `connect-src 'self'` policy. The stable SHA
    /// key still shares one fulfilled or rejected promise across session changes.
    pub async fn load_asset(&self, key: &str, bytes: &[u8]) -> Result<(), String> {
        let existing = self.state.borrow().asset_loads.get(key).cloned();
        let promise = if let Some(existing) = existing {
            existing
        } else {
            let state = self.state.clone();
            let buffer_key = key.to_owned();
            let encoded = js_sys::Uint8Array::from(bytes);
            let context = state.borrow().context.clone();
            let promise = future_to_promise(async move {
                let decoded = JsFuture::from(context.decode_audio_data(&encoded.buffer())?)
                    .await?
                    .dyn_into::<AudioBuffer>()?;
                state.borrow_mut().buffers.insert(buffer_key, decoded);
                Ok(JsValue::UNDEFINED)
            });
            self.state.borrow_mut().asset_loads.insert(key.to_owned(), promise.clone());
            promise
        };
        JsFuture::from(promise)
            .await
            .map(|_| ())
            .map_err(|error| audio_error("load custom audio", error))
    }

    pub fn play_asset(&self, key: &str, options: AudioOptions) -> Result<(), String> {
        validate_options(options)?;
        let mut state = self.state.borrow_mut();
        let buffer = state
            .buffers
            .get(key)
            .ok_or_else(|| format!("custom audio buffer is not loaded: {key}"))?;
        let voice = make_buffer_voice(&state, None, buffer, options, false, 0.0, &state.sound_mix)?;
        state.one_shots.push(voice);
        Ok(())
    }

    pub fn start_asset_loop(
        &self,
        owner: &str,
        key: &str,
        options: AudioOptions,
    ) -> Result<(), String> {
        validate_options(options)?;
        let mut state = self.state.borrow_mut();
        if let Some(previous) = state.asset_loops.remove(owner) {
            previous.stop()?;
        }
        let buffer = state
            .buffers
            .get(key)
            .ok_or_else(|| format!("custom audio buffer is not loaded: {key}"))?;
        let voice = make_buffer_voice(&state, None, buffer, options, true, 0.0, &state.sound_mix)?;
        state.asset_loops.insert(owner.to_owned(), voice);
        Ok(())
    }

    pub fn stop_asset_loop(&self, owner: &str) -> Result<(), String> {
        if let Some(previous) = self.state.borrow_mut().asset_loops.remove(owner) {
            previous.stop()?;
        }
        Ok(())
    }

    pub fn start_asset_music(&self, owner: &str, url: &str, volume: f64) -> Result<(), String> {
        self.stop_asset_music(owner)?;
        let mut state = self.state.borrow_mut();
        let media = HtmlAudioElement::new_with_src(url)
            .map_err(|error| audio_error("custom music", error))?;
        let volume = solomon_sim::hud::clamp_unit(volume);
        media.set_current_time(0.0);
        media.set_loop(true);
        media.set_preload("auto");
        media.set_volume(volume * state.music_volume * state.narration_mix);
        if state.unlocked && !state.music_paused {
            play_music(&media);
        }
        state.asset_music.insert(owner.to_owned(), AssetMusicChannel { media, volume });
        Ok(())
    }

    pub fn stop_asset_music(&self, owner: &str) -> Result<(), String> {
        if let Some(previous) = self.state.borrow_mut().asset_music.remove(owner) {
            previous.media.pause().map_err(|error| audio_error("stop custom music", error))?;
            previous.media.set_current_time(0.0);
            previous.media.set_volume(0.0);
        }
        Ok(())
    }
}

pub(super) fn update_music_volume(state: &AudioState) {
    for channel in state.asset_music.values() {
        channel.media.set_volume(channel.volume * state.music_volume * state.narration_mix);
    }
}
