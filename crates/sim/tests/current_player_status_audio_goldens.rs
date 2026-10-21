//! Accepted rescue/charm events reach the existing audio cursor exactly once.
use serde::Deserialize;
use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::audio::combat::{CombatAudioCursor, CombatAudioFrame};
use solomon_sim::audio::{AudioCue, AudioEvent};
use solomon_sim::enemies::model::{EnemyEvent, EnemyEventKind};
use solomon_sim::secondary::NativeSecondarySimulationState;
use solomon_sim::spells::model::SpellState;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "kebab-case")]
enum RecordedAudio {
    Play {
        cue: String,
        #[serde(rename = "playbackRate")]
        playback_rate: f64,
        volume: f64,
    },
    Stop {
        cue: String,
    },
}

fn relevant(events: &[AudioEvent]) -> Vec<RecordedAudio> {
    let mut rows = Vec::new();
    for event in events {
        match event {
            AudioEvent::Play(cue, options)
                if matches!(
                    cue,
                    AudioCue::LoseReverie | AudioCue::FlashSpell | AudioCue::Flash
                ) =>
            {
                rows.push(RecordedAudio::Play {
                    cue: cue.as_str().into(),
                    playback_rate: options.playback_rate,
                    volume: options.volume,
                });
            }
            AudioEvent::StopStream(AudioCue::LoseReverie) => {
                rows.push(RecordedAudio::Stop { cue: "lose-reverie".into() })
            }
            _ => {}
        }
    }
    rows
}
#[test]
fn owner_charm_stream_and_spatial_rescue_program_replay_current_source() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/current-player-status-audio-goldens.json"
    ))
    .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    let primary = SpellState::default();
    let secondary = NativeSecondarySimulationState::new(1);
    for case in fixture["cases"].as_array().unwrap() {
        let gain = case["gain"].as_f64().unwrap();
        let point_gain = |_: Vec2| gain;
        let frame = CombatAudioFrame {
            tick: 0,
            run_id: Some("run"),
            listener_id: "owner",
            players: &[],
            primary: &primary,
            secondary: &secondary,
            staff_contacts: &[],
            enemies: None,
            enemy_events: &[],
            loot_events: &[],
            encounter: None,
            point_gain: &point_gain,
            hit_gain: &point_gain,
        };
        let mut cursor = CombatAudioCursor::new(&frame);
        let mut events = Vec::new();
        for record in case["frames"].as_array().unwrap() {
            let source = &record["event"];
            let kind = match source["type"].as_str().unwrap() {
                "player-charm-lost" => EnemyEventKind::PlayerCharmLost,
                "player-cheat-death" => EnemyEventKind::PlayerCheatDeath,
                other => panic!("unknown status cue {other}"),
            };
            let mut event = EnemyEvent::new(kind, 0, source["tick"].as_u64().unwrap());
            event.event_id = source["eventId"].as_u64().unwrap() as usize;
            event.target_player_id = Some(Some(source["targetPlayerId"].as_str().unwrap().into()));
            event.source_position = Some(Vec2::new(100.0, 100.0));
            events.push(event);
            let current = CombatAudioFrame {
                tick: source["tick"].as_u64().unwrap(),
                enemy_events: &events,
                ..frame
            };
            let expected: Vec<RecordedAudio> =
                serde_json::from_value(record["expected"].clone()).unwrap();
            assert_eq!(relevant(&cursor.update(&current).events), expected);
            assert!(
                relevant(&cursor.update(&current).events).is_empty(),
                "accepted retained event must not replay"
            );
        }
        let destroyed: Vec<RecordedAudio> =
            serde_json::from_value(case["destroy"].clone()).unwrap();
        assert_eq!(relevant(&cursor.destroy().events), destroyed);
        // A newly attached receiver does not replay retained status events.
        let attached = CombatAudioFrame { enemy_events: &events, ..frame };
        let mut attached_cursor = CombatAudioCursor::new(&attached);
        assert!(relevant(&attached_cursor.update(&attached).events).is_empty());
    }
}
