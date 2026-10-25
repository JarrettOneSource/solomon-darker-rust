# Performance audit

Measured on 2026-10-25. The optimized client sustains 60 FPS in the tested Apple M2
gameplay workloads and uses less frame-callback CPU than the retained PixiJS client in
100- and 300-enemy Boneyards. The audit found and removed substantial redundant Rust
snapshot copies. It does **not** establish general performance parity: title CPU, state
payload size, draw count and some small-scene display microbenchmarks remain higher.
These are local measurements of a performance branch, not an installed production result.

## Compared versions and method

- Actual pre-cutover Website/PixiJS revision `093aee9938e0cae2fabfafc5403b01aed961397e`,
  using its retained production bundle and unchanged TypeScript authority.
- Released Rust `4aa3a7b733a46788e7cb27b601f6bceb5e4f08b9`, with native source at
  documentation-only main `2a3a8b3faed96ec329a73fd4950e9830cfdb32a4`.
- The optimized native host and WASM client built from `perf/runtime-20261025`.
  Module checksums, source diff and build receipts are retained with the measurements.

One Apple M2, 16 GiB RAM, Google Chrome using actual ANGLE/Metal, headless, 1600×900,
device scale 1, 60 Hz. Each run uses one fresh browser and a local authority. No compiler
runs during accepted samples. Other user applications remain running. Both title prompts
are dismissed. Each workload warms for three seconds; title and idle samples last fifteen
seconds, held-fire samples three seconds. The Boneyard sequence alternates engines and
repeats in reverse order: Pixi100, Rust100, Rust300, Pixi300, Pixi300, Rust300, Rust100,
Pixi100. The table reports medians of the repeated run means; p95 is the median of each
run's p95, not a pooled percentile. This is a local comparison, not a confidence interval.

The same canonical save fixtures supply scene geometry, player identity and starting
positions, and 100 or 300 skeletons. Enemy damage and rewards are disabled and enemy
health is raised to retain the population. Existing save conversion and host admission
load the native fixtures; the Pixi client uses its ordinary save import UI. Both wait for
authority tick 300 and then warm. Held fire advances the authoritative cast sequence;
the native host also records five emitted attacks. This is sustained crowd and casting
work, not a complete ordinary battle or a pixel-identical replay after divergent clocks.
Idle 100/300 screenshots from both engines were inspected for scene and population.

`requestAnimationFrame` instrumentation measures callback wall time, including submission
and possible preemption. CDP `TaskDuration` measures browser renderer task time over the
sample, including work outside the frame callback; neither includes the native host's CPU
or measures GPU execution time. Draw/triangle counters observe actual WebGL submissions.
Received JSON byte counts describe decoded WebSocket message bodies, not compressed wire
traffic. Screenshots, all raw frames, errors, state witnesses and cleanup receipts are kept.

## Browser Boneyard results

All 24 primary title/Boneyard samples render at approximately 60.00 FPS. Across 15,844
drawn frames, no measured frame interval exceeds 20 ms. No browser runtime errors occur.

| Scene | Pixi callback mean / p95, ms | Rust callback mean / p95, ms | Pixi task ms/s | Rust task ms/s |
| --- | ---: | ---: | ---: | ---: |
| Title | 0.264 / 0.50 | 0.865 / 1.30 | 28.7 | 63.5 |
| 100 enemies, idle | 2.543 / 2.95 | 1.822 / 2.25 | 193.5 | 176.9 |
| 100 enemies, casting | 2.803 / 3.25 | 1.705 / 2.10 | 221.6 | 169.7 |
| 300 enemies, idle | 3.727 / 4.35 | 2.434 / 2.95 | 269.6 | 260.1 |
| 300 enemies, casting | 4.551 / 5.45 | 2.274 / 2.75 | 340.4 | 252.2 |

For those Boneyard samples, frame-callback CPU is 28–50% lower; total renderer task CPU
is 4–26% lower. A supplemental single run of the original Rust client gives callback
means of 2.032/1.889 ms at 100 enemies and 2.718/2.518 ms at 300, idle/casting respectively.
The optimized client's corresponding repeated means are about 10% lower. That supplemental
comparison is weaker evidence than the repeated native microbenchmarks below.

| Idle scene | Pixi received JSON MB/s | Rust received JSON MB/s | Pixi draws/frame | Rust draws/frame |
| --- | ---: | ---: | ---: | ---: |
| 100 enemies | 0.638 | 6.715 | 50 | 201 |
| 300 enemies | 1.562 | 16.457 | 50 | 207 |

Rust still sends complete dynamic snapshots while the old host uses entity deltas. The
approximately 10.5× decoded payload volume is a material remaining transport cost, even
though these loopback workloads hold their frame budget. Static scene/navigation data is
already cached per connection. A protocol improvement must preserve ACK/backpressure,
reconnect/baseline recovery, authored mutations and exact restored state; changing transport
semantics is separate from sharing in-process immutable data.

Observed JavaScript heaps after idle samples are 95.7/169.6 MB for Pixi and 4.25/6.59 MB
for Rust at 100/300 enemies. Rust additionally has 21.73/22.87 MB of WASM linear memory.
These values have different GC timing and exclude browser process, graphics and native-host
memory. They do not establish total RAM savings or absence of leaks.

## College browser results

Four further runs use Pixi, Rust, Rust, Pixi with ordinary new-game admission, then fifteen
seconds idle and fifteen seconds holding right. All twelve title/College samples hold
60 FPS with zero intervals above 20 ms across 10,801 drawn frames, and no browser errors.

| Scene | Pixi callback mean / p95, ms | Rust callback mean / p95, ms | Pixi task ms/s | Rust task ms/s |
| --- | ---: | ---: | ---: | ---: |
| College idle | 1.214 / 1.65 | 1.395 / 1.75 | 119.9 | 123.5 |
| College moving | 1.303 / 1.70 | 1.285 / 1.60 | 126.5 | 114.0 |

Idle College is about 3% heavier in total renderer task CPU and 15% heavier in the frame
callback; moving is about 10% lighter in task CPU. These are ordinary product journeys
with their default HUDs, not identical rendered frames: the retained screenshots show
Rust's party panel, Pixi's onboarding hint and different ambient student positions. This
limits attribution of the small idle difference to engine overhead alone. The matched
College input used by the native microbenchmark is a separate, controlled comparison.

## Native and JavaScript hot paths

These are optimized native ARM Rust versus Node 22.17 JavaScript, **not WASM or FPS**.
Six sequential runs use original Rust, JavaScript, optimized Rust, optimized Rust,
JavaScript, original Rust. Each result has seven timed batches; JavaScript display work
warms for 10,000 calls. Every timed simulation step starts from the same restored input,
with restoration/cloning outside the timed section. Player/enemy identity, position and
health witnesses agree across languages; all four original/optimized native witnesses
agree. Source golden tests remain the broader behavioral oracle.

| Workload | Original Rust display µs | JavaScript display µs | Optimized Rust display µs | Original Rust tick µs | JavaScript tick µs | Optimized Rust tick µs |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| College | 55.58 | 3.64 | 9.81 | 71.16 | 152.21 | 43.12 |
| Empty Boneyard | 93.59 | 2.40 | 31.89 | 304.15 | 479.16 | 275.26 |
| 100 enemies | 145.44 | 52.02 | 64.81 | 359.00 | 662.50 | 324.68 |
| 300 enemies | 246.57 | 156.07 | 132.97 | 1439.17 | 2261.07 | 1411.22 |

Rust display sampling improves 46–82% against its release baseline, but remains slower
than JavaScript in the three smaller fixtures. Native simulation is faster than JavaScript
in all four fixtures. College's optimized tick repeat medians vary from 35.21 to 51.03 µs;
the table's midpoint must not be treated as a tightly bounded result. These one-tick
microbenchmarks do not represent the evolving host cost of an entire battle.

## Data ownership and GPU changes

- `LoadedBoneyard` shares immutable scene data through `Arc`. Static collision primitives
  have one shared owner; each world keeps its own gate-leaf vector. Display gate updates
  retain private mutable state without copying static geometry.
- Player skill entries use `Arc<[Entry]>`; `OnceLock` builds the stock table once.
  Authored edits use copy-on-write. Existing serde value shapes and save behavior remain.
- Player projection updates the selected endpoint copy in place. Enemy interpolation
  borrows endpoint iterators instead of cloning both input lists before producing output.
- The UI painter retains vertex scratch storage and submits indices without constructing
  another triangle vector. Compatible adjacent meshes keep scissor and sampler state.
  Nested targets and screen scopes restore state, preserving painter order and blending.

Enemy/effect families already use contiguous typed vectors. Participant and manager order
is explicit because it controls gameplay, allocation and RNG consumption. Navigation
preparations are already shared and cached. This is data-oriented storage, not a general
archetype ECS. An enemy actor is still 1,056 bytes on this native target, and some collision
queries scan the static primitive arrays. No hardware cache-miss counters were collected;
there is no claim that CPU cache locality is optimal. Further hot/cold actor splitting or
spatial indexing needs a profile and ordering-preserving acceptance, rather than an ECS
dependency justified by its name.

## Verification and remaining scope

Local format, strict workspace Clippy, strict main-WASM Clippy and all 702 workspace tests
pass. All 45 browser-support tests, 5,679 stock asset and 233 audio checks, the required
game browser smoke and actual GPU composition checks pass. The new regressions cover
shared scene/skill storage with independent mutations, and indexed batching with clipped
pixels, nested-target clip restoration and exactly two expected draw submissions. Existing
snapshot, interpolation, navigation, collision and stat-book goldens retain their exact data.
The GPU composition and ordinary smoke use software rendering for functional acceptance;
the performance samples use Metal.

The local Rust installation's LLVM helper initially lacked its library search path. Builds
completed with the existing toolchain library directory supplied in `DYLD_LIBRARY_PATH`;
no toolchain files or dependencies were changed. Earlier benchmark-driver failures are
retained separately and excluded from accepted samples, including one College run cut off
by a task-owned service restart. No product failure is inferred from those setup failures.

The title still uses roughly 3.3× Pixi's callback CPU and more draw calls, although its
absolute mean remains below 1 ms here. Decoded snapshot volume remains higher, and small
display microbenchmarks still have avoidable copying. These are follow-up priorities.
Mobile hardware, higher refresh rates, four-player load, network latency/bandwidth limits,
long-session allocation growth and GPU execution time were not measured. This branch has
not been installed in production or qualified as a new four-platform release.

## Retained evidence

The M2 evidence directory is `~/codex-runs/solomon-performance-20261025/`:

- `baseline-provenance.json`, `implementation.patch`, build/source-check receipts and logs.
- `cpu-suite.json`, `cpu-{1..6}-*.json`, `cpu-fixture-witnesses.json` and
  `native-before-after-witnesses.json`; standalone native and JavaScript benchmark sources.
- `browser-suite.json`, each `browser-{1..8}-*/receipt.json`, raw frames and screenshots;
  `released-rust-{100,300}/` retains the weaker original-client comparison separately.
- `college-suite.json`, `college-summary.json` and `college-{pixi,rust}-r*/` retain the
  separate ordinary College journeys and their twelve accepted samples.
- `summary.json` and `summarize.py` own aggregation; `composition-r4/` and `visual-checks/`
  own functional browser acceptance. The final local `REPORT.md` records publication,
  reproduction instructions and cleanup.

Benchmark fixtures, scripts and raw evidence stay outside temporary worktrees. They use
the existing repository browser lifecycle/navigation helpers and existing dependencies.
No production save, deployment configuration or rollback generation is changed.
