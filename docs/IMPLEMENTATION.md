# Implementation map

This map connects game behavior to the owning Rust modules and available checks.
Paths under `src/` are relative to `crates/sim/` unless stated otherwise.
The original C++ game supplies retail evidence; fixture provenance is documented
in [Reference data](REFERENCE-DATA.md).

## Simulation

| Behavior | Owner | Verification |
| --- | --- | --- |
| Random streams and numeric evaluation | `src/rng.rs`, `random_domain.rs`, `float.rs` | Native recordings, numeric bit tests, and regression replays |
| College admission, rooms, NPCs, dialogue, and services | `src/hub/`, `profile/`, `session/` | Room/economy fixtures and browser service/input checks |
| Character creation, skill progression, and retirement | `src/profile/` | Loadout/progression tests, saved continuations, and Create checks |
| Three eight-slot belt banks | `src/belt.rs`, session inventory/input owners | Belt, binding, bank, and input-admission regressions |
| Inventory, Sacks, Dowsing, dye, and skill books | `src/economy/`, `player_panels/`, `session/dye.rs` | Inventory/economy fixtures and service/panel journeys |
| Scene loading, collision, navigation, waves, and loot | `src/boneyard/`, `loot/`, `solomon-assets` | Scene, route, world, restore, and loot replays |
| Enemy lifecycle and attacks | `src/enemies/` | Complete enemy-program, contact, membership, and presentation recordings |
| Elemental and welded primary spells | `src/spells/`, `session/` | Cast, contact, lifetime, and independently evolving world replays |
| Secondary spells, summons, status, and death effects | `src/secondary/`, session/world owners | Lifetime, contact, quality, and checkpoint continuation tests |
| Player rescue, damage, poison, and Reverie | `src/player_rescue.rs`, `session/rescue.rs`, combat owners | Contact, damage, rescue, and session control tests |
| Tutorial stages, narration clocks, and rewards | `src/boneyard/tutorial/` | Progression, safety, camera, and reward tests |
| Learned bots | `src/ml_bot/` | Retained-model inference, observer, and action regressions |
| Authored definitions, Lua, and mods | `src/modding/`, `solomon-modding` | Compiler/runtime fixtures, admitted content tests, and CLI checks |

## Presentation and interaction

| Behavior | Owner | Verification |
| --- | --- | --- |
| World drawing, painter order, lights, shadows, and weather | `crates/web/src/` world views and simulation presentation plans | Geometry/material fixtures, GPU checks, and browser captures |
| Fractional display state | `crates/web/src/presentation.rs`, protocol | Timeline, interpolation, endpoint, and draw-cadence checks |
| Menus, HUD, Inventory, Skillbook, and overlays | Web feature controllers and shared GPU UI | Layout/text/input regressions and browser journeys |
| Keyboard, mouse, gamepad, and touch | `crates/web/src/input.rs` and navigation/controllers | Input fixtures and ordinary browser events |
| Sound, music, and narration playback | `sim::audio` and web audio owners | Catalog/event tests, actual playback, and reload checks |
| Save/Continue and local preferences | Web storage and native persistence owners | Validation, revision conflict, restore, and packaged continuation checks |

[GPU UI](GPU-UI.md) defines layer, input, text, and accessibility contracts.
[Client presentation](CLIENT-PRESENTATION.md) defines display ownership.

Book-rank feedback consumes an accepted economy result. Eligible rank awards close
Inventory and retain pause/input ownership until confirmation. Other book outcomes
play their audio without inventing a rank confirmation. Session/world replacement,
player departure, Game Over, or returning to loadout clears the feedback.
World floating text has a separate presentation owner for the same accepted result.

Dialogue focus follows the active speech, choice, or selector identity, including
changes within one input timestamp. HUD and book geometry clip in their transformed
design coordinates. These contracts belong to the shared UI owners rather than
scene-specific offsets or timers.

## Saves and service integration

| Boundary | Owner | Verification |
| --- | --- | --- |
| Browser documents, supported migrations, native containers, portable ZIP | `solomon-assets` save codecs | Continuation, rejection, native-container, and round-trip tests |
| Account import/export | `solomon-save-tools` and website Account UI | Portability tests, browser File ABI, and mounted Account integration |
| Native authority and durable save receipts | `solomon-launcher` | Transport, host command, persistence, and socket integration tests |
| Hosted accounts, content, and party directory | Website services plus Rust supervisor adapters | API/signature fixtures and mounted browser journeys |
| Score, Memorial, and run publishing | Native publishers plus website APIs | Signed publication and archive/storage tests |
| Editor Test/Return | Website editor and disposable Rust authority | Handoff, cancellation, authored-scene, and return checks |
| Website artifact | Packaging scripts and website artifact adapter | Complete manifest/hash validation and installed identity checks |
| Desktop app and direct peers | Desktop shell, Rust authority, GPU client | Archive inspection and per-platform packaged journeys |

Save schema and player-data compatibility remain supported regardless of the
recording's origin. `solomon-assets::website_save` is the existing browser-document
codec name; it is part of the Rust implementation.

Local tests, browser checks, package qualification, and live installation have
different scopes. [Release](RELEASE.md) records the qualified artifact and the
checks needed at each boundary.
