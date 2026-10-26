# Working on Solomon Darker

This repository owns the Rust game, native authority, WebAssembly client, save tools,
mod compiler, and desktop app. The website owns accounts, cloud storage, published
content, and editing.

## Read before editing

- [Architecture](docs/ARCHITECTURE.md) for module ownership and state flow.
- [Coding standards](docs/CODING-STANDARDS.md) for implementation and testing rules.
- [Decisions](docs/DECISIONS.md) when changing a numeric, persistence, rendering,
  protocol, or dependency contract.
- [Glossary](CONTEXT.md) for game terms.
- [Work queue](docs/WORK-QUEUE.md) for open work.

Use current Rust behavior and supported player data as the compatibility baseline.
The original C++ Solomon Dark 0.72.5 executable, assets, recordings, and recovered
native evidence define retail behavior. Name the evidence when implementing a
recovered rule. [Reference data](docs/REFERENCE-DATA.md) explains fixture provenance.

## Implementation

Keep game rules in `solomon-sim`, asset/save decoding in `solomon-assets`, and I/O
at platform boundaries. UI controllers borrow accepted state and emit typed commands;
the session owns gameplay, inventory, random streams, and persistence.

Use this repository's tracked inputs for builds and tests. New features and fixes
belong in Rust. Use the original C++ game when retail behavior needs investigation.
Existing regression recordings retain their recorded expectations and metadata;
new behavior needs an independent regression or captured retail evidence.

For a behavior change:

1. Establish the expected result and affected owner.
2. Add a failing regression when practical.
3. Change the canonical implementation and its callers.
4. Run the relevant checks and inspect composed behavior when presentation changes.
5. Update the relevant current documentation and open work.

Keep changes in a task worktree and preserve other tasks' work. Remove the clean,
published task worktree after completion.

## Checks

The full game gate is:

```sh
python3 scripts/verify-client-assets.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy -p solomon-web --target wasm32-unknown-unknown -- -D warnings
cargo clippy -p solomon-loader --target wasm32-unknown-unknown -- -D warnings
cargo test --workspace --all-features
npm run test:browser-support
```

Mod-tool changes also require `npm run test:sdmod` with the built
`solomon-sdmod` executable. Desktop changes require `npm run test:desktop`
and the packaged journey on each affected platform before release.

For browser changes, run the affected journeys against your own server and review
their screenshots and error reports. See [Testing](README.md#testing) and
[GPU UI](docs/GPU-UI.md). Use a free port from 4181 upward; a separate local service
owns 4180.

Documentation-only work needs link, command, and consistency review. Report the
checks actually run. A local test, published artifact, installed release, and live
browser result establish different things; keep those claims separate.

## Documentation

Describe the implementation and operating contract that exist now. Replace stale
statements instead of appending corrections. Keep open work in the work queue and
historical execution details in Git history or external test artifacts.

Record consequential tradeoffs in `docs/DECISIONS.md`; keep `CONTEXT.md` a glossary.
Do not add task assignments, attempt logs, chronological receipts, or repeated test
totals to product and architecture documents.
