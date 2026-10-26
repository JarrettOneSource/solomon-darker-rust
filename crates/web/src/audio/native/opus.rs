//! Ogg Opus streaming, pre-skip, gain and end trimming (RFC 7845 sections 4–5).
//! `ogg` owns the container and `opus-codec` owns the bundled libopus decoder.

use std::collections::VecDeque;
use std::io::Cursor;
use std::num::NonZero;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ogg::{Packet, PacketReader};
use opus_codec::{MAX_FRAME_SAMPLES_48KHZ, Mapping, MultistreamDecoder, SampleRate};
use rodio::Source;

type Reader = PacketReader<Cursor<Arc<[u8]>>>;

pub(super) struct OpusSource {
    bytes: Arc<[u8]>,
    reader: Reader,
    link: Link,
    stereo: Vec<f32>,
    position: usize,
    looping: bool,
    finished: bool,
    played_cycle: bool,
    error: Arc<Mutex<Option<String>>>,
}

impl OpusSource {
    pub fn open(
        bytes: Arc<[u8]>,
        looping: bool,
        error: Arc<Mutex<Option<String>>>,
    ) -> Result<Option<Self>, String> {
        if !bytes.starts_with(b"OggS") {
            return Ok(None);
        }
        let mut reader = Reader::new(Cursor::new(bytes.clone()));
        let header = read_packet(&mut reader)?.ok_or("Ogg audio contains no packets")?;
        if !header.data.starts_with(b"OpusHead") {
            return Ok(None);
        }
        let link = Link::new(header, &mut reader)?;
        let mut source = Self {
            bytes,
            reader,
            link,
            stereo: Vec::with_capacity(MAX_FRAME_SAMPLES_48KHZ * 2),
            position: 0,
            looping,
            finished: false,
            played_cycle: false,
            error,
        };
        if !source.fill()? {
            return Err("Opus audio contains no samples".into());
        }
        Ok(Some(source))
    }

    fn fill(&mut self) -> Result<bool, String> {
        self.stereo.clear();
        self.position = 0;
        loop {
            if self.link.packets.is_empty() {
                if self.link.ended {
                    if self.link.skip != 0 {
                        return Err("Opus audio ends before its pre-skip".into());
                    }
                    let header = match read_packet(&mut self.reader)? {
                        Some(header) => header,
                        None if self.looping && self.played_cycle => {
                            self.reader = Reader::new(Cursor::new(self.bytes.clone()));
                            self.played_cycle = false;
                            read_packet(&mut self.reader)?.ok_or("Opus loop has no header")?
                        }
                        None => return Ok(false),
                    };
                    self.link = Link::new(header, &mut self.reader)?;
                }
                self.link.read_page(&mut self.reader)?;
            }
            let packet = self.link.packets.pop_front().expect("loaded Opus page");
            let frames = self
                .link
                .decoder
                .decode_float(&packet.data, &mut self.link.pcm, MAX_FRAME_SAMPLES_48KHZ, false)
                .map_err(|error| format!("Decode Opus packet: {error}"))?;
            let kept = frames.min(self.link.keep);
            self.link.keep -= kept;
            let skipped = kept.min(self.link.skip);
            self.link.skip -= skipped;
            let channels = self.link.channels;
            for frame in self.link.pcm[skipped * channels..kept * channels].chunks_exact(channels) {
                self.stereo.extend_from_slice(&downmix(frame));
            }
            if !self.stereo.is_empty() {
                self.played_cycle = true;
                return Ok(true);
            }
        }
    }
}

impl Iterator for OpusSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.finished {
            return None;
        }
        if self.position == self.stereo.len() {
            match self.fill() {
                Ok(true) => {}
                result => {
                    self.finished = true;
                    if let Err(error) = result {
                        *self.error.lock().expect("audio error lock") = Some(error);
                    }
                    return None;
                }
            }
        }
        let sample = self.stereo[self.position];
        self.position += 1;
        Some(sample)
    }
}

impl Source for OpusSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> rodio::ChannelCount {
        // A chained Ogg file may change channel count at a link boundary.
        NonZero::new(2).unwrap()
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        NonZero::new(48_000).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

struct Link {
    serial: u32,
    channels: usize,
    decoder: MultistreamDecoder,
    pcm: Vec<f32>,
    packets: VecDeque<Packet>,
    granule: Option<u64>,
    skip: usize,
    keep: usize,
    ended: bool,
}

impl Link {
    fn new(header: Packet, reader: &mut Reader) -> Result<Self, String> {
        let data = &header.data;
        if !header.first_in_stream()
            || !header.last_in_page()
            || header.last_in_stream()
            || data.len() < 19
            || !data.starts_with(b"OpusHead")
            || data[8] > 15
            || !(1..=8).contains(&data[9])
        {
            return Err("Invalid or unsupported Opus identification header".into());
        }
        let channels = usize::from(data[9]);
        let mapping = match data[18] {
            0 if channels <= 2 => Mapping {
                channels: data[9],
                streams: 1,
                coupled_streams: data[9] - 1,
                mapping: &[0, 1][..channels],
            },
            1 if data.len() >= 21 + channels => Mapping {
                channels: data[9],
                streams: data[19],
                coupled_streams: data[20],
                mapping: &data[21..21 + channels],
            },
            _ => return Err("Unsupported Opus channel mapping".into()),
        };
        let mut decoder = MultistreamDecoder::new(SampleRate::Hz48000, mapping)
            .map_err(|error| format!("Open Opus decoder: {error}"))?;
        decoder
            .set_gain(i32::from(i16::from_le_bytes([data[16], data[17]])))
            .map_err(|error| format!("Set Opus header gain: {error}"))?;
        let tags = read_packet(reader)?.ok_or("Missing Opus comment header")?;
        if tags.stream_serial() != header.stream_serial()
            || tags.first_in_stream()
            || tags.last_in_stream()
            || !tags.last_in_page()
            || !valid_tags(&tags.data)
        {
            return Err("Invalid Opus comment header".into());
        }
        Ok(Self {
            serial: header.stream_serial(),
            channels,
            decoder,
            pcm: vec![0.0; MAX_FRAME_SAMPLES_48KHZ * channels],
            packets: VecDeque::new(),
            granule: None,
            skip: usize::from(u16::from_le_bytes([data[10], data[11]])),
            keep: 0,
            ended: false,
        })
    }

    fn read_page(&mut self, reader: &mut Reader) -> Result<(), String> {
        // Only encoded packets from one page are retained. Its final granule is
        // known before emitting PCM, including a first audio page that is EOS.
        let mut frames = 0_u64;
        let last = loop {
            let packet = read_packet(reader)?.ok_or("Opus audio ends without a final page")?;
            if packet.stream_serial() != self.serial || packet.first_in_stream() {
                return Err("Unexpected Ogg stream inside Opus audio".into());
            }
            frames += opus_codec::packet_sample_count(&packet.data, SampleRate::Hz48000)
                .map_err(|error| format!("Read Opus packet duration: {error}"))?
                as u64;
            let last = packet.last_in_page();
            self.packets.push_back(packet);
            if last {
                break self.packets.back().expect("loaded Opus packet");
            }
        };
        let granule = last.absgp_page();
        self.ended = last.last_in_stream();
        if granule == u64::MAX {
            return Err("Opus audio page has no granule position".into());
        }
        let previous = self.granule.unwrap_or_else(|| granule.saturating_sub(frames));
        let kept = granule.checked_sub(previous).ok_or("Opus granule position moved backwards")?;
        if kept > frames || (!self.ended && kept != frames) {
            return Err("Opus granule position does not match its audio packets".into());
        }
        self.keep = usize::try_from(kept).map_err(|_| "Opus audio page is too long")?;
        self.granule = Some(granule);
        Ok(())
    }
}

fn read_packet(reader: &mut Reader) -> Result<Option<Packet>, String> {
    reader.read_packet().map_err(|error| format!("Read Ogg audio: {error}"))
}

fn valid_tags(mut data: &[u8]) -> bool {
    fn length(data: &mut &[u8]) -> Option<usize> {
        let (bytes, rest) = data.split_at_checked(4)?;
        *data = rest;
        Some(u32::from_le_bytes(bytes.try_into().ok()?) as usize)
    }
    let Some(rest) = data.strip_prefix(b"OpusTags") else { return false };
    data = rest;
    let Some(vendor) = length(&mut data).and_then(|len| data.get(len..)) else { return false };
    data = vendor;
    let Some(count) = length(&mut data) else { return false };
    for _ in 0..count {
        let Some(rest) = length(&mut data).and_then(|len| data.get(len..)) else { return false };
        data = rest;
    }
    true
}

// Xiph libopusfile's OP_STEREO_DOWNMIX, in Vorbis channel order. Its license is
// retained in docs/third-party/opusfile-LICENSE. Mono is duplicated, stereo is
// unchanged; larger layouts retain center, rear and LFE contributions.
const DOWNMIX: [&[[f32; 2]]; 6] = [
    &[[0.5858, 0.0], [0.4142, 0.4142], [0.0, 0.5858]],
    &[[0.4226, 0.0], [0.0, 0.4226], [0.366, 0.2114], [0.2114, 0.336]],
    &[[0.651, 0.0], [0.46, 0.46], [0.0, 0.651], [0.5636, 0.3254], [0.3254, 0.5636]],
    &[
        [0.529, 0.0],
        [0.3741, 0.3741],
        [0.0, 0.529],
        [0.4582, 0.2645],
        [0.2645, 0.4582],
        [0.3741, 0.3741],
    ],
    &[
        [0.4553, 0.0],
        [0.322, 0.322],
        [0.0, 0.4553],
        [0.3943, 0.2277],
        [0.2277, 0.3943],
        [0.2788, 0.2788],
        [0.322, 0.322],
    ],
    &[
        [0.3886, 0.0],
        [0.2748, 0.2748],
        [0.0, 0.3886],
        [0.3366, 0.1943],
        [0.1943, 0.3366],
        [0.3366, 0.1943],
        [0.1943, 0.3366],
        [0.2748, 0.2748],
    ],
];

fn downmix(frame: &[f32]) -> [f32; 2] {
    match frame {
        [mono] => [*mono, *mono],
        [left, right] => [*left, *right],
        _ => frame.iter().zip(DOWNMIX[frame.len() - 3]).fold([0.0; 2], |sum, (sample, gain)| {
            [sum[0] + sample * gain[0], sum[1] + sample * gain[1]]
        }),
    }
}

#[cfg(test)]
mod tests;
