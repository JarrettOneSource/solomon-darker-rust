//! Source GameAudioDirector `playAsset`, `startAssetLoop` and `startAssetMusic`.
//! This borrowed browser handle retains the existing context, not another audio engine.

use std::cell::RefCell;
use std::rc::Rc;

use futures_util::FutureExt;
use solomon_sim::audio::AudioOptions;
use std::sync::Arc;

use super::{AudioBus, AudioState, Music, make_buffer_voice, play_music, validate_options};

pub(super) struct AssetMusicChannel {
    pub media: Music,
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
        let decode = if let Some(existing) = existing {
            existing
        } else {
            let context = self.state.borrow().context.clone();
            let bytes = bytes.to_vec();
            let decode = async move { context.decode(bytes).await }.boxed_local().shared();
            self.state.borrow_mut().asset_loads.insert(key.to_owned(), decode.clone());
            decode
        };
        let buffer = decode.await?;
        self.state.borrow_mut().buffers.entry(key.to_owned()).or_insert(buffer);
        Ok(())
    }

    pub fn play_asset(&self, key: &str, options: AudioOptions) -> Result<(), String> {
        validate_options(options)?;
        let mut state = self.state.borrow_mut();
        let buffer = state
            .buffers
            .get(key)
            .ok_or_else(|| format!("custom audio buffer is not loaded: {key}"))?;
        let voice =
            make_buffer_voice(&state, None, buffer, options, false, 0.0, AudioBus::Effects)?;
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
        let voice = make_buffer_voice(&state, None, buffer, options, true, 0.0, AudioBus::Effects)?;
        state.asset_loops.insert(owner.to_owned(), voice);
        Ok(())
    }

    pub fn stop_asset_loop(&self, owner: &str) -> Result<(), String> {
        if let Some(previous) = self.state.borrow_mut().asset_loops.remove(owner) {
            previous.stop()?;
        }
        Ok(())
    }

    pub fn start_asset_music(
        &self,
        owner: &str,
        url: &str,
        bytes: Arc<[u8]>,
        volume: f64,
    ) -> Result<(), String> {
        self.stop_asset_music(owner)?;
        let mut state = self.state.borrow_mut();
        let media = state.context.asset_music(url, bytes)?;
        let volume = solomon_sim::hud::clamp_unit(volume);
        media.rewind();
        media.set_volume(volume * state.music_volume * state.narration_mix);
        if state.unlocked && !state.music_paused {
            play_music(&media)?;
        }
        state.asset_music.insert(owner.to_owned(), AssetMusicChannel { media, volume });
        Ok(())
    }

    pub fn stop_asset_music(&self, owner: &str) -> Result<(), String> {
        if let Some(previous) = self.state.borrow_mut().asset_music.remove(owner) {
            previous.media.pause()?;
            previous.media.rewind();
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
