use std::collections::BTreeMap;

use serde::Deserialize;
use solomon_sim::audio::{
    AUDIO_CATALOG, AudioCue, CreateAudioAction, MusicScene, create_entry_audio_events,
    create_selection_audio_events, hub_audio_attenuation, hub_teacher_releases_between,
    hub_teacher_summon_pitch, native_footstep_cue,
};
use solomon_sim::profile::WizardElement;

#[derive(Deserialize)]
struct Fixture {
    entry: Vec<CreateCase>,
    selection: Vec<CreateCase>,
    attenuation: Vec<Attenuation>,
    steps: Vec<Step>,
    teacher: Vec<Teacher>,
    releases: Vec<Releases>,
    music: BTreeMap<String, Music>,
}
#[derive(Deserialize)]
struct CreateCase {
    previous: f64,
    current: f64,
    element: Option<String>,
    expected: Vec<CreateEvent>,
}
#[derive(Deserialize)]
struct CreateEvent {
    action: String,
    cue: String,
}
#[derive(Deserialize)]
struct Attenuation {
    distance: f64,
    expected: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Step {
    player_id: String,
    tick: u32,
    expected: String,
}
#[derive(Deserialize)]
struct Teacher {
    release: u32,
    expected: f64,
}
#[derive(Deserialize)]
struct Releases {
    previous: f64,
    current: f64,
    expected: Vec<u32>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Music {
    cue: String,
    transition_ticks: u32,
}

#[test]
fn stock_create_cues_mixing_hashes_and_music_match_the_retained_source() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/website/audio-goldens.json")).unwrap();
    for case in fixture.entry.into_iter().chain(fixture.selection) {
        let actual = if let Some(element) = case.element {
            let element = WizardElement::ALL
                .into_iter()
                .find(|candidate| candidate.as_str() == element)
                .unwrap();
            create_selection_audio_events(element, case.previous, case.current)
        } else {
            create_entry_audio_events(case.previous, case.current)
        };
        let expected: Vec<_> = case
            .expected
            .into_iter()
            .map(|event| {
                let cue = AudioCue::from_id(&event.cue).unwrap();
                match event.action.as_str() {
                    "play-stream" => CreateAudioAction::PlayStream(cue),
                    "pause-stream" => CreateAudioAction::PauseStream(cue),
                    "play-sound" => CreateAudioAction::PlaySound(cue),
                    action => panic!("unknown source action {action}"),
                }
            })
            .collect();
        assert_eq!(actual, expected);
    }
    for case in fixture.attenuation {
        assert_eq!(hub_audio_attenuation(case.distance), case.expected);
    }
    for case in fixture.steps {
        assert_eq!(native_footstep_cue(case.tick, &case.player_id).as_str(), case.expected);
    }
    for case in fixture.teacher {
        assert_eq!(hub_teacher_summon_pitch(case.release), case.expected);
    }
    for case in fixture.releases {
        assert_eq!(hub_teacher_releases_between(case.previous, case.current), case.expected);
    }
    for (id, scene) in [
        ("title", MusicScene::Title),
        ("create", MusicScene::Create),
        ("hub", MusicScene::Hub),
        ("boneyard", MusicScene::Boneyard),
        ("boneyard-combat", MusicScene::Combat),
        ("game-over", MusicScene::GameOver),
    ] {
        let expected = &fixture.music[id];
        assert_eq!(scene.cue().as_str(), expected.cue);
        assert_eq!(scene.transition_ticks(), expected.transition_ticks);
    }
}

#[test]
fn the_complete_current_audio_catalog_and_voice_limits_match_authored_source_bindings() {
    use solomon_sim::audio::AudioKind;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/website/current-audio-catalog-goldens.json"))
            .unwrap();
    assert_eq!(fixture["sourceRevision"], "fa08ae249ff5563570bdb2a653e2ef9358ce7b83");
    let bindings = fixture["bindings"].as_array().unwrap();
    assert_eq!(AUDIO_CATALOG.len(), bindings.len());
    for (definition, expected) in AUDIO_CATALOG.iter().zip(bindings) {
        assert_eq!(definition.id, expected["id"].as_str().unwrap());
        assert_eq!(definition.path, expected["path"].as_str().unwrap());
        assert_eq!(AudioCue::from_id(definition.id), Some(definition.cue));
        let kind = match definition.kind {
            AudioKind::Sound => "Sound",
            AudioKind::Stream => "Stream",
            AudioKind::Loop => "Loop",
            AudioKind::Music => "Music",
        };
        assert_eq!(kind, expected["kind"].as_str().unwrap());
        assert_eq!(
            definition.cue.maximum_voices().map(|value| value as u64),
            expected["maximumVoices"].as_u64(),
            "{} voice limit",
            definition.id
        );
    }
}
