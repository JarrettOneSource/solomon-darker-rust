# Decisions

Choices a maintainer would otherwise be tempted to reverse. Each records what was decided,
why, and what it costs. Add an entry when a change opens a new seam, adds a dependency, or
reverses one of these.

## Authored content and Lua

**Shared authored-content values live in `sim::modding`; Lua VMs belong to a native-only
`solomon-modding` crate.** This follows the pinned Website's `PreparedModHost`, which borrows
the same `GameSimulationState` used by stock play. Compiled definitions, accepted asset
descriptors and runtime checkpoints contain ordinary values. Lua function and migration
handles stay in the native host, keyed by reducer identity. They never enter a Session
snapshot, gameplay RNG or browser presentation store. The source manifest/graph identities
and `PreparedModHostCheckpoint` are persisted alongside the canonical Session, so a reload
recreates the VM from the admitted package rather than serializing executable handles.

The host will use **`mlua` with `lua54` and `vendored`**: no existing workspace dependency
executes Lua, the standard library cannot do so, and a handwritten interpreter would narrow
the source language. The pinned Website uses Wasmoon and actual Lua 5.4. `mlua` embeds that
language rather than substituting Luau or LuaJIT; vendoring makes the native package
independent of a system Lua installation. Its safe interface supplies
[memory limits and execution hooks](https://docs.rs/mlua/latest/mlua/struct.Lua.html), used
for the source 16 MiB VM and explicit definition/callback/tick budgets. Only source-enabled
names and accepted package includes are exposed; filesystem/process/module loading remains
unavailable. The pinned sandbox retains Lua's `math.random` and `math.randomseed`; their
private VM state does not advance gameplay RNG. Native callbacks share a private
`Rc<RefCell<Registry>>` because real Lua functions outlive the constructor; callbacks drop
the borrow before executing Lua or its table conversions. This is an enabled-content trust
boundary, not optional defensive behavior.

Cost: native hosts gain the embedded Lua build. `mlua` documents
[WebAssembly through Emscripten](https://github.com/mlua-rs/mlua#readme), whereas this client
uses `wasm32-unknown-unknown`; therefore client and pure simulation values do not depend on
the VM crate. Definitions/rules/reducers execute once in the Rust authority. Source timing
limits are host admission budgets, not a simulation clock or a reason to approximate Lua.

**Content graph digests retain JavaScript collation and numeric JSON spelling.** Source
`web-lua-definition-compiler.ts` sorts graph keys with `localeCompare`, then hashes
`JSON.stringify`. The pinned Node resolves `en-US`: valid keys `a_b`, `a-b`, `a.b` sort in
that order, unlike Rust byte ordering. Its numeric spelling also differs (`1`, `1e+21` and
`0` for negative zero). The native content crate uses Unicode's
[ICU collator](https://docs.rs/icu_collator/latest/icu_collator/) with the captured locale,
[ryu-js](https://docs.rs/ryu-js/latest/ryu_js/) for ECMAScript numbers, and the cached
[RustCrypto SHA-256 implementation](https://docs.rs/sha2/0.11.0/sha2/). Existing serde_json
still parses and escapes JSON. This avoids handwritten Unicode/number/cryptographic
algorithms and leaves the pure simulation/client dependencies unchanged. Ordered Lua
objects additionally enumerate JavaScript array-index keys numerically before other
properties. The focused digest oracle checks punctuation, case, accents, index keys,
number boundaries and final SHA-256 values; new ICU data requires that oracle to remain
green rather than assuming every library version has identical collation.

## Native multiplayer messages

**Deployment publications have identities separate from signed game recovery revisions.**
The native drain still signs the target Rust revision into the final save and accepts
the existing checkpoint-sequence/revision acknowledgement. Its authenticated
`X-Solomon-Deployment-Id` and `X-Solomon-Deployment-Sequence` headers name the fresh
Website cutover and next publication sequence; both
travel in the restart announcement and drain receipt. Website owns the mutable
`/api/game/deployment` publication with `revision` (Website), `gameRevision`,
`deploymentId`, monotonic `deploymentSequence` and `ready`. Its worker stages `ready: false` and atomically publishes
`true` only after the replacement game and Website pass health and integrity checks.
The immutable game's deployment document, manifest membership and hashes remain intact.

The page can reload an announced update only after its final save receipt commits,
its actual old transport retires, and a healthy publication has the announced sequence
or a later sequence, with the same signed target game revision.
The UUID correlates one drain/receipt; the sequence proves replacement ordering.
Exact UUID matching would strand a sleeping tab that missed its publication or a
healthy retry with another UUID. Failed candidates restore the prior publication;
only readiness after verified health fulfills the announced sequence. Active sessions
cannot reload through idle polling.
Title/idle observes healthy publication changes, including Website-only replacements.
This corrects the inherited `live game revision != current game revision` condition,
which stranded a saved client when Website redeployed the same immutable game.

Cost: publication-aware authorities/clients use deployment protocol 2 alongside the
unchanged gameplay protocol. A Website worker may perform the first changed-game
drain against the legacy authority using the original strict JSON request body; its
extra header is ignored there. An unchanged-game drain against a legacy authority is
rejected before retirement because its immutable clients cannot recognize publications.
The first-cutover legacy manifest bridge remains until those tabs have transitioned.

**Completed score timestamps use the installed native `time` crate at the launcher
edge.** Source signs `new Date().toISOString()` as part of its account-bound receipt,
including exactly three millisecond digits. The standard library supplies elapsed
time but no UTC calendar formatter; a custom calendar or unsafe platform conversion
would add an unrelated implementation. `time` with its formatting feature supplies
the fixed Source representation. Neither the pure simulation nor the browser gains
a wall clock or this dependency. Compact previous-score facts detect completion;
the current authoritative participant supplies the actual Hall entry.

**One shared Rust protocol crate carries canonical simulation values.** The native host and
WebAssembly client use `solomon-protocol` version 1 for direct join. It intentionally does
not claim compatibility with the Website's version 129 protocol. The host serializes a
borrowed `GameSnapshot`; the client decodes its identical `GameCheckpoint` shape. Snapshot
ACKs are keyed by participant, so all peers share one serialized complete world rather than
personalized copies. Reliable commands, accepted audio cues and save receipts remain
separate from replaceable snapshots. Connection credentials, readiness, pause deadlines
and input queues stay in the launcher; the sole `GameSession` owns gameplay. Cost: protocol
changes require the native host and bundled client to advance together.

The native launcher uses `getrandom` (already present through Tungstenite) for host
credentials and run seeds, keeping OS entropy out of every gameplay RNG stream. The small
native-only `ctrlc` crate supplies a portable shutdown handler: the standard library has no
cross-platform signal registration and this repository forbids application `unsafe`. Its
handler only sets a stop flag; the normal authority loop saves and closes its own resources.
See the [crate's documented handler contract](https://docs.rs/ctrlc/latest/ctrlc/fn.set_handler.html).

**A disconnected private Tutorial retains its sole participant.** The source Tutorial
tick and checkpoint require exactly one player; the pinned Website host asserts after
removing its final Tutorial socket. The native adapter retains that actual participant,
freezes the world without a connected player, and resumes the same durable record.
Ordinary party runs use canonical detach/remove/rejoin. This is a transport lifecycle
choice, not another player simulation or a fabricated Tutorial state.

## Native display timeline

**Accepted authority and display frames have separate owners.** The pinned JS client
interpolates Boneyard snapshots and applies bounded local movement prediction only in the
Hub. The Rust browser keeps at most eight frozen accepted Sessions and samples a private
display copy, preserving source actor identity and discrete transition rules. Fractional
clocks and endpoint appearance data accompany that copy because canonical tick types stay
integral. Camera, light, geometry and painter consumers all use the same sampled frame.
Commands, saves, accepted audio and acknowledgements keep reading authority. No Boneyard
sampling step advances gameplay or its random streams; Hub prediction uses the canonical
movement/collision kernel on private state for at most one snapshot interval. Cost: bounded
Session copies per native client; immutable storage can be shared later if measurements
justify changing ownership. The Metal GPU submission comparison established the actual
20 Hz transform holds before this cutover.

## Numbers

**f64 arithmetic, narrowed with `fround` at the reference's rounding points.** The Website
computes in JavaScript doubles and calls `Math.fround` where the native game stored a single.
Computing in `f32` throughout would round at every operation and diverge; computing in `f64`
and rounding at the same points reproduces the doubles bit for bit. Cost: every port has to
find the rounding points by reading, and a missed one surfaces as a fixture failure.

**Lengths use V8's `Math.hypot` algorithm** (`float::hypot`: scale by the larger operand, sum
with Kahan compensation, square root). `sqrt(dx*dx + dy*dy)` differs in the last bit often
enough to move a collision response. Cost: a dozen lines that look like a slow `sqrt`.

**Source angle calculations use the pinned V8 `atan2` implementation.** At template-9
Solomon tick 940, equal route-goal operands produced a one-ulp macOS libm difference that
later grew into the measured 27.38-unit escape-path defect. `float::atan2` ports fdlibm's
constants and the fused evaluation witnessed in the actual Node 22.17.0 ARM64 binary;
`f64::mul_add` keeps that evaluation explicit on native and WASM. All 7,697 recorded IEEE
cases compare exact result bits on both targets. Its Sun/V8 license ships in
`docs/third-party/v8-fdlibm-LICENSE`. Sine, cosine and the measured Fire power envelope now
use the same pinned numeric owner, as described below. Historical trajectory fixtures retain
their named bounds; no assertion that a downstream loop cannot amplify an ulp is justified.

**The ambient state owns its own xorshift sampler**, separate from `NativeRng`. The Website's
`hub-ambient.ts` seeds a `u32` xorshift (`0x2f6e2b1d`) that only the courtyard life consumes;
sharing the native generator would entangle the spray with the collision fallback draws.

## Shape

**Native save containers retain their unknown bytes and use portable installed codecs.**
The assets boundary ports the source SyncBuffer tree, custom Darkdata XOR/LZ stream,
launcher ZIP manifest and portable profile contract. Known profile/wizard fields are
patched inside the retained native tree; opaque native inventory and scene siblings are
not reconstructed from guessed values. Base64 attachment bytes use the already installed
`base64` crate. ZIP framing remains the source's small format, including its exact
validation and deterministic stored output; compressed input uses installed pure-Rust
`miniz_oxide`, and SHA-256 uses the existing content package dependency. These readers run
on native and WASM without a JavaScript codec or an additional archive framework.

**Navigation retains source eager preparations and builds other domains from actual geometry.**
Stock assets still carry the source/geometry-checked native meshes. The loader installs only
the domains `boneyardWorldNavigationPreparations` actually warms: hostile bounds at 25/50
clearance, plus expanded escape bounds when the Default source owns Solomon. Imported Mod
scenes build those graphs in Rust. The builder translates the source's ordered constraint
sampling and incremental Delaunay triangulator; another library could change triangle and
neighbor order, so no triangulation dependency is added.

A private geometry-owned cache builds a missing domain from its first query's current
collision, including moving gates, and retains it thereafter. Clear direct routes do not
initialize the cache. `NavigationSet` clones share immutable graph handles under a small
mutex, matching the source collision-geometry identity. `CatalogNavigation` retains those
owners across materializations of the same stock template or admitted Mod entry, matching
the source catalog's shared scene objects. A cloned catalog shares Mod entry identities;
replacing an entry creates a fresh identity even when its bytes are equal. Decoded saves
use the fresh navigation loader and never borrow a catalog owner. The native host owns one
cache beside its catalog; the browser content owner uses the same shared asset module.
Eight complete graph fixtures, 24 routes, four source-kind preparation inventories, a
100-tick moving-gate case and repeated catalog/decoded/replaced-entry checks verify these
lifetimes without a stock-map substitute.

**Independent trajectories retain their own spatial state between source comparisons.**
The assets escape replay continuously advances all 6,695 transitions across thirteen scenes;
it replaces the earlier per-transition spatial-input overwrite test. After the `atan2` fix,
template 9's largest Euclidean position delta is 6.915300366811401e-13 world units and heading
delta 5.172751116333529e-12 degrees. The largest position delta across all thirteen cases is
9.927276321703277e-13; nine cases are spatially exact. The existing 1e-9 trigonometric bound
is unchanged, and RNG, phases, timers and overlap IDs remain exact. These measured paths do
not establish whole-game bit-identical trajectories. Receipts live in
`codex-runs/solomon-rust-parity-20261021`, job `job_20261021T051936Z_0092923486`.

**The offline Boneyard bank ships as retained stock geometry, decoded only at the browser
edge with the workspace's existing serde dependencies.** The Website already selects among
twelve retained native generator outputs and a separate stock tutorial. Copying those complete
data records preserves that contract without a TypeScript host at runtime. `solomon-sim`
keeps its dependency-free scene and world types; `solomon-web` translates JSON into them.
Cost: the browser artifact includes the 1.8 MB geometry bank and a small explicit wire model.

**Visible game UI belongs to the Rust GPU renderer.** The October 23 full-port requirement
supersedes the earlier DOM presentation decision below. Menus, HUD, panels, social/mod
screens and lifecycle overlays use the existing WebGL2 canvas, stock atlases and native
layout rules. The shared `gpu_ui` module uses egui for Rust interaction, text/layout and
tessellation; it does not replace the game loop or simulation. Browser font rasterization,
text composition, clipboard, file access and other platform capabilities remain adapters,
not a visual HTML/CSS implementation. A CSS interpreter, DOM screenshot renderer, hidden
legacy UI or synthetic-action test facade would not satisfy this contract. The live migration
inventory and acceptance state are in `RUST-UI-MIGRATION.md`; implementation is in progress.
Stock bitmap text keeps its recovered glyph layout. Ordinary system/licensed font glyphs
use the browser font adapter; Sack names preserve Source's complete font-run shaping and
wrapping, including Arabic joins and emoji sequences, in bounded egui-owned GPU textures.
The font adapter paints only text into an unattached canvas, never HTML or game controls.

**Native font geometry and Rust editing share one text layout.** The installed egui
row/cursor model assumes increasing horizontal positions and cannot represent mixed
bidirectional caret/selection geometry with its per-grapheme browser font hook. The shared
GPU text editor therefore uses the browser's standard SVG text and Range font measurements
and Canvas font rasterization, while Rust owns values, selection, interaction and drawing.
The transient, invisible SVG is a noninteractive font-run measurement adapter; it contains
no controls or game-layout proxy. The existing nonvisual input adapter carries native
composition and clipboard operations. This preserves installed/licensed fonts and full-run
joining without shipping system fonts or maintaining a second text editor.

The editor reuses the already installed `unicode-segmentation`1.13.3 and the small
`unicode-linebreak`0.1.5 UAX#14 implementation for grapheme boundaries and wrapping. This
keeps Unicode tables in maintained libraries instead of hand-written character rules.
Browser rendering/input acceptance must cover the complete caller cutover, including
multiline editing, joined scripts, selection, undo and composition before completion.

The one nonvisual textarea carries accepted full-value/UTF-16 selection snapshots and
immutable native edit intents. Field-owned normalization and validation run synchronously
before the next native dispatch. Rust replays those intents between the existing ordered
UI passes, so an earlier Clear/history action cannot be overwritten by a later stale
transport snapshot. Reconciliation acknowledges delivered or discarded receipt sequences;
an earlier layout cannot retire a later native field/composition. Clipboard events write
their trusted native data synchronously. Enter submission is a per-dispatch result and
does not surrender single-line focus.

Editing history follows native operations rather than a render-time timer: continuous
typing/deletion can stay one command across a pause, while cut, paste, selection changes
and complete composition delimit commands. A small `VecDeque` history owns before/after
values and UTF-16 selection together, retaining the existing undo-depth budget. The generic
egui Undoer's timed, opaque snapshots cannot preserve these native clipboard transactions
and cursor states. The independent Chrome cut/paste and paused-typing probes retain the
reason for this boundary. Native selection is sampled before key/default-input/clipboard
work, so a coalesced selectionchange event cannot make an immediately following paste use
an earlier caret.

Font rows preserve complete native shaping, disjoint bidi selection rectangles and
pointer caret affinity. UAX#14 wrapping uses native `Intl.Segmenter` dictionary boundaries
for complex-context scripts. Browser tabs keep their original accepted value; only font
raster input substitutes the nonprinting U+001F segment separator so Canvas does not
normalize TAB into a space and change bidi ordering. Native full-context font pixels are
positioned at the Source tab stops (Console: two spaces) with the same geometry used for
selection. The native tab/embedding and selection probes are retained in the October23
full-UI task. This is font geometry and text rasterization, not a rendered DOM control.

**Superseded: the HUD used browser DOM with stock atlas sprites and repeated strips.** The world stays
in WebGL; the screen-space HUD uses the browser's existing layout, hit testing and accessible
buttons. `web/ui.rs` reads the same stock bundles as the renderer and projects the sprites,
strips, mirrored frames and button art these screens require. This keeps the native art
without introducing a UI engine.
The HUD reads an explicit `HudState`; the local courtyard begins with the Website's fresh
Ether loadout, and an authoritative runtime can replace those values without DOM ownership.

**Menus emit typed commands; the session owns game state and persistence.** Title/Create,
pause and settings share stock art and Rust GPU hit testing. Create emits `NewGameSelection`,
and settings emits `GameSettings`; views never grant ranks or fabricate inventory. Modal
input is scoped to its active surface and requires a controller-neutral handoff. Cost:
the host must apply the preferences and supply the actual saved session behind Continue.

**No engine crate.** Bevy, macroquad and friends bring their own loop, scheduling and asset
pipeline, and the game's whole value is that its loop, order and numbers are the native
ones. The renderer is a few hundred lines of WebGL2 over `web-sys`. Cost: shadows, text and
particles are ours to write.

**The default `solomon-sim` build has no dependencies.** Hosts that
save games enable the optional `persistence` feature, which derives serialization on the
canonical state using the workspace's existing serde. This avoids a second mutable model
and hand-maintained field copies for every actor. The simulation still performs no I/O.
The storage boundary owns format versioning, validation and write conflicts; the map loader
reconstructs immutable scenery and checks its identity before restoring mutable state.
Cost: serialized field changes require a save-schema decision, and host-enabled builds
include serde. Fixed-size native arrays retain their lengths through one shared adapter.
The complete save-document codec lives in `solomon-assets`, alongside identity-checked
world reconstruction. Browser IndexedDB and native files use that same boundary.

**Browser and native hosts use the same retained world decoders.** `solomon-assets`
owns the existing Boneyard catalog and navigation decoder, depending on `solomon-sim`
for their output types and the workspace's serde/serde_json for the stock JSON format.
The browser retains only fetch/upload work. This lets the native authority load precisely
the same geometry without depending on browser code or duplicating format validation.

**Owner overlays are promoted after native row planning.** The source's spell view
raises selected roots to at least the owner's display depth plus 0.5. Its Pixi parent
sorts children in place, retaining the preceding display order on equal depths. The
shared presentation cursor retains view identity, creation order and empty-view depths;
it never changes canonical registrations, rows or insertion admission. Courtyard and
Boneyard pass their actual assigned depths, including the latter's grouped static bands.
The source promotion method and installed Pixi sorter generate the regression fixture.

**Large checkpoint records are heap-owned at the save boundary.** `GameCheckpoint` boxes
its player and optional Boneyard records; serde writes the same JSON shape. Debug wasm
deserialization otherwise retains several large nested return slots, exhausting the stack
even with ample linear memory. The real reload journey measured a negative stack pointer
at the first store of a derived visitor. Heap-pinning the loading future and boxing these
two records removes that duplication without increasing the stack limit or changing live
simulation ownership. College and Boneyard reloads verify the complete boundary.

**Trunk with wasm-bindgen, and no JavaScript bundler.** `web/index.html` is the only
hand-written web file; Trunk builds the wasm, the loader and `web/dist/`. Cost: Trunk's
version matters (0.22 on the M2, fetching a matching wasm-bindgen CLI).

**The browser edge returns `Result<T, String>`.** Startup failures (a bundle that is too
short, a missing WebGL2 context, a fetch that 404s) surface as one sentence in the page's
status line. A typed error enum would add variants nothing matches on. Kernels never return
`Result`; their inputs are typed so invalid input does not exist.

**A 100 Hz accumulator with a 250 ms catch-up cap.** The native game ticks at 100 Hz and the
Website's accepted-frame model does the same; a cap stops a backgrounded tab from running
seconds of simulation on resume. Cost: a tab hidden for a minute loses that minute.

**Website kernel names survive the port** (`hubPortalAt` → `hub_portal_at`,
`stepHubAmbient` → `HubAmbientState::step`, constants verbatim in `SCREAMING_SNAKE`). A grep
on either side finds the other. Cost: a few names that are not what a Rust author would
choose first.

**Fixed-actor tables live in `sim`; the art that dresses them lives in `web`.**
`HUB_COURTYARD_OBSTACLES` carries position, radius and sort bias because they are rules;
`OBSTACLE_RECORDS` in the web crate carries the sprite records because they are art. The
two arrays are index-aligned and each says so.

**The ambient state owns the teacher's registrations for one release.** The Website reserves
two transient painters while the column or frame strip is visible, retaining those ordinals
until both disappear. In a world, the ambient step uses the shared `WorldPainterOrder`; in
the standalone oracle it derives the source fallback ordinals from release order. This keeps
teacher, enemy and spell births in one manager sequence without a second registration owner.

**Painter insertion bands keep their owner's registration.** Air Lightning's invisible
ZAnimSplit owner queues 25-unit bands when its row drains. Those children append after
already-queued roots, including roots from other manager lanes. `PainterLayer::is_insertion`
and `visible` retain that source admission rule in the shared planner; allocating new actor
ordinals or sorting a flat list would change occlusion. The source cases live beside the
ordinary depth/manager oracle in `hub-depth-goldens.json`.

**Released Ether Boulder pieces draw with their actual Actor registrations.** The pinned
Website release spreads the parent's painter list onto every piece while allocating distinct
logical Actor registrations. Its own strict painter planner then rejects the duplicate painter
ordinal. Rust preserves those source simulation rows for replay, and its view uses each piece's
actual Actor registration, keeping the source's shared light/painter ownership contract without
allocating extra IDs. The strict planner remains unchanged. The matched release capture and
original source error are retained in the fleet's `visuals-weld-gpu` receipt.

**Room collision uses the same swept circle against segments as the courtyard.** The Website
expresses every Hub room as a segment table with a table-to-world offset; porting those
tables keeps one collision kernel. Rectangle collision in `collision.rs` serves the knockback
and enemy movement kernels, which the Boneyard port will use.

## Verification

**Gameplay input has one browser sample and a camera-owned aim projection.**
`MovementInput::sample` owns the native gamepad read for movement, primary and belt levels.
It returns canvas-normalized pointer points, player-relative directions, a facing request,
or `Retain`; `None` clears aim. The Game maps requests through its actual renderer viewport
and preserves the last world target after mouse/touch release. Controller aim continues to
reproject while the player moves. The same adapter clears held input on UI blocking, blur,
page hide and touch cancellation, with a neutral controller handoff after modals.

**Belt presses and held casts are distinct.** Queued `belt_pressed` intents activate items
once, while `quickbar` supplies the held secondary level. GUI taps activate on release and
provide one sampled pulse if they begin and end between frames. The session processes this
input path and ignores the HUD's legacy click notification for that same pointer press.
The HUD's controller outline is view context from the input sample, including empty slots.

**Player panels borrow canonical state and emit commands.** `PlayerPanelSnapshot` carries
the actual skillbook, economy, belt, progression, resources and session-derived attributes.
The DOM owns selection, pointer capture, learned-page placement and modal navigation.
`PlayerPanelAction` wraps the existing inventory and belt commands; the session applies
them and supplies the next snapshot. Item selection retains its source ownership, so an
equip or consume cannot turn a stale backpack selection into another equipment command.

**Native tooltip numbers preserve JavaScript's decimal rounding.** Rust's formatter uses
ties-to-even; the pinned source's `toFixed` rounds exact halves away from zero. The panel
kernel formats the original binary significand rather than multiplying the float first.
This preserves both `2.5 → 3` and `2.55 → 2.5` at their respective authored precisions.
All authored stat/bonus templates replay against the source formatter.

**Fixtures are JSON written by one Node script per kernel family**, read with serde's
`float_roundtrip` so every double survives the trip. Each file starts with `source` (the
Website file) and `generator` (the script), so a reader can regenerate it. A single large
script would make every ticket conflict; a per-family script lets tickets add fixtures in
parallel.

**Fixtures are committed.** The tests must pass on a clean checkout without Node; the
scripts exist to regenerate when the Website checkout moves.

**The old Mod Loader is retired and nothing here reads it.** This rebuild replaces it. The
three retail recordings its tests used (`rng-goldens`, `float-rng-goldens`,
`movement-goldens`) are copied into `crates/sim/tests/fixtures/native/` with their headers
intact, so the native evidence travels with this repository. Reverse-engineering notes are
cited from the Website's native-parity ledger (`../Website/docs/game-native-parity-re.md`)
or by native address.

**Unit tests use hand-derived numbers; fixtures carry the truth.** A unit test that merely
restates a fixture value explains nothing. A unit test that derives a value (a cycle of 847
ticks is 268 cast, 79 release, 500 idle) shows the reader why.

## Operations

**The standalone launcher serves bundled assets on a fixed loopback origin.**
`solomon-launcher` uses the standard library to serve the compiled client and opens the
installed browser. The package carries the complete `web/dist` tree; Website, Node, Rust
and Trunk are build/development tools only. Port 4188 is stable because IndexedDB saves
belong to an origin. A busy port produces an error instead of silently moving saves to a
new origin. Four bounded file readers support concurrent atlas loads and single audio byte
ranges. This host adds no simulation rules or new dependencies. Cost: the game uses the
system browser and the launcher process remains open while playing.

**The service binds loopback and `tailscale serve` publishes it.** Nothing listens on a LAN
interface; the tailnet is the only route. The launchd agent `com.jarrett.solomon-rust-game`
owns port 4180 and serves the main checkout's release build. Cost: a developer's dev server
uses another port, and an edit in the main checkout rebuilds the served site, which is why
agents work in worktrees.

**`rustfmt.toml` sets width 100 and `use_small_heuristics = "Max"`.** Reference tables and
struct literals stay on one line each, so a row of a table reads like the row in the source.

**Native WebSocket transport uses synchronous Tungstenite over standard TCP.** The launcher
reuses its existing HTTP asset responder; upgrades to `/session` leave the four asset workers
after the handshake. The browser uses its native WebSocket feature. One caller-owned
authority loop consumes connection/message/disconnection events; transport workers never
interpret JSON or run a simulation. Reliable output is bounded by count and bytes, while one
replaceable `Arc<str>` snapshot can share a serialized world across peers. Event pressure
blocks socket workers instead of losing critical participant lifecycle events; shutdown
drops the receiver and closes owned sockets before joining workers. Cost: one native-only
protocol dependency and explicit thread/queue ownership, without an async web framework or
another implementation of RFC 6455. Gameplay credentials, input sequencing, pause/readiness
and session ownership remain in the host above this transport.

Established WebSockets use nonblocking TCP; HTTP and the upgrade handshake retain their
blocking deadlines. The real Windows package lost its College connection when a blocking
send reached its 250 ms deadline while the native process stayed alive. Windows documents
that a timed-out blocking send leaves the connection indeterminate, so extending or retrying
that kernel deadline is not the transport contract. The standard library's nonblocking
mode gives the installed Tungstenite codec `WouldBlock` instead. The codec retains the
partially written frame; the worker flushes those bytes before taking another message.
Remaining reliable messages stay in the bounded outbox and its single unsent snapshot
remains replaceable. Idle polls keep the existing 10 ms cadence, and the existing missed-pong
and close deadlines still retire stalled readers. No async runtime or new dependency is
needed. An explicit close starts its deadline when observed, allowing preceding reliable
bytes to drain within that same deadline even when its close frame cannot yet be queued.
The actual TCP slow-reader tests require an intact 16 MiB frame exactly once,
ordered subsequent control, the latest snapshot and resumed input, plus heartbeat expiry
and explicit-close identity/deadlines when reading never resumes. See the
[Windows socket contract](https://learn.microsoft.com/en-us/windows/win32/winsock/sol-socket-socket-options)
and [Rust nonblocking TCP](https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_nonblocking).


**Native portable profiles use the source attachment and one canonical JSON writer.**
The import/export bridge retains the original CFG, SAV and opaque slot files with their
canonical base64 and SHA-256 identities. The portable settled-Hub fields are separate from
that byte-preserved native state, matching `save/portable-game-profile.ts`; this layer
contains no Session reconstruction or second gameplay owner. Fresh native export starts
from the unmodified controlled retail Hub template, embedded as a tracked source asset.
The existing shared JSON writer now formats all serializable records with ECMAScript
number spelling, preserving source record order and its finite-Lua-value validation.

The source permits isolated UTF-16 surrogates in portable JSON strings. Rust `String`
cannot represent them; the portable JSON parser rejects those crafted documents instead
of replacing their characters. The exact source witness is retained in the parity evidence directory as
`native-save/utf16-source-edge.json`. Standard valid UTF-8 native files and portable
profiles have exact source fixtures; arbitrary JavaScript string parity is not claimed.


**Authored Boneyards use the editor's binary reader and ordered scene projection.**
`assets::boneyard::decode_boneyard` follows `parseBoneyard` then `projectBoneyard` on
actual imported bytes. It shares the canonical chunk record with native saves, while
retaining the editor's distinct depth/size limits and its acceptance of duplicate,
non-UTF-8 named-buffer names. Unknown object and recipe records project to the same
opaque geometry; content admission remains responsible for playable-scene validation.

`boneyard_scene_source_value` preserves compact field order and absent-field omission for
source geometry digests. Raw nonfinite f32s become JSON null exactly where JSON.stringify
would do so. Roads retain link bits; fences omit native post sentinels; opening Solomon
selection and ground-clutter filtering run before hashing. The DeadHawg index is u64:
the editor adds 114 to a u32 atlas index, which can exceed usize on wasm32. The complete
ordered output and rejection strings replay against five real files and authored binary
cases, without a separate geometry/hash model or a fallback stock scene.


**Saved XP thresholds retain the source's floating values.** The unchanged native portable
import accepts fractional and inverted next/previous thresholds, and a zero-XP grant keeps
them. PlayerProgressionState therefore uses f64 for those two fields. The generated stock
threshold table remains integer data, converted exactly when ordinary XP or a shared
milestone replaces the imported values. Experience progress retains the source's span<=0
branch. A real portable→web→restore fixture verifies the imported values, ratio and later
XP/RNG transitions, without integer truncation or numeric tolerance.


**Native import/export composes the public Website save boundary.**
`native_save::runtime` follows `game-save-portability.ts` and `game-save-files.ts`, using
one canonical GameSession for a settled native import and the existing public Website
restore/create/profile functions for conversion. It contains no browser I/O or competing
legacy migration. Native CFG/SAV bytes and opaque sibling files remain in their attachment;
a native ZIP replaces its one browser support document instead of nesting older copies.

A raw browser JSON import starts a local-only lineage and clears its native attachment
and party resume token. Importing a ZIP's browser support document instead supplies the
ZIP's current, verified native attachment. Export retains the actual continuation, loaded
scene, mod identities and ordered state in that support document while projecting only
retail-supported profile/wizard fields into the native files. The embedded support JSON
is compared as a complete source value; exact disk bytes and portable JSON are compared
separately. ZIP framing has its independent exact-byte codec fixtures.

The source wraps integer words when writing a native disk file, then validates the
original web values in its final portable parser. The conversion retains those originals
through final validation, so values such as 4,294,967,297 gold cannot silently wrap to one
and become accepted. Unsupported Weld/Plane Orb, native storage, advanced purchases,
unserialized bonuses and mod Boasts retain the source's exact warning and reset rules.

**Source sine and cosine use the same pinned V8 numeric owner as headings.** Complete
native-profile import exposed a one-bit student-wander difference after the existing
900-tick College settlement. With the identical radians word `40021c3f8b5a7e55`, pinned
Node's sine is `3fe89e6475b33eb8`; Apple libm produces `3fe89e6475b33eb7`. Multiplying by
the actual 7.688988409936428 magnitude reproduces the original saved-state discrepancy.
`float::{sin,cos}` now retain fdlibm's kernel, medium cancellation passes and large
24-bit-limb reduction. Their explicit fused operations follow the pinned ARM64 binary,
including separately rounded products that precede compensated tails. The existing V8
license applies. The independent `float-sincos-goldens` corpus retains 20,100 IEEE input
words spanning the witnessed argument, signed zero, subnormals, reduction boundaries,
large exponents and nonfinite inputs. Source-owned simulation and pure presentation
kernels call this owner without moving any `fround` boundary. A source pair on one
operand uses `float::sin_cos`; it returns the two scalar results rather than calling a
platform combined sine/cosine implementation.

**Fire's envelope uses the recorded V8 power evaluation.** The current low-quality Fire
fixture exposed the equal-input `0.95 ** 10` difference: the JavaScript result ends in
IEEE word `...f382`, while native `powf` ends in `...f383`. The fdlibm split logarithm
and exponential from Node v22.17.0 reproduce that witness. A broader independent corpus
found four additional one-bit differences; compiling that same C++ source with ARM64
expression contraction enabled matches all 17,431 cases, while disabling it reproduces
the four differences. `float::pow` makes those fused operations explicit so native and
WASM use the same rounding. Both targets pass the unchanged corpus, including negative
bases, signed zero, subnormals and overflow boundaries. The existing V8 license applies.
Only the observed Fire envelope caller changes; this does not assert that every browser
engine or platform implements transcendental functions identically.

**Developer policy inference uses the retained deployed checkpoint and plain Float32 loops.**
`web/assets/native/ml-bot-policy-v7-selected.sdml` is the pinned Source server asset,
SHA-256 `85edda9d6bd7f7f08eca209fb9790c8d4d5e570e20b8ff8d5bb08284b3dd055c`.
`assets::ml_bot_checkpoint` owns its binary/ordered-JSON boundary and the retained v7
schema. `sim::ml_bot` owns the immutable tensor model and inference; the host can share
one model while each real admitted bot retains its own observation/controller memory.
There is no training or ML runtime dependency. Source `fused` requires a rounded Float32
product followed by a separate rounded Float32 sum, not a hardware FMA. Softmax stores
Float32 exponentials but accumulates the original double values before normalization.
Injected sampling draws are separate from gameplay RNG; the deployed host uses argmax.
The source fixture runs the actual selected checkpoint and retains complete latent,
probability, action and value outputs as well as checkpoint admission/re-encoding cases.

**Runtime event delivery stays native and off the authority thread.** The Source
`host/runtime-event-publisher.ts` contract uses loopback HTTP, a 256-entry queue,
five attempts and a two-second request timeout. The launcher reuses the installed
`url` WHATWG parser and `ureq` HTTP client with default features disabled; it does
not add a browser dependency or its own HTTP parser. A single worker owns ordered
delivery and shutdown draining. Actual Source configuration and HTTP fixtures pin
the body, authorization, retry order, queue eviction and closed admission behavior.

**The complete enemy AI recording is stored as lossless zlib data.** Its original JSON
exceeds GitHub's single-file publication limit. The oracle uses Node's standard zlib
encoder; the existing replay loader inflates the owned `.json.zlib` fixture with the
already-installed `miniz_oxide` codec (a simulation test dependency only). The neighboring
`.json.sha256` file records the exact decompressed JSON bytes. The initial conversion
compares every original byte before removing the redundant uncompressed file; subsequent
oracle recordings produce the compressed fixture and its JSON hash together. Simulation
assertions, source inputs and expected numeric values are unchanged by packaging.
