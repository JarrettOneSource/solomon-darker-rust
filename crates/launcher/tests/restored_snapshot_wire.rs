//! Restored Website scenery must survive the actual tagged native wire envelope.

use std::collections::BTreeMap;
use std::io::Read;

use serde_json::Value;
use solomon_assets::website_save;
use solomon_protocol::snapshot::{PreparedSnapshot, SnapshotReader, SnapshotWriter};
use solomon_protocol::{
    GAME_TICK_RATE, HostSnapshot, PROTOCOL_VERSION, ResumeCredentials, SNAPSHOT_RATE,
    ServerMessage, SnapshotFrame,
};
use solomon_sim::session::{GameCheckpoint, GameSession, GameSnapshot};

#[path = "../../sim/tests/support/mod.rs"]
mod support;

fn frame(session: &GameSession) -> SnapshotFrame<GameSnapshot<'_>> {
    SnapshotFrame {
        sequence: 1,
        flame_presentation: None,
        input_acknowledgements: BTreeMap::new(),
        host: HostSnapshot {
            connected_player_ids: session.participant_order.clone(),
            ..HostSnapshot::default()
        },
        snapshot: session.snapshot(),
    }
}

#[test]
fn restored_current_and_historical_scenery_roundtrips_through_welcome_and_snapshot() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/save-scenery-retention-goldens.json"
    ))
    .unwrap();
    let mut bytes = Vec::new();
    flate2::read::ZlibDecoder::new(
        include_bytes!(
            "../../sim/tests/fixtures/website/current-save-continuation-goldens.json.zlib"
        )
        .as_slice(),
    )
    .read_to_end(&mut bytes)
    .unwrap();
    let current: Value = serde_json::from_slice(&bytes).unwrap();
    let cases = fixture["cases"].as_array().unwrap().iter().chain(
        current["cases"].as_array().unwrap().iter().filter(|case| {
            matches!(
                case["name"].as_str(),
                Some("current-tutorial-effects-true" | "current-boneyard-effects-true")
            )
        }),
    );
    let mut failures = Vec::new();
    let mut nonempty_variant_maps = 0;
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let restored = website_save::restore(&case["document"].to_string())
            .unwrap_or_else(|error| panic!("{name} source restore: {error}"));
        let session = &restored.session;
        let checkpoint_bytes = serde_json::to_vec(&session.snapshot()).unwrap();
        let checkpoint: GameCheckpoint = serde_json::from_slice(&checkpoint_bytes)
            .unwrap_or_else(|error| panic!("{name} direct checkpoint: {error}"));
        checkpoint.validate().unwrap();
        let checkpoint_json: Value = serde_json::from_slice(&checkpoint_bytes).unwrap();
        let durable_checkpoint_json = serde_json::to_value(&checkpoint).unwrap();
        let variants =
            &checkpoint_json["boneyard"]["scenery_target_retention"]["earthquake_variants"];
        let source_variants: serde_json::Map<String, Value> =
            case["document"]["continuation"]["simulation"]["world"]["earthquakeSceneryTargets"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|target| {
                    target.get("secondaryVariant").map(|variant| {
                        (target["id"].as_u64().unwrap().to_string(), variant.clone())
                    })
                })
                .collect();
        assert_eq!(variants, &Value::Object(source_variants), "{name} restored variants");
        nonempty_variant_maps += usize::from(!variants.as_object().unwrap().is_empty());
        let mut messages = vec![
            (
                "welcome",
                ServerMessage::Welcome {
                    version: PROTOCOL_VERSION,
                    credentials: ResumeCredentials {
                        player_id: restored.player_id.clone(),
                        token: "wire-roundtrip-test-only".into(),
                    },
                    tick_rate: GAME_TICK_RATE,
                    snapshot_rate: SNAPSHOT_RATE,
                    hosted: None,
                    frame: frame(session),
                },
            ),
            ("snapshot", ServerMessage::Snapshot { frame: frame(session) }),
        ]
        .into_iter()
        .map(|(kind, message)| (kind, serde_json::to_string(&message).unwrap()))
        .collect::<Vec<_>>();
        let mut writer = SnapshotWriter::default();
        messages.push((
            "full stream frame",
            writer.encode(PreparedSnapshot::new(&frame(session)).unwrap()).unwrap().to_string(),
        ));
        let mut next = frame(session);
        next.sequence = 2;
        messages.push((
            "compact stream frame",
            writer.encode(PreparedSnapshot::new(&next).unwrap()).unwrap().to_string(),
        ));
        let mut reader = SnapshotReader::default();
        for (kind, text) in messages {
            if kind == "full stream frame" {
                reader.clear();
            }
            let decoded = match reader.decode::<GameCheckpoint>(&text) {
                Ok(message) => message,
                Err(error) => {
                    failures.push(format!("{name} {kind}: {error}"));
                    continue;
                }
            };
            let decoded = match decoded {
                ServerMessage::Welcome { frame, .. } | ServerMessage::Snapshot { frame } => frame,
                _ => panic!("{name} changed its {kind} message kind"),
            };
            // Dye sessions travel in snapshots but are omitted when an owned checkpoint
            // is serialized for disk. Check them directly beside the durable projection.
            assert_eq!(
                decoded.snapshot.inventory_dye_sessions, checkpoint.inventory_dye_sessions,
                "{name} {kind} live dye sessions"
            );
            support::assert_website_json(
                &serde_json::to_value(&decoded.snapshot).unwrap(),
                &durable_checkpoint_json,
                &format!("{name} {kind}"),
                &[],
            );
        }
    }
    assert_eq!(
        nonempty_variant_maps, 2,
        "current Source Tutorial and Boneyard are the nonempty witnesses"
    );
    assert!(failures.is_empty(), "restored wire failures: {failures:#?}");
}
