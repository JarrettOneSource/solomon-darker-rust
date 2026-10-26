# Coding standards

Write code around game concepts and explicit ownership. Use
[the glossary](../CONTEXT.md) for names and [the architecture](ARCHITECTURE.md)
for module boundaries.

## Rules and state

Keep simulation rules independent of clocks, files, browsers, and networks.
Pass time as ticks and randomness as owned state. Keep state that changes together
under one owner, with an interface that preserves its update order.

Use direct control flow. Keep short, single-use logic inline; extract a helper
when it removes duplication or isolates substantial behavior. Split modules by
responsibility rather than line count. Prefer one implementation shared by
native and WebAssembly callers.

Use precise types for game concepts. Closed sets belong in enums; absent values
belong in `Option`. Keep protocol and file validation at their input boundaries.
Avoid speculative retries, swallowed errors, unused extension points, and
fallbacks that hide an invariant failure.

## Numeric behavior

Use `float::fround` at the established single-precision boundaries.
Rounding every intermediate operation changes the simulation.

Use `float::{hypot, atan2, sin, cos, sin_cos, pow}` where the existing kernels
require their recorded evaluation. Platform math substitutions can change
collision, navigation, or long-running spell trajectories. Preserve explicit
fused operations and evaluation order. [Decisions](DECISIONS.md#numeric-contract)
explains the compatibility contract.

Keep RNG consumption, actor insertion, contact order, and painter registration
deterministic. Ambient, cosmetic, and gameplay streams have distinct owners.

## Interfaces and persistence

UI controllers borrow canonical state and return typed commands. They do not
apply inventory changes, accepted chat, purchases, or saves locally.

Persistence must retain the full supported document, including opaque native
container fields. Validate before replacing a save; use the existing revision
and atomic-write contracts. Display copies never enter persistence.

## Names and comments

Name the game concept: `HubRegion`, `painter_row`, `registration_ordinal`.
Name booleans for their true case.

Comments explain evidence, ordering constraints, numeric behavior, or lifecycle
ownership. Cite original C++ addresses, asset records, or fixture paths where they
establish a recovered rule. A comment should not restate an assignment or preserve
an implementation diary.

## Rust conventions

- Edition 2024, using the repository's rustfmt configuration.
- `#[must_use]` on pure functions whose return value matters.
- `const fn` for constructors used in constant tables.
- Standard library imports first, then external crates, then crate-local imports.
- Re-export only the small set of types shared across module boundaries.
- Keep `unsafe` out of game code; use maintained platform libraries.
- Restrict shared mutable wrappers to the owner that needs them, such as browser
  callbacks or native Lua handles.
- Add a dependency when it provides needed behavior that existing code, the
  standard library, and platform APIs cannot supply. Record a consequential
  tradeoff in [Decisions](DECISIONS.md).

## Tests

Test observable rules, supported inputs, failure paths, and integration boundaries.
Use hand-derived unit cases for intent and independently captured fixtures for
regressions. New expected values must not come from the implementation under test.

Keep independently evolving trajectories independent throughout a replay. Replacing
their state with expected values each tick hides accumulated divergence. Retain
the fixture's numeric bounds unless evidence supports changing the contract.

Browser checks use ordinary input and read-only observations of accepted state
and submitted GPU geometry. Review screenshots for presentation changes.

Use the commands in [AGENTS.md](../AGENTS.md#checks) and the affected package's
checks. Report failures and unavailable checks accurately. Documentation-only
changes need review and link/command validation rather than a game rebuild.
