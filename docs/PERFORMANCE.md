# Performance audit and fixes

Measured on 2026-10-25. The released performance implementation sustains 60 FPS in the
tested Apple M2 workloads. Ordered snapshot compaction removes the earlier Boneyard payload regression;
shared immutable scenery tables and retained GPU storage reduce repeated copying. The
tables below keep browser and host costs visible. They do **not** establish general
performance parity: title and College callback CPU, draw counts and the two smallest
display microbenchmarks remain higher than PixiJS. These are local measurements;
production deployment does not establish production FPS or CPU performance.

## Versions and method

- Actual pre-cutover Website/PixiJS `093aee9938e0cae2fabfafc5403b01aed961397e`, using its
  retained production bundle and unchanged TypeScript authority.
- The first performance pass, Rust `a1cc71ca99006d8e483b6dca75ea5ffed6887f58`, for the
  repeated native comparison. Its [original report](https://github.com/JarrettOneSource/solomon-darker-rust/blob/a1cc71ca99006d8e483b6dca75ea5ffed6887f58/docs/PERFORMANCE.md)
  retains the released-Rust comparison and earlier browser measurements.
- Qualified performance commit `ae8783f3ef18c8f452f6281ad8093dc4823b55e2`, merged through
  [PR 1](https://github.com/JarrettOneSource/solomon-darker-rust/pull/1). Released
  `92e3fe4c1bd7ca1eacff871ff79e604f904dfcf1` has the same runtime, assets and build
  configuration; its only additional code change corrects an integration-test client's
  WebSocket handshake timeout. Source, executable and client checksums accompany the
  retained benchmark build receipts.

One Apple M2, 16 GiB RAM, Google Chrome with actual ANGLE/Metal, headless, 1600×900,
device scale 1, 60 Hz. Twelve fresh browser runs alternate engines and repeat in reverse
order: Pixi College, Rust College, Rust100, Pixi100, Pixi300, Rust300, then the reverse.
Each run owns a fresh native or TypeScript authority, with graceful shutdown recorded.
No compiler or other acceptance workload runs during measured samples; ordinary user
applications remain running. Title prompts are dismissed. Each sample warms for three
seconds; title, idle and College movement last fifteen seconds, held fire three seconds.
Tables report medians of repeated run means; p95 is the median of run p95s. These are
local comparisons, not confidence intervals.

The same canonical save fixtures provide geometry, player identity, initial positions and
100 or 300 skeletons. Enemy damage/rewards are disabled and enemy health is raised to
retain the population. Native admission uses existing save conversion; Pixi uses its
ordinary import UI. Both wait for tick 300 and then warm. Held fire must advance the
authoritative cast sequence. This measures sustained crowds and casting, not a complete
ordinary battle or identical evolving frames. College uses ordinary new-game admission
and each product's default HUD: Rust has the party panel and Pixi an onboarding hint;
ambient student positions also differ. Those differences limit attribution to the engine.

Frame-callback wall time includes submission and possible preemption. CDP `TaskDuration`
measures renderer task time, including work outside callbacks. Separate process CPU-time
differences from `ps` measure each authority, including compression; they have 10 ms
resolution. Their sum is browser renderer plus authority, not total machine CPU: browser
GPU, network and benchmark-observer processes are excluded. GPU execution time is not
measured. Draw counts observe actual WebGL submissions. Byte counts are UTF-8 WebSocket
application messages, including zlib/base64 for compact Rust updates, excluding framing,
TLS and any WebSocket-level compression.

## Browser and host results

All 36 samples hold approximately 60.00 FPS, with no interval above 20 ms across
26,649 drawn frames and no browser runtime errors. Raw intervals, state witnesses,
screenshots, failures and browser/host cleanup receipts are retained.

| Scene | Pixi callback mean / p95, ms | Rust callback mean / p95, ms | Pixi renderer ms/s | Rust renderer ms/s |
| --- | ---: | ---: | ---: | ---: |
| Title | 0.303 / 0.50 | 0.717 / 1.10 | 32.4 | 56.3 |
| College idle | 1.280 / 1.55 | 1.649 / 2.00 | 121.1 | 141.6 |
| College moving | 1.453 / 1.80 | 1.471 / 1.85 | 136.3 | 127.8 |
| 100 enemies, idle | 2.817 / 3.75 | 2.226 / 2.60 | 215.1 | 175.1 |
| 100 enemies, casting | 3.101 / 3.90 | 2.041 / 2.40 | 244.6 | 165.6 |
| 300 enemies, idle | 4.270 / 5.30 | 2.621 / 3.10 | 307.4 | 217.0 |
| 300 enemies, casting | 5.339 / 6.60 | 2.425 / 2.85 | 398.2 | 208.1 |

| Scene | Pixi host ms/s | Rust host ms/s | Pixi renderer + host ms/s | Rust renderer + host ms/s |
| --- | ---: | ---: | ---: | ---: |
| Title | 15.0 | 13.0 | 46.9 | 70.3 |
| College idle | 119.3 | 55.7 | 240.5 | 197.3 |
| College moving | 115.3 | 53.3 | 251.6 | 181.2 |
| 100 enemies, idle | 197.3 | 160.6 | 412.5 | 335.8 |
| 100 enemies, casting | 241.4 | 156.6 | 486.0 | 322.2 |
| 300 enemies, idle | 333.9 | 291.3 | 641.3 | 508.3 |
| 300 enemies, casting | 466.4 | 281.5 | 864.6 | 489.6 |

Host savings do not erase a browser-side regression. In particular, the title and College
results above remain separate limitations despite meeting the 60 Hz frame budget. The
earlier and current browser suites were recorded at different times; use their raw
measurements as observations, not a controlled before/after percentage. The current
Pixi/Rust alternating suite is the primary cross-engine comparison.

| Idle scene | Pixi encoded MB/s | Rust encoded MB/s | Pixi draws/frame | Rust draws/frame |
| --- | ---: | ---: | ---: | ---: |
| College idle | 0.125 | 0.360 | 28 | 115 |
| 100 enemies, idle | 0.638 | 0.255 | 50 | 201 |
| 300 enemies, idle | 1.567 | 0.697 | 50 | 207 |

The earlier Rust Boneyards received about 6.7/16.5 MB/s at 100/300 enemies. Compact
updates carry the complete reconstructed state while omitting repeated unchanged bytes.
College drops from about 1.72 to 0.36 MB/s, but still sends roughly 2.9 times Pixi's
application bytes; that remaining payload difference is not a parity result.
The receiver still retains and decodes canonical state; encoded size is not decoded heap
size. JS heap, WASM linear memory and authority RSS are retained separately in
`summary.json`; differing GC timing and excluded graphics/process memory prevent a
total-RAM or leak claim.

## Native hot paths and codec

Optimized ARM Rust versus Node 22.17, **not WASM or FPS**. Six sequential runs use the
first Rust performance pass, JavaScript, current Rust, current Rust, JavaScript, first
Rust pass. Each result uses seven timed batches; JavaScript display work warms for
10,000 calls. Each simulation step starts from the same restored input, with setup
outside the timer. Original/current native witnesses match exactly; cross-language
identity, positions and health agree. The source goldens remain the broader oracle.

| Workload | First Rust pass display µs | JavaScript display µs | Current Rust display µs | JavaScript tick µs | Current Rust tick µs |
| --- | ---: | ---: | ---: | ---: | ---: |
| college | 9.75 | 3.59 | 9.77 | 175.58 | 44.80 |
| boneyard-empty | 32.02 | 2.38 | 6.72 | 560.47 | 272.71 |
| boneyard-100 | 65.08 | 53.63 | 41.72 | 765.35 | 322.69 |
| boneyard-300 | 133.26 | 161.46 | 112.44 | 2432.00 | 1398.69 |

Sharing imported scenery target tables reduces Boneyard display time another 15–79%
against the first pass. The 100/300-actor display cases are faster than JavaScript; College
and empty Boneyard remain slower. Native simulation is faster in these four fixtures.
College tick repeat medians span roughly 36–54 µs, so its midpoint is not tightly bounded.
One-tick timings do not represent the evolving cost of an entire battle.

The codec microbenchmark replays sixteen captured complete frames per crowd size over
eight rounds, retaining 128 exact round trips per case. The first round warms; the table
uses medians of seven rounds and excludes the initial full packet from packet averages.

| Actors | Full packet kB | Compact packet kB | Full JSON decode µs | Compact decode µs | Prepare / encode µs |
| --- | ---: | ---: | ---: | ---: | ---: |
| 100 | 332.0 | 12.4 | 951 | 698 | 1005 / 1231 |
| 300 | 828.4 | 34.5 | 2008 | 1623 | 2569 / 3176 |

Packets are about 96% smaller and native decoding is about 27%/19% faster. Preparation
and compression are real authority costs, included in the browser-suite host table.
Re-serializing the entire reconstructed document and recursively derived untagged
deserialization initially cost more CPU; incremental exact size accounting and a direct
serde visitor removed that native decode regression. Earlier candidates remain in the
evidence directory and are excluded from final results.

## Ownership and implementation

- Immutable loaded scenes, static collision primitives and skill definitions use shared
  `Arc` storage. Gate leaves and mutable actors stay private. Imported scenery target
  tables detach only when cleanup actually changes membership. Serde keeps value-based
  saves unchanged.
- Player projection updates its selected endpoint copy; enemy interpolation borrows its
  endpoint lists. UI batches retain vertex/index storage and compatible GPU state while
  preserving painter, clipping and nested-target order.
- Idle blocking menus use one final layout pass. Real input dispatches still commit in
  order; running gameplay retains its early input-admission layout. Title graves sample
  their existing `Title.bundle` atlas crops, reducing roughly 72 submissions to 32 and
  removing nine redundant startup textures. All crop pixels match the old PNGs exactly.
  Fixed-time GPU images differ in only 126 of 1,440,000 pixels, maximum channel delta 5,
  from atlas filtering; geometry and appearance are preserved.
- Protocol version 2 prepares one canonical publication shared across matching peers.
  Each socket encodes against its last **selected and written** publication after outbox
  replacement. Each receiver reconstructs every packet before render coalescing. The
  existing JSON, zlib and base64 libraries carry lossless private field/array changes;
  first packets and larger changes use complete messages. Malformed sequences, array
  edits or size bounds fail the connection. ACKs, flame history, controls and Welcome
  transfers keep their own contracts. Protocol-1 diagnostic archives remain readable.
- Asset response writes are nonblocking and check the existing shutdown flag, retaining
  their byte offset while readers pause. This addresses the Windows paused-media shutdown
  failure without adding a consumption deadline. Admission serializes with the shutdown
  socket sweep.

Enemies/effects already use contiguous typed vectors; participant and manager order
controls gameplay and RNG. Navigation preparations are cached. No general ECS dependency
was added. An enemy actor is still 1,056 bytes on this native target; some collision queries
scan static arrays. Hardware cache misses were not measured. Further actor splitting or
spatial indexing needs a profile and ordering-preserving acceptance.

## Verification and limits

Local format, strict workspace/all-feature Clippy, strict main-WASM Clippy and all
709 native workspace tests pass. All 54 browser-support, 33 desktop contract and seven
sdmod tests pass, as do 5,679 stock-file/233 audio checks and the required browser smoke.
Exact-state regressions cover historical/current saves, Unicode, signed zero, full-width
integers, nested size accounting, reconnects, controls and actual slow-socket replacement.
An independent JavaScript reader reconstructs the captured packet streams. Earlier GPU
composition regressions remain applicable; the title atlas additionally has actual GPU
readback and pixel comparison. Functional smoke uses software rendering; performance uses
Metal. The [canonical main workflow](https://github.com/JarrettOneSource/solomon-darker-rust/actions/runs/40248178185)
passes all six jobs, including the Windows regression and complete packaged journeys on
Windows x64, Linux x64, Intel Mac and Apple silicon Mac. Each package passes 33 desktop
contracts; Windows passes 48 native launcher tests and static CRT inspection. All 32
final screenshots, archive inspections and 28 safe child stops are reviewed. The Intel
Mac guest's three raw music cancellations have complete ready-channel suspension and
subsequent-playback evidence; the other platforms have none. No unexpected browser
errors remain. The release contains all eleven public assets, with their exact revision,
checksums, metadata and anonymous availability verified.

Title/College browser CPU and the smallest display copies remain follow-up work. Higher
refresh rates, mobile hardware, four-player load, constrained networks, long-session
allocation growth and GPU execution time were not measured. [PORT-COMPLETION.md](PORT-COMPLETION.md)
owns release and installed acceptance. The local LLVM helper required its existing toolchain
library path in `DYLD_LIBRARY_PATH`; no toolchain files were changed.

## Evidence and reproduction

Current evidence is `~/codex-runs/solomon-performance-fixes-20261025/`: `REPORT.md`,
`candidate-artifacts.json`, build receipts, `codec-result-r3.json`, exact encoded packets,
`cpu-suite.json`, `browser-suite-r3.json`, each browser receipt/raw frames/screenshots,
`summary.json`, final source checks, functional smoke and title-atlas comparisons. The
local report owns exact commands, PR publication and cleanup. Original release/first-pass
comparisons remain under `~/codex-runs/solomon-performance-20261025/` and in the linked
earlier report. Scripts, fixtures, measured executables and clients are retained outside
temporary worktrees. Production data, deployment configuration and rollback generations
are preserved.
