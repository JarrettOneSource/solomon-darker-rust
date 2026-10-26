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
snapshot volume remain profiling targets. Four-player, mobile, high-refresh,
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
