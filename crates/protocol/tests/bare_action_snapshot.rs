use solomon_protocol::snapshot::{PreparedSnapshot, SnapshotReader, SnapshotWriter};
use solomon_protocol::{HostSnapshot, ServerMessage, SnapshotFrame};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};
use solomon_sim::secondary::{
    SecondaryCastAction, SecondaryPlayerState, step_secondary_cast_action,
};
use solomon_sim::session::{GameCheckpoint, GameSession, GameSessionOptions};
use solomon_sim::spells::hand_cast::HandCast1;

#[test]
fn borrowed_snapshots_delta_and_reconnect_preserve_bare_actions_and_rng() {
    let mut session = GameSession::new(
        "wizard".into(),
        NewGameSelection {
            name: "Bare snapshot".into(),
            element: WizardElement::Ether,
            discipline: WizardDiscipline::Arcane,
        },
        GameSessionOptions { combat_rng_seed: 125, ..Default::default() },
    );
    let mut hand = HandCast1::new(0.095_f32 as f64, 0, &mut session.combat_rng);
    hand.step(0.5);
    {
        let participant = session.participants.get_mut("wizard").unwrap();
        participant.wizard.primary_cast.hand_cast = Some(hand);
        participant.wizard.primary_cast.action_tick = Some(1.0);
    }
    let secondary = SecondaryPlayerState {
        cast_action: step_secondary_cast_action(Some(SecondaryCastAction::new(None)), 0.0, 0.5)
            .action,
        ..SecondaryPlayerState::default()
    };
    session.secondary.players.insert("wizard".into(), secondary);
    let mut writer = SnapshotWriter::default();
    let mut reader = SnapshotReader::default();
    let mut packets = Vec::new();
    for sequence in 1..=2 {
        if sequence == 2 {
            let participant = session.participants.get_mut("wizard").unwrap();
            participant.wizard.primary_cast.hand_cast.as_mut().unwrap().step(0.5);
        }
        let prepared = PreparedSnapshot::new(&SnapshotFrame {
            sequence,
            snapshot: session.snapshot(),
            host: HostSnapshot::default(),
            flame_presentation: None,
            input_acknowledgements: Default::default(),
        })
        .unwrap();
        let packet = writer.encode(prepared).unwrap();
        if sequence == 2 {
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&packet).unwrap()["type"],
                "snapshot_delta"
            );
        }
        let ServerMessage::Snapshot { frame } = reader.decode::<GameCheckpoint>(&packet).unwrap()
        else {
            panic!("snapshot")
        };
        let restored = GameSession::restore(frame.snapshot, None).unwrap();
        assert_eq!(restored.combat_rng, session.combat_rng);
        assert_eq!(
            restored.wizard("wizard").unwrap().primary_cast,
            session.wizard("wizard").unwrap().primary_cast
        );
        assert_eq!(
            restored.secondary.players.get("wizard").unwrap().cast_action,
            session.secondary.players.get("wizard").unwrap().cast_action
        );
        packets.push(packet);
    }
    let mut reconnected = SnapshotReader::default();
    assert!(reconnected.decode::<GameCheckpoint>(&packets[1]).is_err());
    let full = PreparedSnapshot::new(&SnapshotFrame {
        sequence: 3,
        snapshot: session.snapshot(),
        host: HostSnapshot::default(),
        flame_presentation: None,
        input_acknowledgements: Default::default(),
    })
    .unwrap();
    let packet = SnapshotWriter::default().encode(full).unwrap();
    let ServerMessage::Snapshot { frame } = reconnected.decode::<GameCheckpoint>(&packet).unwrap()
    else {
        panic!("full reconnect snapshot")
    };
    assert_eq!(
        frame.snapshot.participants["wizard"].wizard.primary_cast,
        session.wizard("wizard").unwrap().primary_cast
    );
    assert_eq!(frame.snapshot.combat_rng, session.combat_rng);
}
