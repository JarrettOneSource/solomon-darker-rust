# Solomon Dark — Rust / WebAssembly rebuild

A Rust rebuild of the Solomon Darker web game: deterministic simulation kernels, a reader for
the stock sprite bundles, and a WebGL2 browser client compiled to WebAssembly. The Website
current behavior is the reference. During the cutover, `../website-rust-current` holds that
reference and `../Website` retains earlier evidence. This standalone repository owns the
game, its native host, WASM client, save tools and desktop packages. The Website owns
accounts, content and editing, and embeds the built WASM artifact.

[The source inventory](docs/SOURCE-PARITY.md) maps current behavior to its implementation
and verification. [The acceptance record](docs/PORT-COMPLETION.md) records exact tested
revisions, release artifacts and deployment status.

## Documents

- [AGENTS.md](AGENTS.md): instructions for coding agents: read order, sources, gates, the
  porting loop, shared-file etiquette.
- [CONTEXT.md](CONTEXT.md): the glossary.
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): crates and seams, the frame loop, painter
  order and depth, verification layers, where a feature goes.
- [docs/CODING-STANDARDS.md](docs/CODING-STANDARDS.md): how code here is written, with
  reasons and examples.
- [docs/DECISIONS.md](docs/DECISIONS.md): choices that would otherwise be reversed, and why.
- [docs/WORK-QUEUE.md](docs/WORK-QUEUE.md): tickets for parallel agents and what is done.
- [docs/SOURCE-PARITY.md](docs/SOURCE-PARITY.md): current Website behavior, implementation
  ownership and verification boundaries.

## Layout

| Path | Crate | Owns |
| --- | --- | --- |
| `crates/sim` | `solomon-sim` | Pure kernels with no browser dependency: native lagged-Fibonacci RNG and the semantic-word random domain, f32-exact float helpers (`fround`, V8-compatible `hypot`), headings, player/enemy movement, segment-sweep collision, the Hub courtyard (spawn, camera, segments, wizard tick), painter order and the fixed-actor tables, courtyard ambient life (fountain spray, seal pulse, statue bob, teacher clock), the teacher cycle and burst, Hub regions with portals and scripted transitions, starter equipment tints, wizard layer offsets. |
| `crates/assets` | `solomon-assets` | Stock `.bundle` records, retained Boneyard catalog, identity-checked navigation and the shared save-document codec. |
| `crates/web` | `solomon-web` | Asset loading, keyboard/gamepad/touch input, canonical `GameSession`, WebGL2 world and actor rendering, browser UI and audio. Static previews run locally; native sessions consume accepted host snapshots. |
| `crates/protocol` | `solomon-protocol` | Typed host commands, completed snapshots, input acknowledgements and reliable results. |
| `crates/launcher` | `solomon-launcher` | Native executable serving bundled assets and a WebSocket authority, owning durable saves and opening the system browser. |
| `web/` | — | Trunk root: `index.html`, `Trunk.toml`, and the synced stock assets under `web/assets/stock/` (tracked and hash-verified). |
| `tools/website-oracle` | — | Runs the unmodified Website TypeScript kernels under Node to produce differential golden fixtures. |
| `tools/browser-smoke` | — | Headless-Chrome smoke test for the served client. |
| `scripts/sync-stock-assets.sh` | — | Copies the stock atlases the client needs into `web/assets/stock/`. |
| `scripts/record-website-goldens.sh` | — | Runs every oracle script and rewrites the differential fixtures. |
| `docs/` | — | Architecture, coding standards, decisions and the work queue; `AGENTS.md` and `CONTEXT.md` sit at the root. |

## Prerequisites

- Rust 1.98+ with the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`).
- [Trunk](https://trunkrs.dev) (`brew install trunk`); it downloads a matching `wasm-bindgen` CLI on first build.
- Python 3.11+ for asset verification and artifact packaging.
- Node 22.6+ for source oracles, authoring tools, browser checks and the optional Electron shell.
  Run `npm ci` for the repository's pinned development dependencies.
- A current Website checkout is needed only to record new differential fixtures; set
  `SOLOMON_WEBSITE_DIR` explicitly. Normal builds and committed replay tests are self-contained.

## Commands

```sh
python3 scripts/verify-client-assets.py # verify tracked stock inputs and complete audio catalog
cargo test --workspace                # unit tests + native goldens + Website differential goldens
cargo clippy --workspace --all-targets
cargo clippy -p solomon-web --target wasm32-unknown-unknown
cd web && trunk serve                 # dev server at http://127.0.0.1:4180/ (WASD / arrows to walk)
cd web && trunk build --release       # production bundle in web/dist/
```

Build a standalone folder and ZIP from the tracked inputs:

```sh
python3 scripts/package-game.py --output artifacts/solomon-darker
```

Run the packaged `solomon-darker` executable (`solomon-darker.exe` on Windows), or
double-click `Play Solomon Darker.command` on macOS. It serves its sibling `game/` assets
at `http://127.0.0.1:4188/?host=1` and opens the browser. Keep the launcher running while playing;
Ctrl+C stops it. The package requires no Website checkout, Node or Rust toolchain.
The launcher stores the shared game in its platform application-data directory; preferences
and your wizard's resume credential remain in that browser profile at the same address.
See [Native authoritative host](#native-authoritative-host) for the path and direct joins.
Build-time options include
`--profile debug`, `--web-dist PATH` to reuse an already verified Trunk output, and
`--no-archive` for a launchable directory only.

Build a Website artifact from a clean commit:

```sh
python3 scripts/package-hosted-game.py --output artifacts/hosted-game \
  --mount /game --website-mount ''
```

The directory and sibling `.tar.gz` contain the native authority, mounted WASM client,
disposable editor entry and independent Account save-tools module. `game-artifact.json`
records the source revision, native target and every payload file's SHA256. Website consumers
verify complete membership and content before installing the artifact. `--profile debug`
selects an unoptimized development build; a staging mount can use `/game-stg` for both
prefixes. Changing mounts requires rebuilding the artifact.

The optional desktop shell and its native child are owned here. See
[Desktop runtime](docs/DESKTOP-RUNTIME.md) for packaging and the real Electron acceptance
journey. `npm run test:desktop` checks its transport, asset policy and build preparation.

Stock input changes use `scripts/sync-stock-assets.sh` against an explicitly selected
reference, followed by review of `web/assets/stock-files.json`. Verification does not fetch
or silently regenerate assets.

On the M2 the launchd agent below already owns port 4180, so a manual dev server there needs
`trunk serve --port 4181` or the agent stopped first.

## Tailnet access on the M2

The M2 serves the release build to the tailnet at
**https://mac-mini-m2.tail442fb4.ts.net:4180/** (tailnet only, no Funnel).

- The launchd agent `com.jarrett.solomon-rust-game`
  (`~/Library/LaunchAgents/com.jarrett.solomon-rust-game.plist`) runs
  `trunk serve --release --no-autoreload` from `web/` with the `Trunk.toml` settings (loopback,
  port 4180), restarts it if it exits, and logs to
  `~/.local/state/solomon-rust-game/trunk.stdout.log` and `trunk.stderr.log`. Trunk watches
  `../crates`, so source edits rebuild the served bundle in release mode.
- `tailscale serve --bg --https=4180 http://127.0.0.1:4180` publishes the port; the mapping
  persists in tailscaled (`tailscale serve status`). `tailscale serve --https=4180 off` removes
  it. The macOS CLI is `/Applications/Tailscale.app/Contents/MacOS/Tailscale`.

```sh
launchctl print "gui/$(id -u)/com.jarrett.solomon-rust-game"        # state, pid, last exit
launchctl kickstart -k "gui/$(id -u)/com.jarrett.solomon-rust-game"  # restart
launchctl bootout "gui/$(id -u)/com.jarrett.solomon-rust-game"       # stop until bootstrapped again
```

### Regenerating the Website differential fixtures

```sh
./scripts/record-website-goldens.sh
```

Runs every `tools/website-oracle/*-goldens.mjs` under Node against the unmodified Website
kernels in `../Website` (override with `SOLOMON_WEBSITE_DIR`) and rewrites the fixtures in
`crates/sim/tests/fixtures/website/`:

| Script | Fixtures | Records |
| --- | --- | --- |
| `hub-goldens.mjs` | `hub-walk-goldens.json`, `starter-appearance-goldens.json` | six scripted and seeded courtyard walks, 6,080 ticks of position, velocity, heading, gait, walk cycle and collision RNG state; 54 owner × seed tint cases |
| `hub-ambient-goldens.mjs` | `hub-ambient-goldens.json`, `hub-teacher-goldens.json` | 3,000 ambient ticks (sampler state, seal and statue phases, every spray drop, the teacher's world release) with seal colours, statue offsets and spray alphas every 50 ticks; the teacher's phase and frame for every tick of three cycles under two seeds, the burst members through each release, and the random-domain reductions of 20 words |
| `hub-regions-goldens.mjs` | `hub-regions-goldens.json`, `hub-painter-goldens.json` | regions, portals, transition edges, placements, a 32-unit portal-contact sweep of every region, six scripted walks; the 48 fixed painter ids, the obstacles, the statue, the north-arch rectangle and 1,540 sort-bias samples |
| `hub-depth-goldens.mjs` | `hub-depth-goldens.json` | all 15 drawing bands, 24 initial actor depths, one actor/scenery/transient row-order case |
| `wizard-attachments-goldens.mjs` | `wizard-attachments-goldens.json` | 288 samples across all 24 facings, four gait phases and three walk phases, including unselected attachment pose 4 and fixed robe pose 13 |
| `hub-teacher-view-goldens.mjs` | `hub-teacher-view-goldens.json` | 50 view samples at cast, release, fade and cycle boundaries under two seeds; the ambient fixture also records transient registrations on every tick |
| `hub-rooms-goldens.mjs` | `hub-rooms-goldens.json` | four segment tables and room actor/prop anchors; 240 traversability points and eight swept moves from each clear point; 14 walks with 3,960 ticks covering all eight doorway directions, off-centre mortuary return, fixed furniture, and the College intro fade; flame anchors/flicker, Memorator facings, regional camera clamps |
| `hud-goldens.mjs` | `hud-goldens.json` | 16 resting/modal layouts, 8 health/shield cases with both poison states, 6 mana/reserve cases |
| `movement-input-goldens.mjs` | `movement-input-goldens.json` | 45 stick positions, all 16 key combinations, 4 gamepad mappings/d-pad cases, 4 touch vectors and 3 device-priority cases |
| `primary-profile-goldens.mjs` | `primary-profile-goldens.json` | 43 elemental and welded cast profiles, including upgrades, frozen weld ranks and equipment; five offensive/non-offensive damage and mana lane cases |
| `primary-cast-goldens.mjs` | `primary-cast-goldens.json` | authored staff, wand and bare emitters, cast poses and movement-facing ownership |
| `spell-targeting-goldens.mjs` | `spell-targeting-goldens.json` | native cell contact order, Air target retention, Ether tracking and missile fans |
| `water-spell-goldens.mjs` | `water-spell-goldens.json` | frost births, obstruction clipping, motion, colour quantization and both painter passes |
| `earth-spell-goldens.mjs` | `earth-spell-goldens.json` | boulder growth, damage pools, 3D orientation and lit impact fragments |
| `fire-spell-goldens.mjs` | `fire-spell-goldens.json` | patch contact/retirement, Ember pre-ticks and all retirement branches, friendly Imp movement and complete RNG state |
| `ether-blast-goldens.mjs` | `ether-blast-goldens.json` | charge/reset rules, health-scaled damage and the 108-particle program with its complete RNG state |
| `primary-step-goldens.mjs` | `primary-step-goldens.json` | 14 live elemental scenarios at 15 cast/channel/release/terrain boundaries, including partial and exact-cost mana, actor membership and shared painter allocation |
| `weld-step-goldens.mjs` | `weld-step-goldens.json` | all ten welded primaries across 26 held, weak and terrain journeys; 8,346 ticks preserve mana, cast clocks, physical queues and complete shared RNG |
| `primary-contact-goldens.mjs` | `primary-contact-goldens.json` | 46 actual-enemy transactions covering elemental and welded damage, status ownership, chained contacts, damage pools, knockback, detonations and painter order |
| `air-water-player-visual-goldens.mjs` | `air-water-player-visual-goldens.json` | 543 live cast/synchronize/finalize ticks, 48 full actor and visual snapshots, Hail bounces, Aura expiry and Hurricane owner/world changes |
| `player-world-effects-goldens.mjs` | `player-world-effects-goldens.json` | 15 death burst frames, 56 thrown weapon samples, 30 level-up frames and ten multi-player death observations, including rewinds, late joins and new runs |
| `player-attachments-goldens.mjs` | `player-attachments-goldens.json` | Actual authored Enchant Staff sockets, all six selectors, fifteen Weld colors and the front/orb pulse boundaries |
| `ambient-weather-goldens.mjs` | `ambient-weather-goldens.json` | Four ambient rain modes, 36 full population snapshots, collision retries, camera-height changes, cached birth lighting and the authored streak ramp |
| `primary-batch-goldens.mjs` | `primary-batch-goldens.json` | One shared primary epoch, participant insertion order, departed owners, region changes, per-player mana and College authority omission |
| `compact-mask-goldens.mjs` | `compact-mask-goldens.json` | Authored ground stamps, dynamic Spider stains, seven frame plans, cell-border admission, player insertion order, per-player flicker and direct aperture alpha |
| `session-multiplayer-goldens.mjs` | `session-multiplayer-goldens.json` | Five three-player public-Game journeys, one shared physics/spell epoch, mixed casts, barriers, restored continuation and empty Staff contacts |
| `session-membership-goldens.mjs` | `session-membership-goldens.json` | Native add/remove/merge, shared XP, detached projections/choices/rerolls/rejoin and durable-profile retirement/hydration |
| `host-input-goldens.mjs` | `crates/launcher/tests/fixtures/website/host-input-goldens.json` | Eight real host input-branch scenarios, 43 complete queue checkpoints and 17 numeric admission cases |
| `host-control-goldens.mjs` | `crates/launcher/tests/fixtures/website/host-control-goldens.json` | Seven real host renderer, pause, picker and rejoin scenarios with 52 complete checkpoints |
| `element-vfx-goldens.mjs` | `element-vfx-goldens.json` | 135 element/phase/scale plans and 42 selected-primary plans, including all fifteen weld effects and every authored record bank |
| `menu-goldens.mjs` | `menu-goldens.json` | 19 Create motion/reveal samples, four seeded title snapshots, eight pause fade samples, controller directions/repeat/spatial focus and binding swaps |
| `primary-presentation-goldens.mjs` | `primary-presentation-goldens.json` | 26 stock compositor plans, 24 lightning plans with exact float32 ribbon/branch geometry, and native path-light cases |
| `equipment-appearance-goldens.mjs` | `equipment-appearance-goldens.json` | 79 starter, unequipped, recipe, dye and generated-selector appearance projections |

Each fixture starts with `source` (the Website file) and `generator` (the script). A new
kernel family gets its own script and a focused replay in `crates/sim/tests/`; the fixtures
are committed so `cargo test` needs no Node.

### Browser smoke

With a dev server running (on the M2 the launchd agent owns 4180, so start your own with
`cd web && trunk serve --port 4181` and pass `URL=http://127.0.0.1:4181/`):

```sh
PW=../Website/frontend/node_modules/playwright-core node tools/browser-smoke/smoke.cjs
```

Launches headless Google Chrome (software WebGL), starts a wizard through the title/Create flow, checks the wizard
spawns in the north arch, walks south and east on key presses at the 100 Hz tick, fails on any
console error, and writes `rust-game-smoke.png`. `URL`, `SHOT` and `CHROME` override the
defaults. The `wizard_state()` export the script reads exists only for this purpose.

The room journey smoke walks from the spawn through each private room and back with actual
keyboard input, validates settled transitions, captures every room and its return, and fails
on browser errors. Its courtyard route uses the unchanged Website's collision geometry:

```sh
PW=../Website/frontend/node_modules/playwright-core URL=http://127.0.0.1:4181/ \
  node --no-warnings --experimental-strip-types tools/browser-smoke/rooms.mjs
```

`SHOTS` selects the screenshot/receipt directory (default `room-smoke/`); `ROOM=library`
selects one room for a focused rerun.

`tools/browser-smoke/hud-input.cjs` uses the same options, plus `OUT` for screenshots. It
checks the HUD's native bounds, wide and touch layouts, gamepad/d-pad movement and dead
zones, keyboard blur release, and real touch movement/cancellation. Controller samples use
the browser's native Gamepad prototypes; a physical controller is not required by the smoke.

`tools/browser-smoke/menus.cjs` checks title/Create, actual pause freezing, settings, movement
rebinding (including arrow reassignment), HUD scale and in-memory Continue. `menu-input.cjs`
checks controller-only creation, neutral button handoffs, modal focus ownership and touch
creation/pause. Both accept `PW`, `URL`, `CHROME` and `OUT` like the HUD journey.

`tools/browser-smoke/player-panels.cjs` exercises native Inventory/Skillbook controls against
the real Rust economy and belt: potion consumption, column-first slot moves, protected
clothing, equip/unequip, Unforge confirmation/results, skill binding and modal Escape.
`PlayerPanels` takes borrowed canonical snapshots and returns typed commands; the owning
session supplies spell factors and derived attributes. The stock art is fitted to the
native 1600×900 stage; phone panels also pass actual touch equipment, dragging, skills,
shop and dye journeys.

`tools/browser-smoke/gameplay-input.cjs` checks the real browser input adapter: overlapping
mouse/keyboard holds, release aim, one-shot potion intents, GUI release activation, blur,
controller selection, analog values, modal neutrality and one gamepad read per frame.

`hub-services.cjs` checks real purchases, addressed storage, insufficient-gold outcomes and
NPC choices. `hub-dialogue-rooms.mjs` walks to the teacher and librarian, opens the actual
conversations and checks selector navigation and one-shot book removal. Both capture stock
service/dialogue screenshots. `save-resume.cjs` exercises durable College/Boneyard reload,
inventory and preferences, then verifies atomic stale-tab write rejection.
`MovementInput::sample` returns raw `GameplayAim` requests for the owning camera plus
primary/quickbar levels, queued belt presses and menu/interact actions. The caller supplies
primary scene admission and the secondary-at-pointer setting, and maps `Retain` to its
previous world target. `movement()` remains available for movement-only callers; use one
of these sampling methods per frame. Run HUD context is `Hud::set_mode(HudMode::Run)`.
### Native authoritative host

The packaged launcher serves its bundled client and owns one shared `GameSession` at
100 Hz, publishing complete borrowed snapshots at at most 20 Hz. It opens
`http://127.0.0.1:4188/?host=1`; the marker selects the same-origin `/session` WebSocket.
An ordinary static preview continues to run the standalone browser simulation.

```sh
cargo run -p solomon-launcher --bin solomon-darker -- --assets web/dist
```

`--listen ADDRESS` enables direct join on a chosen interface; the default is loopback.
`--port`, `--save FILE` and `--no-open` support an isolated local session. On macOS the
default durable file is `~/Library/Application Support/Solomon Darker/host-4188.json`.
Ctrl+C commits the completed authority and shuts down its sockets and workers. Resume
credentials stay private in that file and in the browser's per-origin storage.

Title inspection reports Continue availability without admitting another player. Commands
return canonical results only after their containing snapshot; reliable retries retain the
same transaction. The host owns input edges, pause ownership, renderer/resume ACKs and
durable save receipts. Active-party rejoins finish pending skills before materialization.
Tutorial retains its sole authoritative participant while disconnected and freezes until
resume, preserving its one-player checkpoint invariant. Direct sessions have no global
directory: Leave/Kick archives the real profile and permits its later Continue in the College.

`tools/browser-smoke/native-session.cjs` drives two actual browser contexts through College
admission, movement, inventory, Social UI, Boneyard readiness, shared pause, fullscreen,
save/rejoin and launcher restart. The process runs from an unrelated directory with an empty
command path; requests must use its bundle and its save lives in an isolated temporary file. Set `PW` to the installed `playwright-core`, `BINARY` and `ASSETS` to a
native executable and its Trunk output, or set `PACKAGE` to a complete package directory.
`OUT` retains screenshots and the receipt; `GAMEOVER=1` includes actual death and disconnect.

## How the port is verified

1. **Native goldens** — `crates/sim/tests/native_goldens.rs` replays the retail-recorded,
   language-neutral fixtures committed in `crates/sim/tests/fixtures/native/` (RNG and float
   RNG bit-for-bit, movement within the fixtures' own epsilons); each file's header carries
   its native contract.
2. **Website differential goldens** — `crates/sim/tests/website_goldens.rs` replays the oracle
   fixtures above and requires bit-for-bit equality with the Website's TypeScript kernels,
   including the collision fallback sampler. The Website's own 308 unit-test files are
   TypeScript-coupled and are not ported; the kernels they cover get oracle fixtures instead as
   each layer is ported.
3. **Browser smoke** — the headless-Chrome run above, plus a look at its screenshot.

Float semantics matter for the differential: f64 math with `fround` at the same points as the
TypeScript, `hypot` implemented the way V8 does it, and `serde_json` with `float_roundtrip` so
fixture parsing is correctly rounded.

## Preparing Boneyard scenery

With the pinned Website's Vite server running, compile its retained stock scenery into the
Rust client's asset directory:

```sh
SOLOMON_WEBSITE_URL=http://127.0.0.1:4178 node tools/export-boneyard-art.mjs
```

The compiler reuses the Website's existing painter. It writes deduplicated PNGs, per-object
layers and ground/road meshes under `web/assets/stock/boneyard/`. The game consumes those
assets directly; JavaScript is only needed when preparing them. `BONEYARD_LIMIT=1` selects
the first map when inspecting the asset pipeline. A full build prepares all twelve retained
maps and the tutorial.

## What the browser shows today

The Hub courtyard from the stock College atlas (floor tiling, scenery banks, the translucent
runic ring, the additive seal glyphs and core pulsing along their colour tracks, the fountain's
additive spray, the eight obstacle props, the College statue with its bob and multiplied aura,
the foreground bank) in the stock painter order relative to the player, with the wizard
(shadow, robe, fixed robe pose 13, hat, staff, scroll and hands; ether starter tints rolled from the native RNG) walking under
keyboard control with the Website's exact movement and segment collision, camera clamp at
scale 1.2, and the 1600×900 logical view letterboxed into the window. Fixed drawing bands
are depth data. The teacher casts every 8.47 seconds and releases its four-member burst;
the column and frame strip sort among the actors. All four private rooms are reachable
through their doorways, with outgoing fade, destination placement, entrance walk and incoming
fade. Stock layers, portraits, furniture, fixed staff, flames, memorial glow and foreground
masks use the recorded anchors and painter order.

The pure College admission controller is available for the session owner: renderer-ready
holding, the native courtyard and Office splines, title/cover, Archchancellor contact,
loadout confirmation, primary-cast admission and the final courtyard entrance. Its source
fixture replays 2,081 ticks and actual arm operations. The browser's ordinary New Game
journey uses this controller through the host, dialogue and loadout UI.

The offline Boneyard foundation is also implemented and checked natively: all twelve retained
stock generator outputs and the stock tutorial, authored collision footprints, swept movement,
seeded swinging gates, entrance bounds and camera clamp. The retained scenery renderer draws
native ground/road meshes, live gates and tree fading, the multiplied light field, and
directional shadows. Scenery Puppet hits reuse each resident's ordinary or diffuse material,
including the second upper proxy; Earthquake rotates both tree layers around the authored
tree pivot. The actual pinned Sprite/Mesh views and transforms are recorded in
`scenery-views-goldens.json`. `tools/browser-smoke/boneyard.cjs` enters and walks two actual maps,
including replacement of the previous map's GPU resources. The inspected map-seven entry
closely matches the retained JS scenery and lighting.

Every stock or authored Boneyard now compiles its scenery from accepted scene geometry.
`tools/export-boneyard-catalog.mjs` retains the original scene-independent glyph bank and
shape tables; runtime field/roads, composite art, generated Terrain/Wall pixels, painter
registrations and cleanup no longer require a manifest named after each geometry hash.
The historical per-scene exports remain visual oracles. Source planner and material
fixtures cover explicit records, composite proxies, shared posts, broken-fence private
randomness and native Tree/hit color branches. Actual local/cloud editor Test and Return
journeys cover the runtime compiler and disposal of its owned resources.

Scenery owners also paint their original root masks, flat glyph shadows and Wall skirts.
Scrub retains its private stopped-world visual clock and lazy Terrain/compact-surface
reflection query. Live Goodies share their painter slot with either the original flat
glyph or directional outline. Source fixtures and actual hosted browser captures cover
the shadow settings and these rendered layers.

Tree scans borrow delivered actor positions and the current camera epoch. Missing history
and off-camera scans preserve their previous fade target; they do not replay today's local
position through a suspended-tab gap. The focused current-Source history fixture also checks
departing/arriving actors, Coffin admission, Maggot membership and secondary foliage offsets.

The canonical world epoch composes player contacts, cast-facing locks, the opening Solomon
encounter, arena safety, all survival wave programs, the complete enemy store, late births
and ordered terminal rewards. The actual Website world entrypoint replays for 5,200 ticks
across five journeys, including template 9 and pre-encounter births against a moving gate.
Its 255 world checkpoints compare IDs, clocks, random streams and loot exactly; 1,032
consecutive Solomon spatial checkpoints retain the existing 1e-9 trigonometric bound.
Checkpoint restoration preserves the same future evolution. Enemy placement uses identity-checked stock navigation or runtime meshes
from actual imported geometry, preserving first-query gate constraints, native policy
retries, light fallback, offscreen views and consumed random words. The live
session and renderer consume this same world authority and its actual actor stores.

Native loot selection and materialization now produce real equipment, potion, skill-book,
key, Gold, Orb and Bonus drops. Random and named clothing retain their exact colour/RNG
rules; all item trees use the canonical inventory model. The ground store reproduces delayed
scatter/bounce, Orb attraction and decay, Bonus
expiry, keyed Goodie opening and fragment effects, ordered pickups, shared enemy seed writes,
and Last Word cleanup. The canonical world/checkpoint owns this store and its exact future
state, seeded from the actual scene Goodies. The movement → loot → encounter sequence uses
current gate collision and retained Sacks for arena sealing safety. Terminal rewards create
real drops and Tutorial items through this same store, including the browser's earned
Tutorial amulet and actual pickup/equipment journey.
The primary simulation now runs all five elements and all ten welded builds through one
`SpellState::step_primary` queue. Its Boneyard adapter preserves cast → secondary → late
primary contact ordering, including secondary Steam pulses. Damage acceptance, Boulder/Hail
pools, cold/electric status, pushback, Meteor pulses and authored Fire payloads use the actual
enemy and secondary owners. Hurricane orbit/contact and player-following Cold Aura/Water
Hail now use source-ordered synchronize/finalize phases in that same queue.
The player world-effect view borrows each participant's actual combat, equipment and death
weapon registration. It composes dyed death body layers, thrown weapons, the one-shot death
burst and level-up sparkles; the root session supplies their live painter/depth bindings.
Stock Weld compositor plans share the primary GPU view and native beam clipping. Actual
Tutorial, normal combat/death and native multiplayer journeys verify the integrated client.

Ambient Boneyard rain has its own presentation owner, separate from ability rain and gameplay
randomness. It advances snapshot ticks against the actual visible bounds and gate collision,
caches each streak's first-render light, and draws the stock ground rings and authored ramp.
The scene caller supplies its pre-light splash and foreground streak passes.
Compact ground masks reuse the same stock atlas and diffuse-capture primitive. Each actual
participant retains a 256² target of admitted stamps, multiplied by the native radial glyph
and added immediately after that participant's environment aperture. Dynamic Spider stains
also enter the pre-world ground group; hidden targets persist until their player departs.
The direct aperture helper uses the stock glyph and source rAF flicker; the scene enables it
when environment mode is nonzero, separately from logical light collection.

Logical enemy, enemy-projectile, secondary and weld light factories now share the same
native source values and semantic presentation randomness. The world collector binds actual
enemy, boss, projectile, elemental-primary and secondary actors, including Air/Mage MiscLights,
to their native registrations and participant cameras. Weld lights consume the canonical
welded-primary actors in that same world collector.
The stock Tutorial kernel now retains all twenty stages, narration timing, authored waves,
survival intervals, camera locking, entrance spawn domain, HUD access and reward item identity.
Its world adapter derives safety, item counts and progression inputs from the real retained
entities. The canonical `GameSession` now owns the complete ordered tick, profile/resources,
XP barriers and run lifecycle. It keeps participants by string ID plus explicit entity order;
Hub and Boneyard characters have one participant-owned movement state. Browser and native
hosts share its commands. Network snapshots borrow the same checkpoint schema and apply
same-world updates without rebuilding immutable collision or compiled wave catalogs.
Thirteen public Game journeys replay after this ownership cutover. Five multiplayer source
journeys and the actual native two-browser movement/save/rejoin journey also pass.

Projectile queries preserve native line masks, separate point/bounds tests and adaptive
retention cameras. Ordered explosion and retirement callbacks update the world feedback
state retained by the same checkpoint.

The screen-space HUD reuses the stock UI, Skills, Inventory and LevelPicker atlases: health
(including poison/shield layers), mana/reserves, primary and concentration icons, populated
belt entries, backpack, tome, XP, College portraits, map, help and skull. It starts from the
selected starter loadout through canonical `PlayerState` values. The session applies actual
inventory, skillbook and belt commands; College portraits open the real NPC services.
WASD/arrows, standard gamepad sticks/d-pad and a touch joystick
walk the wizard. Touch retains a 90 CSS-pixel joystick floor so a portrait phone can operate
it; belt binding labels use browser serif text rather than a new bitmap text engine.

The client starts on its stock-art title screen, with Play, New Game, Continue and settings.
Create shows the native hands, element effects and all five elements/three disciplines;
the chosen profile sets the starter tints and HUD icons. Pause uses the stock three-row
menu and fixed-tick reveal, and freezes world ticks. Settings uses native frame/button art
with browser form fields and source binding swaps. UI scale, controls, camera field of view,
light quality and scenery lighting/shadows reach the running client. Portrait menus fit their
physical controls while preserving desktop stage geometry. IndexedDB saves restore the complete
College or Boneyard checkpoint after a page reload, retain preferences, and reject stale-tab
writes atomically. Stock UI effects, music scenes and volume settings use browser audio;
real playback and save/reload journeys pass together.

The local Hall of Fame title route borrows completed run records through
`MenuUi::set_hall_snapshot`. It preserves native rank ties, stock frames, element portraits,
expanded survival/skill/perk details and the current-run scroll. The session adapter records
completed local runs through browser storage and supplies those entries to the menu.
Phone users pan the native record canvas at readable text size.

The HUD primary/concentration buttons open the source's small skill selector from the actual
learned skillbook. Player panels emit canonical Tutorial surface events when their visible
surface changes. College-owned Office dialogue acknowledges its first choice or close once;
the admission loadout reuses Create and emits `ConfirmLoadout` while keeping Back locked.

`sim::social` defines the native host's Party/Chat projection and typed actions. It preserves
durable offline roster rows, leader succession, kick admission, bounded accepted chat history,
source text validation and the per-connection rate limit. Direct native sessions leave broker
codes/listings absent and explicitly disable directory/Global capabilities. The UI must consume
accepted host records rather than create membership or echo its own pending messages.

Phone Tutorial text uses its measured wrapped bounds so headings stay visible and clear
the selected-spell strip. That strip preserves native offsets with a 44px touch floor;
compact Skills opens over the native book center instead of an empty canvas edge.

Hagatha, Fomentius, the Annalist and Luthacus occupy their stock painter ordinals. Skorcha
uses the native appearance schedule, gestures and hat pulse. Students enter on the eighteen
stock splines, wander and retire through the native population lifecycle, with scaled body
and prop layers and the separate head pass. The player and students share the stock circle
push response and segment collision; trader clocks and Hagatha's particles tick at 100 Hz.

The optimized macOS ARM64 standalone release passes the final native two-browser,
save/restart, combat death, Game Over readiness and disconnect journeys. Its display timeline
projects between approximately 20 Hz snapshots: actual Metal draw positions change at a
median 16.7 ms in the bounded cadence check. The matched 1600×900 Tutorial scene closely
matches the pinned JS reference. Complete Tutorial lessons, actual phone casting, Social
focus, shared holds and fullscreen also have retained browser receipts. The finished package
is `../Packages/solomon-darker-macos-arm64-20261020/`, with a sibling ZIP; double-click its
`Play Solomon Darker.command` to play.
[docs/PORT-COMPLETION.md](docs/PORT-COMPLETION.md) owns the current acceptance state.
[docs/WORK-QUEUE.md](docs/WORK-QUEUE.md) holds the tickets.

The standalone profile/skillbook and stock inventory/service kernels are also available in
`solomon-sim`: actual starter equipment and potions, seeded shop stock, addressed backpack
and nested sacks, equipment admission/swaps, Luthacus storage, Hagatha purchases, Shlorio
dowsing, dye, and Unforge. Their Website differential journeys and real browser service
journeys pass. Inventory and Skillbook panels use those same canonical components.

The complete stock NPC dialogue, questions, dismissals, service selectors, books, boasts,
teacher explanations, range/hit tests and marker rules are source-verified in
`hub::interaction`. Courtyard help markers already draw at their owning actor's painter tail
with native pulse and profile suppression. Actual conversations, selectors and service
transactions and College onboarding are connected to the canonical session.
