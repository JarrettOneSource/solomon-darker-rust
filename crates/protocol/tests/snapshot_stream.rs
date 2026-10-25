use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use solomon_protocol::snapshot::{PreparedSnapshot, SnapshotReader, SnapshotWriter};
use solomon_protocol::{HostSnapshot, ServerMessage, SnapshotFrame};
use std::sync::Arc;

fn document(sequence: u64, snapshot: Value) -> Arc<PreparedSnapshot> {
    PreparedSnapshot::new(&SnapshotFrame {
        sequence,
        snapshot,
        host: HostSnapshot::default(),
        flame_presentation: None,
        input_acknowledgements: Default::default(),
    })
    .unwrap()
}

fn canonical(message: ServerMessage<Value>) -> String {
    serde_json::to_value(message).unwrap().to_string()
}

fn delta(base_sequence: u64, sequence: u64, changes: Value) -> String {
    json!({"type":"snapshot_delta", "base_sequence":base_sequence, "sequence":sequence,
        "changes":STANDARD.encode(miniz_oxide::deflate::compress_to_vec_zlib(changes.to_string().as_bytes(),1))}).to_string()
}

#[test]
fn compact_updates_reconstruct_arrays_nulls_unicode_and_exact_numeric_values() {
    let mut writer = SnapshotWriter::default();
    let mut reader = SnapshotReader::default();
    let mut state = json!({"actors":[{"x":0.0,"id":u64::MAX},{"x":1.5,"name":"🧙\n\u{0}"}],
        "nullable":null, "retired":true, "authored": "unchanged scenery definition ".repeat(1000)});
    let variants = [
        state.clone(),
        {
            state["actors"][0]["x"] = json!(-0.0);
            state["nullable"] = json!({"value":null});
            state.clone()
        },
        {
            state["actors"][0]["x"] = json!(0.0);
            state["actors"][0]["id"] = json!(u64::MAX - 1);
            state.as_object_mut().unwrap().remove("retired");
            state["actors"].as_array_mut().unwrap().remove(0);
            state.clone()
        },
        {
            state["actors"].as_array_mut().unwrap().push(json!({"new":[null,[],{}]}));
            state["__proto__"] = json!({"constructor":"ordinary saved text"});
            state.clone()
        },
        {
            state["actors"] = json!([]);
            state["nullable"] = Value::Null;
            state.clone()
        },
    ];
    for (index, state) in variants.into_iter().enumerate() {
        let prepared = document(index as u64 + 1, state);
        let packet = writer.encode(prepared.clone()).unwrap();
        if index > 0 {
            assert_eq!(serde_json::from_str::<Value>(&packet).unwrap()["type"], "snapshot_delta");
            assert!(
                packet.len() < prepared.full_message().len() / 4,
                "unchanged authored state stays off the wire"
            );
        }
        let expected: ServerMessage<Value> = serde_json::from_str(prepared.full_message()).unwrap();
        assert_eq!(
            canonical(reader.decode(&packet).unwrap()),
            canonical(expected),
            "frame {index}, including signed zero and full-width integers"
        );
    }
}

#[test]
fn replaced_publications_controls_and_reconnects_cannot_break_the_written_baseline() {
    let mut writer = SnapshotWriter::default();
    let mut reader = SnapshotReader::default();
    let state = |x| json!({"position":x,"unchanged":"scenery".repeat(500)});
    let first = writer.encode(document(1, state(1))).unwrap();
    reader.decode::<Value>(&first).unwrap();
    let discarded = document(2, state(2));
    drop(discarded);
    let next = writer.encode(document(7, state(3))).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&next).unwrap()["base_sequence"], 1);
    assert!(matches!(
        reader.decode::<Value>("{\"type\":\"pong\",\"nonce\":42}").unwrap(),
        ServerMessage::Pong { nonce: 42 }
    ));
    let ServerMessage::Snapshot { frame } = reader.decode::<Value>(&next).unwrap() else {
        panic!("snapshot")
    };
    assert_eq!(frame.sequence, 7);
    assert_eq!(frame.snapshot["position"], 3);
    let mut reconnected = SnapshotReader::default();
    assert!(reconnected.decode::<Value>(&next).is_err());
    let full = SnapshotWriter::default().encode(document(8, state(4))).unwrap();
    assert!(matches!(reconnected.decode::<Value>(&full).unwrap(), ServerMessage::Snapshot { .. }));
}

#[test]
fn invalid_updates_fail_without_allocating_missing_array_entries_or_reusing_a_broken_baseline() {
    let first = document(1, json!({"actors":[],"unchanged":"scene".repeat(500)}));
    let invalid = [
        delta(0, 2, json!({"sequence":[2]})),
        delta(1, 1, json!({"sequence":[1]})),
        delta(1, 2, json!({"sequence":[3]})),
        delta(1, 2, json!({"sequence":[2],"snapshot":{"actors":[usize::MAX,[]]}})),
        delta(1, 2, json!({"sequence":[2],"snapshot":{"actors":[2,[[1,["missing first item"]]]]}})),
        delta(1, 2, json!({"sequence":[2],"snapshot":{"actors":[1,[[0,[1]],[0,[2]]]]}})),
        delta(1, 2, json!({"sequence":[2],"snapshot":{"missing":null}})),
        delta(1, 2, json!([])),
        delta(1, 2, json!({"sequence":[2],"snapshot":{"actors":[0,[],"extra"]}})),
        delta(1, 2, json!({"sequence":[2],"snapshot":{"actors":[0.5,[]]}})),
        json!({"type":"snapshot_delta","sequence":2,"base_sequence":1,"changes":"not base64"})
            .to_string(),
    ];
    for packet in invalid {
        let mut reader = SnapshotReader::default();
        reader.decode::<Value>(first.full_message()).unwrap();
        assert!(reader.decode::<Value>(&packet).is_err());
        assert!(
            reader.decode::<Value>(&delta(1, 2, json!({"sequence":[2]}))).is_err(),
            "a failed packet retires its baseline"
        );
        assert!(reader.decode::<Value>(first.full_message()).is_ok());
    }
}
