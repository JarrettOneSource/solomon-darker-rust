# Performance

## Measured scope

Release `92e3fe4c` sustains approximately 60 FPS in the measured Apple M2 workloads.
These measurements were captured on 2026-10-25; they are not production telemetry.

The setup is one Apple M2 with 16 GiB RAM, headless Google Chrome using ANGLE/Metal,
a 1600×900 viewport, device scale 1, and a 60 Hz display target. Samples warm for
three seconds. Title, idle, and College movement samples last fifteen seconds;
held casting lasts three seconds. The tables use medians of repeated run means;
p95 values are medians of run p95s.

Each run uses a fresh browser and authority, with graceful shutdown recorded.
Samples exclude compiler and other acceptance workloads, though ordinary user
applications remain running. The crowd fixtures fix initial geometry, player
identity, and positions; the evolving frames need not remain identical. Runs
wait for tick 300 before warming. Title prompts are dismissed, and College uses
ordinary new-game admission and the default HUD. These details affect comparisons
between builds even on the same machine.

Crowd cases start from retained saves with 100 or 300 skeletons, increased enemy
health, and damage/rewards disabled to retain the population. Casting must advance
the authoritative cast sequence. These cases measure sustained crowds, not a full
ordinary battle.

## Browser and host

| Scene | Callback mean / p95, ms | Renderer CPU, ms/s | Host CPU, ms/s |
| --- | ---: | ---: | ---: |
| Title | 0.717 / 1.10 | 56.3 | 13.0 |
| College idle | 1.649 / 2.00 | 141.6 | 55.7 |
| College moving | 1.471 / 1.85 | 127.8 | 53.3 |
| 100 enemies, idle | 2.226 / 2.60 | 175.1 | 160.6 |
| 100 enemies, casting | 2.041 / 2.40 | 165.6 | 156.6 |
| 300 enemies, idle | 2.621 / 3.10 | 217.0 | 291.3 |
| 300 enemies, casting | 2.425 / 2.85 | 208.1 | 281.5 |

Callback wall time includes submission and possible preemption. CDP
`TaskDuration` measures renderer task time. Host CPU uses process CPU-time
differences at 10 ms resolution. These exclude GPU, network, and observer processes;
they do not measure total machine CPU or GPU execution time.

| Idle scene | Encoded application MB/s | Draws/frame |
| --- | ---: | ---: |
| College | 0.360 | 115 |
| 100 enemies | 0.255 | 201 |
| 300 enemies | 0.697 | 207 |

Application bytes include compact-update zlib/base64 but exclude WebSocket framing,
TLS, and transport compression.

## Native kernel and codec measurements

These are optimized ARM native timings, not WebAssembly or frame-rate measurements.
Each simulation step begins from a restored fixture, with setup outside the timer.

| Workload | Display projection, µs | Simulation tick, µs |
| --- | ---: | ---: |
| College | 9.77 | 44.80 |
| Empty Boneyard | 6.72 | 272.71 |
| 100 enemies | 41.72 | 322.69 |
| 300 enemies | 112.44 | 1398.69 |

One-tick timings do not represent the evolving cost of an entire battle.
College tick repeat medians span roughly 36–54 µs.

The codec measurement replays sixteen complete frames per crowd size over eight
rounds, with 128 exact round trips per case. The first round warms; medians use the
remaining rounds and packet averages exclude the first full publication.

| Actors | Full / compact packet, kB | Full / compact decode, µs | Prepare / encode, µs |
| --- | ---: | ---: | ---: |
| 100 | 332.0 / 12.4 | 951 / 698 | 1005 / 1231 |
| 300 | 828.4 / 34.5 | 2008 / 1623 | 2569 / 3176 |

Preparation and compression are host costs and remain included in the browser-suite
host measurements.

## Component storage

Native session measurements compare component storage at `c1017a5` with the
pre-cutover `dc16cfca` implementation on an Apple M5 Pro. Both use Rust 1.98.1,
optimization level 3, no LTO and 16 codegen units. The table contains medians from
five alternating runs after a paired warm-up. Benchmark builds completed before
timing; ordinary background services remained running.

College cases advance 1,000 movement ticks. Boneyard cases advance 300 ticks with
held primary casting and 32 or 128 mixed Skeleton, Skeleton Archer, Zombie and Imp
actors. Players have additional health for this benchmark so hits still resolve
and combat continues throughout the measurement. Every requested tick advances
in the expected run phase; all players remain alive. Seeds, inputs and navigation
are identical, and all final checkpoints match byte for byte. The first admission
tick is measured separately and excluded from the retained-tick timings below.

| Scene | Previous tick, µs | ECS tick, µs | Previous clone, µs | ECS clone, µs |
| --- | ---: | ---: | ---: | ---: |
| College, one player | 24.72 | 29.09 | 4.36 | 30.34 |
| College, four players | 45.87 | 52.40 | 10.54 | 37.80 |
| 32 enemies, one player | 190.29 | 195.68 | 35.10 | 61.64 |
| 128 enemies, four players | 471.04 | 494.45 | 48.47 | 92.73 |

Tick costs increase by 3–18% in these workloads. Clone timings cover construction
and destruction of 500 complete session copies per run. Serialized snapshots cost
98–641 µs before the cutover and 99–650 µs with components. Process peak RSS rises
by 1.4–2.4 MiB; it includes asset loading, setup and temporary copies. The inline
`GameSession` grows from 11,064 to 12,080 bytes. Component ownership improves
structural consistency but does not produce an overall speedup in these cases.

These measurements establish the cost of the tested native simulation workloads.
They do not establish browser frame rate, GPU cost or long-session memory use.
The reproducible driver, raw samples and checkpoint checksums are retained in
`~/codex-runs/solomon-ecs-cutover-20261026/`: `session-bench.rs`,
`session-benchmark.py`, `session-benchmark.json` and `REPORT.md`.

## Implementation constraints

Immutable scene, collision, skill, and unchanged scenery-target data share storage.
Display projection borrows endpoint lists and updates the selected private copy.
GPU submission retains buffers and batches adjacent compatible state without
changing painter order, clipping, or nested targets.

Idle blocking menus need one final layout pass. Running gameplay retains input
admission before painting. Title graves sample existing atlas crops.

Snapshot compression preserves complete canonical state and connection ordering.
An unsent publication cannot become a baseline. Serialization savings must not
change numeric precision, acknowledgements, or reliable results.

## Open measurements

Title and idle College browser cost, small-scene display copies, and College
snapshot volume remain profiling targets. Four-player browser, mobile, high-refresh,
constrained-network, and long-session workloads are unmeasured.

Future measurements should record the exact source and artifact identities, GPU,
viewport, actor population, warm-up, input sequence, sample length, raw intervals,
CPU boundaries, application bytes, and process cleanup. Compare Rust revisions
under the same workload and keep raw evidence with the measured build.

## Reproduction records

The measured runtime, assets, and build configuration are those of `ae8783f3`,
included in release `92e3fe4c`. The retained local evidence is
`~/codex-runs/solomon-performance-fixes-20261025/`: `REPORT.md` records commands,
`candidate-artifacts.json` identifies the build, and `browser-suite-r3.json`,
`cpu-suite.json`, and `codec-result-r3.json` describe the measured runs.
Receipts, raw frame intervals, encoded packets, screenshots, and shutdown results
support the tables. These records live outside temporary worktrees and are not
inputs to the normal game build.
