# Architecture

The pieces, which way the dependencies point, and where a new piece goes.

## Crates and seams

```
solomon-web ──▶ solomon-sim     kernels: rules, tables, stepped state
     │              ▲
     └───────▶ solomon-assets   stock records and retained world decoding
```

- **`solomon-sim`** (`crates/sim`): every rule of the game as pure functions and value types.
  No platform randomness, clock or I/O. Testable from JSON fixtures; compiled unchanged into
  the browser.
- **`solomon-assets`** (`crates/assets`): `parse_bundle` turns a stock `.bundle` into
  `SpriteRecord`s: atlas rectangle, logical frame, registration centre. Its Boneyard catalog
  and navigation decoders produce simulation values for browser and native hosts. They
  reuse serde/serde_json and perform no filesystem or network I/O. The shared
  `save_document` codec enables simulation persistence and validates complete session
  continuations for both browser and native storage.
- **`solomon-graphics`** (`crates/graphics`): the shared WebGL2 renderer and asset decoder.
  Both loader and main client use its textures, blend modes and native geometry; neither
  owns a copied renderer.
- **`solomon-web`** (`crates/web`): the browser client. Fetches atlases and bundles, reads
  input, runs a local fixed-tick loop or accepts native host snapshots, and composes the
  world with Rust GPU menus, HUD and overlays. Controllers read explicit simulation state
  and return typed actions. `gpu_ui` owns egui interaction/layout, stock bitmap text,
  licensed browser font rasterization and GPU resources. `web/` is the Trunk root.
- **`solomon-protocol`** (`crates/protocol`): the native host and browser's versioned messages.
  Inputs, commands and snapshots reuse canonical simulation types. A borrowed host snapshot
  and an owned browser checkpoint have the same serialized shape. Its ordered snapshot
  codec compacts selected publications and reconstructs them before browser coalescing.
- **`solomon-launcher`** (`crates/launcher`): the native package entry point, bundled asset
  server and bounded WebSocket transport. Connection workers deliver messages to one host
  owner; they do not interpret game rules.

The seam rule: a value that changes what the game *does* lives in `sim`; a value that changes
what is *shown* lives in `web`. Painter order is a rule, so `HUB_COURTYARD_OBSTACLES`
(position, radius, sort bias) is in `sim`, while `OBSTACLE_RECORDS` (which sprites dress each
obstacle) is in `web`, index-aligned with it.

## The frame loop

`crates/web/src/lib.rs` owns the browser entry points and `requestAnimationFrame` callback.
`crates/web/src/game.rs` owns the running `Game`, its assets, UI and simulation composition:

1. `frame(now)` accepts native messages, updates the active UI and samples
   touch/gamepad/keyboard input once. The native message callback retains its browser
   receipt timestamp through coalescing and asynchronous world loading.
2. Static browser mode advances `GameSession::step` for each whole 10 ms tick, with a
   250 ms catch-up cap. Native mode applies validated completed checkpoints; the native
   `NativeAuthority` owns the 100 Hz simulation and publishes at 20 Hz. Both use the same
   ordered Session rules. College panels seal local input while its world keeps ticking;
   admitted Boneyard panels and level-up barriers pause the shared world.
3. Native mode samples one `NativePresentationTimeline` after movement input and before
   camera/aim projection. Its bounded history returns a display-only Session copy plus
   fractional actor clocks and endpoint appearance metadata. Boneyard interpolates received
   states; Hub alone predicts its local movement for one snapshot interval. The accepted
   Session still owns UI commands, audio events, acknowledgements and persistence.
4. `render()` sends the same display frame to the camera, player/world geometry, lighting
   and painter consumers. Fractional clocks stay in presentation inputs rather than changing
   simulation tick types. `CourtyardScene` and `BattleViews` submit the ordered world queue;
   GPU and cosmetic caches never commit gameplay. Static mode draws its completed Session.
   Scenery occlusion advances through whole display ticks using that displayed local
   position; snapshot receipt and simulation stepping do not also advance its view state.

Display copies share immutable loaded scenery and static collision primitives through
`Arc`; each collision world retains its own small gate-leaf buffer. Player skill definitions
also use shared immutable entries, with the stock table initialized once and authored
changes using copy-on-write. The player projector updates the selected endpoint copy in
place, and enemy interpolation borrows the two endpoint lists. These choices reduce copies
without sharing mutable actor state or advancing authoritative clocks during rendering.
Imported scenery target tables share storage until cleanup changes their membership.
Idle menus need only their final layout pass; real input dispatches still commit in order
before painting, and running gameplay keeps its early input-admission layout.
Title gravestones sample their original `Title.bundle` crops from the already loaded title
atlas. Adjacent graves can share a GPU batch; row gradients retain their painter boundaries,
and the nine duplicate cropped PNGs no longer need separate startup textures.

Enemies and effect families use contiguous typed `Vec` storage. Explicit participant and
manager order still controls gameplay, painter allocation and RNG consumption. A general
archetype ECS is not part of this design; changes to entity storage must preserve those
ordering contracts and demonstrate an improvement in a measured workload.

`GpuUi::begin_frame` collects native input in dispatch order. Each `paint_next` releases
controller borrows so the Game owner can commit typed actions and refresh models before
the next event. Transport polling, gameplay input and simulation advance once per browser
frame. After the world draw, `paint_layout` refreshes the final shapes without replaying
input; `finish_frame` submits only those shapes and completes texture retirement even on
failure. Modal scope and actual hit geometry route input without hidden DOM controls. Native composition,
clipboard, file, audio and accessibility APIs remain browser adapters. `runtime_ui` carries
real completed image/font/audio counts to the separate Rust splash loader and releases it
only after account/save admission and the first complete GPU frame. Match transitions retain
their separate source delay and progress owner. The surface acceptance matrix and current
verification limits are in [RUST-UI-MIGRATION.md](RUST-UI-MIGRATION.md).

The GPU painter retains its vertex conversion buffer and reads mesh indices directly into
the renderer's retained submission buffer, without a separate triangle list. Adjacent
compatible meshes retain their scissor and sampler. A target, projection, texture, blend,
color or clip change flushes
the preceding batch; nested targets and screen-space scopes restore incoming state.

The read-only `gpu_ui_state()` reports only submitted geometry, readiness, capture and
visible widget bounds. Browser journeys use those bounds with ordinary input events.
The read-only `wizard_state()` diagnostic reports accepted simulation state. Actual WebGL
submission observations establish display cadence separately; simulation diagnostics alone
cannot prove smooth rendering between snapshots.
The [client presentation contract](CLIENT-PRESENTATION.md) records the source timeline,
field-family rules and focused replay coverage.

`SpellState::step_primary_players` owns one shared old-transient/projectile epoch, followed
by casts in participant insertion order and one called-rock / painter finalization pass.
`PrimarySpellEpochContext` supplies owner-aware world queries for retained actors, including
departed owners. Each `PrimarySpellPlayer` borrows its actual wizard, optional cast authority,
input and owner-specific cast context. College's missing authority preserves selection while
advancing its native clock. The single-player entry points use this same batch implementation.
The Boneyard adapter borrows the participant wizard separately from the shared world.

## Kernel kinds

Native spell presentation plans also live in `sim::spells`: stock record selection,
compositor colours, lightning geometry and boulder projection remain testable without a
browser. `web::spells::SpellView` owns the BadGuys/DeadHawg GPU atlases, prepares registered
world painters and submits the pre/post-world passes. Its atlas handles can be shared with
enemy and secondary views. `WizardView` draws the same inventory-projected clothing and
weapon layers in the world and in the authored Inventory preview pose.

| Kind | Examples | Shape |
| --- | --- | --- |
| Pure function | `hub_portal_at`, `painter_row`, `hub_seal_colors` | values in, value out, `#[must_use]` |
| Stepped state | `HubWizard::tick`, `HubAmbientState::step`, `NativeRng::draw_word` | `&mut self` once per tick; fields readable for tests and fixtures |
| Table | `HUB_PORTALS`, `HUB_COURTYARD_OBSTACLES`, `HUB_COURTYARD_SEGMENTS` | `const` arrays of `Copy` structs in reference order |
| Time function | `hub_teacher_phase_at`, `hub_teacher_burst_at` | absolute tick in, derived state out, nothing stored |

Floating point is `f64`, narrowed by `float::fround` exactly where the reference narrows,
with lengths through `float::hypot`. [DECISIONS.md](DECISIONS.md) has the reasoning and
[CODING-STANDARDS.md](CODING-STANDARDS.md) §3 the practice.

`actor_physics` owns the shared ordered circle-response solver. Courtyard and Boneyard
adapters supply static placement and swept movement; recursive push arithmetic and the
non-pushing enemy specialization have one implementation. Actor bodies retain source order.

`boneyard::world` owns scene state. Player movement, loot pickup, encounter/arena transition,
enemy simulation and late wave births keep the source phase order. The phased methods allow
the actual loot and enemy owners to interleave their work; they do not stand in for those
owners with empty results. `BoneyardCheckpoint` stores mutable owned values and identities.
Static collision and navigation rebuild from the matching retained scene; profile/vitals/run
lifecycle and the shared painter allocator remain session-owned.

Enemy geometry callbacks are per-tick adapters. Their ordered crowd uses `RefCell` because
the existing enemy query interface borrows `&self`; explosion/retirement feedback uses an
ephemeral `Cell` of two copied numbers for the same reason. The owner commits those numbers
after stepping. Neither adapter enters saved state. Logical light uses the incoming arena
camera captured by the movement phase; projectile views use newly active bounds and their
separate height-adaptive scale.

## Painter order and depth

The stock game draws world actors from one painter queue, and the port keeps that model
(`crates/sim/src/hub/painter.rs`):

- Every actor has a **registration ordinal**. The 48 fixed actors hold 0..47 in
  `HUB_FIXED_PAINTER_IDS` order; players follow from 48.
- An actor's **row** is `trunc((trunc(y) + trunc(bias) − trunc(refY)) / 2)`, and
  `hub_painter_order` sorts by row, then manager lane (actor, scenery, transient), then ordinal.
- The player's **sort bias** is ±20 inside the north arch rectangle
  (`courtyard_player_sort_bias`).
- Effects register **transient** painters for as long as they are visible: the teacher's
  burst column and frame strip (`HubTeacherBurst::needs_world_painters`).

Layers that are not actors draw in fixed **depth bands** around them (Website
`hub-depth.ts` and `HUB_WORLD_DEPTH` in `renderer/hub-render-contract.ts`): courtyard floor
0, seal glyphs 10, seal core 11, useful-thyngs shadow 900, fountain spray 980, pre-world
animation 999, the ordered actor queue at 1001 and up, post-world animation 2499, courtyard
foreground 2500 and up. `hubActorDepth` provides the initial `1000 + row` depth; the Website's
`NativeHubPainterPlanner.apply` replaces it with consecutive queue depths. This keeps northern
actors above fountain depth 980. `hub/depth.rs` lists all fifteen bands as data, including
the fractional onboarding depth. The statue's initial aura depth is `hubActorDepth(834) − 1`;
the active planner keeps it directly below the ordered body (`bodyDepth - 0.25` in the Website).

## Rendering

`crates/graphics/src/lib.rs` batches textured triangles over WebGL2. `Vertex` carries logical
position, atlas texel coordinates and tint; `draw_triangles` accepts typed three-vertex
triangles. Native attachment quads use `draw_quad`, and axis-aligned sprite `draw(Quad)` uses
that same primitive. Uploads clamp by default; `upload_with_repeat(image, true)` gives native
ground and road meshes repeating UVs. Per frame: `begin_frame`, draws, `end_frame`. Output is
premultiplied: `Add` is GL `(ONE, ONE)`, which is how Pixi's `'add'` resolves, and `Multiply`
darkens (the statue aura). Tints are quantised to 8 bits per channel with alpha separate, as
Pixi applies them.

The camera is a kernel: `hub_camera_origin` clamps the 1600 × 900 logical view
(`HUB_LOGICAL_VIEW`, scale `HUB_CAMERA_SCALE`) inside the region, and the client letterboxes
that view into the canvas. Screen-space layers (the HUD) draw after the
letterbox, in canvas coordinates.

## Assets

The stock atlases are PNGs and the stock `.bundle` files index them.
`scripts/sync-stock-assets.sh` copies both into the tracked `web/assets/stock/`, together with
the pinned Website's stock-derived unselected wizard attachments and registered teacher frames;
`crates/web/src/assets.rs` fetches them; `courtyard.rs` and `wizard.rs` name the records they
draw. `scripts/verify-client-assets.py` validates complete file membership and SHA256 values;
normal builds use these owned inputs without a sibling checkout. A record draws at its **registered origin** inside the room's logical frame
(`registered_origin`, or `registered_origin_scaled` for the seal at scale 2).

## Browser audio

`sim::audio` names the 229 retained stock cues, recorded voice limits, scene music and
source Create/College presentation helpers. It never advances a gameplay random stream.
Canonical actor events supply sound, rate and gain; the browser must not infer a cast,
hit or death from a rendered sprite. Spatial gain remains in `sim::region_feedback`.

`web::audio::GameAudio` owns one native AudioContext, decoded buffers, named streams,
owned loops and six HTML music channels. It resumes inside trusted document pointer/key
events and applies the saved sound/music percentages. `update` uses AudioContext time for
voice expiry and source-duration music fades. Loop owners share the loudest request;
changing its rate recreates the source, while changing only gain preserves playback.
`MenuUi`, `PlayerPanels` and `SkillPicker` queue `AudioEvent` values for one drain per frame.
World audio keeps its own authoritative event cursor. Narration duration stays in the
simulation; playing or muting a voice never moves that clock.

The asset sync script copies the existing Website audio files into the standalone asset
folder. The browser artifact contains Rust/WASM gameplay and ordinary media files, with no
Website runtime or Node requirement. Playback uses native AudioBufferSourceNode resampling;
it does not reproduce the Website's custom worklet interpolation sample for sample.

## Verification

Three layers, each cheap to extend:

1. **Native goldens** (`crates/sim/tests/native_goldens.rs`): the language-neutral fixtures
   in `crates/sim/tests/fixtures/native/`, captured from the instrumented retail executable
   with the native contract in each header. RNG bit-exact; movement within the fixtures' own
   epsilons.
2. **Website differential fixtures** (`crates/sim/tests/website_goldens.rs`): one Node script
   per kernel family under `tools/website-oracle/` imports the unmodified Website TypeScript
   and records what it does; `scripts/record-website-goldens.sh` runs them all; each test
   replays its fixture with `assert_eq!`. Fixture files start with `source` and `generator`
   and live in `crates/sim/tests/fixtures/website/`.
3. **Browser smoke** (`tools/browser-smoke/smoke.cjs`): headless Chrome with software WebGL
   loads the served client, checks the spawn, walks, confirms 100 Hz ticking, requires zero
   console errors and writes a screenshot.

Unit tests inside each module pin intent with hand-derived numbers; the fixtures pin truth.

## Module map

```
crates/sim/src
  lib.rs            crate doc, module list, the re-exports every caller needs
  constants.rs      tick rate, movement and collision constants from the native game
  float.rs          fround, V8 hypot, float32 bits
  rng.rs            NativeRng, the 55-word lagged-Fibonacci generator
  random_domain.rs  semantic word → bounded integer or scaled float
  heading.rs        24 facings, walk cycle, gait
  collision.rs      Vec2, rectangles, swept-circle moves
  actor_physics.rs  ordered actor contacts, recursive pushes and non-pushing enemy moves
  movement.rs       player and enemy movement, knockback
  appearance.rs     starter tints
  presentation.rs   wizard layer offsets
  hub.rs            courtyard world: spawn, camera, HubWizard::tick
  hub/segments.rs   the 129 courtyard segments
  hub/painter.rs    ordinals, rows, sort bias, fixed actors
  hub/depth.rs      all fixed drawing bands and initial actor depth
  hub/regions.rs    regions, portals, transitions, placements, scripted walks
  hub/ambient.rs    HubAmbientState: spray, seal, statue, teacher clock; colours
  hub/teacher.rs    teacher cycle, frames, burst
crates/assets/src
  bundle.rs         parse_bundle, SpriteRecord
  boneyard.rs       retained catalog, scene validation and seed materialization
  navigation.rs     identity-checked prepared navigation decoding
crates/web/src
  lib.rs            Game, frame loop, start()
  assets.rs         fetch atlases and bundles
  input.rs          one browser sample for keyboard, pointer, gamepad and both touch joysticks
  hud.rs            stock HUD; explicit state in, typed actions out
  menu.rs           title/Create/pause/settings navigation and typed session actions
  menu/title.rs     title art and seeded grave rows
  menu/create.rs    native hands, element effects and identity selection
  menu/settings.rs  offline preferences and control rebinding
  menu/navigation.rs controller edges, focus ownership and neutral handoffs
  player_panels.rs  Inventory/Skillbook/College surfaces, pointer capture and typed commands
  player_panels/    stock panel art, learned pages, chat, services, dye and Unforge
  ui/               shared GPU item, skill, portrait and native text presentations
  gpu_ui/           browser input, fonts, stock primitives and UI mesh submission
  runtime_ui.rs     counted startup handoff and GPU runtime status
crates/graphics/    shared WebGL2 quads, blend modes, textures and decoded assets
  courtyard.rs      the courtyard scene: background, ambient, actors, foreground
  wizard.rs         the player's layers
  teacher.rs        teacher, rune, shadow and the four release painters
tools/website-oracle   oracle.mjs and one *-goldens.mjs per family
tools/browser-smoke    smoke.cjs
scripts                record-website-goldens.sh, sync-stock-assets.sh
web                    Trunk root: index.html, assets/, dist/
```

## Where a feature goes

| You are adding | It goes in | With |
| --- | --- | --- |
| A rule, number or order from the game | `crates/sim`, in the module whose domain it is | unit tests and a fixture |
| A new Hub subsystem (rooms, NPCs, depth) | a new `crates/sim/src/hub/<name>.rs`, listed in `hub.rs` | its own `*-goldens.mjs` |
| A region beyond the Hub | `crates/sim/src/<region>.rs`, with a `<region>/` folder once it splits | its own fixtures |
| Something drawn | `crates/web/src/<scene>.rs`, called from `render()` at its band | a smoke check |
| Something read from the browser | `crates/web/src/input.rs` or a sibling, sampled once per frame before the ticks | a smoke check |
| A stock file format | `crates/assets` | a test against the stock file |
| A dependency or a new seam | a [DECISIONS.md](DECISIONS.md) entry first | |

## Player and College surfaces

`PlayerPanels` consumes a borrowed `PlayerPanelSnapshot` and returns typed actions. It never
updates canonical resources, inventory, ranks or progression itself. `inventory_action_feedback`
accepts the actual `HubEconomyResult`: only acceptance advances a book, spell or boast response,
and rejection keeps an active Dye transaction available. Authored notices are projected by
`sim::player_panels::native_inventory_rejection_notice`; ordinary rejection uses the native
bad-action cue instead of displaying an internal reason code.

`show_dialogue(HubDialogueContext, &NativeHubNpcState, now)` owns only reading, selector
scrolling and focus. `show_service(HubTraderId, now)` shares the native companion Inventory.
The simulation's `hub::interaction` owns all dialogue, selector rows and click/range admission.
`is_open()` blocks gameplay input; `covers_world()` distinguishes the full player/shop surfaces
from chat, which leaves the world and HUD visible. Opening a conversation acknowledges its
canonical hint through an action; closing Hagatha or Shlorio queues their actual close command.

Native frames have an explicit local origin. Without it, a frame appended inside a
centered browser button starts at the button center and draws across neighboring rows. Browser text inherits
the player-panel font inside native buttons, so selector titles retain the intended line layout.

`SkillPicker` is a separate modal because its lifetime follows the canonical progression
offer, rather than Inventory navigation. `update(offer, special_available, now, width, height)`
retains only the displayed offer through reveal/close. Its typed `Choose(PlayerSkillChoice)`,
`Reroll { offer_sequence }` and `Save { offer_sequence }` actions leave all rank/RNG changes
to the session. Pointer activation retains the displayed sequence; stale events cannot choose
from a replacement offer. `reject_action(now)` restores the still-authoritative offer after
an admission rejection. The canonical automatic index alone drives automatic Boast selection.

## Shared session authority

`GameSession` owns a `BTreeMap<String, SessionParticipant>` and an explicit
`participant_order`. The map provides stable identity lookup; the order preserves the
Website's entity insertion order for actor physics, casts, painter births and player
feedback. Level-up cohorts are separately sorted. Each participant owns its one
`HubWizard`, profile, resources, lighting, College controller and score. `BoneyardWorld`
borrows characters for movement and enemy callbacks and retains no player copy.

A tick accepts the participant input map and advances each shared actor epoch once.
The global damage packet stream is not regrouped by recipient. Secondary actors use
source-sorted player rows; primary actors advance once before ordered player casts.
Deferred enemy projectile painters commit once before late contacts and snapshots.
Public inventory, belt, selection, skill, Goodie and membership commands apply the
same admission and world side effects for browser and native hosts.

`GameSnapshot` borrows the mutable authority using the owned `GameCheckpoint` JSON
schema. Applying a same-world checkpoint retains immutable collision and compiled
wave catalogs; a changed world requires matching retained scene identity. Restores
and network application validate participant order, run/cohort membership, world
identity and wave references before mutation. Durable checkpoints remain owned and
infrequent. Active-run detach stores only one participant; Hub import and fresh-profile
retirement are explicit operations rather than cloning a Session per connection.

`solomon-assets::save_document` uses schema 2 for that participant ownership. Schema 1
was a single-player candidate and is rejected explicitly before checkpoint decoding;
there is no guessed identity migration. The codec validates every saved skill book and
wizard heading, then calls the session's canonical shared-identity validation.
Detached rejoin projections clone only the single detached participant and small
RNG/barrier/order records. Their snapshots borrow the running Session's retained
participants and shared world, adding the staged participant to the same checkpoint
schema. Accepted detached skill transactions commit that participant and gameplay
RNG; stale/rejected commands commit nothing. Private lighting enrollment never consumes
the active painter allocator. The native direct host owns one shared party-run. New Game
requires that shared run to finish before replacing a participant; it cannot reset peers.

## Native authority boundary

The launcher consumes transport events on one 100 Hz authority thread. `GameSession`
owns every gameplay participant and shared actor; the host owns connection generations,
input-edge queues, snapshot ACKs, renderer readiness and wall-clock pause/resume holds.
Normal 20 Hz snapshots prepare one borrowed `GameSnapshot` into a shared canonical JSON
document. After selecting the latest pending publication, each socket compares it with
the previous document actually selected for that connection and sends compressed changes
when smaller than a complete message. The browser reconstructs every ordered packet before
coalescing completed checkpoints. Both endpoints retain one baseline; snapshot ACKs and
flame history remain independent. Full state and numeric precision are preserved.
Pending rejoins use the canonical detached projection of one participant, borrowing the
same running world. Reliable command feedback names its completed publication boundary.

The checkpoint writer accepts at most one owned completed checkpoint. JSON encoding and
atomic synced file replacement happen outside the tick deadline; revision cursors coalesce
later requests. Welcome waits for the new reconnect credential to become durable, and
SaveCommitted follows the real file completion. A title probe reads only admission status.
Native per-listener combat cursors produce reliable one-shots from accepted state and
actual listener cameras; browser snapshots retain loop and narration-mix ownership.
## Direct-host social surfaces

`sim::social` defines the host-authored membership, accepted chat and profile records.
`web::social::SocialUi` borrows that snapshot and the canonical ally projection; it owns only
open surfaces, draft/channel/unread state and presentation fade. `take_actions()` returns
`SocialAction` commands. Sending never inserts a message or changes membership locally.
`input_active()` seals local gameplay input while chatting or using a dialog; it does not
pause the shared Boneyard. World/menu admission is supplied through `SocialUiContext`.

A native direct session has no global broker. Absent directory/global capabilities hide
codes, discovery, invitations and the Global channel. The actual roster, player cards,
Whisper, leader Kick and Leave remain available. Controller navigation reuses `MenuNavigation`;
Open Chat reuses the normal binding machinery and the source default T key.

Chat/profile/ally GPU layouts preserve the pinned Website's geometry and locally packed
font faces. Party chrome uses stock records and the same message frame as title confirmations.
ControlPanel's retail bundle interleaves font metrics: asset sync projects its existing
manifest into the sprite-only bundle codec; no font engine or runtime Website is required.
