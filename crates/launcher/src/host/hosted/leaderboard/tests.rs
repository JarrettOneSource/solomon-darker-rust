use serde::Deserialize;

use super::*;

#[derive(Deserialize)]
struct Fixture {
    scenarios: Vec<Scenario>,
    utc: Vec<UtcCase>,
}

#[derive(Deserialize)]
struct Scenario {
    name: String,
    steps: Vec<Step>,
}

#[derive(Deserialize)]
struct Step {
    action: Action,
    expected: Observation,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Action {
    Clients {
        clients: Vec<Client>,
    },
    Taint,
    Tutorial {
        #[serde(rename = "runId")]
        run_id: String,
    },
    Publish {
        previous: Option<Frame>,
        completed: Option<Frame>,
        #[serde(default = "enabled")]
        enabled: bool,
    },
    Prune {
        shared: bool,
        active: Vec<String>,
    },
}

const fn enabled() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Client {
    id: String,
    user_id: Option<u32>,
    eligible: bool,
    staged: bool,
    run_id: Option<String>,
    submit: bool,
    open: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Frame {
    run_id: String,
    scores: BTreeMap<String, Option<u64>>,
    phase: Option<String>,
    portrait: Option<bool>,
}

impl Frame {
    fn snapshot(&self) -> LeaderboardRunSnapshot {
        LeaderboardRunSnapshot { run_id: self.run_id.clone(), scores: self.scores.clone() }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Observation {
    recipients: Vec<String>,
    clock_reads: usize,
    ineligible: Vec<String>,
    issued: Vec<String>,
}

#[derive(Deserialize)]
struct UtcCase {
    milliseconds: i64,
    expected: String,
}

#[test]
fn score_receipt_eligibility_and_once_only_edges_match_the_source_host() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../../../sim/tests/fixtures/website/host-leaderboard-goldens.json"
    ))
    .unwrap();
    for scenario in fixture.scenarios {
        let mut owner = LeaderboardRunState::default();
        let mut clients = Vec::new();
        for (index, step) in scenario.steps.into_iter().enumerate() {
            let mut recipients = Vec::new();
            let mut clock_reads = 0;
            match step.action {
                Action::Clients { clients: next } => clients = next,
                Action::Taint => {
                    for client in &clients {
                        if !client.eligible
                            && !client.staged
                            && let Some(run_id) = &client.run_id
                        {
                            owner.taint(run_id);
                        }
                    }
                }
                Action::Tutorial { run_id } => owner.taint(&run_id),
                Action::Publish { previous, completed, enabled } => {
                    let before = previous.as_ref().map(Frame::snapshot);
                    let after = completed.as_ref().map(Frame::snapshot);
                    for client in &clients {
                        let receipt = owner
                            .receipt_for(
                                before.as_ref(),
                                after.as_ref(),
                                LeaderboardClient {
                                    player_id: &client.id,
                                    user_id: client.user_id,
                                    submit_runs: client.submit,
                                    socket_open: client.open,
                                },
                                enabled,
                                |_| {
                                    clock_reads += 1;
                                    let frame = completed.as_ref().unwrap();
                                    let valid = frame.phase.as_deref().unwrap_or("game-over")
                                        == "game-over"
                                        && frame.portrait != Some(false);
                                    // This replay covers host eligibility and callback
                                    // order. The existing 20 exact signing fixtures cover
                                    // the payload/crypto at this same production seam.
                                    Ok(valid.then(|| client.id.clone()))
                                },
                            )
                            .unwrap();
                        if receipt.is_some() {
                            recipients.push(client.id.clone());
                        }
                    }
                }
                Action::Prune { shared, active } => {
                    let active = if shared {
                        active.into_iter().collect()
                    } else {
                        active.into_iter().take(1).collect()
                    };
                    owner.prune(&active);
                }
            }
            let actual = Observation {
                recipients,
                clock_reads,
                ineligible: owner.ineligible_run_ids.iter().cloned().collect(),
                issued: owner
                    .issued_receipts
                    .iter()
                    .map(|(run, player)| format!("{run}\0{player}"))
                    .collect(),
            };
            assert_eq!(actual, step.expected, "{} observation {index}", scenario.name);
        }
    }
    for case in fixture.utc {
        let now = time::OffsetDateTime::from_unix_timestamp_nanos(
            i128::from(case.milliseconds) * 1_000_000,
        )
        .unwrap();
        assert_eq!(source_utc_timestamp(now).unwrap(), case.expected);
    }
}
