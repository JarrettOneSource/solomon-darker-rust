# Private run diagnostic archives

The native supervisor retains the current Website's `host/run-archive.ts` lifecycle.
Archives are enabled with its Memorial file configuration and live in the sibling
`run-archives/` directory. Disposable editor tests do not create archives. These are
operator diagnostics, separate from player saves and cloud continuations.

Each actual Boneyard step records its pre-step input and spawn intents, elapsed kernel
time and scheduler lag. The recorder retains the last active living state for each
participant, the last active living state overall, the strictly slowest tick, and the
final state. Performance includes total and over-budget ticks plus the latest sixty
100-tick windows. Game Over, world removal, host close and fatal server errors finish
the corresponding capture. Connect, run entry, disconnect and orderly close also
capture the current state at the same boundaries as the Website.

One native worker serializes and compresses completed captures in order. A UUID-named
`.sdrrun.gz` and its `.json` summary are published with non-overwriting hard links from
a private temporary directory. Directories use mode `0700`; capture and summary files
use `0600` on Unix. The worker drains before supervisor shutdown finishes, then the
operator event publisher drains. I/O failures emit `run_archive.failed` and do not turn
a completed diagnostic into a player save.

The private payload intentionally differs from the Website's V8 object serialization.
It is gzip-compressed JSON with `encoding: "solomon-rust-checkpoint-table"` and
`encodingVersion: 1`. One table stores complete canonical Rust checkpoints, while
archive checkpoints reference table indices. The transient inventory dye permission
map is retained beside each durable checkpoint. Shared retained states occupy one
table entry. Metadata records the native runtime version, gameplay protocol, source
revision, content manifest and actual loaded geometry. `read_run_archive` decodes this
format; it is not an HTTP upload or player-save import API.

Startup and hourly pruning use the Website's thirty-day policy. Only correctly named
archives and six-character `.writing-` artifacts are candidates. A sibling `.keep`
protects a candidate; removing an archive also removes its summary. No other directory
contents are swept.

Mutable Rust sessions require checkpoint copies where the Website retains immutable
references. Capture copies occur outside the reported kernel time; scheduler lag still
includes their cost. Serialization timing measures each complete state and the final
metadata on the archive worker. Sustained live checks must account for capture overhead;
the recorder does not skip ticks or replace states with a partial projection.

Verification uses the unchanged current Website recorder in
`tools/website-oracle/run-archive-goldens.mjs`: living/death boundaries, strict worst-tick
ties, explicit checkpoints, run removal, host/error closure and 6,001 observed ticks
covering window retention. Native tests cover gzip round trips, shared checkpoint
references, publication permissions, non-overwrite behavior, queue drain, retention,
and real shared/private Boneyard socket journeys.
