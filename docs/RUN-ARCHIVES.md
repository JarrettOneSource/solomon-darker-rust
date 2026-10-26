# Private run archives

The native supervisor can retain run diagnostics alongside its Memorial file
configuration, in a sibling `run-archives/` directory. Disposable editor tests
do not create archives. These files are operator diagnostics, separate from
player saves and cloud continuation.

## Capture

Each Boneyard step records pre-step input, spawn intents, kernel time, and
scheduler lag. The recorder retains the last active living state per participant,
the last living state overall, the strictly slowest tick, and the final state.
Performance summaries include totals, over-budget ticks, and the latest sixty
100-tick windows.

Game Over, world removal, host close, and fatal errors finish a capture.
Connect, run entry, disconnect, and orderly close also capture state.

Checkpoint copies occur outside the reported kernel time; scheduler lag includes
their cost. A capture retains complete state rather than a partial projection.

## Storage

One owned worker serializes and compresses captures in order. It publishes a
UUID-named `.sdrrun.gz` and `.json` summary through non-overwriting hard links
from a private temporary directory. Unix directories use `0700`; files use
`0600`. The worker drains before the operator event publisher and supervisor stop.

Payloads are gzip-compressed JSON with
`encoding: "solomon-rust-checkpoint-table"` and `encodingVersion: 1`.
A table stores complete Rust checkpoints; archive entries reference their indices.
The transient inventory dye-permission map is retained beside each checkpoint.
Shared states use one table entry.

Metadata records runtime and protocol versions, revision, content manifest, and
loaded geometry. `read_run_archive` decodes the private format. It is not a
player-save import or public upload format.

I/O failures emit `run_archive.failed`.

## Retention and checks

Startup and hourly pruning apply a thirty-day policy. Only correctly named
archives and six-character `.writing-` artifacts are candidates. A sibling
`.keep` protects a candidate. Removing an archive also removes its summary;
other directory contents are left alone.

Tests cover capture boundaries, strict worst-tick ties, rolling windows, gzip
round trips, shared checkpoint references, permissions, non-overwrite behavior,
queue drain, pruning, and shared/private Boneyard sessions.
