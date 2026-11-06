//! Snapshots send a Weld build as the original game's build number, and a client rejects a
//! snapshot with any other number when it decodes the packet.

use serde_json::{Value, json};
use solomon_protocol::snapshot::{PreparedSnapshot, SnapshotReader};
use solomon_protocol::{HostSnapshot, ServerMessage, SnapshotFrame};
use solomon_sim::profile::{NewGameSelection, WeldBuildId, WizardDiscipline, WizardElement};
use solomon_sim::session::{GameCheckpoint, GameSession, GameSessionOptions};

const WELD_BUILD: &str = "/frame/snapshot/participants/wizard/player/book/weld_build_id";

fn welded_packet() -> String {
    let mut session = GameSession::new(
        "wizard".into(),
        NewGameSelection {
            name: "Welder".into(),
            element: WizardElement::Water,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions::default(),
    );
    let participant = session.participants.get_mut("wizard").unwrap();
    participant.player.grant_weld_build(WeldBuildId::Hailstones, &mut session.gameplay_rng);
    let prepared = PreparedSnapshot::new(&SnapshotFrame {
        sequence: 1,
        snapshot: session.snapshot(),
        host: HostSnapshot::default(),
        flame_presentation: None,
        input_acknowledgements: Default::default(),
    })
    .unwrap();
    prepared.full_message().to_string()
}

#[test]
fn a_snapshot_sends_the_weld_build_number() {
    let packet = welded_packet();
    let message: Value = serde_json::from_str(&packet).unwrap();
    assert_eq!(message.pointer(WELD_BUILD), Some(&json!(1008)));
    let ServerMessage::Snapshot { frame } =
        SnapshotReader::default().decode::<GameCheckpoint>(&packet).unwrap()
    else {
        panic!("a full snapshot decodes as a snapshot")
    };
    let book = &frame.snapshot.participants["wizard"].player.book;
    assert_eq!(book.weld_build_id, Some(WeldBuildId::Hailstones));
}

#[test]
fn a_snapshot_with_an_unknown_weld_build_number_is_rejected() {
    let mut message: Value = serde_json::from_str(&welded_packet()).unwrap();
    *message.pointer_mut(WELD_BUILD).unwrap() = json!(1010);
    let error =
        SnapshotReader::default().decode::<GameCheckpoint>(&message.to_string()).unwrap_err();
    assert!(error.contains("unknown Weld build ID 1010"), "{error}");
}
