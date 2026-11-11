use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use solomon_protocol::snapshot::{PreparedSnapshot, SnapshotReader, SnapshotWriter};
use solomon_protocol::{HostSnapshot, ServerMessage, SnapshotFrame};
use std::sync::Arc;

fn document(sequence: u64, snapshot: Value) -> Arc<PreparedSnapshot> {
    PreparedSnapshot::new(SnapshotFrame {
        sequence,
        snapshot,
        host: HostSnapshot::default(),
        flame_presentation: None,
        input_acknowledgements: Default::default(),
    })
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
            state["field\n\"\\🧙"] = json!({"nested\tname": 1});
            state.clone()
        },
        {
            state["actors"][0]["x"] = json!(0.0);
            state["actors"][0]["id"] = json!(u64::MAX - 1);
            state.as_object_mut().unwrap().remove("retired");
            state["field\n\"\\🧙"]["nested\tname"] = json!(2);
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
            state.as_object_mut().unwrap().remove("field\n\"\\🧙");
            state.clone()
        },
    ];
    for (index, state) in variants.into_iter().enumerate() {
        let prepared = document(index as u64 + 1, state);
        let packet = writer.encode(prepared.clone()).unwrap();
        if index > 0 {
            assert_eq!(serde_json::from_str::<Value>(&packet).unwrap()["type"], "snapshot_delta");
            assert!(
                packet.len() < prepared.full_message().unwrap().len() / 4,
                "unchanged authored state stays off the wire"
            );
        }
        let expected: ServerMessage<Value> =
            serde_json::from_str(prepared.full_message().unwrap()).unwrap();
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
fn shared_publications_keep_each_socket_writers_selected_baseline() {
    let state = |x| json!({"position":x,"unchanged":"scenery".repeat(500)});
    let first = document(1, state(1));
    let second = document(2, state(2));
    let final_frame = document(7, state(3));
    std::thread::scope(|scope| {
        for baseline in [first, second] {
            let final_frame = final_frame.clone();
            scope.spawn(move || {
                let mut writer = SnapshotWriter::default();
                let mut reader = SnapshotReader::default();
                let first_packet = writer.encode(baseline).unwrap();
                let ServerMessage::Snapshot { frame: first_frame } =
                    reader.decode::<Value>(&first_packet).unwrap()
                else {
                    panic!("full initial snapshot")
                };
                let packet = writer.encode(final_frame.clone()).unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(&packet).unwrap()["base_sequence"],
                    first_frame.sequence
                );
                assert_eq!(
                    canonical(reader.decode(&packet).unwrap()),
                    canonical(serde_json::from_str(final_frame.full_message().unwrap()).unwrap())
                );
            });
        }
    });
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
        delta(1, 2, json!({"sequence":[2],"snapshot":{"missing":{}}})),
        delta(1, 2, json!([])),
        delta(1, 2, json!({"sequence":[2],"snapshot":{"actors":[0,[],"extra"]}})),
        delta(1, 2, json!({"sequence":[2],"snapshot":{"actors":[0.5,[]]}})),
        json!({"type":"snapshot_delta","sequence":2,"base_sequence":1,"changes":"not base64"})
            .to_string(),
        json!({"type":"snapshot_delta","sequence":2,"base_sequence":1,
            "changes":STANDARD.encode(miniz_oxide::deflate::compress_to_vec_zlib(
                b"{\"sequence\":[2],\"snapshot\":{\"name\":[\"\xff\"]}}", 1))})
        .to_string(),
    ];
    for packet in invalid {
        let mut reader = SnapshotReader::default();
        reader.decode::<Value>(first.full_message().unwrap()).unwrap();
        assert!(reader.decode::<Value>(&packet).is_err());
        assert!(
            reader.decode::<Value>(&delta(1, 2, json!({"sequence":[2]}))).is_err(),
            "a failed packet retires its baseline"
        );
        assert!(reader.decode::<Value>(first.full_message().unwrap()).is_ok());
    }
}

#[test]
fn retained_items_preserve_order_exact_values_and_following_edits() {
    let first = document(
        1,
        json!({"actors":[
        {"id":u64::MAX,"x":-0.0,"name":"🧙\n"},
        {"id":8,"x":1.0}, {"id":9,"x":2.0}
    ],"label":"old"}),
    );
    let mut reader = SnapshotReader::default();
    reader.decode::<Value>(first.full_message().unwrap()).unwrap();
    let packet = delta(
        1,
        2,
        json!({"sequence":[2],"snapshot":{
            "actors":["r",[[2,{"x":[3.0]}],[0,null],[null,[{"id":10,"x":0.0}]]]],
            "label":["r"]
        }}),
    );
    let expected = document(
        2,
        json!({"actors":[
        {"id":9,"x":3.0}, {"id":u64::MAX,"x":-0.0,"name":"🧙\n"}, {"id":10,"x":0.0}
    ],"label":"r"}),
    );
    assert_eq!(
        canonical(reader.decode(&packet).unwrap()),
        canonical(serde_json::from_str(expected.full_message().unwrap()).unwrap())
    );
    let last = delta(2, 3, json!({"sequence":[3],"snapshot":{"actors":["r",[]]}}));
    let ServerMessage::Snapshot { frame } = reader.decode::<Value>(&last).unwrap() else {
        panic!("snapshot")
    };
    assert_eq!(frame.snapshot, json!({"actors":[],"label":"r"}));
}

#[test]
fn retiring_and_reordering_records_does_not_resend_their_unchanged_contents() {
    let mut seed = 1_u64;
    let actors: Vec<_> = (0..64)
        .map(|id| {
            let text: String = (0..512)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    char::from(b'!' + ((seed >> 32) % 90) as u8)
                })
                .collect();
            json!({"id":id,"x":id,"authored":text})
        })
        .collect();
    let mut next = actors[1..].to_vec();
    next.reverse();
    next[0]["x"] = json!(-0.0);
    let mut writer = SnapshotWriter::default();
    let mut reader = SnapshotReader::default();
    reader.decode::<Value>(&writer.encode(document(1, json!({"actors":actors}))).unwrap()).unwrap();
    let final_frame = document(2, json!({"actors":next}));
    let packet = writer.encode(final_frame.clone()).unwrap();
    assert!(
        packet.len() < final_frame.full_message().unwrap().len() / 20,
        "retired records must not force surviving authored contents back onto the wire"
    );
    assert_eq!(
        canonical(reader.decode(&packet).unwrap()),
        canonical(serde_json::from_str(final_frame.full_message().unwrap()).unwrap())
    );
}

#[test]
fn invalid_retained_references_retire_the_connection_baseline() {
    let first = document(1, json!({"actors":[{"id":7},{"id":8}]}));
    for change in [
        json!(["r", [[0, null], [0, null]]]),
        json!(["r", [[2, null]]]),
        json!(["r", [[usize::MAX, null]]]),
        json!(["r", [[-1, null]]]),
        json!(["r", [[0.5, null]]]),
        json!(["r", [[null, null]]]),
        json!(["r",[[null,{"id":[9]}]]]),
        json!(["r", [[0]]]),
        json!(["r", [[0, null, "extra"]]]),
        json!(["r", [], "extra"]),
        json!(["unknown", []]),
    ] {
        let mut reader = SnapshotReader::default();
        reader.decode::<Value>(first.full_message().unwrap()).unwrap();
        let packet = delta(1, 2, json!({"sequence":[2],"snapshot":{"actors":change}}));
        assert!(reader.decode::<Value>(&packet).is_err(), "reject {change}");
        assert!(
            reader.decode::<Value>(&delta(1, 2, json!({"sequence":[2]}))).is_err(),
            "a failed retained update clears the whole baseline"
        );
        assert!(reader.decode::<Value>(first.full_message().unwrap()).is_ok());
    }
}

#[test]
fn replaced_publications_do_not_serialize_and_selected_peers_share_one_encoding() {
    use std::sync::{Arc, Mutex};

    struct Witness {
        serializations: Arc<Mutex<Vec<std::thread::ThreadId>>>,
        value: u64,
    }
    impl serde::Serialize for Witness {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            self.serializations.lock().unwrap().push(std::thread::current().id());
            self.value.serialize(serializer)
        }
    }
    let serializations = Arc::new(Mutex::new(Vec::new()));
    let publication = |sequence| {
        PreparedSnapshot::new(SnapshotFrame {
            sequence,
            flame_presentation: None,
            input_acknowledgements: Default::default(),
            host: Default::default(),
            snapshot: Witness { serializations: serializations.clone(), value: sequence },
        })
    };
    let obsolete = publication(1);
    let selected = publication(2);
    drop(obsolete);
    assert!(serializations.lock().unwrap().is_empty(), "publication must not format on authority");
    let (writer_id, first) = std::thread::spawn(move || {
        let first = SnapshotWriter::default().encode(selected.clone()).unwrap();
        let second = SnapshotWriter::default().encode(selected).unwrap();
        assert!(Arc::ptr_eq(&first, &second), "peers share the completed full packet");
        (std::thread::current().id(), first)
    })
    .join()
    .unwrap();
    assert_eq!(*serializations.lock().unwrap(), [writer_id]);
    let decoded: ServerMessage<u64> = serde_json::from_str(&first).unwrap();
    let ServerMessage::Snapshot { frame } = decoded else { panic!("snapshot") };
    assert_eq!(frame.sequence, 2);
    assert_eq!(frame.snapshot, 2);
}

#[test]
fn a_failed_deferred_encoding_is_shared_without_advancing_the_writer_baseline() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Rejected(Arc<AtomicUsize>);
    impl serde::Serialize for Rejected {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(serde::ser::Error::custom("rejected network state"))
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let rejected = PreparedSnapshot::new(SnapshotFrame {
        sequence: 2,
        snapshot: Rejected(calls.clone()),
        host: HostSnapshot::default(),
        flame_presentation: None,
        input_acknowledgements: Default::default(),
    });
    let mut writer = SnapshotWriter::default();
    let mut reader = SnapshotReader::default();
    reader.decode::<Value>(&writer.encode(document(1, json!({"tick":1}))).unwrap()).unwrap();
    for _ in 0..2 {
        assert_eq!(writer.encode(rejected.clone()).unwrap_err(), "rejected network state");
    }
    assert_eq!(
        rejected.full_message().unwrap_err(),
        solomon_protocol::snapshot::SnapshotEncodingError::Serialization(
            "rejected network state".to_owned()
        )
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let packet = writer.encode(document(2, json!({"tick":2}))).unwrap();
    let ServerMessage::Snapshot { frame } = reader.decode::<Value>(&packet).unwrap() else {
        panic!("complete state after rejected encoding")
    };
    assert_eq!(frame.sequence, 2);
    assert_eq!(frame.snapshot, json!({"tick":2}));
}

#[test]
fn deferred_publication_retains_the_full_protocol_byte_bound() {
    let oversized = PreparedSnapshot::new(SnapshotFrame {
        sequence: 1,
        snapshot: "x".repeat(solomon_protocol::GAME_WEBSOCKET_MAX_PAYLOAD_BYTES),
        host: HostSnapshot::default(),
        flame_presentation: None,
        input_acknowledgements: Default::default(),
    });
    assert_eq!(
        oversized.full_message().unwrap_err(),
        solomon_protocol::snapshot::SnapshotEncodingError::PayloadTooLarge
    );
    assert!(SnapshotWriter::default().encode(oversized).is_err());
}
