//! Source `host/runtime-event-publisher.ts`: bounded, ordered, best-effort operator
//! events. Network retries live on one worker, never on the gameplay thread.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use solomon_sim::modding::{LuaObject, LuaValue};

const MAXIMUM_QUEUED_EVENTS: usize = 256;
const MAXIMUM_ATTEMPTS: u8 = 5;
const RETRY_DELAYS_MS: [u64; 4] = [100, 500, 2_000, 5_000];

pub struct RuntimeEventConfig {
    endpoint: url::Url,
    secret: String,
}

impl RuntimeEventConfig {
    pub fn new(endpoint: &str, secret: String) -> Result<Self, String> {
        let endpoint = url::Url::parse(endpoint).map_err(|_| "Invalid URL")?;
        // Preserve the actual Source URL.hostname check. In particular, WHATWG
        // serializes IPv6 as "[::1]", which its unbracketed set does not admit.
        if endpoint.scheme() != "http"
            || !matches!(endpoint.host_str(), Some("127.0.0.1" | "::1" | "localhost"))
        {
            return Err("The runtime event endpoint must use loopback HTTP".into());
        }
        if secret.encode_utf16().count() < 32 {
            return Err("The runtime event secret must contain at least 32 bytes".into());
        }
        Ok(Self { endpoint, secret })
    }
}

pub struct RuntimeEventEntry {
    pub component: String,
    pub details: LuaObject,
    pub event: String,
    pub message: String,
    pub occurred_at_utc: Option<String>,
}

struct QueuedRuntimeEvent {
    entry: RuntimeEventEntry,
    attempts: AtomicU8,
}

struct EventQueue {
    accepting: bool,
    entries: VecDeque<Arc<QueuedRuntimeEvent>>,
    first_pending: Option<Arc<QueuedRuntimeEvent>>,
}

struct SharedQueue {
    state: Mutex<EventQueue>,
    changed: Condvar,
}

#[derive(Clone)]
pub struct RuntimeEventSink(Arc<SharedQueue>);

impl RuntimeEventSink {
    pub fn publish(&self, entry: RuntimeEventEntry) {
        let mut state =
            self.0.state.lock().expect("the event worker does not panic under its queue lock");
        if !state.accepting {
            return;
        }
        if state.entries.len() >= MAXIMUM_QUEUED_EVENTS {
            state.entries.pop_front();
        }
        let queued = Arc::new(QueuedRuntimeEvent { entry, attempts: AtomicU8::new(0) });
        if state.entries.is_empty() {
            // Source starts fetch synchronously before its first await. Reserve
            // that first event even if the worker has not received CPU time yet.
            state.first_pending = Some(Arc::clone(&queued));
        }
        state.entries.push_back(queued);
        self.0.changed.notify_one();
    }
}

pub struct RuntimeEventPublisher {
    sink: RuntimeEventSink,
    worker: Option<JoinHandle<()>>,
}

impl RuntimeEventPublisher {
    pub fn new(config: RuntimeEventConfig) -> Result<Self, String> {
        let shared = Arc::new(SharedQueue {
            state: Mutex::new(EventQueue {
                accepting: true,
                entries: VecDeque::new(),
                first_pending: None,
            }),
            changed: Condvar::new(),
        });
        let queue = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("game-runtime-events".into())
            .spawn(move || pump(config, &queue))
            .map_err(|error| format!("Cannot start runtime event publisher: {error}"))?;
        Ok(Self { sink: RuntimeEventSink(shared), worker: Some(worker) })
    }

    #[must_use]
    pub fn sink(&self) -> RuntimeEventSink {
        self.sink.clone()
    }

    /// Source stops admission then drains the current retry pump before shutdown.
    pub fn close(&mut self) {
        {
            let mut state = self.sink.0.state.lock().expect("the event queue remains usable");
            state.accepting = false;
            self.sink.0.changed.notify_one();
        }
        if let Some(worker) = self.worker.take() {
            worker.join().expect("the runtime event worker does not panic");
        }
    }
}

impl Drop for RuntimeEventPublisher {
    fn drop(&mut self) {
        self.close();
    }
}

fn pump(config: RuntimeEventConfig, queue: &SharedQueue) {
    let mut endpoint = config.endpoint.clone();
    endpoint.set_fragment(None);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .http_status_as_error(false)
        .max_redirects(20)
        .proxy(None)
        .build()
        .into();
    loop {
        let queued = {
            let mut state = queue.state.lock().expect("the event queue remains usable");
            while state.entries.is_empty() && state.accepting {
                state = queue.changed.wait(state).expect("the event queue remains usable");
            }
            let Some(queued) =
                state.first_pending.take().or_else(|| state.entries.front().cloned())
            else {
                return;
            };
            queued
        };
        let accepted = document(&queued.entry)
            .ok()
            .and_then(|body| {
                // Source fetch rejects URL credentials, even though its constructor
                // and the endpoint's loopback check accept that URL.
                if !endpoint.username().is_empty() || endpoint.password().is_some() {
                    return None;
                }
                agent
                    .post(endpoint.as_str())
                    .header("authorization", format!("Bearer {}", config.secret))
                    .header("content-type", "application/json")
                    .send(body)
                    .ok()
            })
            .is_some_and(|response| response.status().is_success());
        let attempts =
            if accepted { 0 } else { queued.attempts.fetch_add(1, Ordering::Relaxed) + 1 };
        if accepted || attempts >= MAXIMUM_ATTEMPTS {
            // Like Source shift(), this removes the current head even if a burst
            // already evicted the in-flight entry at the queue's 256-event bound.
            queue.state.lock().expect("the event queue remains usable").entries.pop_front();
        } else {
            thread::sleep(Duration::from_millis(RETRY_DELAYS_MS[usize::from(attempts - 1)]));
        }
    }
}

#[cfg(test)]
mod tests;

fn document(entry: &RuntimeEventEntry) -> Result<String, String> {
    let occurred = match &entry.occurred_at_utc {
        Some(value) => value.clone(),
        None => super::source_utc_timestamp(time::OffsetDateTime::now_utc())?,
    };
    solomon_assets::json_value::stringify(&LuaValue::Object(
        [
            ("component".into(), LuaValue::String(entry.component.clone())),
            ("event".into(), LuaValue::String(entry.event.clone())),
            ("message".into(), LuaValue::String(entry.message.clone())),
            ("occurredAtUtc".into(), LuaValue::String(occurred)),
            ("details".into(), LuaValue::Object(entry.details.clone())),
        ]
        .into_iter()
        .collect(),
    ))
}
