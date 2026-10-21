//! Current Source run archives must consume the actual authority's Boneyard ticks.

use std::collections::BTreeMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use solomon_launcher::host::run_archive::{RunArchiveEndReason, read_run_archive};
use solomon_sim::Vec2;
use solomon_sim::session::GameSessionInput;

use super::*;

#[test]
fn actual_shared_and_private_run_ticks_are_archived_before_supervisor_shutdown_completes() {
    let directory = std::env::temp_dir().join(format!(
        "solomon-hosted-archives-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    let mut host = Host::start_with_storage(
        &"a".repeat(40),
        false,
        None,
        Some(directory.join("memoratorium.json")),
    );
    let mut clients = Vec::new();
    let mut runs = BTreeMap::new();
    for (route, name) in [
        ("/admin/hub/tickets", "Shared archive wizard"),
        ("/admin/sessions", "Private archive wizard"),
    ] {
        let (status, endpoint) = host.request("POST", route, content(), true);
        assert_eq!(status, 201, "{route}: {endpoint}");
        let (mut client, id, _) = Client::join(&host, &endpoint, name);
        // Leave one actual idle College frame in the normal unmatched queue.
        // Two observations avoid assuming how many ticks a snapshot spans.
        let saw_idle_hub = std::cell::Cell::new(false);
        client.until(|message| {
            matches!(message, ServerMessage::Snapshot { frame }
                if frame.snapshot["boneyard"].is_null()
                    && frame.snapshot["ticks"].as_u64().unwrap() >= 25)
                && saw_idle_hub.replace(true)
        });
        client.send(ClientMessage::Hosted {
            command: HostedCommand::StartMatch { boneyard_id: "default-random".into() },
        });
        let ServerMessage::BoneyardLoaded { boneyard } =
            client.until(|message| matches!(message, ServerMessage::BoneyardLoaded { .. }))
        else {
            unreachable!()
        };
        let ServerMessage::Snapshot { frame } = client.until(|message| {
            matches!(message, ServerMessage::Snapshot { frame }
                if frame.host.resume_grace.is_some())
        }) else {
            unreachable!()
        };
        client.send(ClientMessage::ReadyBoneyard { run_id: boneyard.run_id.clone() });
        client.send(ClientMessage::ResumeReady {
            sequence: frame.host.resume_grace.unwrap().sequence,
        });
        let ServerMessage::Snapshot { frame } = client.until(|message| {
            matches!(message, ServerMessage::Snapshot { frame }
                if frame.snapshot["boneyard"]["run_id"].as_str() == Some(boneyard.run_id.as_str())
                    && frame.host.resume_grace.is_none()
                    && frame.host.loading_run_id.is_none()
                    && frame.host.pause.is_none()
                    && frame.snapshot["ticks"].as_u64().unwrap() > 10)
        }) else {
            unreachable!()
        };
        let tick = frame.snapshot["ticks"].as_u64().unwrap();
        client.send(ClientMessage::Input {
            sequence: 1,
            target_tick: tick + 1,
            input: GameSessionInput { movement: Vec2::new(1.0, 0.0), ..Default::default() },
        });
        let ServerMessage::Snapshot { frame } = client.until(|message| {
            // The socket listener retains snapshots emitted before send(). Tick
            // progress alone cannot prove that this input reached the authority.
            matches!(message, ServerMessage::Snapshot { frame }
                if frame.input_acknowledgements.get(&id) == Some(&1)
                    && frame.snapshot["ticks"].as_u64().unwrap() > tick + 10)
        }) else {
            unreachable!()
        };
        runs.insert(boneyard.run_id, (id, frame.snapshot["ticks"].as_u64().unwrap()));
        clients.push(client);
    }
    host.finish();
    let archive_directory = directory.join("run-archives");
    let archives: Vec<_> = fs::read_dir(&archive_directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.to_string_lossy().ends_with(".sdrrun.gz"))
        .map(|path| read_run_archive(&path).unwrap())
        .collect();
    assert_eq!(archives.len(), 2);
    for archive in archives {
        let (id, observed_tick) = &runs[&archive.archive.run_id];
        assert_eq!(archive.archive.end_reason, RunArchiveEndReason::HostClosed);
        assert!(archive.archive.performance.tick_count > 10);
        assert_eq!(archive.archive.last_alive_by_player.len(), 1);
        assert_eq!(archive.archive.last_alive_by_player[0].0, *id);
        let alive = archive.archive.last_alive.unwrap();
        assert!(archive.states[alive.state].checkpoint.ticks >= *observed_tick);
        assert_eq!(alive.inputs[id].movement, Vec2::new(1.0, 0.0));
        assert_eq!(
            archive.states[archive.archive.final_state].checkpoint.participant_order,
            std::slice::from_ref(id)
        );
    }
    drop(clients);
    fs::remove_dir_all(directory).unwrap();
}
