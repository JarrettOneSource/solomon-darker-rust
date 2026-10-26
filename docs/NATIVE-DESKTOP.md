# Rust game, native desktop and browser embedding

The goal is one coherent Rust game, with explicit ownership of gameplay, GPU UI,
platform operations and application lifetimes. The browser embeds its WASM build;
standalone packages run the same game in an OS window. Remaining browser-shaped
scaffolding, duplicated behavior and superseded runtime paths must be removed
across callers, tests, packaging and documentation.

The requested desktop application owns a native game window. Windows runs a Rust
executable; macOS and Linux run the same native client. The Website continues to
run Rust compiled to WASM. Electron, Chromium, Node and a JavaScript engine are
not desktop runtime dependencies. Opening a system browser is not the desktop
implementation.

The accepted starting product is Rust `92e3fe4c`, documented on main `8f120aea`.
Its gameplay and GPU UI are Rust, but its desktop renderer and platform services
currently run inside Electron. This migration is in progress; the existing
release remains the qualified product until the checks below pass.

## Ownership and cutover

- Keep one canonical `GameSession`, presentation timeline, world painter and GPU
  UI implementation. Desktop and web compile those same Rust owners.
- Isolate graphics context and image decoding in `solomon-graphics`; preserve
  the existing blend, clipping, sampler, filter and resource-lifetime contracts.
- Separate native and browser event, text/IME, clipboard, accessibility, audio,
  file, storage, HTTP and socket operations from shared client behavior. Native
  code must not emulate a DOM or execute the WASM client in an embedded browser.
- Replace the Electron process/preload and Node peer transport with native
  window, session, save-before-close, encrypted peer and update ownership.
- Preserve schema51 saves, import/export, revision-conflict choices, offline
  play, peer invitations and optional update behavior. Make the existing
  Electron save migration explicit before removing the old packaging.
- Cut desktop packaging, commands and CI over together. Keep the separately
  built Website artifact and its deployment interface compatible.

## Execution and acceptance

- Current foundation evidence: the shared renderer and native asset tests compile
  and pass on M2; the main client and loader still compile for WASM. The actual
  `graphics_probe` opens a winit/glutin window, loads the stock 2048x2048 College
  atlas, renders through the shared shader and verifies its GPU readback. The
  corrected probe exits zero on Apple M2 (`4.1 Metal - 90.5`, graphics error zero).
  This is graphics-foundation evidence, not complete native gameplay acceptance.
- Native text engine tests pass for stock WOFF2, kerning, ligatures, combining
  marks, bidirectional clusters, actual raster pixels and invalid fonts. Shared
  font-run wrapping, tab stops, caches and GPU submission now select browser or
  native metrics/raster operations. Strict WASM client/loader checks pass after
  this separation. The actual native UI probe now renders those font runs, with
  corrected synthetic italic and blank tab rasterization. Its AccessKit adapter
  attaches before macOS first shows the window. OS typing, select-all, clipboard
  paste, undo and redo pass through the actual native editor; accessibility exposes
  the field value and focus. IME commit/cancellation and bidirectional navigation
  have focused regression coverage. Native accessibility SetValue and macOS dead-key composition also pass in the
  live window; broader screen-reader and OS IME acceptance remains pending.
- Gameplay keyboard/mouse rules, pointer capture state and hotbar/mod/cheat
  shortcuts are shared; the native adapter feeds winit and standard gamepad events.
- The audio director is shared over WebAudio and Rodio/CPAL. The M2 native probe
  loads 227 PCM buffers and six resident music tracks, advances the real device
  clock, and exercises delayed sound, owned loops, narration mixing, music change
  and pause before clean exit. CPU sample/timing tests and all 187 client tests
  pass on M5; strict WASM client/loader checks pass. Ogg Opus compatibility is covered by the focused tests described below.
- Native storage now preserves the complete schema51 document across reopening,
  rejects stale writers and retains existing files after malformed-input or write
  failure. Preferences and playtime share its locked atomic document owner with
  host checkpoints. Those CPU tests are separate from save-before-close and legacy
  Electron migration acceptance.
- Shared menus now select platform file dialogs, fullscreen, clipboard and mobile
  layout storage. Native locale metadata reuses ICU4X; browser Intl is retained.
  The actual M2 probe passes file export/re-import, clipboard copy and fullscreen
  entry/exit, with GPU error zero and clean exit. The symbol fallback regression is fixed and the subsequent live screenshot
  shows the checkmark and color emoji correctly; its window also exits cleanly.
- Native HTTP status/headers/body limits, verified content download, corrupt-cache
  replacement, offline cache reopening and resource retirement pass real local
  server tests. Strict WASM checks still pass after this shared-controller cutover.
- The actual native `game_probe` now constructs the complete shared Game, owns a
  loopback authority and uses native WebSockets. On M2, ordinary character creation,
  College, inventory and Boneyard entry render in the OS window. Closing saves the
  full schema51 document before exit; a fresh process reopens the same wizard,
  loadout and Boneyard, then saves and exits again. GPU error is zero. Receipts are
  in `native-game-r1` and `native-game-r2` under the task evidence root. Sustained
  movement, the full desktop owner and close-failure choices remain unqualified.
- Native socket tests cover real handshakes, Origin, chunked Welcome/save transfer,
  latest snapshot delivery, background pings, malformed frames, shutdown and
  resume ownership. Browser sockets retain the same shared protocol controller.
  Strict WASM checks pass after full native Game construction. Encrypted peer
  hosting/joining and production desktop lifecycle are separate remaining work.

- The native desktop dependency graph contains no wasm-bindgen/js-sys/web-sys
  runtime dependency. Browser editor iframe and IndexedDB authoring paths compile
  only for WASM. The standalone desktop continues to open Website authoring externally.
- `solomon-darker-app` now owns native launcher/game windows, asynchronous startup,
  single-profile ownership/focus, save-before-close choices and optional updates.
  Its peer service preserves v1 invites, HKDF-SHA256 and TLS 1.3 AES-128-GCM through
  vendored OpenSSL and Tokio. These application and peer paths are under validation;
  they are not yet qualified packages. The published product remains unchanged.

- The native application passes strict desktop/client lint and 204 unit tests;
  strict WASM client/loader lint also passes. TLS transport passes native pairs
  and both directions against the previous Node transport with 2 MiB exchanges.
  Actual M2 application checks pass launcher/Solo, Tutorial rendering, College,
  second-launch focus, Cmd-Q save/exit, and save/return to a new launcher window.
  A real permission failure preserves the previous save bytes and keeps the game
  alive; Keep playing cancels Cocoa Quit and a subsequent save succeeds. Two native
  apps reach the shared College and Boneyard. The client leader guard (present in
  Website HubScene) now disables the guest map control; the leader starts both
  clients. Both native windows save and exit zero, and their persisted documents
  identify the same Boneyard run with their respective wizard/loadout. The receipt
  is `native-peer-app-r2/peer-boneyard.json`, with both reviewed screenshots. The
  existing fresh-host-only saved-world admission rule remains unchanged (Website
  `frontend/src/game/host/game-host.ts:1120`); a second saved world cannot overwrite
  an active peer host. Its previously obscured rejection notice now paints above
  the title, verified in the rebuilt app (`native-app-r9/guest-rejected-resume.png`).
  Previous-app export/import passes with the real 92e3fe4c release: LegacyAster
  retains player progression, inventory, equipment, skills and position, continues
  in the native app and reopens in a fresh process. Both native processes exit zero.
  Existing Hub restoration rebuilds ambient NPC state and attaches the verified
  ZIP nativeSource; the whole JSON document is not claimed identical. The receipt
  is `legacy-app/import-receipt.json`.
  Current receipts: `native-app-r1`, `native-peer-app-r1`, `peer-interop.json`,
  `native-app-r8.log`, `native-app-peer-r2-build.log` under the task evidence root.

- Native Opus decoding passes nine focused audio tests and strict desktop/client
  lint. Actual encoded fixtures cover mono/stereo/5.1, multi-page timing, large
  pre-skip, header gain, chained streams, repeated playback and damaged content.
  Eleven surround frame pairs match an independent float FFmpeg/libopus decode.
  Music retains compressed input and bounded decoding buffers. Receipts are in
  `native-opus-r3.log`; this does not claim a packaged Opus listening test.
  The actual Close without saving choice also exits zero after a save-directory
  permission failure, retaining the exact preceding save bytes. Its screenshot,
  receipt and restored permission check are in `native-app-r9`.

- The actual M2 app also accepts a user-held D key for 2.613 seconds across
  313 game frames, moves the wizard right and saves on Cmd-Q. The receipt and
  reviewed screenshot are `native-app-r9/held-key-movement.json` and
  `held-key-after.png`. Temporary input tracing has been removed.
- Native package staging and final ZIP inspection pass 17 focused build/archive
  and retained browser bootstrap tests. The native manifest hashes every file;
  the gate rejects stale revisions, changed assets, corrupted ZIP entries and
  undeclared or legacy runtime payloads. Four-platform execution remains pending.

- [ ] Shared game state and application ownership use clear Rust interfaces, with
  browser-specific behavior confined to the browser platform and embedding.
- [x] Shared graphics and asset loading compile for native and WASM.
- [ ] Shared game and GPU UI run in a native window with native input, text,
  audio, clipboard, accessibility and persistence.
- [ ] Ordinary Solo/Create/Tutorial/College/Boneyard, settings, inventory,
  Console/mods and save/import/export behavior passes native acceptance.
- [ ] Closing or returning saves before disposal; failure retains the game and
  offers the established choices. Reopening continues the same wizard.
- [x] Two native apps connect through authenticated encrypted peer transport,
  share a Boneyard and close without losing saved state.
- [ ] Existing Electron user data has a verified migration or supported import
  route; fresh installation and revision conflicts are covered separately.
- [ ] Optional update check/download works and network failure leaves offline
  play available.
- [ ] Windows x64, Linux x64 and both Mac architectures package only their
  required native runtime/assets, with actual platform execution receipts.
- [ ] Native and WASM formatting, linting, tests, assets and browser gates pass;
  actual browser gameplay remains accepted after the shared-code changes.
- [ ] Review confirms no retained Electron/Chromium/Node desktop runtime path,
  dropped feature, placeholder implementation or fabricated acceptance state.
- [ ] Publication, installed/live status and task-owned worktree/process cleanup
  are recorded separately from implementation and local validation.

Performance follow-ups in `PERFORMANCE.md` remain explicit. This migration does
not turn the earlier 60 Hz M2 measurements into native or universal benchmarks.
