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
        let peer = PeerSender(Arc::new(PeerState {
            id: 1,
            closed: false.into(),
            outbox: Default::default(),
            transfers: Default::default(),
            config: Arc::new(TransportConfig::default()),
        }));
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
fn background_saves_coalesce_and_atomic_saves_supersede_source_windows() {
    for case in fixture()["saves"].as_array().unwrap() {
        let mut sender = SaveCheckpointSender::default();
        let mut results = Vec::new();
        for step in case["steps"].as_array().unwrap() {
            let result = match step["kind"].as_str().unwrap() {
                "publish" => sender.publish(
                    Checkpoint {
                        save: step["char"]
                            .as_str()
                            .unwrap()
                            .repeat(step["length"].as_u64().unwrap() as usize),
                        sequence: step["sequence"].as_u64().unwrap(),
                        reason: SaveCheckpointReason::Progress,
                    },
                    step["mode"] == "atomic",
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
                Ok(messages)=>(messages.into_iter().map(|message|match message {
                    HostedServerMessage::SaveCheckpoint{save,reason,sequence}=>json!({"kind":"complete","save":runs(&save.encode_utf16().collect::<Vec<_>>()),"reason":reason,"sequence":sequence}),
                    HostedServerMessage::SaveCheckpointChunk{data,offset,total_length,reason,sequence}=>{
                        let mut value=chunk(GameStringChunk{data,offset,total_length});
                        value["reason"]=json!(reason);value["sequence"]=json!(sequence);value
                    },
                    _=>unreachable!(),
                }).collect::<Vec<_>>(),None),
                Err(error)=>(Vec::new(),Some(error)),
            };
            results.push(json!({"error":error,"events":events}));
        }
        assert_eq!(json!(results), case["results"], "{}", case["name"]);
    }
}
