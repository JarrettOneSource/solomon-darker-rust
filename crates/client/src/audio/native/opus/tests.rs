use super::*;
use ogg::{PacketWriteEndInfo, PacketWriter};

const MONO: &[u8] = include_bytes!("../../../../tests/fixtures/audio/opus-mono.ogg");
const STEREO: &[u8] = include_bytes!("../../../../tests/fixtures/audio/opus-stereo.ogg");
const SURROUND: &[u8] = include_bytes!("../../../../tests/fixtures/audio/opus-surround.ogg");
const LONG: &[u8] = include_bytes!("../../../../tests/fixtures/audio/opus-long.ogg");

fn samples(bytes: &[u8]) -> Vec<f32> {
    super::super::decode(bytes.to_vec()).unwrap().0.collect()
}

fn rewrite(bytes: &[u8], mut edit: impl FnMut(&mut Vec<u8>, &mut u64)) -> Vec<u8> {
    let mut reader = PacketReader::new(Cursor::new(bytes));
    let mut writer = PacketWriter::new(Vec::new());
    while let Some(packet) = reader.read_packet().unwrap() {
        let end = if packet.last_in_stream() {
            PacketWriteEndInfo::EndStream
        } else if packet.last_in_page() {
            PacketWriteEndInfo::EndPage
        } else {
            PacketWriteEndInfo::NormalPacket
        };
        let mut granule = packet.absgp_page();
        let serial = packet.stream_serial();
        let mut data = packet.data;
        edit(&mut data, &mut granule);
        writer.write_packet(data, serial, end, granule).unwrap();
    }
    writer.into_inner()
}

#[test]
fn opus_channels_preserve_mono_and_stereo_and_downmix_surround() {
    let mono = samples(MONO);
    assert!(mono.as_chunks::<2>().0.iter().all(|frame| frame[0] == frame[1]));
    for bytes in [STEREO, SURROUND] {
        let stereo = samples(bytes);
        assert_eq!(stereo.len(), 6576 * 2);
        assert!(stereo.as_chunks::<2>().0.iter().any(|frame| (frame[0] - frame[1]).abs() > 0.01));
        assert!(stereo.iter().all(|sample| sample.is_finite()));
    }
    #[derive(serde::Deserialize)]
    struct OracleFrame {
        frame: usize,
        stereo: [f32; 2],
    }
    #[derive(serde::Deserialize)]
    struct Oracle {
        frames: Vec<OracleFrame>,
    }
    let oracle: Oracle = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/audio/opus-surround-oracle.json"
    ))
    .unwrap();
    let actual = samples(SURROUND);
    let differences: Vec<_> = oracle
        .frames
        .into_iter()
        .map(|frame| {
            (
                frame.frame,
                actual[frame.frame * 2] - frame.stereo[0],
                actual[frame.frame * 2 + 1] - frame.stereo[1],
            )
        })
        .collect();
    assert!(
        differences.iter().all(|(_, left, right)| left.abs() < 0.000_01 && right.abs() < 0.000_01),
        "surround frame differences: {differences:?}"
    );
}

#[test]
fn multi_page_music_preserves_duration_and_preskip_across_a_page_boundary() {
    let original = samples(LONG);
    assert_eq!(original.len(), 102_576 * 2);
    let delayed = rewrite(LONG, |data, _| {
        if data.starts_with(b"OpusHead") {
            data[10..12].copy_from_slice(&50_000_u16.to_le_bytes());
        }
    });
    assert_eq!(samples(&delayed), original[(50_000 - 312) * 2..]);
}

#[test]
fn preskip_can_cross_packets_and_header_gain_applies_to_the_whole_song() {
    let original = samples(STEREO);
    let delayed = rewrite(STEREO, |data, _| {
        if data.starts_with(b"OpusHead") {
            data[10..12].copy_from_slice(&2232_u16.to_le_bytes());
        }
    });
    assert_eq!(samples(&delayed), original[1920 * 2..]);
    let louder = rewrite(STEREO, |data, _| {
        if data.starts_with(b"OpusHead") {
            data[16..18].copy_from_slice(&(6_i16 * 256).to_le_bytes());
        }
    });
    let gain = 10_f32.powf(6.0 / 20.0);
    let amplified = samples(&louder);
    assert_eq!(amplified.len(), original.len());
    for (expected, actual) in original.iter().zip(amplified) {
        assert!((expected * gain - actual).abs() < 0.000_01);
    }
}

#[test]
fn chained_channels_and_loops_restart_each_decoders_preskip_and_tail() {
    let bytes = [MONO, STEREO].concat();
    let expected = [samples(MONO), samples(STEREO)].concat();
    assert_eq!(samples(&bytes), expected);
    let error = Arc::new(Mutex::new(None));
    let source = OpusSource::open(bytes.into(), true, error.clone()).unwrap().unwrap();
    let played: Vec<_> = source.take(expected.len() * 2 + 100).collect();
    assert_eq!(&played[..expected.len()], expected);
    assert_eq!(&played[expected.len()..expected.len() * 2], expected);
    assert_eq!(&played[expected.len() * 2..], &expected[..100]);
    assert!(error.lock().unwrap().is_none());
}

#[test]
fn nonzero_initial_granule_does_not_discard_authored_audio() {
    let cropped = rewrite(STEREO, |data, granule| {
        if !data.starts_with(b"Opus") {
            *granule += 10_000;
        }
    });
    // With an initial offset, this one-page file retains all 8 encoded packets;
    // the original EOS granule trims 792 padding frames from the last packet.
    let decoded = samples(&cropped);
    assert_eq!(decoded.len(), (8 * 960 - 312) * 2);
    let original = samples(STEREO);
    assert_eq!(&decoded[..original.len()], original);
}

#[test]
fn malformed_headers_crc_truncation_and_late_decode_errors_are_reported() {
    let invalid_mapping = rewrite(STEREO, |data, _| {
        if data.starts_with(b"OpusHead") {
            data[18] = 1;
            data.extend_from_slice(&[1, 1, 0, 3]);
        }
    });
    let excessive_skip = rewrite(STEREO, |data, _| {
        if data.starts_with(b"OpusHead") {
            data[10..12].copy_from_slice(&20_000_u16.to_le_bytes());
        }
    });
    let mut corrupt = STEREO.to_vec();
    *corrupt.last_mut().unwrap() ^= 0x40;
    for invalid in [invalid_mapping, excessive_skip, corrupt.clone(), STEREO[..100].to_vec()] {
        assert!(super::super::decode(invalid).is_err());
    }
    let late_error = [MONO, &corrupt].concat();
    let error = Arc::new(Mutex::new(None));
    let source = OpusSource::open(late_error.clone().into(), true, error.clone()).unwrap().unwrap();
    assert_eq!(source.collect::<Vec<_>>(), samples(MONO));
    assert!(error.lock().unwrap().is_some());
    assert!(super::super::decode(late_error).is_err());
}
