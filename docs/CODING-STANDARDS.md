# Coding standards

How code in this repository is written, and why. Read it before your first change and again
whenever a review points here. Every "after" example is real code from this tree; every
"before" is a shape we rewrite on sight.

## The spirit

This repository is a translation of evidence. Each kernel reproduces a behaviour that was
recovered from the retail game and already written down once, in the Website's TypeScript. A
reader of any line should be able to answer three questions without leaving the file:

1. **What is this thing called in the game?** The names are game nouns.
2. **Where did it come from?** The reference is cited.
3. **How do we know it is right?** The test or fixture that pins it is named.

Code that answers those three reads at the speed of thought. Code that does not is a puzzle
the next agent solves again, and the next. Elegance here is not cleverness; it is the absence
of anything the reader has to reconstruct.

The other half of the spirit: a kernel is a fact, not a feature. It enters the crate with its
reference, its unit tests and its differential fixture, or it does not enter. A stubbed kernel
looks exactly like a verified one to the next reader, and that difference is the whole product.

## 1. Name the domain thing

A name tells the reader which game concept they are looking at. Mechanism words (`manager`,
`helper`, `handler`, `util`, `data`, `info`, `process`, `calc`, `tmp`, `result2`) say nothing,
so the next sentence has to explain, and the sentence after that explains again.

Where a module ports a Website kernel, keep the Website's name in snake_case so a grep crosses
the language boundary: `hubPortalAt` is `hub_portal_at`; `stepHubAmbient` is
`HubAmbientState::step`, and the doc comment says so. Keep the stock game's vocabulary for the
concepts the Website also kept (`painter`, `registration ordinal`, `semantic word`);
[CONTEXT.md](../CONTEXT.md) is the glossary.

Before:

```rust
fn calc_row(a: f64, b: f64, c: f64) -> i64 {
    ((a.trunc() + b.trunc() - c.trunc()) / 2.0).trunc() as i64
}
```

After (`crates/sim/src/hub/painter.rs`):

```rust
/// `nativeRegionPainterRow`: the two-unit row of an actor relative to the reference actor,
/// truncated toward zero.
#[must_use]
pub fn painter_row(world_y: f64, sort_bias: f64, reference_y: f64) -> i64 {
```

Same arithmetic. Now the reader knows which Website kernel this is, that rows are two units,
how they round, and what each argument is, before reading the body.

Booleans are named for the true case and read as an answered question: `needs_world_painters`,
`movement_active`, `preserve_contact_x`. Enumerations name the cases, never the integers:
`HubTeacherPhase::Release`, not `phase == 1`.

## 2. Cite the reference

Every ported module's doc comment names the file it ports. Every constant whose value would
surprise a reader says where the value comes from.

```rust
//! The native 55-word lagged-Fibonacci generator.
//!
//! Native addresses: constructor `0x00401110`, unit float `0x004011F0`, scaled float
//! `0x00401310`. The state is 232 bytes: two ring indices, 55 words, and the float divisor at
//! `+0xE4`. Every draw is reproduced bit-exactly by `tests/native_goldens.rs`.
```

A reference is a path or an address, not a feeling. "Matches the original" cites nothing;
`core-kernels/hub-regions.ts` and `0x00401110` do. A value you derived yourself says how
(`// one eighth of the lane cap`), because a number with no provenance is the first thing a
later agent "fixes".

## 3. Exactness is the contract, so make the rounding visible

The differential fixtures require bit-for-bit equality with the Website. The Website rounds to
single precision at specific points (`Math.fround`), uses V8's `Math.hypot`, and mixes
constants that were already singles. The port reproduces each of those points and nowhere else.

- Round with `fround` exactly where the reference rounds. One extra `fround` is as wrong as one
  missing.
- Lengths use `crate::float::hypot`, never `(dx * dx + dy * dy).sqrt()`.
- A constant the reference holds as a single is written with its rounding showing
  (`crates/sim/src/appearance.rs`):

```rust
// The Website mixes these as `Math.fround` singles, so the constants carry f32 rounding;
// the channel mix is the single-precision remainder of the luminance mix.
const JITTER_MAXIMUM: f64 = 0.1_f32 as f64;
const LUMINANCE_MIX: f64 = 0.8_f32 as f64;
const CHANNEL_MIX: f64 = (1.0_f32 - 0.8_f32) as f64;
```

Before:

```rust
const JITTER: f64 = 0.1; // close enough
```

"Close enough" is a differential failure that surfaces three kernels later, in someone else's
ticket.

Use `crate::float::atan2` for the source's angle calculation. Equal-input platform-libm
differences were proved to amplify into a material Solomon path divergence; the canonical
kernel reproduces the pinned V8 evaluation and has exact native/WASM primitive fixtures.
Use `crate::float::{sin, cos, sin_cos}` for Source trigonometry as well: an equal-input
last-bit difference was measured in native-save settlement, and the canonical owner has
20,100 exact primitive pairs. Preserve existing `fround` boundaries. Historical fixtures
may retain a named trigonometric tolerance; do not extend it to other assertions or replace
a trajectory's independently evolving inputs. See `DECISIONS.md` for the evidence.
The Fire envelope uses `float::pow`: its observed one-bit platform difference is covered
by 17,431 native/WASM cases. Its explicit fused operations reproduce the recorded ARM64
V8 build. Preserve that evaluation order; do not substitute platform `powf`.

## 4. Pure kernels, I/O at the edge

`solomon-sim` takes values and returns values. It reads no clock, no randomness, no network,
no DOM, and writes no log. Randomness is state threaded through (`&mut NativeRng`,
`rng_state: u32`); time is a tick count. That is what lets a kernel be tested from a JSON
fixture and behave identically in a native test and in the browser.

`solomon-web` is thin on purpose: fetch and decode assets, read input, run the fixed-tick
loop, draw. When a function in `web` grows a rule about the game (a speed, an order, a colour,
a threshold), the rule moves into `sim` as a kernel and `web` calls it.

The deletion test: imagine deleting the module. If its complexity reappears in every caller,
it was earning its keep. If nothing changes but a name, it was a pass-through; inline it.

After (`crates/sim/src/hub/regions.rs`):

```rust
/// A movement plan that walks the wizard toward `target` without input
/// (`planHubScriptedMovement`). Nothing is retained between ticks, so the walk stops dead.
#[must_use]
pub fn plan_hub_scripted_movement(position: Vec2, target: Vec2, speed: f64) -> HubWizardPlan {
```

Two numbers in, a plan out. The oracle calls the TypeScript twin with the same two numbers and
the test compares the plans.

## 5. Deep modules, small interfaces

Put a lot of behaviour behind a few entry points, and make the entry points impossible to call
in the wrong order.

`HubAmbientState` exposes `new()`, `step()`, `teacher_burst()` and three presentation
functions. Behind them: a xorshift sampler with a zero escape, a spawn roll, drop ageing, two
seal tracks, the statue phase and the teacher clock. A caller cannot forget the spawn roll or
run the seal increments before the drop lifetime, because that order lives inside `step`:

```rust
pub fn step(&mut self) {
    let (state, roll) = random_integer(self.rng_state, FOUNTAIN_ROLL_RANGE);
    self.rng_state = state;
    for particle in &mut self.fountain_particles {
        particle.remaining -= FOUNTAIN_LIFETIME_DECREMENT;
        particle.scale *= FOUNTAIN_SCALE_MULTIPLIER;
    }
    self.fountain_particles.retain(|particle| particle.remaining > 0.0);
    if roll == FOUNTAIN_SPAWN_ROLL {
        ...
```

The shallow version exposes `roll_spawn`, `age_drops`, `advance_seal_phases`,
`advance_statue` and `advance_teacher`, and expects the frame loop to call all five in the
reference order every tick. Every caller becomes a copy of the kernel, and the next thing
that needs "one ambient tick" (a replay, a server, a test) copies it again.

A deep module is still made of small private functions (`random_integer`,
`interpolate_color`, `saturate`). They sit inside the seam, serve the implementation and its
own tests, and do not widen the interface.

## 6. Direct control flow

Handle the degenerate case first with an early return, then write the straight-line happy
path. Guard clauses at the top; no flag set in one branch and inspected in another; no `else`
after a `return`.

```rust
pub fn hub_scripted_delta(position: Vec2, target: Vec2, speed: f64) -> Vec2 {
    let dx = target.x - position.x;
    let dy = target.y - position.y;
    let distance = hypot(dx, dy);
    if distance == 0.0 {
        return Vec2::ZERO;
    }
    let step = distance.min(speed * MOVEMENT_LANE_CAP * MOVEMENT_TICK_SECONDS);
    Vec2::new(dx / distance * step, dy / distance * step)
}
```

A `match` on an enum lists every arm; the compiler is the checklist. A chain of `if` over
ranges reads in the order of the ranges (`hub_teacher_phase_at`). Nesting past two levels
means a private function or an early return is missing.

Loops iterate the thing (`for particle in &mut self.fountain_particles`), not an index, unless
the index is the domain (painter ordinals, sprite records).

## 7. Constants are tables, tables are data

Reference tables (portals, fixed painters, obstacles, segment lists, colour tracks) are `const`
arrays of value structs in the reference's order, one entry per line, built with `const fn`
constructors so a row reads like the row in the source:

```rust
pub const HUB_COURTYARD_OBSTACLES: [HubFixedActor; 8] = [
    HubFixedActor::standing(1458.5, 320.5, 40.0),
    HubFixedActor::standing(955.5, 239.5, 40.0),
    HubFixedActor::standing(749.5, 162.5, 40.0),
```

Before: a `HashMap<String, Obstacle>` filled by `fn init()`, a `match` on
`"college-obstacle-3"`, or a `static` built by `OnceLock`. Each hides the table's order, and
in this game order *is* behaviour (painter ordinals are registration order). A `const` table
has no init order, no lifetime, no lock, and the test can index it.

Lookups by Website id exist only at the oracle seam (`HubRegion::from_id`,
`hub_fixed_painter_ordinal`). Inside the kernels, use the enum or the ordinal.

## 8. Types that cannot lie

- A closed set is an `enum` (`HubRegion`, `HubTeacherPhase`, `BlendMode`), never a string or an
  integer code.
- "Not yet" is an `Option` (`age_ticks: Option<u64>`), never a sentinel.
- Tick counters are `u64`; indices and ordinals are `usize`; world units and phases are `f64`;
  packed colours and sampler state are `u32`.
- Small value structs derive `Clone, Copy, Debug, PartialEq` (`Eq` too when there are no
  floats) and have `pub` fields, because they are data, not invariants.
- State with invariants (`HubAmbientState`, `NativeRng`) keeps its fields readable, because
  the tests and the differential need to see them, and changes them only through its methods.

Before:

```rust
pub age_ticks: i64, // -1 means hidden
```

After:

```rust
/// Ticks since the release, while the burst is visible.
pub age_ticks: Option<u64>,
```

The sentinel compiles when you forget the check. The `Option` does not.

Sentinels survive in one place: the JSON fixtures, where −1 stands in for `None` because JSON
has no better spelling. The test converts at the seam and its doc comment says so.

## 9. Errors mean something

Kernels do not return `Result`. Their inputs are typed so that invalid input does not exist,
and a contract the types cannot express panics with a sentence about the game, because a
kernel that keeps going with a wrong value corrupts the replay silently.

The browser edge returns `Result<T, String>`, and the string is something a person reading the
status line can act on (`crates/web/src/courtyard.rs`):

```rust
return Err(format!(
    "College.bundle has {} records; the courtyard needs {highest}",
    records.len()
));
```

Nothing that came from the network, the DOM or a file is unwrapped. No `.ok()` throws an
error away. No `console.log` stands in for handling. `expect("...")` is for the case whose
message states the invariant that makes failure impossible.

## 10. Comments carry reasons

A doc comment says what the thing is in the game and where it came from. An inline comment
exists only where the code cannot say *why*: a V8 quirk, a native rounding, an order that
matters, an event the browser never sends (`crates/web/src/input.rs`):

```rust
// Keys released while another window had focus never send keyup here.
let released = held.clone();
let blur = Closure::<dyn FnMut(Event)>::new(move |_: Event| released.borrow_mut().clear());
```

Comments that narrate (`// increment the counter`), mark sections (`// ===== HELPERS =====`),
apologise (`// hacky but works`) or defer (`// TODO: clean up`) are deleted on sight. A TODO is
a ticket in [WORK-QUEUE.md](WORK-QUEUE.md), not a comment.

## 11. Tests are sentences

A test's name is a claim about the game that reads as a sentence in the runner's output:

```
the_cycle_casts_then_releases_then_idles
spray_is_born_small_lives_up_to_two_and_a_quarter_seconds_and_grows
hub_walks_replay_the_website_kernels_exactly
```

Each kernel has two layers. Unit tests pin intent with hand-derived numbers, so a reader can
see *why* a value is what it is. A differential fixture pins truth against the Website.
Assertion messages carry what locates the divergence (`"scenario {} diverged at tick
{index}"`), not "assertion failed".

Tests exercise the interface (`step`, `hub_portal_at`), not private helpers, unless the helper
carries its own numeric claim (`decay_stops_at_zero_in_single_precision`). The kernels have no
mocks because nothing in them needs mocking. No test asserts a `Debug` string.

## 12. Small finished changes, nothing fake

A change is one ticket: one module family, its tests, its fixture, its wiring, its README
line. It compiles, it is formatted, clippy is clean on both targets, every test passes, and the
browser smoke still passes when the web crate changed.

Nothing in the tree pretends. There is no `todo!()`, no `unimplemented!()`, no empty
`fn draw_hud() {}` waiting for a later ticket, no placeholder art, no branch that quietly does
nothing until "later". A feature that is not ported is absent, and the README's "not ported"
line says so.

## 13. Rust as we write it

- Edition 2024. `rustfmt` with the repository's `rustfmt.toml` (width 100,
  `use_small_heuristics = "Max"`). Clippy's default lints clean for native and
  `wasm32-unknown-unknown`. An `#[allow(...)]` carries its reason in a comment on the same item.
- `#[must_use]` on every pure function that returns a value. `const fn` on constructors used in
  tables.
- `pub use` at the crate root only for the handful of types every caller needs (`Vec2`,
  `HubWizard`, `NativeRng`). Everything else is named by its module path, which doubles as
  documentation.
- No `unsafe`. No `Rc<RefCell<_>>` outside the browser frame loop and event closures. No
  `Box<dyn Trait>` where an enum lists the cases. No `static mut`, no globals, no `OnceLock`
  for data that can be a `const`.
- Iterators over indices. A `map`/`filter` chain that fits in a few lines; a `for` loop when
  the chain would need a comment.
- Widening casts use `f64::from`. Narrowing casts use `as` with the reason evident from the
  name or a comment (`absolute_tick as u32` reproduces the reference's 32-bit wrap).
- Dependencies: the default `solomon-sim` build has none; its optional persistence uses
  serde. `solomon-assets` uses the simulation types and existing serde JSON decoders;
  `solomon-web` adds the wasm-bindgen family. Record new dependency decisions in
  [DECISIONS.md](DECISIONS.md).
- Module layout: `hub.rs` with `hub/` children is the pattern. Split along responsibilities
  (ambient, painter, regions, teacher), never by line count.

## 14. The shape of a file

```
//! What this module is in the game, and which reference it ports.

use std::...;                 // std first, then external crates, then this crate
use crate::...;

pub const ...                 // reference constants, each documented
const ...                     // private constants, grouped under a reason comment

pub struct / pub enum ...     // types, one doc line each
impl ...

pub fn ...                    // kernels, the reference name in the doc comment
fn ...                        // private helpers after the public surface

#[cfg(test)]
mod tests { ... }             // sentence-named tests
```

## 15. A port, worked

The Website (`frontend/src/game/hub-teacher.ts`):

```ts
export function hubTeacherFrameAt(elapsedSeconds: number, seed = 0): number {
  const absoluteTick = teacherAbsoluteTick(elapsedSeconds)
  const tick = absoluteTick % HUB_TEACHER_CYCLE_TICKS
  if (tick === 0) return 0
  if (tick < HUB_TEACHER_CAST_TICKS) {
    return nativeRandomIntFromSemanticWord(teacherWord(seed, absoluteTick, 0), 2)
  }
  return tick < HUB_TEACHER_IDLE_START_TICK ? 2 : 3
}
```

The port (`crates/sim/src/hub/teacher.rs`):

```rust
/// The teacher's own frame: a per-tick coin flip between the two cast frames, then the
/// release and idle frames.
#[must_use]
pub fn hub_teacher_frame_at(absolute_tick: u64, seed: u32) -> u32 {
    let tick = absolute_tick % HUB_TEACHER_CYCLE_TICKS;
    if tick == 0 {
        0
    } else if tick < HUB_TEACHER_CAST_TICKS {
        native_random_int_from_word(teacher_word(seed, absolute_tick, 0), 2)
    } else if tick < HUB_TEACHER_IDLE_START_TICK {
        2
    } else {
        3
    }
}
```

What changed, and why:

- The port takes the tick, not seconds. The Website's seconds exist because its caller has a
  float clock; the simulation's clock is the tick count, and the oracle converts at the seam
  (`tick / 100`) when it records. The float stays out of the kernel.
- The name survived the language boundary. The doc comment says in game terms what the integer
  means, which the TypeScript left to the reader.
- The ternary became the last two arms of one `if` chain, so the three ranges read in order.
- The constants kept their names so the two files can be read side by side.
- The fixture `hub-teacher-goldens.json` records every tick of three cycles for two seeds, and
  `teacher_cycle_burst_and_random_domain_replay_the_website_kernels` replays them.

## 16. Before you call it done

- The three questions have answers in the file: game name, reference, verification.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets`,
  `cargo clippy -p solomon-web --target wasm32-unknown-unknown` and `cargo test --workspace`
  are clean.
- The new kernel has unit tests with hand-derived numbers and a differential fixture recorded
  by a script under `tools/website-oracle/`.
- Nothing fake, nothing commented out, no TODO comments, no debug prints.
- The README's "what the browser shows" and "not ported" lines are true again.
- Read the diff once as the next agent: every name a game noun, every number with a source,
  every branch reachable.
