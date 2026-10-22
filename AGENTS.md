# Agent instructions

Solomon Dark, rebuilt in Rust and WebAssembly. Every kernel reproduces a behaviour recovered
from the retail game and ported once already in the sibling Website. This repository ports it
again, with current Website behavior and existing user data as the acceptance target.

## Read first

1. [CONTEXT.md](CONTEXT.md): the glossary. Use its words in code, tests and reports.
2. [docs/CODING-STANDARDS.md](docs/CODING-STANDARDS.md): how code here is written, with
   reasons and examples. Read it before the first edit.
3. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): crate seams, the frame loop, the painter
   model, the verification layers, and where a feature goes.
4. [docs/WORK-QUEUE.md](docs/WORK-QUEUE.md): the tickets. Take one; it lists what it owns,
   what to read, its oracle and its acceptance.
5. [docs/DECISIONS.md](docs/DECISIONS.md): when a change touches a listed decision, adds a
   dependency, or opens a new seam.
6. [README.md](README.md): commands, prerequisites, tailnet access.

## Sources

Everything in `~/Projects/Solomon Dark` is open to read, run and test. Use whatever serves
the port; the result that matters is this rebuild extended with behaviour that traces to a
reference, with the source named so the next agent can check it.

- [Website at `fa08ae24`](https://github.com/JarrettOneSource/solomon-dark-website/tree/fa08ae249ff5563570bdb2a653e2ef9358ce7b83):
  the acceptance reference, including its TypeScript kernels, tests and native-parity ledger.
  To record new fixtures, check out the required reference revision and set
  `SOLOMON_WEBSITE_DIR` to that checkout. Existing fixtures retain their actual source
  revision; normal builds and replay tests use this repository's tracked inputs.
- This repository owns the game, native authority, WASM client, save tools and desktop shell.
  The Website owns accounts, content, storage and editing and consumes a verified game
  artifact. `docs/PORT-COMPLETION.md` owns publication and installed acceptance.
- `../SolomonDarkAbandonware` (also `../Stock Game 0.72.5`): the retail game with its
  atlases and bundles, and the decompilation outputs beside it in the workspace.
- `crates/sim/tests/fixtures/native/`: retail recordings with their native contract in each
  header.
- `../Mod Loader`: superseded by this rebuild, which reads nothing from it.

## Gates

A change is complete when all of these are clean:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo clippy -p solomon-web --target wasm32-unknown-unknown
cargo test --workspace
```

and, when `crates/web` or `web/` changed, the browser smoke passes against your own dev
server (README, "Browser smoke"). Port 4180 belongs to the launchd agent; serve on another
port and pass it as `URL`. That M2 service uses the separate legacy `Rust Game` checkout;
it does not serve this repository. Desktop changes also run `npm run test:desktop` and
the packaged journey on each affected platform before release publication.

## The porting loop

Each ticket runs this loop. Finish each step before starting the next.

1. **Find the reference.** The ticket names the Website files. Read each one whole and note
   every `Math.fround`, `Math.hypot` and constant that is already a single. Done when you can
   say which functions are the kernel and which are presentation.
2. **Record the oracle.** Add `tools/website-oracle/<family>-goldens.mjs`, importing the
   unmodified Website kernel through `oracle.mjs` and covering every branch and edge the
   ticket lists. Run `scripts/record-website-goldens.sh`. Done when the fixture exists under
   `crates/sim/tests/fixtures/website/` and every listed branch has records.
3. **Port the kernel** into `crates/sim` with the reference's names, a module doc naming the
   source, and unit tests with hand-derived numbers.
4. **Replay the fixture.** Append one `#[test]` to `crates/sim/tests/website_goldens.rs` that
   replays every record with `assert_eq!`; trigonometry alone may use `TRIG_TOLERANCE`. Done
   when it passes against the fixture as first recorded.
5. **Wire it into `crates/web`** when the ticket draws something. Rules stay in `sim`; `web`
   fetches, reads input, ticks and draws.
6. **Run the gates**, and the smoke when the web crate changed.
7. **Update** the README's "what the browser shows" and "not ported" lines, and your ticket's
   status in `docs/WORK-QUEUE.md`.

## Shared files

Several tickets touch these. Append in place, keep existing order, and on a merge conflict
keep both sides:

- `crates/sim/src/lib.rs`, `crates/sim/src/hub.rs`: module lists stay alphabetical.
- `crates/sim/tests/website_goldens.rs`: one new `#[test]` with its serde structs, at the end.
- `crates/web/src/lib.rs`: one call added to `tick` or `render` at the position
  [ARCHITECTURE.md](docs/ARCHITECTURE.md) gives for that layer.
- `tools/browser-smoke/smoke.cjs`: a new check after the existing ones.
- `README.md`, `docs/WORK-QUEUE.md`: your ticket's lines only.

## Report

When you stop, state the ticket; the files added and changed; each gate with its result as
run; the fixture's name, record count and coverage; what the reference left unanswered; and
what remains. Name any check you did not run.
