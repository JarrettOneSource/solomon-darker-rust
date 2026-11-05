# Standalone cutover

This is the plan for retiring the PixiJS client and making this repository the
only Solomon Darker game. Read it before working on the cutover, and keep the
status column current as work lands. It describes where things stand now; Git
history holds how they got there.

## Goal

Solomon Darker has two maintained clients today. The PixiJS client, which lives in
the Website repository, serves `/game`. This Rust client serves `/game-rust`.
Players are still active on PixiJS, so both stay online until Rust is ready.

When Rust passes the quality checks in [the cutover checklist](#cutover-checklist),
it moves to `/game` and PixiJS retires. Until then Rust stays at `/game-rust`.

The Rust client should be:

- **Standalone.** Nothing in this repository needs the TypeScript game to build,
  test, run or make sense.
- **Easy to read.** Plain, direct Rust that a new contributor can follow. Clear
  comments that explain why, written for someone who never saw the PixiJS code.
- **Maintainable for the long term.** One owner for each rule, typed data, and no
  layers that exist only because of how the port happened.
- **Native on desktop as well as the browser.** The desktop app is a standalone
  executable built from the same code.

## Decisions

These were agreed with the project owner on 2026-11-04. Each records the reason so
later work can apply it to cases not listed here.

**Retail wins.** When PixiJS and the original C++ game (Solomon Dark 0.72.5)
behave differently, match the original game. PixiJS was itself a port; once it
retires it is not a reference for anything. Existing recordings keep their
recorded expectations, but a PixiJS-only quirk is not a reason to keep behavior.
[Reference data](REFERENCE-DATA.md) describes the evidence rules.

This rule covers accidental differences. Deliberate product changes that the
owner approved, such as the Inventory navigation policy in
[Decisions](DECISIONS.md#optional-inventory-and-skills-navigation), are design
choices rather than PixiJS quirks. They stay unless the owner reverses them, and
their records should name the decision rather than the Website ledger.

**The leftover test.** For every piece of code, data or tooling that came from the
PixiJS era, ask: *would we have written this if the TypeScript game never
existed?* If yes, keep it and describe it on its own terms. If no, remove it.
Three outcomes are common:

- *Keep and rename.* The deterministic math in `solomon-sim::float` is needed
  regardless, because native and WebAssembly `libm` can disagree in the last
  bit. It is an fdlibm port; describe it as the game's deterministic math, not as
  an imitation of a browser engine.
- *Keep while a live contract needs it.* `ryu-js` number formatting stays while the
  Website verifies mod package digests in JavaScript. The save interchange format
  stays while players move saves between the two clients.
- *Remove.* PixiJS presentation quirks (word wrap, blend modes, JavaScript string
  length rules), comments that cite TypeScript files and functions, and tools that
  need a Website checkout.

**PixiJS gets critical fixes only.** Crashes, save loss and exploits are fixed in
both clients. Everything else is fixed in Rust only. Work already scoped by an
active report campaign finishes under that campaign's plan; new work follows this
rule.

**Storage follows performance evidence.** Entity storage moved to `bevy_ecs` in
October 2026 to gain structure and, it was hoped, speed. The measurements in
[Performance](PERFORMANCE.md#entity-storage) showed the opposite, although its
Boneyard cases never reached combat: simulation ticks became 3–18% slower and
session copies 2–7× slower, with no workload faster.
The game used `bevy_ecs` only as storage (no systems or schedules), and the
wrapper types that adapted it added most of the hard-to-read syntax in
`solomon-sim`. Storage is now plain `Vec`, `Option` and `BTreeMap` collections
again, and the benchmark, now including casting combat, shows ticks 1–12% faster
and session copies 2.1–6.3× faster than with `bevy_ecs`. A storage change is
accepted only when the session benchmark shows no regression and every final
checkpoint stays byte-identical, which also proves painter, contact and
random-number order are unchanged. Its Boneyard cases must reach combat, which
stays locked until Solomon's entrance finishes.

**Simple Rust.** Prefer code a newcomer can read over code that is short or
general. [Coding standards](CODING-STANDARDS.md#simple-rust) lists the concrete
rules. Apply them to code you touch rather than in one large rewrite, except where
this plan calls for a dedicated pass.

## How to work on this

- Use a task worktree and branch. The cutover branch is
  `cutover/standalone-20261104`.
- The branch starts from the report campaign's integration branch, because that
  branch carries fixes that main will receive when its pull request merges. Do not
  merge cutover work into main before that pull request lands; its release
  qualification depends on main staying put. After it lands, merge or rebase onto
  main.
- Large mechanical changes, such as renames, conflict with every open branch. Do
  them as one focused commit per rename, ideally generated by a script you can
  rerun after rebasing.
- Every step keeps the full game gate in [AGENTS.md](../AGENTS.md#checks) green.
  Changes to simulation storage or order also need the session benchmark and
  byte-identical checkpoints described above.
- Recorded fixture expectations do not change to make a refactor pass. A
  behavior change toward retail needs its own regression and named evidence.

## Hitlist

Status values: **Done**, **In progress**, **Next**, **Not started**, **Blocked**
(with the reason), **Won't do** (with the reason).

### Before the cutover

These must be finished before Rust moves to `/game`.

| # | Item | Status | Notes |
| --- | --- | --- | --- |
| 1 | PixiJS critical-fix-only policy | Done | Recorded above and in the device wiki for future campaigns. |
| 2 | Cutover checklist (what "passes QC" means) | Not started | See [Cutover checklist](#cutover-checklist). Needs owner review before use. |
| 3 | Rebuild boneyard art without PixiJS | Not started | `web/assets/stock/boneyard/images` holds 4,886 PNGs (424 MB) baked by the Website's PixiJS painter through `tools/export-boneyard-art.mjs`. Nobody can regenerate them once PixiJS retires. Draw boneyards with the Rust renderer from source sprites, or compile them with a Rust tool. Also cuts most of the ~600 MB release download. |
| 4 | Move PixiJS players' saves | Not started | Cloud saves use the shared save document that `solomon-assets::website_save` reads; test real production documents. Browser-local PixiJS saves need a one-time import. |
| 5 | Browser storage keys independent of the URL | Not started | `WebsiteMount::storage_key` (`crates/web/src/hosted.rs`) appends the Website mount to every key. The Website's `/game-rust` remount sets that mount, so Rust's guest saves live in `solomon-dark-game-saves:/game-rust` while PixiJS owns the unsuffixed names. Moving Rust to `/game` as built today would orphan its data and read PixiJS's. Have the deployment supply a storage namespace separate from the URL path (staging at `/game-stg` keeps its own), and copy the suffixed keys once at the swap. Coordinate with the Website's guest-save migration and item 8. |
| 6 | Mount-independent build | Not started | The hosted build is made for `/game` and the Website's `build-parallel-apps.mjs` rewrites it for `/game-rust`. Relative asset URLs remove the rewrite. |
| 7 | Community content in Rust | Not started | Published boneyards and mods load, and the Website editor's test-play uses Rust. |
| 8 | Swap plan | Not started | Rust to `/game`, PixiJS to `/game-legacy` with a banner for a few weeks, `/game-rust` redirects to `/game`. Needs Website and Caddy changes. |
| 9 | README play link | Done | It pointed at `/game/` (PixiJS); it now points at `/game-rust/` until the swap. |

### Standalone: no PixiJS baggage

| # | Item | Status | Notes |
| --- | --- | --- | --- |
| 10 | Tools that need a Website checkout | Not started | `tools/website-oracle`, `tools/export-boneyard-art.mjs`, `tools/export-boneyard-catalog.mjs`, `tools/browser-smoke`, `scripts/record-website-goldens.sh`, `scripts/sync-native-ui-fonts.py`, `scripts/sync-player-portrait-assets.py`, `scripts/sync-stock-assets.sh`. Keep what they produced as tracked source; remove or replace the tools. |
| 11 | Neutral names for frozen fixtures | Won't do | The `website` fixture directories and `website_goldens` tests are named for where the recordings came from. [Reference data](REFERENCE-DATA.md#recording-provenance) keeps those names so the evidence stays attributable, and already treats them as frozen regression data rather than a target. |
| 12 | PixiJS behavior quirks | Not started | Match retail: PixiJS word wrap (`ui/sack_name.rs`), blend modes (`graphics`), overlay order (`painter/overlays.rs`), and the UTF-16 length check in `menu/create_name.rs`. Each needs retail evidence. |
| 13 | Comments that cite TypeScript | Not started | Replace with what the code does and why, citing the original game where a rule needs a source. Then add a CI check that rejects `Pixi`, `.ts` and `Website` references in `crates/` outside named Website interop points. |
| 14 | Save schema ownership | Not started | `solomon-assets::website_save` mirrors the Website's TypeScript save schema. After PixiJS retires, Rust owns the format; the Website stores documents and validates with the `save-tools` WebAssembly build if it needs to. |

### Readability

| # | Item | Status | Notes |
| --- | --- | --- | --- |
| 15 | Replace `bevy_ecs` storage with plain collections | Done | Every population is a `Vec`, `Option` or `BTreeMap` of plain records, and `bevy_ecs` is no longer a dependency. Checkpoints stay byte-identical; [Performance](PERFORMANCE.md#entity-storage) has the measurements. |
| 16 | `Game` as explicit states | Not started | `crates/web/src/game.rs` has an ~80-field struct with flags such as `has_game`, `world_loading`, `save_requested` and `save_in_flight`. Enums make impossible combinations unrepresentable. The file also repeats the local participant lookup with `.expect("participant exists")` about 50 times; one accessor removes that. |
| 17 | Integer codes to enums | Not started | `native_weld_primary_cast_mode(build_id: u32)` and the loot `range_mode: u32` panic on unknown values. Parse into enums when content loads. Weld build IDs 1000–1009 are matched in about 60 places and saved as numbers, so the enum needs serde that keeps the number. Give player IDs a `PlayerId` type. |
| 18 | Port-era names | Not started | The `native_` prefix (about 1,300 items) meant "matches the original game" and now reads as "desktop". Drop it, along with `kernel`, `Website` and `current_` in names. Rename the `solomon-web` crate, which also runs the desktop app, to `solomon-client`. |
| 19 | Deterministic math and `fround` | Not started | Describe `float` as the game's fdlibm math. Where the 157 `fround` calls mirror 32-bit fields in the original game, consider storing those fields as `f32`. |
| 20 | Simple Rust rules | Done | [Coding standards](CODING-STANDARDS.md#simple-rust). |

### Desktop

| # | Item | Status | Notes |
| --- | --- | --- | --- |
| 21 | Code signing and notarization | Blocked | Needs an Apple Developer ID and a Windows signing certificate from the owner. Release notes currently say the builds are unsigned. |
| 22 | Update path | Not started | An update check, or distribution through itch.io or Steam. |
| 23 | Protocol version handshake | Not started | Old desktop builds should fail clearly against a newer host. |
| 24 | Accounts and cloud saves on desktop | Not started | Owner decision. Desktop currently plays offline and points players to the web app for cloud features. |

### Tooling and repository

| # | Item | Status | Notes |
| --- | --- | --- | --- |
| 25 | Mod CLI in Rust | Not started | `tools/sdmod/cli.mjs` is about 730 lines of Node wrapping the `solomon-sdmod` binary. Make the binary the command-line tool. |
| 26 | `cargo xtask` | Not started | Replace the Python and shell packaging and verification scripts so contributors need only Rust and Trunk. Node stays for Playwright browser tests. |
| 27 | Repository size | Not started | The Git pack is about 614 MiB, mostly PNG. Stop committing generated art once item 3 lands. Rewriting history is a separate owner decision. |
| 28 | Local spell journey | Not started | No repository journey casts primary spells locally: `electric-burn.cjs` and the other combat journeys need the hosted staging fixtures. Against the native host, `solomon-entrance.cjs` walks a wizard into Solomon, which starts combat, so a local journey can cast every school. Weld builds need a supported local grant, because Create does not offer them and the developer Console runs only on hosted servers. |

## Cutover checklist

Draft. Rust moves to `/game` only when all of these hold:

- Every item in [Before the cutover](#before-the-cutover) is done.
- Feature parity with PixiJS for everything players use: each mode, menu, spell,
  item, enemy, boss, boneyard and multiplayer flow, with differences resolved in
  favor of retail.
- Real production saves load, play and save again, from both cloud and browser
  storage.
- Published community boneyards and mods load; the editor's test-play uses Rust.
- Supported browsers pass the browser journeys; the desktop packages pass their
  platform journeys.
- Performance is at least as good as PixiJS on the workloads in
  [Performance](PERFORMANCE.md).
- A public period on `/game-rust` ends with no open serious Rust-only bugs.

## Evidence

- Entity storage cost: [Performance](PERFORMANCE.md#entity-storage).
- Numeric contract and why the math is custom: [Decisions](DECISIONS.md#numeric-contract).
- Fixture provenance: [Reference data](REFERENCE-DATA.md).
- Release download (about 600 MB) by type: PNG 490 MB, WAV 64 MB, MP3 29 MB.
