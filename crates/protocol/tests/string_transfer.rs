//! Every data unit, chunk boundary, acknowledgement and rejection comes from Source.

use serde_json::{Value, json};
use solomon_protocol::transfer::*;

fn expand(value: &Value) -> Vec<u16> {
    value
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|run| {
            std::iter::repeat_n(run[0].as_u64().unwrap() as u16, run[1].as_u64().unwrap() as usize)
        })
        .collect()
}

fn runs(data: &[u16]) -> Value {
    let mut runs: Vec<(u16, usize)> = Vec::new();
    for &unit in data {
        if let Some((previous, count)) = runs.last_mut()
            && *previous == unit
        {
            *count += 1;
        } else {
            runs.push((unit, 1));
        }
    }
    json!(runs)
}

fn chunks(data: Vec<GameStringChunk>) -> Value {
    json!(data.into_iter().map(|chunk|json!({"data":runs(&chunk.data),"offset":chunk.offset,"totalLength":chunk.total_length})).collect::<Vec<_>>())
}

#[test]
fn four_chunk_window_and_complete_utf16_values_match_current_source() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/string-transfer-goldens.json"
    ))
    .unwrap();
    for case in fixture["senders"].as_array().unwrap() {
        let sender = GameStringSender::new(expand(&case["data"]));
        if case.get("error").is_some() {
            assert_eq!(sender.err().unwrap(), case["error"].as_str().unwrap());
            continue;
        }
        let mut sender = sender.unwrap();
        let mut results = Vec::new();
        for step in case["steps"].as_array().unwrap() {
            let result = match step["kind"].as_str().unwrap() {
                "start" => json!({"status":null,"chunks":chunks(sender.start())}),
                "close" => {
                    sender.close();
                    json!({"status":null,"chunks":[]})
                }
                "ack" => match sender.acknowledge(step["offset"].as_u64().unwrap() as usize) {
                    Ok(progress) => json!({"status":match progress.acknowledgement {
                        GameStringAcknowledgement::Stale=>"stale",GameStringAcknowledgement::Advanced=>"advanced",GameStringAcknowledgement::Complete=>"complete",
                    },"chunks":chunks(progress.chunks)}),
                    Err(error) => json!({"error":error,"chunks":[]}),
                },
                _ => unreachable!(),
            };
            results.push(result);
        }
        assert_eq!(json!(results), case["results"], "{}", case["name"]);
    }
    for case in fixture["receivers"].as_array().unwrap() {
        let mut receiver = GameStringReceiver::new(
            case["maximumBytes"].as_u64().unwrap() as usize,
            "fixture".into(),
        );
        let mut results = Vec::new();
        for (index, chunk) in case["chunks"].as_array().unwrap().iter().enumerate() {
            if case["closeBefore"][index] == true {
                receiver.close();
            }
            let result = receiver.accept(&GameStringChunk {
                data: expand(&chunk["data"]),
                offset: chunk["offset"].as_u64().unwrap() as usize,
                total_length: chunk["totalLength"].as_u64().unwrap() as usize,
            });
            results.push(match result {
                Ok(value) => json!({"value":value.map(|data|runs(&data))}),
                Err(error) => json!({"error":error}),
            });
        }
        assert_eq!(json!(results), case["results"], "{}", case["name"]);
    }
}
