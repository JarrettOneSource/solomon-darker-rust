# Coding standards

How code in this repository is written, and why. Use [the glossary](../CONTEXT.md)
for game terms and [the architecture](ARCHITECTURE.md) for ownership boundaries.

## The spirit

Value readability over cleverness, compression, or unnecessary abstraction. A
reader should be able to understand the game rule and its constraints without
reconstructing the conversation that produced it.

For recovered behavior, make three things clear:

1. **What is this thing called in the game?** Use the game's vocabulary.
2. **Where did the rule come from?** Cite the original C++ evidence, asset record,
   or recorded contract. Explain a derived value where its derivation matters.
3. **How do we know it is right?** Name the test or fixture that establishes the
   behavior, including any limits on what it proves.

New Rust behavior needs a clear purpose and independent regression coverage; it
does not need an invented retail origin. Keep evidence near non-obvious rules,
with broader ownership and tradeoffs in the architecture and decisions documents.

Finish the behavior being exposed. A stub, empty handler, or successful-looking
fallback can appear complete to both callers and future maintainers. Wire the
real operation through its callers, errors, persistence, and presentation before
calling it done. Keep genuinely unfinished work explicit in the work queue.

## Deep modules, small interfaces

Put substantial behavior behind a small interface. Keep state and invariants that
change together under one owner, and make the public operations preserve their
required order. Callers should ask for a game operation without assembling its
internal steps themselves.

For example, [HubAmbientState](../crates/sim/src/hub/ambient.rs) owns the fountain
particles, ambient random stream, seal phases, statue phase, teacher clock, and
teacher release registrations. Its `step` method performs one complete tick:

1. Consume the spawn roll.
2. Age and remove existing drops, then create a drop if the roll succeeds.
3. Consume the two seal increments and advance their phases.
4. Advance the statue and teacher clocks, maintaining the release's painters.

A caller that owns the world painter order uses one operation:

```rust
ambient.step(Some(&mut painter_order));
```

Separate public `roll_spawn`, `age_drops`, `advance_seals`, and `advance_teacher`
operations would make every caller responsible for that ordering. A replay, server,
or browser could then produce different state from the same intended ambient tick.
Keeping the sequence inside `step` prevents that coordination from spreading.

Small private helpers still belong inside a deep module. They can isolate numeric
work, remove duplication, or explain a substantial operation without becoming
part of the caller's interface. Keep short, single-use logic inline when that is
clearer. Split files along responsibilities, rather than arbitrary line counts.

As a design check, imagine removing a module: would its complexity reappear in
its callers, or would only a pass-through name disappear? Keep abstractions that
own behavior or enforce a useful boundary.

## Name the domain thing

Prefer `painter_row(world_y, sort_bias, reference_y)` to
`calc_row(a, b, c)`. The first name identifies the operation and each argument's
role. Use `HubRegion`, `registration_ordinal`, and the other established game
terms consistently across code, tests, and documentation.

Name booleans for their true case: `needs_world_painters`, `movement_active`,
`preserve_contact_x`. Closed states belong in named enum variants, such as
`HubTeacherPhase::Release`, rather than unexplained integer codes.

## Cite evidence where it matters

A recovered module's documentation should identify the rule and its evidence.
Surprising constants need a source or derivation. The RNG module provides a useful
example:

```rust
//! The native 55-word lagged-Fibonacci generator.
//!
//! Native addresses: constructor `0x00401110`, unit float `0x004011F0`, scaled float
//! `0x00401310`. The state is 232 bytes: two ring indices, 55 words, and the float divisor at
//! `+0xE4`. Every draw is reproduced bit-exactly by `tests/native_goldens.rs`.
```

“Matches the original” is not enough to locate evidence. A function address,
fixture path, asset record, or short derivation lets the next reader verify it.
Keep fixture provenance accurate; [Reference data](REFERENCE-DATA.md) describes
the retained recordings.

## Make numeric behavior explicit

Simulation arithmetic uses `f64` with deliberate single-precision boundaries.
Use `float::fround` at those boundaries. Rounding every intermediate operation
changes the result just as removing a required rounding step does.

Constants that were stored as singles should show that rounding. These constants
from [appearance.rs](../crates/sim/src/appearance.rs) preserve both the stored
values and the single-precision subtraction:

```rust
const JITTER_MAXIMUM: f64 = 0.1_f32 as f64;
const LUMINANCE_MIX: f64 = 0.8_f32 as f64;
const CHANNEL_MIX: f64 = (1.0_f32 - 0.8_f32) as f64;
```

Writing `0.1` as an `f64` literal produces a different value from widening
`0.1_f32`. Such differences can accumulate across a trajectory.

Use `float::{hypot, atan2, sin, cos, sin_cos, pow}` where the existing kernels
require their recorded evaluation. A square-root expression or platform libm
replacement can change rounding and later alter collision or navigation.
Preserve explicit fused operations and evaluation order.
[The numeric contract](DECISIONS.md#numeric-contract) explains why these owners
exist and which checks protect them.

Keep RNG consumption, actor insertion, contact order, and painter registration
deterministic. Ambient, cosmetic, collision fallback, and gameplay streams have
distinct owners; drawing a frame must not consume gameplay randomness.

## Pure kernels, I/O at the edge

Simulation rules take values and explicitly owned state. Pass time as ticks and
randomness through the relevant sampler. Keep clocks, files, networks, browser
APIs, and logging at the platform edge so a rule can run identically in a native
test and WebAssembly.

For example, `plan_hub_scripted_movement(position, target, speed)` returns a
movement plan without reading input or retaining a second character. The caller
then applies that plan through the canonical movement and collision owner.

UI controllers borrow canonical models and return typed commands. The session
applies purchases, inventory changes, and other gameplay mutations. Presentation
copies never enter persistence or acknowledge an authoritative operation.

## Direct control flow

Handle a degenerate case where it becomes relevant, then express the ordinary
operation directly. In [regions.rs](../crates/sim/src/hub/regions.rs), a scripted
walk stops at its target without carrying a flag through later branches:

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

Prefer exhaustive enum matches and range checks that read in range order. Iterate
over values when possible; use indices when they express identity or ordering,
such as sprite records and painter ordinals. Use a straightforward loop when an
iterator chain would obscure mutation or failure handling.

## Constants and ordered tables

Keep fixed portals, painters, obstacles, segments, and color tracks as data in
their authored order. Use `const` arrays and `const fn` constructors when the
values are known at compile time. One entry per line makes a table easy to compare
with its source.

For example, `HUB_COURTYARD_OBSTACLES` contains ordered `HubFixedActor` values;
its indices also select the corresponding art records. Replacing that table with
a dynamically populated map would hide an order that affects painter registration.
Use maps for actual keyed lookup, with any required iteration order explicit.

## Types that express the contract

- Use enums for closed sets and `Option` for absent or not-yet values.
- Tick counters are generally `u64`, indices and registration ordinals `usize`,
  world coordinates and phases `f64`, and packed colors and sampler words `u32`.
  Follow the owning format when a serialized field has a different width.
- Small value structs can have public fields. State with invariants should expose
  operations that preserve them; visibility alone does not enforce that rule.
- Derive `Clone`, `Copy`, and equality when they match the value's semantics,
  rather than making large owned state cheap-looking to copy.

For a visible burst, `age_ticks: Option<u64>` expresses absence directly. An
`i64` with `-1` meaning hidden requires every caller to remember a separate rule.
If an external format uses a sentinel, convert it at that boundary.

## Errors mean something

Validate file, protocol, authored-content, and browser input before admitting it
to canonical state. Use `Result` for failures a caller must handle. Keep enough
context to act on the error: identify the file, record, operation, or violated
constraint.

An `expect` message should state the invariant that makes failure impossible
after validation. Do not unwrap untrusted input, discard a meaningful failure
with `.ok()`, or continue with fabricated state. Explicitly benign failures can
be handled at their owner; optional update failure, for example, leaves offline
play available.

Save operations retain the full supported document, including opaque native
container fields. Validate before replacing a save, and preserve the existing
revision checks and atomic-write contract. A success receipt follows persistence.

## Comments carry reasons

Comments explain evidence, ordering, precision, ownership, or a platform constraint
that the code alone does not show. For example, clearing held keys on blur needs
the reason: keys released while another window has focus do not send keyup here.

Avoid comments that merely narrate assignments, excuse a workaround, or preserve
an implementation diary. Document a necessary workaround's concrete constraint
and verification. Put broader unfinished work in [Open work](WORK-QUEUE.md).

## Tests explain behavior

Name a test as a claim about the game, such as
`the_cycle_casts_then_releases_then_idles`. Assertion messages should locate a
failure by scenario, tick, actor, or field.

Use hand-derived unit cases to explain a rule and independently captured fixtures
to protect recorded behavior. Do not generate expected values with the code under
test. Prefer public operations; a private numeric helper can warrant its own test
when it has a distinct mathematical contract. Avoid assertions on incidental
`Debug` formatting or mocks that only repeat the implementation.

Keep independently evolving trajectories independent throughout a replay.
Replacing their state with expected values each tick hides accumulated divergence.
Retain a fixture's numeric bounds unless evidence supports changing the contract.

Browser checks use ordinary input and read-only observations of accepted state
and submitted GPU geometry. Review screenshots for presentation changes. Cover
important rejection, cancellation, restore, and failure paths as well as success.

## Rust conventions

- Edition 2024 and the repository's rustfmt configuration: width 100 and
  `use_small_heuristics = "Max"`.
- Explain a necessary lint allowance on the item it affects.
- Use `#[must_use]` on pure functions whose return value matters and `const fn`
  for constructors used in constant tables.
- Order imports as standard library, external crates, then crate-local imports.
- Re-export the small set of types callers share; use module paths for the rest.
- Keep `unsafe` out of game code; use maintained platform libraries.
- Restrict shared mutable wrappers to owners that need them, such as browser
  callbacks or native Lua handles. Prefer enums when the variants form a closed set.
- Use `From` for lossless widening where available. Make intentional narrowing or
  wrapping evident from the operation or a comment.
- Reuse existing code, then the standard library and platform APIs. Add a dependency
  when it supplies needed behavior; record consequential tradeoffs in
  [Decisions](DECISIONS.md).

A module should introduce its purpose and evidence, then its types and public
operations, with supporting implementation and tests close enough to follow.
Use the existing `hub.rs` with `hub/` children pattern when responsibilities need
separate files. File layout should help a reader find the public contract.

## Worked example: the teacher's frame

The [teacher frame sampler](../crates/sim/src/hub/teacher.rs) brings these choices
together in a small example:

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

It accepts an integer tick, names the game concept, checks ranges in order, and
uses an explicit seed without consuming another owner's random stream. The
comment explains what its integer result means. Unit cases check the cycle
boundaries; `hub-teacher-goldens.json` records complete cycles and seeds. Both
the code and its verification remain understandable without the original chat.

## Before calling it done

- Review the complete operation and affected callers, including errors and saves.
- Confirm that names, non-obvious rules, and evidence are understandable without chat.
- Remove abandoned code, temporary debugging, and placeholders for required behavior.
- Run the relevant [repository checks](../AGENTS.md#checks) and package checks;
  report actual failures or unavailable gates accurately.
- Update documentation to describe the resulting behavior and ownership.

Documentation-only changes need review and link/command validation rather than
a game rebuild. Preserve useful principles, examples, constraints, and procedures
when editing prose; shorter text is not an improvement if it removes their meaning.
