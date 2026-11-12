use super::*;
use crate::transport::{PeerState, TransportConfig};
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../sim/tests/fixtures/website/transfer-senders-goldens.json"
    ))
    .unwrap()
}

fn runs(data: &[u16]) -> Value {
    let mut rows: Vec<(u16, usize)> = Vec::new();
    for &unit in data {
        if let Some((last, count)) = rows.last_mut()
            && *last == unit
        {
            *count += 1;
        } else {
            rows.push((unit, 1));
        }
    }
    json!(rows)
}

fn chunk(chunk: GameStringChunk) -> Value {
    json!({"kind":"chunk","data":runs(&chunk.data),"offset":chunk.offset,"totalLength":chunk.total_length})
}

fn test_peer(config: TransportConfig) -> PeerSender {
    PeerSender(Arc::new(PeerState {
        id: 1,
        closed: false.into(),
        outbox: Default::default(),
        transfers: Default::default(),
        config: Arc::new(config),
        test_hooks: Default::default(),
    }))
}

fn checkpoint_messages(peer: &PeerSender) -> VecDeque<HostedServerMessage> {
    let mut outbox = peer.0.outbox.lock().unwrap();
    outbox.reliable_bytes = 0;
    outbox
        .reliable
        .drain(..)
        .map(|payload| {
            let ServerMessage::<()>::Hosted { message } = serde_json::from_str(&payload).unwrap()
            else {
                panic!("expected a hosted checkpoint message");
            };
            message
        })
        .collect()
}

fn drain(peer: &PeerSender) -> Vec<Value> {
    let mut outbox = peer.0.outbox.lock().unwrap();
    outbox.reliable_bytes = 0;
    outbox
        .reliable
        .drain(..)
        .map(|payload| {
            if payload.starts_with('{') {
                let ServerMessage::<()>::WelcomeChunk { data, offset, total_length } =
                    serde_json::from_str(&payload).unwrap()
                else {
                    panic!("only welcome chunks are emitted in this prefix");
                };
                chunk(GameStringChunk { data, offset, total_length })
            } else {
                json!({"kind":"sideband","payload":payload.as_ref()})
            }
        })
        .collect()
}

#[test]
fn welcome_prefix_orders_sidebands_and_refreshes_only_on_source_progress() {
    for case in fixture()["welcomes"].as_array().unwrap() {
        let peer = test_peer(TransportConfig::default());
        let origin = Instant::now();
        let mut now = origin;
        let mut results = Vec::new();
        for step in case["steps"].as_array().unwrap() {
            let mut events = Vec::new();
            let result = match step["kind"].as_str().unwrap() {
                "start" => peer
                    .send_welcome_at(
                        "w".repeat(case["length"].as_u64().unwrap() as usize).into(),
                        now,
                    )
                    .map_err(|error| error.to_string()),
                "defer" => peer
                    .send_reliable(step["value"].as_str().unwrap().into())
                    .map_err(|error| error.to_string()),
                "ack" => {
                    let was_pending = peer.welcome_pending();
                    let result =
                        peer.acknowledge_welcome_at(step["offset"].as_u64().unwrap() as usize, now);
                    if was_pending && !peer.welcome_pending() {
                        events.push(json!({"kind":"complete","length":case["length"]}));
                    }
                    result
                }
                "close" => {
                    peer.cancel_welcome();
                    Ok(())
                }
                "advance" => {
                    now += Duration::from_millis(step["ms"].as_u64().unwrap());
                    if peer.expire_welcome(now) {
                        events.push(json!({"kind":"timeout"}));
                    }
                    Ok(())
                }
                _ => unreachable!(),
            };
            events.extend(drain(&peer));
            results.push(
                json!({"error":result.err(),"events":events,"pending":peer.welcome_pending()}),
            );
        }
        assert_eq!(json!(results), case["results"], "{}", case["name"]);
    }
}

#[test]
fn prepared_checkpoints_preserve_unicode_at_the_stream_threshold_and_chunk_boundary() {
    let split_surrogate = format!("{}🧙\"\\\n\0{}", "a".repeat(8191), "b".repeat(65_536));
    for save in [String::new(), "🧙".repeat(32_768), split_surrogate] {
        for atomic in [false, true] {
            let peer = test_peer(TransportConfig::default());
            let captured = save.clone();
            let prepared = std::thread::spawn(move || {
                PreparedSaveCheckpoint::new(captured, SaveCheckpointReason::GameOver, 1, atomic)
            })
            .join()
            .unwrap()
            .unwrap();
            assert!(checkpoint_messages(&peer).is_empty());
            peer.publish_prepared_save_checkpoint(prepared).unwrap();
            let mut messages = checkpoint_messages(&peer);
            assert!(!messages.is_empty());
            let streamed = !atomic && save.encode_utf16().count() > 65_536;
            let mut received = Vec::new();
            while let Some(message) = messages.pop_front() {
                match message {
                    HostedServerMessage::SaveCheckpoint { save: actual, reason, sequence } => {
                        assert!(!streamed);
                        assert_eq!(actual, save);
                        assert_eq!(reason, SaveCheckpointReason::GameOver);
                        assert_eq!(sequence, 1);
                    }
                    HostedServerMessage::SaveCheckpointChunk {
                        data,
                        offset,
                        total_length,
                        reason,
                        sequence,
                    } => {
                        assert!(streamed);
                        assert_eq!(offset, received.len());
                        assert_eq!(total_length, save.encode_utf16().count());
                        assert_eq!(reason, SaveCheckpointReason::GameOver);
                        assert_eq!(sequence, 1);
                        received.extend(data);
                        peer.acknowledge_save_checkpoint(sequence, received.len()).unwrap();
                        messages.extend(checkpoint_messages(&peer));
                    }
                    _ => panic!("expected a save checkpoint"),
                }
            }
            if streamed {
                assert_eq!(String::from_utf16(&received).unwrap(), save);
            }
        }
    }
}

#[test]
fn checkpoint_publication_preserves_payload_and_reliable_queue_limits() {
    let peer = test_peer(TransportConfig { max_message_bytes: 256, ..Default::default() });
    let prepared =
        PreparedSaveCheckpoint::new("x".repeat(300), SaveCheckpointReason::Progress, 1, true)
            .unwrap();
    let error = peer.publish_prepared_save_checkpoint(prepared).unwrap_err();
    assert_eq!(error, PeerSendError::PayloadTooLarge.to_string());
    assert!(checkpoint_messages(&peer).is_empty());

    let peer = test_peer(TransportConfig { reliable_byte_capacity: 128, ..Default::default() });
    peer.send_reliable("x".repeat(100).into()).unwrap();
    peer.publish_save_checkpoint("y".repeat(100), SaveCheckpointReason::Progress, 1, true).unwrap();
    assert!(peer.is_closed());
    let outbox = peer.0.outbox.lock().unwrap();
    assert_eq!(outbox.close.as_ref().unwrap().code, Some(1013));
    assert_eq!(outbox.reliable.len(), 1);
    assert_eq!(outbox.reliable.front().unwrap().as_ref(), "x".repeat(100));
}

#[test]
fn prepared_checkpoint_cannot_replace_a_newer_synchronous_save() {
    let peer = test_peer(TransportConfig::default());
    let earlier =
        PreparedSaveCheckpoint::new("old".repeat(30_000), SaveCheckpointReason::Progress, 1, false)
            .unwrap();
    peer.publish_save_checkpoint("new save".into(), SaveCheckpointReason::Progress, 2, true)
        .unwrap();
    let mut messages = checkpoint_messages(&peer);
    assert!(matches!(messages.pop_front(), Some(HostedServerMessage::SaveCheckpoint {
        save, sequence: 2, reason: SaveCheckpointReason::Progress,
    }) if save == "new save"));
    assert!(messages.is_empty());
    assert_eq!(
        peer.publish_prepared_save_checkpoint(earlier).unwrap_err(),
        "Checkpoint publication sequence must advance"
    );
    peer.acknowledge_save_checkpoint(1, 8192).unwrap();
    assert!(checkpoint_messages(&peer).is_empty());
}

#[test]
fn background_saves_coalesce_and_atomic_saves_supersede_source_windows() {
    for case in fixture()["saves"].as_array().unwrap() {
        let mut sender = SaveCheckpointSender::default();
        let mut results = Vec::new();
        for step in case["steps"].as_array().unwrap() {
            let result = match step["kind"].as_str().unwrap() {
                "publish" => sender.publish(
                    PreparedSaveCheckpoint::new(
                        step["char"]
                            .as_str()
                            .unwrap()
                            .repeat(step["length"].as_u64().unwrap() as usize),
                        SaveCheckpointReason::Progress,
                        step["sequence"].as_u64().unwrap(),
                        step["mode"] == "atomic",
                    )
                    .unwrap(),
                ),
                "ack" => sender.acknowledge(
                    step["sequence"].as_u64().unwrap(),
                    step["offset"].as_u64().unwrap() as usize,
                ),
                "close" => {
                    sender.close();
                    Ok(Vec::new())
                }
                _ => unreachable!(),
            };
            let (events,error)=match result {
                Ok(messages)=>(messages.into_iter().map(|payload| {
                    let ServerMessage::<()>::Hosted { message } = serde_json::from_str(&payload).unwrap() else {
                        panic!("expected hosted checkpoint wire payload");
                    };
                    match message {
                    HostedServerMessage::SaveCheckpoint{save,reason,sequence}=>json!({"kind":"complete","save":runs(&save.encode_utf16().collect::<Vec<_>>()),"reason":reason,"sequence":sequence}),
                    HostedServerMessage::SaveCheckpointChunk{data,offset,total_length,reason,sequence}=>{
                        let mut value=chunk(GameStringChunk{data,offset,total_length});
                        value["reason"]=json!(reason);value["sequence"]=json!(sequence);value
                    },
                    _=>unreachable!(),
                }}).collect::<Vec<_>>(),None),
                Err(error)=>(Vec::new(),Some(error)),
            };
            results.push(json!({"error":error,"events":events}));
        }
        assert_eq!(json!(results), case["results"], "{}", case["name"]);
    }
}
