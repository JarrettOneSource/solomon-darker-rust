# Work queue

Tickets for parallel agents. Each ticket is one porting loop ([AGENTS.md](../AGENTS.md)):
reference → oracle fixture → kernel → replay test → wiring → gates. Take a ticket by writing
your name on its status line; finish it by writing `done` and the commit or branch.

Current Website behavior, ownership and verification are mapped in
[SOURCE-PARITY.md](SOURCE-PARITY.md). [PORT-COMPLETION.md](PORT-COMPLETION.md) owns the
complete release/deployment acceptance record. Earlier ticket receipts below retain their
actual revisions and bounded scope.

The game GPU UI and its browser/candidate qualification are complete at `b264cac5`.
The desktop launcher replaces its three HTML/CSS/JavaScript presentation files with
`crates/web/src/desktop/launcher`. Its 1dce46ad implementation passes 701 native tests,
the strict native/WASM checks, complete browser smoke and reviewed Mac package journey,
including held-pointer release after saved resume;
candidate dc9a103 passes game/Linux/Apple Silicon/Intel qualification; Windows saved-resume
Settings input failed, and the later Windows trace reproduces the repaired held-click
expiry on the Tutorial prompt. Current four-platform qualification remains required. The [migration matrix](RUST-UI-MIGRATION.md) owns the accepted surface
inventory and the earlier package journeys. Canonical main release, Website artifact
admission, live recovery and task cleanup remain. Earlier DOM-era receipts below retain
their original behavior scope and do not substitute for GPU acceptance.

Scenery Puppet hits, Earthquake pivots, repaint/retirement metadata, dynamic scenes, trusted
mod content, custom frame/wearable geometry and current shadow behavior are integrated.
Their Source fixtures and mounted authored-scene/browser journeys pass; see the acceptance
record for their evidence.

## Working on a follow-up

- Use this standalone repository's current main as the base. The earlier fleet commits
  below describe historical acceptance; their temporary integration worktrees are removed.
- **Work in a worktree**, one per task (`git worktree add ../rust-game-<task> -b
  <task>`), and remove it after publication when its work and evidence are preserved.
- **Stock inputs are tracked**: run `python3 scripts/verify-client-assets.py` in a fresh
  worktree. Asset regeneration is a reviewed source change, not a build prerequisite.
- **Dev servers use ports 4181 and up** (`cd web && trunk serve --port 4181`); 4180 is the
  service. Pass your port to the smoke as `URL`. Native acceptance uses isolated 4191/4192
  hosts; Node's fetch rejects reserved port 4190 before making a connection.
- Historical paths below are relative to the recorded Website revision's
  `frontend/src/game/`. Set `SOLOMON_WEBSITE_DIR` explicitly when recording new fixtures.

## Tickets

### 1. Depth bands as data

Status: done — fleet/visuals-20261020, d3f3f48 and c961cb2.

- **Owns:** `crates/sim/src/hub/depth.rs` (new), the ordering in `crates/web/src/courtyard.rs`.
- **Reads:** `hub-depth.ts` (33), `HUB_WORLD_DEPTH` and `HUB_WORLD_LAYER_BOUNDS` in
  `renderer/hub-render-contract.ts` (119), and the `zIndex` assignments in
  `renderer/hub-world-scene.ts` (1303).
- **Oracle:** `hub-depth-goldens.mjs` recording every `HUB_WORLD_DEPTH` entry and
  `hubActorDepth(y, referenceY)` over a grid of `y` and `referenceY`.
- **Acceptance:** a `HubDepth` table lists every band in drawing order with its value;
  `courtyard.rs` draws layers by that order instead of by the order of its draw calls. The
  statue aura stays immediately below the statue body. `hubActorDepth(834) − 1` is its initial
  depth; `HubWorldScene.applyPainterOrder` replaces it with ordered body depth minus 0.25.
- **Traps:** depths are floats (`+ 0.5` for onboarding); `hubActorDepth` reuses
  `nativeRegionPainterRow`, which `painter_row` already ports. Active world queue depths are
  consecutive (`NativeHubPainterPlanner.apply`), rather than raw row values.

### 2. Wizard attachments: staff, hands, fixed robe pose

Status: done — fleet/visuals-20261020, c961cb2.

- **Owns:** `crates/sim/src/presentation.rs` additions, `crates/web/src/wizard.rs`.
- **Reads:** `player-character-presentation.ts` (293), `renderer/world-player-view.ts` (872),
  `renderer/world-player-textures.ts` (740), `player-enchant-staff-view.ts` (144).
- **Oracle:** `wizard-attachments-goldens.mjs` recording attachment offsets, records and
  visibility per heading index × walk phase × gait, including the unselected fixed-robe
  pose (native pose 13).
- **Acceptance:** the courtyard wizard shows staff and hands in all 24 facings; the
  attachment offset kernel replays bit-exactly; the smoke still finds the wizard ticking.
- **Traps:** the staff flips with facing; hand records differ for the fixed-robe pose; the
  Website applies `Math.fround` to attachment anchors.
- **Living combat presentation:** `WizardView::set_player_presentation` borrows the actual
  participant, secondary cast, lighting overlay phase and web status. It preserves melee /
  secondary / primary pose precedence, Enchant Staff, Damage ×4, native Puppet hit masks,
  Magic Shield and the complete articulated Harden / Webbed capture. Source ownership is
  `player-enchant-staff-presentation.ts`, `player-character-presentation.ts`,
  `renderer/player-{harden,webbed}-view.ts` and `native-player-diffuse-capture.ts`.
  `player-attachments-goldens.mjs` and matched actual PlayerWorldView screenshots cover this
  extension. The capture reuses the existing renderer and atlas uploads.

### 3. Teacher view and burst world painters

Status: done — fleet/visuals-20261020, c961cb2.

- **Owns:** `crates/web/src/teacher.rs` (new); additions to `hub/painter.rs` for transient
  registrations.
- **Reads:** `hub-teacher.ts` (186), `hub-presentation.ts` (728), `core-server/hub-ambient.ts`
  (141, `registerWorldPainter`), `renderer/hub-world-scene.ts` (1303), `hub-actors.ts` (124),
  `hub-painter-order.ts` (93), `region-painter-order.ts` (350).
- **Oracle:** extend `hub-ambient-goldens.mjs` to record `teacherWorldRelease.painterRegistrations`
  (ordinal and lane) per tick, then `hub-teacher-view-goldens.mjs` for record ids, anchors,
  scales and alphas of the teacher sprite and the four burst members per tick of a cycle.
- **Acceptance:** the teacher stands at the College and casts every 8.47 s; the burst's
  column and frame strip sort among the actors by their transient ordinals. As in the pinned
  `HubTeacherView`, the flare draws at pre-world depth 999 and the core at post-world depth
  2499; the rune, shadow and body draw with the teacher. `cargo test` replays both fixtures.
- **Traps:** transient registrations are allocated in release order (`releaseIndex * 2 +
  index` when no manager is attached); `HubTeacherBurst::needs_world_painters` already says
  when they exist. Frame strip frames clamp to `0..=10`.

### 4. Private rooms: segment tables and portals in the simulation

Status: complete. `hub/rooms.rs` supplies the translated segment tables and fixed-circle
placements; `HubWizard` owns its region and doorway lifecycle. The room fixture replays
all eight doorway directions, fixed-prop contacts and the off-centre mortuary return.

- **Owns:** `crates/sim/src/hub/rooms.rs` (new), `HubWizard` support for a current region.
- **Reads:** `core-kernels/hub-private-room-layout.ts` (436: `MORTUARY_SEGMENT_COORDINATES`
  89–102, `STOREROOM_` 103–139, `OFFICE_` 140–190, `LIBRARY_` 191–220, table-to-world offsets
  234–237, `HUB_PRIVATE_ROOM_LAYOUTS` 260), `core-kernels/hub-regions.ts` (363, ported),
  `hub-participant-movement.ts` (335).
- **Oracle:** `hub-rooms-goldens.mjs` recording each room's segment table after the offset,
  a traversability grid (swept moves from a lattice of positions in eight directions), and
  walks that cross each doorway in both directions.
- **Acceptance:** `HubRegion::segments()` returns each room's table; the wizard ticks in any
  region and collides with its segments; portal contact starts a transition; both fixtures
  replay bit-exactly.
- **Traps:** offsets are per room and applied once; the courtyard's segment list is already
  `HUB_COURTYARD_SEGMENTS` (129) and stays where it is.

### 5. Private rooms: scenes and transitions in the client

Status: complete. Stock-derived room layers, portraits, furniture, named staff, flames,
memorial glow, foregrounds and masks draw in the recorded painter order. The keyboard
journey smoke enters and returns from all four rooms, with screenshots and no browser errors.

- **Owns:** `crates/web/src/rooms.rs` (new), transition handling in `crates/web/src/lib.rs`.
- **Reads:** `hub-private-room-scene.ts` (850), `native-college-intro.ts` (211),
  `hub-participant-movement.ts` (335), `hub-regions.ts` (fade rates, placements).
- **Oracle:** extend `hub-rooms-goldens.mjs` with the record ids and anchors of each room's
  paintings and props.
- **Acceptance:** walking into a doorway fades out, places the wizard in the room, walks it in
  by the scripted movement and fades in; walking out returns to the courtyard at the recorded
  placement; the smoke walks into the mortuary and back.
- **Traps:** the College intro fade rate differs from the room rates; `preserve_contact_x`
  keeps the doorway x on return.

### 6. NPC traders and students

Status: implemented. `hub-npc-goldens.json` covers all eighteen routes, 54 seeded spawns,
the 900-tick live warmup, 3,000 population ticks, 2,400 actor-world ticks, and 6,002 trader
frames. Browser smoke verifies live students and movement; stock trader/student art joins
the shared painter queue. Standalone profile, equipment, economy and NPC conversation kernels
are implemented, source-verified and connected to the browser, with actual service journeys.

- **Owns:** `crates/sim/src/hub/npc.rs` (new), `crates/web/src/npc.rs` (new).
- **Reads:** `core-server/hub-students.ts` (574), `core-kernels/native-hub-npc.ts` (334),
  `core-server/hub-world.ts` (499), `core-server/hub-ambient.ts` (141).
- **Oracle:** `hub-npc-goldens.mjs` recording student spawns, wander targets, facings and
  trader idle frames over several thousand ticks from the seeded state.
- **Acceptance:** traders stand at their painter ordinals; students wander the courtyard with
  the recorded paths; the fixture replays bit-exactly; the smoke counts at least one student.
- **Traps:** the pinned Website's student population and each student use independent
  xorshift streams, with double-precision spline/wander values. Trader clocks use the native
  generator with its explicit single-precision stores. Neither shares the ambient sampler.

### 7. HUD

Status: done, fleet HUD worker (`fleet/hud-20261020`); stock DOM HUD, exact layout/meter
fixtures and desktop/wide/touch browser journeys. Actions use the canonical session/menu owner.

- **Owns:** `crates/sim/src/hud.rs` (new, layout), `crates/web/src/hud.rs` (new).
- **Reads:** `native-hud-layout.ts` (117), `native-hud-presentation.ts` (285),
  `hub-render-contract.ts` (119), `hub-textures.ts` (247).
- **Oracle:** `hud-goldens.mjs` recording layout rectangles for several canvas sizes and the
  presentation state for a few ether and health values.
- **Acceptance:** the hotbar, ether and health draw at the recorded rectangles for the
  letterboxed canvas; the fixture replays bit-exactly.
- **Traps:** the HUD lives in screen space after the camera letterbox, not in world space;
  text needs a glyph atlas the stock bundles provide.

### 8. Gamepad and touch movement

Status: done, fleet HUD worker (`fleet/hud-20261020`); standard pad stick/d-pad, keyboard and
touch joystick; exact input fixtures and browser movement/cancellation acceptance.

- **Owns:** `crates/web/src/input.rs` extensions, `crates/sim/src/input.rs` (new, the stick
  deadzone and lane mapping).
- **Reads:** `input/movement-input.ts` (248), `input/standard-gamepad.ts` (185),
  `input/gamepad-sampling.ts` (73).
- **Oracle:** `movement-input-goldens.mjs` recording the movement vector for a grid of stick
  positions and for each key combination.
- **Acceptance:** a gamepad walks the wizard with the Website's deadzone and diagonal
  normalisation; keyboard output is unchanged; the fixture replays bit-exactly.
- **Traps:** sampling happens once per frame before the tick loop, as the keyboard does today.

### 9. Offline Boneyard runtime, protocol and audio

Status: complete. The enemy/loot/Tutorial epoch and owned multiplayer session replay the
source with exact checkpoints. Browser/native save and restart, stale-tab rejection, actual
audio, complete Tutorial play, native multiplayer and the final standalone release pass.
The source display timeline also passes actual GPU cadence and full-scene comparison.

- **Owns:** `crates/sim/src/boneyard.rs`, `boneyard/{world,world_geometry,world_light,collision,escape,gate,arena,encounter,navigation,spawn,waves}.rs`,
  `crates/assets/src/{boneyard,navigation}.rs`, the browser navigation fetcher, and retained Boneyard geometry and navigation assets.
- **Reads:** Website `core-kernels/boneyard.ts`, `boneyard-gate.ts`,
  `boneyard-arena-transition.ts`, `core-server/boneyard-collision.ts`,
  `boneyard-world.ts`, `host/boneyard-catalog.ts`, the complete retained native generator
  bank and stock tutorial, `renderer/boneyard-render-contract.ts`.
- **Oracle:** `boneyard-goldens.mjs`; 35 authored collision families and fence codes,
  315 movement requests, 1,300 world walk ticks with gates, 160 gate ticks, both entrance
  sides through 400-tick sealing, and camera clamps for large and small scenes.
- **Implemented:** exact twelve-template bank selection by the seed's first big-endian word,
  complete scene data, static collision footprints, swept tangent collision, swinging gates,
  entrance bounds, camera and the offline 100 Hz world movement loop; the complete Solomon
  opening encounter, native wave compiler and scripts, exact wave checkpoints, route queries
  over 63 source preparations, native policy-driven spawn placement and actual world geometry
  callbacks. Canonical world state composes contacts, cast-facing locks, webbed movement,
  encounter release and arena safety; checkpoint continuation preserves future state.
  Real ground loot and keyed Goodies retain shared RNG, address allocation and exact future
  state. The logical light collector binds canonical entity owners and MiscLights in native
  manager order; the elemental lifecycle oracle also checks the resulting fields.
  All twenty Tutorial stages, narration, authored wave groups and survival programs, camera
  lock and entrance domain, surface actions and actual reward items now replay the source.
  The normal world epoch composes the real enemy driver, shared loot entropy, ordered terminal
  rewards, Tutorial item drops, late wave births and permanent scenery cleanup. Its public
  source replay covers 3,000 ticks, 150 snapshots and 2,100 exact restored-continuation ticks.
  Terminal-run title fades, the Solomon Riff animation and both audio edges replay the
  pinned source; their browser projection follows the accepted session lifecycle and passes
  native GameOver readiness/disconnect.
- **Reuse:** the existing `HubWizard` movement planner/commit and `Vec2`. Boneyard scenery
  needs circles, polygon footprints and fence capsules; its reference uses those primitives
  rather than the earlier rectangle-only `PlayerActor` fixture solver.
- **Ambient presentation:** `boneyard/weather.rs` and `BoneyardWeatherView` port
  `native-boneyard-weather.ts` and its view: snapshot catch-up, native tick-seeded RNG,
  radius-four collision retries, cached streak lighting and stock splash rings. Four modes
  replay in 36 source snapshots; four actual Source/Rust GPU pairs match populations.
  The scene owner binds ground splashes before region lighting and streaks above foreground.
  `boneyard/compact_masks.rs` and `CompactMaskView` also port the source compact diffuse
  compositor, including whole-cell admission, authored stamps, Spider stains and retained
  participant targets. Seven source frame plans and eight actual Source/Rust GPU pairs verify
  masks, alpha, hidden/reused targets and departure. The scene caller interleaves these
  composites with environment apertures and supplies sealed combat or full scene bounds.
  The same module supplies the source direct-aperture alpha/draw helper using existing
  DeadHawg record 18; environment-mode gating belongs to that scene caller.
- **Final acceptance:** optimized native/WebAssembly package `96ca53b` passes fourteen
  real two-browser checks, native-file restart, GameOver Continue readiness and disconnect.
  Actual GPU movement changes at a 16.7 ms median while snapshots remain about 20 Hz.
  The matched Tutorial scene agrees closely with pinned JS. Complete Tutorial progression
  and phone Acid Rain input have retained receipts. Direct native multiplayer omits the
  public Website broker's directory/Global capabilities. The TypeScript host remains an
  oracle and is not a runtime dependency. See `PORT-COMPLETION.md` for exact checks and limits.

## Done

- Native host social contracts (`sim::social`): typed actual membership/chat/card snapshots
  and commands, source text/history/rate rules, leader/kick/departure rules, durable roster
  projection and player/Golem ally presentation. Source replays cover Unicode boundaries,
  channel availability, sequence deduplication, rate windows and offline health/identity.
- Tutorial panel producers and HUD skill selectors: canonical open/close events, real
  primary/concentration choices, one-time College dialogue acknowledgement and locked
  Create/loadout confirmation. The selector source replay covers 48 option combinations
  and 12 native layouts; browser receipts cover real opening, selection and phone targets.
- Local Hall of Fame: canonical archive entry/ranking/time types, native row/scroll geometry,
  fifteen class titles and stock DOM title route. Borrowed records keep score/archive writes
  with the session; the source replay and matched populated/empty browser component captures
  cover expansion, five element portraits, scroll and responsive native-canvas panning.
- College admission controller (`hub/college_intro.rs`): shared native splines, readiness hold,
  title/cover, Office contact/dialogue, loadout barrier and admission predicates. The pinned
  source fixture records 119 path cases, 32 actual arm operations and the complete 2,081-tick
  admission; the canonical session owner integrates the explicit before/after-commit seam.
- Courtyard walk, collision, painter order, starter tints (`hub-walk-goldens.json`,
  `starter-appearance-goldens.json`).
- Courtyard ambient life: fountain spray, seal pulse, statue bob, teacher clock
  (`hub-ambient-goldens.json`).
- Teacher cycle, burst and random domain (`hub-teacher-goldens.json`).
- Hub regions, portals, placements, scripted walks (`hub-regions-goldens.json`).
- Fixed painter ids, obstacles, statue, north-arch bias (`hub-painter-goldens.json`).
