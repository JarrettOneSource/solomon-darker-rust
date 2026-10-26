# Native desktop qualification

The source now runs one shared Rust Game and GPU UI through native OS and browser
WASM entry points. Native application lifetime and packaging are described in
[Desktop runtime](DESKTOP-RUNTIME.md); [Architecture](ARCHITECTURE.md) and
[Decisions](DECISIONS.md) own the implementation contract.

The released artifact remains the qualified revision in [Release](RELEASE.md)
until the native candidate completes these checks. Implementation is not release
or deployment acceptance.

## Required acceptance

- [x] Shared graphics, fonts, input, text editing, clipboard, files, locale, audio,
  storage, HTTP and socket implementations compile for their native/WASM targets.
- [x] Actual Mac native play covers character creation, Tutorial, College,
  Inventory, Boneyard, held movement and fresh-process continuation.
- [x] Native close/return waits for persistence; real permission failure retains
  the game and save bytes, and both failure choices have been exercised.
- [x] Two actual native apps share a Boneyard through authenticated encrypted
  transport, preserve their own wizard and save before clean exit.
- [x] A real previous-release export imports into a fresh native profile and
  continues in subsequent native processes with its supported player state.
- [x] Native archive staging and integrity checks reject stale revisions,
  corrupted or undeclared files, changed assets and retired runtime payloads.
- [ ] Final Windows x64, Linux x64, Intel Mac and Apple silicon Mac packages pass
  the ordinary OS-input journey and screenshot/error/cleanup review.
- [x] Native Console/mods and optional update presentation/browser handoff are exercised;
  failed update access leaves offline play available.
- [ ] Final source passes formatting, strict native/WASM lint, workspace tests,
  asset validation, browser-support checks and actual browser journeys, including
  Windows browser gameplay and settings/persistence.
- [ ] Final review resolves defects in affected callers, data compatibility,
  failure paths, dependencies, packaging and current documentation.
- [ ] Publication, installed/live acceptance and removal of task-owned worktrees,
  build caches and processes are recorded by their owning workflows.

Functional software-renderer checks do not establish native performance across
hardware. [Performance](PERFORMANCE.md) retains the measured scope and follow-ups.
