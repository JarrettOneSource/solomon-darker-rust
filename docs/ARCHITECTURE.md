# Architecture

Solomon Darker shares one Rust simulation between its native authority and browser
client. The website supplies accounts, storage, content publication, and editing.

## Ownership

| Module | Responsibility |
| --- | --- |
| `solomon-sim` | Game rules, sessions, players, enemies, spells, inventory, and deterministic state |
| `solomon-assets` | Sprite bundles, scene decoding, navigation inputs, save formats, and validation |
| `solomon-graphics` | Shared WebGL2 rendering, textures, blend modes, and asset decoding |
| `solomon-web` | Browser input, GPU UI, world views, audio, and platform adapters |
| `solomon-loader` | Startup splash using the shared graphics code |
| `solomon-protocol` | Versioned messages, snapshots, commands, and acknowledgements |
| `solomon-launcher` | Native authority, HTTP/WebSocket transport, persistence, and service integration |
| `solomon-modding` | Definition compiler, native Lua execution, and authoring CLI |
| `solomon-save-tools` | Independent WebAssembly entry for Account save import/export |
| `desktop/` | Desktop shell, native process ownership, peer transport, and updates |

A rule that changes what the game does belongs in `sim`. Asset or save decoding
belongs in `assets`. Drawing and platform I/O belong at the edge. Browser and
native hosts share the same session and scene types.

## Session and frame flow

`GameSession` owns participants, world state, inventory, run lifecycle, and random
streams. Its ordered tick composes movement, contacts, casts, world effects, loot,
waves, and rewards. Actor insertion and random consumption are part of that contract.

The native authority runs at 100 Hz and publishes snapshots at up to 20 Hz.
Static browser mode advances the same rules locally with a 250 ms catch-up cap.

For each browser frame:

1. Receive and validate host messages; retain their original receipt timestamps.
2. Apply completed authoritative state and sample input once.
3. Sample one display frame and project aim through its camera.
4. Commit admitted UI commands against accepted state.
5. Draw world geometry, actors, lights, and UI from their owning models.
6. Drain accepted audio events and retire unused GPU resources.

`crates/web/src/lib.rs` owns browser entry points.
`crates/web/src/game.rs` composes the running game.
[Client presentation](CLIENT-PRESENTATION.md) defines interpolation and prediction.

## Rendering and UI

World actors have a stable registration ordinal. The painter planner orders actors
by row, manager lane, and ordinal; fixed depth bands place scenery and effects around
that queue. Art records supply the sprite; simulation tables supply ordering rules.

Game menus, HUD, overlays, and the desktop launcher are Rust GPU UI. Egui handles
interaction/layout and tessellation. The shared graphics code submits stock sprites,
bitmap text, shaped font runs, and meshes.

One application layer order controls painting, hit testing, modal ownership, and
controller scope. UI controllers borrow inventory, social, and session models and
emit typed actions for their owners to apply.
[GPU UI](GPU-UI.md) describes the interaction and platform boundaries.

Immutable scenes, collision primitives, skill definitions, and unchanged scenery
target tables share `Arc` storage. Dynamic gate segments and mutable display actors
remain private.

## Assets and audio

Tracked assets include stock atlases, bundle records, scenes, fonts, and audio.
`scripts/verify-client-assets.py` validates complete membership and SHA256 values.
[Reference data](REFERENCE-DATA.md) owns input provenance.

`sim::audio` defines cues and event rules. Browser `GameAudio` owns decoded buffers,
loops, and music channels. Trusted user input resumes the AudioContext. Accepted
actor events supply sounds.
Narration duration remains simulation state even when sound is muted.

## Transport and persistence

`solomon-protocol` carries canonical values between matched host/client builds.
Snapshot publications are replaceable; commands, accepted audio, save receipts,
and acknowledgements have their own reliable ordering.

Snapshot compaction runs after each connection selects its next publication.
The first packet is complete; later packets may contain lossless changes against
the last document actually written. The receiver reconstructs every packet before
coalescing display state. Unsent replacements cannot become baselines.

The host owns connection generations, admission credentials, input edges,
readiness, pause/resume holds, and durable save receipts. The session owns rules.
A checkpoint writer performs encoding and atomic synced replacement away from
the tick deadline. Welcome and SaveCommitted wait for their required disk writes.

Static asset transfer workers retain connection leases and check shutdown between
nonblocking writes. Slow readers cannot occupy the request-framing pool.
[HTTP media](NATIVE-HTTP-MEDIA.md) defines that contract.

## Website integration

A verified game artifact supplies the native host, mounted WebAssembly client,
editor entry, and independent save-tools module. The website validates the manifest,
file membership, and hashes before installation.

Website APIs own accounts, cloud documents, community content, party discovery,
and editing. Game revision/assets and website API/storage mounts are independent.
Direct standalone play retains local saves and peer transport without those services.

The website editor hands Test a disposable authority and restores the editor on
Return. The Account initializes save tools without starting a game, renderer, or
socket. See [Implementation](IMPLEMENTATION.md) for the feature map.

## Authored content and diagnostics

Compiled definitions, admitted asset descriptors, and checkpoints are plain values
in `sim::modding`. Native Lua VMs and executable handles belong to
`solomon-modding`; they are reconstructed from admitted content on restore.

Run archives are private operator diagnostics, separate from player saves.
They serialize complete retained states on an owned worker.
[Run archives](RUN-ARCHIVES.md) defines their format, permissions, and retention.

## Verification

Unit tests exercise rules and failure paths. Regression replays use tracked
recordings, and browser/package journeys check composed behavior.
[AGENTS.md](../AGENTS.md#checks) lists the game gates;
[Release](RELEASE.md) separates source validation, artifacts, installation,
and live acceptance.
