//! Native device output and decoding. The shared director owns cue and scene rules.

mod opus;

use super::*;
use rodio::{
    Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source,
    buffer::SamplesBuffer,
    mixer::{Mixer, mixer},
    source::Zero,
};
use std::cell::Cell;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
pub(super) struct AudioBuffer(SamplesBuffer);
impl AudioBuffer {
    pub fn duration(&self) -> f64 {
        self.0.total_duration().expect("PCM duration").as_secs_f64()
    }
}

struct Bus {
    mixer: Mixer,
    player: Player,
}
impl Bus {
    fn new(output: &Mixer, channels: rodio::ChannelCount, rate: rodio::SampleRate) -> Self {
        let (mixer, source) = mixer(channels, rate);
        // A resident bus continues accepting later voices after its last voice ends.
        mixer.add(Zero::new(channels, rate));
        let player = Player::connect_new(output);
        player.append(source);
        Self { mixer, player }
    }
}
struct Engine {
    root: Bus,
    master: Bus,
    effects: Bus,
    error: Arc<Mutex<Option<String>>>,
    _device: MixerDeviceSink,
}
#[derive(Clone)]
pub(super) struct AudioEngine(Rc<Engine>);

impl AudioEngine {
    pub fn new(_surface: &crate::platform::Surface) -> Result<Self, String> {
        let error = Arc::new(Mutex::new(None));
        let callback_error = error.clone();
        let mut device = DeviceSinkBuilder::from_default_device()
            .map_err(|error| format!("Open native audio device: {error}"))?
            .with_buffer_size(rodio::cpal::BufferSize::Fixed(1024))
            .with_error_callback(move |error| {
                *callback_error.lock().expect("audio error lock") =
                    Some(format!("Native audio output: {error}"));
            })
            .open_sink_or_fallback()
            .map_err(|error| format!("Start native audio output: {error}"))?;
        device.log_on_drop(false);
        let channels = device.config().channel_count();
        let rate = device.config().sample_rate();
        let root = Bus::new(device.mixer(), channels, rate);
        let master = Bus::new(&root.mixer, channels, rate);
        let effects = Bus::new(&master.mixer, channels, rate);
        Ok(Self(Rc::new(Engine { root, master, effects, error, _device: device })))
    }
    pub fn current_time(&self) -> f64 {
        self.0.root.player.get_pos().as_secs_f64()
    }
    pub fn check(&self) -> Result<(), String> {
        self.0.error.lock().expect("audio error lock").clone().map_or(Ok(()), Err)
    }
    pub fn set_sound_volume(&self, value: f32) {
        self.0.master.player.set_volume(value);
    }
    pub fn set_narration_mix(&self, value: f32) {
        self.0.effects.player.set_volume(value);
    }
    pub async fn decode(&self, bytes: Vec<u8>) -> Result<AudioBuffer, String> {
        blocking::unblock(move || decode(bytes)).await
    }
    pub async fn load_music(&self, cue: AudioCue) -> Result<Music, String> {
        let bytes =
            assets::fetch_bytes(&format!("assets/stock/audio/{}", cue.definition().path)).await?;
        Music::new(self.0.root.mixer.clone(), bytes.into(), self.0.error.clone())
    }
    pub fn asset_music(&self, _url: &str, bytes: Arc<[u8]>) -> Result<Music, String> {
        Music::new(self.0.root.mixer.clone(), bytes, self.0.error.clone())
    }
    pub fn voice(
        &self,
        buffer: &AudioBuffer,
        options: AudioOptions,
        looping: bool,
        delay: f64,
        bus: AudioBus,
    ) -> Result<Voice, String> {
        self.check()?;
        let mixer = match bus {
            AudioBus::Effects => &self.0.effects.mixer,
            AudioBus::Narration => &self.0.master.mixer,
        };
        let source = voice_source(buffer, options, looping, delay)?;
        let player = Player::connect_new(mixer);
        player.set_volume(options.volume.clamp(0.0, 1.0) as f32);
        player.append(source);
        Ok(Voice(player))
    }
}

fn decode(bytes: Vec<u8>) -> Result<AudioBuffer, String> {
    let error = Arc::new(Mutex::new(None));
    let decoder = stream_source(bytes.into(), false, error.clone())?;
    let channels = decoder.channels();
    let rate = decoder.sample_rate();
    let samples: Vec<f32> = decoder.collect();
    if let Some(error) = error.lock().expect("audio error lock").take() {
        return Err(error);
    }
    if samples.is_empty() {
        return Err("Decoded audio contains no samples".into());
    }
    Ok(AudioBuffer(SamplesBuffer::new(channels, rate, samples)))
}

fn stream_source(
    bytes: Arc<[u8]>,
    looping: bool,
    error: Arc<Mutex<Option<String>>>,
) -> Result<Box<dyn Source<Item = f32> + Send>, String> {
    if let Some(source) = opus::OpusSource::open(bytes.clone(), looping, error)? {
        return Ok(Box::new(source));
    }
    if looping {
        Decoder::new_looped(Cursor::new(bytes))
            .map(|source| Box::new(source) as Box<dyn Source<Item = f32> + Send>)
            .map_err(|error| format!("Decode native audio: {error}"))
    } else {
        Decoder::try_from(Cursor::new(bytes))
            .map(|source| Box::new(source) as Box<dyn Source<Item = f32> + Send>)
            .map_err(|error| format!("Decode native audio: {error}"))
    }
}

fn voice_source(
    buffer: &AudioBuffer,
    options: AudioOptions,
    looping: bool,
    delay: f64,
) -> Result<Box<dyn Source<Item = f32> + Send>, String> {
    let duration = |value| {
        Duration::try_from_secs_f64(value)
            .map_err(|_| "Audio time exceeds the supported range".to_owned())
    };
    let offset = if looping {
        options.offset_seconds % buffer.duration()
    } else {
        options.offset_seconds.min(buffer.duration())
    };
    let source: Box<dyn Source<Item = f32> + Send> = if looping {
        Box::new(buffer.0.clone().repeat_infinite().skip_duration(duration(offset)?))
    } else {
        Box::new(buffer.0.clone().skip_duration(duration(offset)?))
    };
    Ok(Box::new(source.speed(options.playback_rate as f32).delay(duration(delay)?)))
}

pub(super) struct Voice(Player);
impl Voice {
    pub fn set_volume(&self, value: f32) {
        self.0.set_volume(value);
    }
    pub fn stop(&self) -> Result<(), String> {
        self.0.stop();
        Ok(())
    }
    pub fn disconnect(&self) -> Result<(), String> {
        self.stop()
    }
}

pub(super) struct Music {
    mixer: Mixer,
    bytes: Arc<[u8]>,
    player: RefCell<Option<Player>>,
    volume: Cell<f32>,
    error: Arc<Mutex<Option<String>>>,
}
impl Music {
    pub fn paused(&self) -> bool {
        self.player.borrow().as_ref().is_none_or(Player::is_paused)
    }
    fn new(
        mixer: Mixer,
        bytes: Arc<[u8]>,
        error: Arc<Mutex<Option<String>>>,
    ) -> Result<Self, String> {
        // Prime the actual decoder so startup rejects invalid resident songs.
        let mut decoder = stream_source(bytes.clone(), false, error.clone())?;
        if decoder.next().is_none() {
            return Err("Music contains no samples".into());
        }
        Ok(Self { mixer, bytes, player: RefCell::new(None), volume: Cell::new(0.0), error })
    }
    fn play(&self) -> Result<(), String> {
        let mut player = self.player.borrow_mut();
        if let Some(player) = player.as_ref() {
            player.play();
        } else {
            // The decoder seeks its encoded input at loop boundaries; songs never
            // accumulate a whole decoded PCM copy just to repeat.
            let source = stream_source(self.bytes.clone(), true, self.error.clone())?;
            let next = Player::connect_new(&self.mixer);
            next.set_volume(self.volume.get());
            next.append(source);
            *player = Some(next);
        }
        Ok(())
    }
    pub fn pause(&self) -> Result<(), String> {
        if let Some(player) = self.player.borrow().as_ref() {
            player.pause();
        }
        Ok(())
    }
    pub fn rewind(&self) {
        self.player.borrow_mut().take();
    }
    pub fn set_volume(&self, value: f64) {
        self.volume.set(value as f32);
        if let Some(player) = self.player.borrow().as_ref() {
            player.set_volume(value as f32);
        }
    }
}

pub(super) fn play_stock_music(
    _state: &Rc<RefCell<AudioState>>,
    _index: usize,
    channel: &mut MusicChannel,
) -> Result<(), String> {
    channel.media.play()
}
pub(super) fn play_music(media: &Music) -> Result<(), String> {
    media.play()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZero;

    #[test]
    fn decoded_stock_sound_contains_pcm_and_rejects_invalid_content() {
        let cue = AUDIO_CATALOG.iter().find(|cue| cue.kind == AudioKind::Sound).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../web/assets/stock/audio")
            .join(cue.path);
        let buffer = decode(std::fs::read(path).unwrap()).unwrap();
        assert!(buffer.duration() > 0.0);
        assert!(buffer.0.clone().any(|sample| sample.abs() > 0.001));
        assert!(decode(b"not audio".to_vec()).is_err());
    }

    #[test]
    fn native_opus_keeps_the_authored_duration_and_stereo_channels() {
        for bytes in [
            &include_bytes!("../../tests/fixtures/audio/opus-mono.ogg")[..],
            &include_bytes!("../../tests/fixtures/audio/opus-stereo.ogg")[..],
        ] {
            let buffer = decode(bytes.to_vec()).unwrap();
            assert_eq!(buffer.0.channels().get(), 2);
            assert_eq!(buffer.0.sample_rate().get(), 48_000);
            assert_eq!(buffer.0.clone().count(), 6_576 * 2);
            assert!((buffer.duration() - 0.137).abs() < 0.000_001);
            assert!(buffer.0.clone().any(|sample| sample.abs() > 0.05));
        }
    }

    #[test]
    fn native_voice_offset_rate_delay_and_loop_keep_source_timing() {
        let buffer = AudioBuffer(SamplesBuffer::new(
            NonZero::new(1).unwrap(),
            NonZero::new(1000).unwrap(),
            vec![1.0, 0.5, -1.0, -0.5],
        ));
        let options = AudioOptions { volume: 1.0, playback_rate: 2.0, offset_seconds: 0.001 };
        let source = voice_source(&buffer, options, false, 0.003).unwrap();
        assert_eq!(source.sample_rate().get(), 2000);
        assert_eq!(source.collect::<Vec<_>>(), vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, -1.0, -0.5]);
        let source = voice_source(&buffer, AudioOptions::default(), true, 0.0).unwrap();
        assert_eq!(
            source.take(8).collect::<Vec<_>>(),
            vec![1.0, 0.5, -1.0, -0.5, 1.0, 0.5, -1.0, -0.5]
        );
    }
}
