# Performance

## Title and College rendering

The sprite program retains its projection, pixel-center offset, crop bounds and
two premultiplication values. Each draw uploads only the values that changed;
program activation, batch order, geometry and shaders remain the same.

Measurements on 2026-10-31 compare this change with `f4e79a9d` on an Apple M2,
using Rust 1.98.1 release builds and Chrome 156.0.8078.74 with ANGLE/Metal. Each
fresh browser and native authority uses a 1600×900 viewport at device scale 1.
The title prompt is dismissed and College uses ordinary new-game admission,
waiting for courtyard arrival. Each scene warms for five seconds and runs for
twenty seconds. Three adjacent pairs use baseline/candidate, candidate/baseline,
then baseline/candidate order. The table reports medians of run means and p95s.

| Scene | Previous callback mean / p95, ms | Current callback mean / p95, ms | Previous / current renderer task CPU, ms/s |
| --- | ---: | ---: | ---: |
| Title | 0.731 / 1.50 | 0.731 / 1.40 | 56.03 / 56.47 |
| College idle | 1.859 / 2.30 | 1.741 / 2.20 | 156.17 / 148.12 |

College callback means fall by 5.3–9.8% across the three pairs, with a 6.3%
reduction between their medians. Renderer task CPU falls by 5.2%. Title timing
does not show a meaningful improvement. Both versions sustain approximately
60 FPS, with no sampled frame interval above 20 ms. Draw counts remain 32 for
title and approximately 223 for College. The unchanged authority uses about
60 ms of CPU per second during College.

Separate API-count passes record 166→6 uniform calls per title frame and
approximately 1,297→351 per College frame. Those instrumented timings are
excluded from the table. College's ambient actors evolve normally; the baseline
API-count pass has two additional draws per frame, so those counts describe
the observed workloads rather than identical frames.

Callback wall time includes submission, draw/buffer instrumentation and possible
preemption. CDP task time excludes GPU, authority and observer processes. No
compiler or acceptance suite runs during sampling; ordinary background apps and
OS services remain active. All runs retain raw intervals, GPU identity, actual
input, images, error reports and graceful browser/authority exits. Separate CPU
profiles identify redundant uniform uploads but are not timing comparisons.
These short local samples establish neither native performance nor mobile,
high-refresh, network or endurance behavior.

The reproducible driver and exact build identities are retained in
`~/codex-runs/solomon-title-college-20261031/`: `browser-profile.cjs`,
`baseline-build/receipt.json`, `candidate-build/receipt.json`, the candidate
source patch, `comparison-suite.json` and `comparison-review.json`. Native and
browser composition probes cover projection, blend, crop and program restoration.

## Boulder collision exclusions

The original Report 81 schema51 save contains 309 gravestones. Earth and
Ethereal Boulder ignore those scenery owners during their sampled body-path
queries. The old collision loop searched the full ignored-owner list for every
shape at every sample, including distant shapes. A separate 15.368-second
browser CPU profile attributes 9.732 seconds of self time to `first_contact`
through primary casting's path query, plus 0.437 seconds in its `memcmp` child.
The function names come from a symbol map whose non-custom WASM sections match
the measured baseline artifact byte for byte.

`first_contact` now checks geometry before searching the ignored-owner list.
Its geometry functions are pure. Polygon, static/dynamic segment and circle
order, contact normals, tangency tests, sweep samples and numeric expressions
are unchanged. A public collision contract compares placement, first blocked
progress and resolved movement with a world from which only the ignored shapes
were removed, including overlapping shapes, unowned barriers, and duplicate or
unknown exclusions. A 1,500-tick original-save charge/release control produces
byte-identical baseline/candidate final checkpoints, including seven releases.
Native timing from that state-equality run overlaps builds and is not qualified.

The browser comparison uses Rust `4f447266`, Apple M2/ANGLE Metal, Chrome
156.0.8078.80, 1600×900, DPR1 and current default visuals. Each fresh browser
restores the same unmodified original through the public Rust save codec, then
uses ordinary Play → Continue. The 15-second sample follows Continue, with
2-second pointer holds and 0.2-second releases at the disclosed fixed screen
point (1120,360). Three pairs use baseline/candidate, candidate/baseline and
baseline/candidate order. Each arm accepts seven casts (194→201), remains alive,
saves through ordinary Leave and exits Chrome gracefully. No compiler, test
suite or profiler runs during these samples.

Medians across the three runs of each variant:

| Metric | Baseline | Deferred exclusion lookup |
| --- | ---: | ---: |
| Drawn-frame rate | 37.70 FPS | 60.00 FPS |
| Callback mean | 21.86 ms | 4.09 ms |
| Callback p95 | 70.50 ms | 6.20 ms |
| Frame-interval p95 | 66.70 ms | 18.20 ms |
| Maximum frame interval | 332.70 ms | 18.70 ms |
| Long Tasks per window | 34 | 0 |

Every candidate's maximum interval is at most 18.7 ms. Page, console, warning,
HTTP and request-failure arrays are empty in all six paired receipts. Composed
world images are reviewed without a pixel-equivalence claim. The separate
profile is excluded from timing comparisons. These local static-client controls
identify the current exclusion-lookup cost; they do not replay hosted transport,
two-player stalls, the original saved bearing, reporter hardware or the
unspecified October31 recurrence. The historical duplicate split-painter repair,
More Missiles count and Smart Missiles guidance remain separate contracts.

## Broader measured scope

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

## Entity storage

Entity populations are plain `Vec`, `Option` and `BTreeMap` collections. They
replaced `bevy_ecs` component storage in November 2026, which had itself replaced
plain collections in October. Both changes were measured with a native session
driver built with Rust 1.98.1, optimization level 3, no LTO and 16 codegen units.
Runs alternate between the two builds after a paired warm-up; builds complete
before timing, and ordinary background services keep running.

College cases advance 1,000 movement ticks. Boneyard combat stays locked until a
wizard touches Solomon and his entrance finishes, so each Boneyard case first plays
that entrance, untimed, then times 300 ticks of primary casting among 32 or 128
mixed Skeleton, Skeleton Archer, Zombie and Imp actors. One player casts Fire; four
players cast Fire, Water, Earth and Ether; and four Weld players cast Flame Lash,
Blizzard Beam, Steam Jet and Hailstones, which keep the most effects alive. The
timed windows cast 260, 990 and 3,802 spells, and the Weld case ends with 169 live
effects. Players have additional health and mana so combat continues throughout.
Every requested tick advances in the expected run phase; all players remain alive.
Seeds, inputs and navigation are identical, and all final checkpoints match byte
for byte. The first admission tick is measured separately and excluded from the
retained-tick timings below. Clone timings cover construction and destruction of
500 complete session copies per run.

The current storage (`68fb9390`) compared with `bevy_ecs` storage (`bf9d8644`) on
an Apple M2, as medians of 20 alternating pairs:

| Scene | ECS tick, µs | Plain tick, µs | ECS clone, µs | Plain clone, µs |
| --- | ---: | ---: | ---: | ---: |
| College, one player | 40.94 | 35.80 | 40.85 | 6.49 |
| College, four players | 69.03 | 61.28 | 51.05 | 15.05 |
| 32 enemies, one Fire player | 1,113.99 | 1,103.67 | 104.15 | 50.26 |
| 128 enemies, four players | 1,119.42 | 1,057.03 | 146.32 | 70.74 |
| 32 enemies, four Weld players | 985.77 | 903.44 | 171.29 | 78.25 |

Ticks are 1–12% faster and session copies 2.1–6.3× faster, by median per-pair
ratio. Serializing 100 network snapshots costs the same within 1.3%. Process peak
RSS, which includes asset loading, setup and temporary copies, falls by
1.0–1.4 MiB. Every primary transient takes the space of the largest effect variant,
512 bytes, yet the Weld case, which holds the most live effects, ticks 8% faster
and copies 2.2× faster than it did with a table per effect type.

The October change (`c1017a5` compared with `dc16cfca` on an Apple M5 Pro, medians
of five alternating runs) made ticks 3–18% slower and session copies 2–7× slower,
raised peak RSS by 1.4–2.4 MiB and grew the inline `GameSession` from 11,064 to
12,080 bytes. Its driver did not play Solomon's entrance, so its Boneyard cases
measured enemy crowds without any spells.

These measurements establish the cost of the tested native simulation workloads.
They do not establish browser frame rate, GPU cost or long-session memory use.
The reproducible drivers, raw samples and checkpoints are retained in
`~/codex-runs/solomon-ecs-cutover-20261026/` for October and
`~/codex-runs/solomon-standalone-cutover-20261104/bench/` for November:
`session-bench.rs`, `session-benchmark.py` and the per-run results.

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

Further title and College hot paths, small-scene display copies, and College
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
