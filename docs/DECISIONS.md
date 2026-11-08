# Design decisions

These decisions preserve deterministic gameplay, save compatibility, and consistent
behavior across native and WebAssembly builds.

## Native desktop platform

Native client saves and authority checkpoints share `solomon-storage`'s existing
fsync/rename implementation. A separate persistent OS lock file covers every
revision check and replacement; renaming the data file cannot release the lock.
The client keeps complete schema52 documents and the established preference
formats. Save I/O runs off the window thread; malformed or stale records fail
without replacing the prior save. Browser storage still uses its original
IndexedDB transaction. The previous Electron profile stays intact; the existing
save ZIP export/import route moves complete supported saves into the native profile.

Native file selection and export use [RFD 0.17.2](https://docs.rs/rfd/0.17.2/rfd/)
with the actual parent window. Validation, inspection, replacement choices and
export warnings remain in the shared controller. Fullscreen uses winit; browser
fullscreen retains its trusted-click and WebKit paths. Clipboard operations share
one retained native owner with text editing, which keeps Linux clipboard ownership
alive while the client runs.

Display-only number conversion reuses the installed `ryu-js` implementation of
ECMAScript `toFixed`. Native locale collation, casing, counts and dates use the
ICU4X data and the system locale. Collation, casing and counts retain the 2.3
family and the existing mod compiler's collation data. Dates pin the 1.5.1
component formatter because 2.3 semantic `medium` chooses numeric months in
locales such as German; the source explicitly requests abbreviated month names.
The component API preserves that choice, local field order, localized digits and
locale calendar selection without handwritten locale patterns. Six locale
fixtures cover those distinctions. The installed
`time` crate supplies date parsing and the operating system's local UTC offset.
Browser metadata keeps its actual Intl and Date APIs.

Website account, admission and save controllers share one HTTP client over browser
fetch and native `ureq`. Requests validate their origin before attaching account
or session headers, native account tokens retain the browser's per-origin storage
boundary, and response bodies enforce explicit size limits while reporting bytes
actually received. Native HTTP and SHA-256 work run off the window thread.
Immutable mod content uses the same descriptor length/digest checks over browser
CacheStorage or atomic native files. Invalid cache bytes are discarded and only
verified downloads are committed. A view-owned resource URL retains in-memory
native bytes or a browser Blob until the image/media consumer retires; native
resources remain on their owning window thread.

The shared audio director retains cue limits, delay, owned loop arbitration,
narration mixing and music transitions. Browser playback uses its actual WebAudio
and HTML media APIs; native playback uses [Rodio 0.22.2](https://docs.rs/rodio/0.22.2/rodio/)
and CPAL. Resident native music loops by seeking its encoded stream. PCM effect
buffers are shared, and the playback clock comes from samples consumed by the
native mixer. Browser activation listeners are removed with their owner. Native
keyboard capture subscribers retain registration order and cancellation ownership
before the shared GPU editor receives an event.

Desktop compiles the same Game and GPU UI as the browser WASM embedding.
[Desktop runtime](DESKTOP-RUNTIME.md) defines application lifetime and packaging.

The existing renderer's shaders, blend modes, clipping, captures and draw order
stay canonical. `glow` supplies typed OpenGL/WebGL2 driver bindings instead of a
second renderer. `winit`/`glutin` own the native window/context and `egui-winit`
supplies the native integration for the already pinned egui version. The native
shader changes only its GLSL version/precision declarations. Browser image
uploads retain their fallible WebGL image API; native decoding uses `image` and
uploads the resulting RGBA pixels. `blocking` keeps native file/HTTP work off the
window thread, and the existing `ureq` family supplies HTTPS with certificate
validation. Packaged paths reject traversal and symlink escape.

The client has target-selected clocks, entropy, local futures and window geometry.
The native event loop owns its local executor and cancels remaining futures before
retiring the graphics context. That client owner also releases retained clipboard
and controller services. The Windows controller backend joins a worker, which must
happen before process exit can terminate threads during
[thread-local destruction](https://doc.rust-lang.org/std/thread/struct.LocalKey.html#platform-specific-behavior).
`Surface` represents a real canvas or native window;
it supplies drawing sizes and pointer projection rather than emulating DOM nodes.
Source Lua display values reuse `ryu-js` and the existing ordered JSON serializer.

Native typography uses [Parley](https://docs.rs/parley/0.11.1/parley/) for shaping,
bidirectional text, system fallbacks and selection geometry. The installed egui
text engine does not yet cover bidirectional runs, so using it alone would lose
existing editor behavior. [Wuff](https://docs.rs/wuff/0.2.9/wuff/) decodes the same
licensed stock WOFF2 files; system fonts remain on their host. Vello CPU, Skrifa
and HarfRust already belong to the egui font stack and supply native text tiles.
Wrapping, tab stops, line heights, caching, clipping, effects and GPU submission
remain shared with the browser font-run owner. Native text tests cover stock decoding, kerning, ligatures, combining marks,
Arabic visual order, raster pixels and invalid inputs.

Platform interoperation is a narrow exception to the usual no-unsafe rule: glutin
requires validated OS handles, and glow requires a current live graphics context.
Those unsafe calls remain in graphics/window ownership, with explicit safety
contracts and initialization, upload and readback checks. No gameplay, UI layout,
save parsing or network protocol depends on unsafe code. Windows motion-preference
reading also uses its documented BOOL output buffer at the platform boundary.
The window must outlive
its renderer and caches, including failure/close paths. Native and WASM both run
the renderer acceptance; compilation alone does not establish pixel equivalence.

Ordinary textured draws use a small sprite shader; shadows, blur, color filters
and alpha masks retain the effect shader. Apple Software Renderer spends substantial
time executing the effect shader even with its effect uniforms disabled. Separate
programs remove that instruction cost without changing draw order, texture
sampling, vertex layout, projection or blend equations. Both consume the same
batch buffer. Native and browser pixel checks compare straight and premultiplied
textures across normal, additive and multiply blending, and verify state restoration
when draws alternate between the programs.

Sprite uniform values stay with their owning program across effect draws. The
sprite program retains the last uploaded values and changes only those that
differ, avoiding repeated driver calls for unchanged projection and color state.
Constructing a new program starts with no retained values.

Courtyard records replace the reference client's independently cropped images.
Sampling their raw atlas edges let the statue read an opaque white row from its
neighbor. `Renderer::draw_clamped` clamps sample coordinates to the selected
record's texel centers in both shader programs while preserving the original
quad and UV interpolation. UVs and bounds retain high precision for large stock
atlases. Bounds belong to each draw and therefore participate in batching;
ordinary draws retain their existing repeating or authored UV behavior. The
browser composition and native graphics probes compare these draws against an
independently uploaded crop at fractional scales and positions, including state
restoration and the color-filter path.

## One simulation

Game rules live in `solomon-sim`. Browser, native, hosted, and modded play use the
same session types and update order. Contacts, actor admission, and random-number
consumption retain their defined order on both targets.

## Entity storage and simulation order

Each entity population is a plain collection owned by its domain store: a `Vec` in
admission order, an `Option` for one optional actor, or a `BTreeMap` keyed by
player ID. Domain stores own admission order and checkpoint conversion. Enemies
are records in a `Vec`, each holding its family's controller state in `brain`.
Ground loot keeps its body with the reward a player collects. A small particle
keeps its related fields together.

From October to November 2026 most populations were stored in `bevy_ecs`. The
game used it only as storage, with no systems or schedules. Ticks and session
copies were slower with it than with plain collections, both before it arrived
and after it left ([Performance](PERFORMANCE.md#entity-storage)), and its handle,
query and view types made the simulation harder to read. A storage change is
accepted only when the session benchmark shows no regression and every final
checkpoint stays byte-identical, which also shows that painter, contact and
random-number order are unchanged. The Boneyard cases must reach combat: casting
stays locked until Solomon's entrance finishes, so the benchmark plays the
entrance before timing.

Removing an entity from the middle of a `Vec` keeps survivor order, which
determines contact selection, RNG draws, and painter registration. Births enter at
the owning system's boundary: secondary effects wait for the next actor pass,
while terminal enemy children can advance later in the same pass. Boss spell
children enter before their retained parent and receive their first update on the
next pass. Primary transient children follow their retained parent, or occupy its
position when it retires, and also wait until the next pass. These rules keep
Meteor debris, spent Embers and boss spells from receiving an extra update on
their birth tick.

Flash defensive responses can also be born during the post-secondary player
contact or poison callback. The common actor pass cannot enroll those births
after it has returned. Flash materialization owns eight grow and four fade
children, and each needs a distinct transient painter registration at birth,
in that order, from the session's world painter order. The same callback owner
registers twenty Magic Shield break children, plus an explosion and shockwave
when the shield is explosive. The shockwave's transient painter and separate
actor-lane light both exist at birth. Physical Stoneskin contact registers its
chip at the damage callback. These are producer invariants; snapshot validation
still rejects missing ownership.

Tick functions run in their declared sequence. Damage accepted by one phase can
retire a target or create an effect before the next phase runs, and several phases
consume the same random stream. Parallel work must preserve those dependencies
and demonstrate a benefit on an equivalent workload before changing that sequence.

Enemy controllers update a working copy of their record and write it back in one
assignment. Peer queries during the action observe the previous committed record.
Mutating the live record throughout an action would change those queries.
Read-only targeting and rendering borrow records; they do not need working copies.
Student route plans similarly read the preceding population before any movement
commits, so crowd following and collision responses do not depend on which
student moves first.

Save and snapshot fields retain their supported layout and game identities.
Records derive serde with their fields in the save record's order and under the
save record's struct name: supported save-import decoders use typed field and
enum visitors to translate values. `serde(flatten)` buffers untyped values and
would bypass those conversions, so records stay flat instead.

Closed integer codes become enums that keep their number in saves and snapshots.
`WeldBuildId` serializes through `serde(try_from = "u32", into = "u32")`, so
documents still carry the original build IDs 1000–1009 and the save-import
visitors see the same integer. Reading any other number fails where the document
is decoded instead of reaching a panic in a later match.

## Numeric contract

Kernels use `f64` arithmetic with explicit `fround` boundaries where required.
Using `f32` throughout adds rounding; removing a boundary changes saved and
recorded trajectories.

The numeric owner supplies recorded `hypot`, `atan2`, sine, cosine, and power
evaluation, including explicit fused operations. Platform libm results can differ
by an ulp and later change collision or navigation. These implementations retain
the required numeric behavior across native and WebAssembly.

The fdlibm/V8 notices remain in
[the license](third-party/v8-fdlibm-LICENSE). Native/WASM bit tests and complete
trajectory replays protect the contract.

Ethereal Boulder without Bind has toughness zero in the retail build-1006 vector.
The shared Boulder contact kernel accepts nonnegative toughness. A lethal contact
may spend positive infinity internally; the native nonnegative pool clamp then
exhausts the actor. Replacing zero with one would incorrectly preserve its pool
and allow another hit. Negative/nonfinite inputs remain invalid, and terminal
actors must retain finite serialized state. The recovered division and clamp are
at `0x00621231..0x006212C2` in Solomon Dark 0.72.5.

Gameplay RNG, ambient xorshift state, collision fallback state, and presentation
randomness have separate owners. Changing their consumption order changes play.

`float::hypot` scales its operands before compensated summation. Replacing it with
`sqrt(x*x + y*y)` changes both rounding and overflow behavior. The trigonometric
owner also retains argument reduction for very large inputs. `sin_cos` returns
the two canonical scalar results rather than using a platform combined operation.
Recorded equal-input last-bit differences in sine and power are sufficient to
change saved positions and effect lifetimes after repeated steps.

Player-facing numeric text has its own rounding contract. Decimal labels preserve
the binary value and decimal tie handling used by `tooltip_fixed`; for example,
2.5 at zero places becomes 3, while 2.55 at one place becomes 2.5. Rust's default
formatting can choose a different tie. Numeric presentation tests protect this
behavior separately from simulation arithmetic.

The learned-bot model has a distinct Float32 contract. Its multiply and add round
separately; replacing them with a hardware FMA changes inference. Softmax stores
rounded Float32 exponentials but accumulates their unrounded double values before
normalization. The immutable model can be shared, while each admitted bot owns
its observation/controller memory. Sampling randomness is separate from gameplay
RNG. Checkpoint and inference fixtures cover the deployed model without adding
a training runtime to the game.

## Display copies and immutable data

Accepted session state owns commands, audio, checkpoints, and saves.
`PresentationTimeline` owns bounded display history and fractional clocks.
Boneyard frames interpolate without extrapolation; College prediction is limited
to one snapshot interval of local movement.

The standalone local authority feeds Tree occlusion after every advanced session
tick, using that tick's actual actor roots and camera. The 25-tick scans must not
depend on which 100 Hz ticks happen to be painted by a slower display. Drawing
the same tick again does not advance the fade. Transported clients retain their
delivered pose history; genuinely unavailable history still preserves the prior
scan target rather than inventing an actor position.

College prediction resolves the region's permanent fixed bodies, the Office
polisher during pending admission, and Skorcha's latest accepted position/presence
through the same actor solver as authority. Ordinary movement retains scenery
placement; scripted movement retains its wall exemption. Prediction does not
advance Skorcha's appearance timer or other participants. A scenery-rejected
full circle correction still retains the root move, as recovered in Website
parity ledger 013; this display correction does not change the bench-pocket rule.

Immutable loaded scenes, static collision primitives, skill definitions, and
unchanged scenery target tables use `Arc` storage to avoid per-frame deep copies.
Mutable gates and actors remain private. Authored changes use copy-on-write;
serde continues to serialize values rather than process-local pointers.

## GPU interface and layer order

Rust owns visible game controls, menus, HUD, overlays, and launcher presentation.
The browser provides canvas and platform APIs. Text rasterization may use an
unattached font canvas; game UI is submitted through GPU textures and geometry.

The vendored egui patch supplies fixed layer ordering, passive containers, authored
hit predicates, and pointer cancellation. One comparator serves painting, hit
testing, modal scope, and controller ownership. Layer ordering preserves the
existing egui Area state, including focus and geometry.

Egui's ordinary sublayer relation does not express the application's full layer
hierarchy, and disabling an Area's interaction also disables its children.
The patch therefore supplies fixed ordering and passive container backgrounds
while retaining real child controls. These capabilities prevent reappearing panels
from changing input priority merely because an Area was raised.

The College Party chip belongs above the gameplay HUD and below the book and
service panels. Registering it with the higher social overlays covered Hagatha's
first owned charm and intercepted its Remove action. Only that chip moves below
books; chat, Party menus, invitations and Player Cards keep their overlay order.
Website's Party chip likewise sits below its opaque Inventory/service surface.

The patch retains upstream metadata and licenses. Egui upgrades must review
[the patch contract](../vendor/egui/SOLOMON-PATCH.md) and its focused regressions.

## Input and text

### Optional Inventory and Skills navigation

Inventory's configured binding returns one open Sack at a time, then closes at
the backpack root. Menu and Escape close the whole optional Inventory, College
service or Skills interface through the existing close/resume lifecycle. This
is the approved Report 22 quality-of-life policy retained in Website ledger 299
and release `4c6a1f5c`; it deliberately differs from the recovered stock
Inventory-binding whole-close action. It does not redefine mandatory choices.

Rename, notices, confirmations and dye editing own their cancellation before
book navigation. Sack traversal still rejects another Back until its 37 native
ticks settle. The first-cell parent Sack returns on an unselected primary
press, and its release cannot return again. A selected backpack item retains
click-to-parent transfer and drag-drop ownership. Website ledger 252 records
the recovered first-press branch at `0x0056FA6A–0x0056FA85`, the parent composite
and the unchanged 370 ms page clock.

Sample gameplay input once per frame. The selected display camera projects aim.
Held casts and queued belt presses are different inputs; both host and client
validate all `PLAYER_BELT_SLOT_COUNT` slots.

UI dispatch commits each admitted action before resolving the next event.
A prior modal, sort action, or binding capture can change the next event's owner.
Pointer cancellation retires click and drag state without activating a control.
Stationary held buttons activate on release; dragging outside does not.

Fullscreen entry stays in the trusted browser input callback. Pointer activation
and keyboard/controller activation have distinct provenance, preventing duplicate
toggles. Tab is captured when an interactive GPU region or editor needs focus,
including the first traversal after touch.

Native font shaping and Rust editing share text geometry. Preserve complete font
runs, Unicode composition, joining, emoji, caret position, selection, and wrapping.
The text adapter uses bounded textures owned by the current GPU context.

Editing uses grapheme boundaries for user-visible characters and Unicode line
breaking for wrapping. Bidirectional text can have a weak caret and disjoint
selection rectangles; those come from the same shaped run as painting. Undo
transactions retain text and selection, including the browser adapter's UTF-16
positions. Copy/cut/paste and focus changes commit inside native input dispatch,
so a clipboard event immediately after Tab sees the newly focused editor.

Browser input dispatch and drawing read the same document timeline. Mixing
`performance.now()` at dispatch with an earlier requestAnimationFrame timestamp
can move egui's history clock backward. An inactive document has no UI time to
commit. This constraint belongs to the browser adapter; it does not change the
simulation's tick clock.

## Persistent state

Native save containers retain unknown bytes; known fields are patched in the
retained tree so editing a supported field preserves the rest of the player's
document. ZIP, XOR/LZ, profile, and document codecs live in `solomon-assets` and
work on native and WebAssembly targets.

Large save/checkpoint records are heap-owned at deserialization boundaries.
Resource construction completes before saved-session admission so their large
futures and parsing frames do not overlap the WebAssembly stack.

Persistence uses revision checks and atomic synced replacement. Encoding and disk
I/O run outside the tick deadline. Welcome waits for a reconnect credential to be
durable; SaveCommitted follows actual file completion.

Direct-host reconnect rotates its credential while retaining the credential the
client presented until that connection acknowledges the Welcome snapshot. Both
are durable during admission, so losing the final Welcome bytes or restarting
the host does not strand the wizard. Only the current connection can retire the
previous credential; stale snapshot acknowledgements cannot confirm admission.
The optional previous credential is omitted after confirmation, and existing
host saves without that field remain readable.

### Save conversion

The native attachment retains CFG, SAV, and opaque sibling files with their
base64 and SHA256 identities. Portable profile fields are separate from those
bytes. A fresh retail export starts from the tracked retail Hub template, and
known fields are patched into the retained container. The conversion code lives
in `assets::native_save::runtime` and uses the existing player-document APIs.

A raw player JSON import starts a local lineage and clears the native attachment
and party resume token. A ZIP support document instead receives the ZIP's verified
native attachment. Export stores the complete Rust continuation, scene, mod
identities, and ordered state in that support document, while projecting only
retail-supported profile/wizard fields into native files. Re-export replaces the
support document instead of nesting older copies.

Validate original values even when the disk encoding wraps integer words. For
example, 4,294,967,297 gold must not wrap to one and become accepted. Unsupported
fields retain the codec's warning/reset rules, and opaque retail storage remains
in its attachment. Native byte round trips and complete continuation tests cover
different parts of this boundary.

Imported XP thresholds may be fractional or inverted, so previous/next thresholds
remain `f64`. A zero-XP grant preserves them; ordinary progression replaces them
with exactly converted stock thresholds. The portable parser rejects isolated
UTF-16 surrogates, which Rust strings cannot represent, rather than substituting
characters silently.

Retained loot events use the supported interchange discriminator `type` and the
audio owner's sound IDs, including `drop-bag-1` and `drop-bag-2`. Import also reads
the earlier Rust spellings `eventType`, `drop-bag1` and `drop-bag2`; re-export writes
the canonical source spellings. Native checkpoints retain their own typed fields.

Portable item rarities retain `Epic` and `Rare`, including items inside nested
Sacks. Earlier Rust exports wrote lowercase enum names; import accepts those
values and re-export writes the canonical spelling. This preserves existing
player inventories while keeping the shared portable-item contract compatible with
earlier web documents. Unknown rarities remain rejected.

## Snapshot ordering

The shared protocol carries canonical session values between matched native and
client builds. Reliable results and save acknowledgements remain separate from
replaceable snapshots.

Compaction uses the last publication selected and written on that connection.
Unsent replacements cannot become a baseline. The receiver reconstructs each
ordered packet before coalescing completed state for display. First publications
and changes that are larger than a full message use complete packets.

This reduces repeated bytes while retaining exact state. It adds per-connection
baseline memory and compression work; malformed sequences close the connection
rather than fabricate a partial checkpoint.

Protocol version 2 prepares one canonical JSON publication and shares it across
matching peers. Lossless field/array changes use the existing JSON, zlib, and
base64 codecs. The decoder bounds reconstructed size through incremental exact
accounting; it does not reserialize the whole document to measure it. A direct
serde visitor avoids recursively cloning untagged intermediate values.
Objects with matching field names walk paired values directly; object shape
changes retain the insertion/removal path. This avoids repeated map searches
without changing delta contents or exact numeric comparisons.
Reconnect starts with a full publication. ACKs, controls, Flame history, and large
Welcome/save transfers retain their separate sequencing contracts.

Protocol version 3 requires the primary HandCast1 record and the bare secondary
pose/terminal state. The authority owns the opening gameplay RNG draw, born rate,
current pose and retained terminal frame; clients render those accepted values.
An older matched-build protocol cannot silently ignore them. Existing disk saves
without the optional fields remain readable without sampling RNG. The hosted
deployment protocol remains version 2 because its publication contract is separate.

## Deployment identity and recovery

The game recovery revision and website publication identity have separate roles.
The deployment publication names website and game revisions, a unique deployment
ID, a monotonic sequence, and readiness.

Automatic reload requires the final durable save receipt, retirement of the old
transport, and a healthy publication at the announced sequence or later with the
signed target game revision. A new ID alone does not establish ordering.

Requiring an exact deployment UUID would strand a sleeping client that missed
that publication. A later healthy sequence with the same target game revision
can complete recovery. Idle website-only refresh uses its own publication path;
an active game still observes the save and transport-retirement barrier.

## Mount-independent browser build

One browser build serves at `/game`, `/game-rust` or a staging mount. A build that
wrote its mount into the document base and initializer path would need its HTML and
JavaScript rewritten for every other mount, including Rust's move to `/game`.

Every URL the build writes is therefore relative to its page. The base is `./`,
or `../` in `editor-test/`; modules, WASM and assets resolve from it or from
`import.meta.url`. The post-build hook rejects a document whose base names a mount.
The cost is that a page must load at its directory URL with the trailing slash: at
`/game` without it, `./` would name the site root. The Website, like most static
servers, redirects a directory URL to that form.

The mounts are deployment settings rather than build inputs. `solomon-game-mount`
records where the artifact is served, and `solomon-website-mount` selects the
Website API, socket and storage prefix. Serving an artifact at another mount
changes only those two values.

## Retained hosted authority after transport interruption

An unexpected `1001`/`1006` after a playable hosted Welcome is a transport failure,
not a saved restart. Hosted membership owns one absolute 30-second monotonic
lease on the existing participant and world; an authenticated replacement can
start the same deadline before the old Close arrives. The bound is an explicit
operational policy, longer than ordinary open/Welcome progress intervals and
shorter than an unclaimed ticket's 120-second lifetime. A large valid Welcome and
actual renderer readiness must fit it on M2. HTTP redelivery, another reservation
and partial transfer progress never extend it.

Caddy 2.11.7's reload cleanup can write a raw Close control inside an unfinished
text frame. Private byte captures confirm the resulting invalid UTF-8 payload.
The browser error callback waits for the actual Close event; eligible
`1001`/`1006` losses keep their original diagnostic and use this same bounded
recovery. Protocol validation remains at the receiver. The upstream
[stream cleanup and Close writer](https://github.com/caddyserver/caddy/blob/v2.11.7/modules/caddyhttp/reverseproxy/streaming.go)
owns this proxy boundary.

`hosted/reconnect.rs` owns retained membership, neutral input, peer-generation
binding and final cleanup. It keeps accepted state, action/transient history,
RNG, party/VM/bot owners and monotonic save sequence without a second simulation.
A recovering participant still occupies capacity; connected and recovering
humans are counted separately, and private retirement waits. The affected
private/party world uses existing loading/renderer holds; unrelated Hub worlds
continue. Old peer events cannot route into the replacement generation.

The Website admission owner accepts a logical Hub/private selector and resume
capability, then forwards its current JWT subject to the configured supervisor.
`supervisor/http.rs` checks the original anonymous/account binding, current live
actor and capability before minting a fresh one-use reconnect-purpose ticket.
It binds that ticket to the request and expected peer; redelivery returns the
same pending credential, and a consumed ticket cannot replay or create an actor.
Browser saves, content and permissions cannot replace retained authority state.
No new WebSocket message family or save schema is needed: ordinary Hello without
save/New Game intent receives a fresh full Welcome.

`web::game::hosted::reconnect` owns at most two client attempts within that same
absolute deadline, with one bounded one-second pause before the second. Only an
uncertain HTTP transport response reuses its request ID; a sent Hello needs a
new reservation. The existing native connection/presentation and hosted save
coordinator survive, while transport input/command/replication cursors restart.
The client keeps its last frame, stops prediction/input, and never replays an
unacknowledged action. Full accepted Welcome plus real renderer readiness ends
the transport lease and requests a complete current checkpoint. Existing pause,
skill selection and resume controls still govern gameplay; solo readiness does
not acquire a multiplayer countdown.

Leave/destruction, account change, observer/editor/standalone/desktop sessions,
authentication/protocol failure, announced deployment and host retirement are
excluded. An accepted Close without a status is local code 1005 and uses ordinary
cleanup, including intentional browser/native connection disposal. Only TCP loss
without an accepted Close has no code and may retain the live actor. The transport
replies to an empty Close with an empty frame, never a reserved 1005 wire code.
The driver treats Tungstenite's `ResetWithoutClosingHandshake` as EOF; actual
malformed frames remain protocol error 1002. A known close code and reason survive
later I/O failure during that close handshake.
Expiry uses ordinary cleanup once. Deployment has priority and retains
its checkpoint/ACK/publication barrier. An open replacement still awaiting
renderer readiness joins that same final checkpoint and storage ACK barrier;
only a genuinely absent peer has no browser save owner to acknowledge.
Failure reports the original transport
cause and preserves prior durable storage; it never imports an older checkpoint
or falls back to provisioning. Ordinary explicit active-run rejoin remains a
separate contract and can recover only its last complete durable checkpoint.

Focused public HTTP/WebSocket regressions cover membership and tickets. Closure
also requires actual exact-built browser journeys through a local Caddy reload
for both maintained clients. Compilation alone is not that acceptance.

## Resume readiness and multiplayer countdown

Renderer loading, resume acknowledgements, required rejoin members and mandatory
skill-picker close presentation are authority barriers. They remain required even
for a solo run. The two-second countdown after those barriers is a separate
reorientation policy for two or more connected materialized human participants in
that active run. Closing solo Pause, Inventory or full Skills resumes directly.
A pending solo loading/recovery record clears only after its existing readiness
checks pass; an active countdown clears when a departure leaves at most one human.

This preserves the accepted Website product policy from `e3f100ae` (Report 103),
published in `850064c8`, rather than introducing a new retail timing claim.
`GameplayControl` owns the policy for standalone and hosted authorities. The
intersection of its borrowed connected/materialized identities excludes bots,
observers, lobby visitors, other runs and staged rejoin actors from the audience.
Required absent recovery identities still hold the pending record before that
audience check. The protocol, save shapes, input neutralization and scheduler
remain shared.

Ordinary party transfers recheck the existing Hub-only destination contract at
Hello, after ticket preparation. A party that has entered combat cannot acquire
a new College actor through an older ticket; that would mix world ownership and
make owner-save partitioning invalid. The matching stale reservation is consumed
before actor creation. Active-party recovery/rejoin has a separate retained-run
admission and remains available. Valid Hub transfers and both owners' save
documents retain their existing behavior.

Public control regressions and actual standalone/hosted socket journeys cover
direct solo release, lobby/run isolation, restart/rejoin readiness, both departure
phases and retained multiplayer timing. The original host-control recording stays
unchanged; its one older disconnect countdown expectation is explicitly superseded
in the replay by this independently tested policy.

## Where a rejoining wizard enters

A wizard that joins or rejoins a Boneyard enters at the scene's spawn point, which
lies in the 400-unit entrance strip. Once the arena begins to lock, that strip is
outside the combat bounds: the camera follows the shrinking bounds, so a late wizard
showed at the top or bottom edge, partly behind the health bar, outside the fight.
PixiJS and the Website host behave the same way, and there is no recorded retail
evidence for where a late joiner should enter.

While the arena is locking or sealed, `BoneyardWorld::spawn_wizard` moves the
spawn point just inside the combat bounds (`ArenaTransition::entry_inside`), keeping
its column: the path every wizard walked in on. Before the lock nothing changes.
The rule lives in the simulation, so standalone, hosted and desktop authorities
agree. `a_wizard_joining_after_the_arena_locks_enters_inside_it` covers both
entrance sides and every arena phase.

## PixiJS retires at the swap

The swap is deferred under the owner's November 7 instruction: keep PixiJS at
`/game`, Rust at `/game-rust`, and apply fixes to both maintained clients. The
preparation and future swap rules below stay available for a separately requested
cutover; they do not authorize retiring PixiJS during paired fix work.

When Rust moves to `/game`, the PixiJS client is no longer served: there is no
`/game-legacy`. A second playable copy would let players advance a save the Rust
game never sees. Its source stays in the Website repository until the owner
confirms the result, and reverting the swap restores it. Decided by the owner on
2026-11-06.

Saves carry over with nothing for a player to import or choose. The Rust game keeps
its own data under the `/game-rust` storage namespace and cloud slot 1. The
Website's deployment harness, which owns PixiJS's storage, settles each player's
PixiJS save against the Rust one once at the swap: cloud slots on the server,
guest IndexedDB before the Rust page loads. Both copies descend from the
parallel-launch fork and count revisions from it, so the copy written more often
since then is the one the player kept playing. A tie keeps PixiJS's, the save most
players have, and the other copy is kept as a backup. Preferences follow where
Rust has none, Hall of Fame runs combine and playtime adds up.

## Navigation and painter registration

Pike constraints pass through the active Boneyard's player-placement callback.
The native Skeleton calculation produces the requested root; the world retains
that point when legal and resolves a blocked request with existing movement
collision. The wizard and Pike retain the same accepted position. This preserves
the supported Website behavior from `ab72d149`, which the initial Rust world
adapter omitted. A focused seeded-arena test reproduces the exact blocked root
observed at wave 54 and the subsequent failed small-enemy spawn. Movement also
recovers a living collision-enabled player already overlapping scenery before
spawn reachability, so an older saved root cannot keep every new enemy unreachable.
This live-combat recovery follows the shared movement solve; movement-only
recordings retain the independently captured narrow-phase result. Standalone enemy
recordings retain their direct constraints; navigation and enemy spawn collision
requirements remain unchanged.

Retained scene geometry and prepared meshes are checked by source and geometry
identity. Other navigation domains are built from the admitted scene. Route
selection depends on constraint sampling, triangulation, and neighbor order;
these operations are part of the deterministic simulation.

Prepared navigation belongs to retained geometry. Catalog clones share unchanged
preparations; replacing a catalog entry gives it a fresh identity even if its bytes
match. Other domains are prepared from the actual admitted geometry and gate state.
A direct route avoids unnecessary graph construction. Restored data gets its own
cache lifetime, so process-local cache identity never becomes a save contract.

Endpoint attachment is a paired connectivity query. The nearest triangle can
belong to a small isolated component even when the same endpoint has a clear
connector to the main component. Connected first choices keep their original
route; otherwise the query considers the components reachable by the start and
requires a valid destination connector in the same component. It retains each
endpoint's own footprint, every graph edge and collision check. It does not join
disconnected meshes or relax spawn policy. The seeded wave-48 arena, a two-sided
attachment case and recorded false-null queries in stock and imported scenes
cover this correction while retaining the original recordings.

Stock preparation warms hostile domains at 25/50 clearance and expanded escape
bounds when the Default scene owns Solomon. A missing domain is built on its
first blocked query using the then-current collision, including moving gates,
and retained thereafter. Rebuilding it on every query would change both cost and
route behavior. Graph, moving-gate, and catalog replacement tests cover that lifetime.

Painter registration belongs to the actor or transient owner. Fixed depth bands,
row sorting, and owner overlays are explicit data.

Ether Drain world callbacks enroll their newly born flares at the capture, loot
consumption, or corpse consumption boundary, including Game Over. These callbacks
run after the secondary actor pass; waiting for its next tick can publish an
unregistered flare to the renderer. Existing actors retain their ordinals.

`ZAnimSplit::Draw` at retail `0x005E0233` reads the current Enhanced Effects
setting: painter bands are 25 units On and 50 Off. Air and Faculty ribbons keep
their immutable born-quality geometry while their wrappers follow the current
scene setting, as do Blizzard, Flame Lash and ElectricBurn. Changing quality
does not change the actor's registration, clock, material or gameplay state.

Owner overlays are promoted after row planning, preserving their owner-relative
position and stable ties. Insertion bands retain the owner's registration rather
than allocating a new ordinal for each visual layer. Released Ether Boulder
pieces use their actual actor registrations. The teacher's ambient state retains
the two transient painters for one release. These details matter when splitting
an actor into several draw operations: visual decomposition must not change its
place among other actors.

## Tutorial disconnects

A disconnected private Tutorial retains its sole participant and freezes until
resume. Removing that participant would violate the Tutorial checkpoint invariant.
Eligible hosted transport interruptions first use the bounded retained-authority
lease above. Other ordinary party disconnects use detach/remove/rejoin.

## Authored content and Lua

Compiled definitions, admitted asset descriptors, and checkpoints are plain
`sim::modding` values. Lua VMs and function handles live in native
`solomon-modding` and are reconstructed from the admitted package on reload.

`mlua` with vendored Lua 5.4 preserves the supported language without requiring a
system Lua installation. Execution hooks, memory limits, allowed globals, and
accepted includes bound the VM. Filesystem, process, and module loading are not
exposed. VM randomness does not consume gameplay RNG.

Lua player input addresses all 24 belt slots, using the same `PLAYER_BELT_SLOT_COUNT`
as ordinary network input. Controller observations can expose a learned ability in
any bank. Restricting admission to the first bank rejected valid casts and rolled
back the controller's entire transaction, including status refreshes; a protected
endurance pilot consequently died at wave 28. The frozen controller recording's
old slot-8 rejection is exercised at the current invalid boundary, slot 24, while
an independent regression accepts observed slots across all three banks.

Console API values use the same Rust-to-Lua conversion as event payloads:
missing optional values become Lua `nil`, including fields in frame snapshots.
The retired JavaScript bridge's null exception is not a supported contract; it
prevented the default Console query from reading a normal College frame. Frozen
recordings remain intact, with explicit corrected expectations for the affected
queries, subsequent commands and callbacks, plus a real College-frame regression.

The VM memory limit is 16 MiB. Registry borrows must be released before invoking
Lua callbacks, which can re-enter the bridge and need that registry. Stable
definition/function identities are saved as values; restore reconstructs executable
handles from admitted content rather than serializing VM pointers.

Package graph digests are a data compatibility contract: en-US collation,
ECMAScript number spelling, array-index ordering, and SHA256 must stay stable.
ICU collation and `ryu-js` provide those representations. Compiler tests protect
the digest when dependencies change.

For example, `a_b`, `a-b`, and `a.b` sort differently under en-US collation than
under byte order. Number serialization also distinguishes forms such as `1` and
`1e+21`, and spells negative zero as `0`. Array-index properties sort before other
object fields. Changing any of these representations changes a package's identity.

Authored Boneyards use the shared binary chunk reader with scene-specific limits.
Duplicate and non-UTF-8 buffer names can be decoded; playable-content admission
is a separate validation step. Ordered scene projection preserves field order,
omits absent fields, and maps raw nonfinite floats to JSON null for geometry
digests. DeadHawg atlas arithmetic uses `u64` because adding its offset to a `u32`
index can exceed `usize` on WebAssembly. Scene fixtures cover both the binary
reader and the projected identity.

## Native I/O

The native transport uses synchronous Tungstenite and standard TCP with owned
connection lifetimes. Asset transfers leave the framing pool after header parsing
and use shutdown-aware nonblocking writes. Slow media readers can pause without
truncating the declared response or preventing new request framing.

Established WebSockets also use nonblocking writes. Tungstenite retains a
partially written frame, and its worker flushes that frame before taking another
message. Reliable output is bounded by count and bytes; the single unsent snapshot
remains replaceable. A close deadline begins when close is requested, including
time spent draining earlier reliable output. After receiving peer Close, the
worker stops reading and flushes the reply until it drains or the deadline expires.
Heartbeat deadlines retire readers that never resume. Real TCP tests cover intact
large frames, later controls, snapshot replacement, and close-reply delivery.

Admission/reconnect credentials use operating-system randomness, separately from
the deterministic gameplay RNG. Signal handlers request shutdown; the owning
authority loop performs persistence and closes its resources.

Event publishing and diagnostic serialization stay off the authority thread.
The UTC formatter at the launcher edge emits signed score timestamps with three
millisecond digits; the simulation itself does not read wall-clock time.

## Desktop packaging

Native packages contain the Rust executable, tracked game assets, licenses and
build metadata. Desktop compiles the shared client directly. The separately
packaged browser artifact keeps its external WASM initializer and content policy.

A package records its release repository and immutable revision. Release archives
are inspected and exercised through the [packaged journey](RELEASE.md#qualification)
before publication. Optional update failures leave offline play available.

## Native client sockets and application ownership

Desktop compiles the same Game, rendering, menu and session protocol owners as
WASM. `network::ConnectionCore` owns Welcome deadlines, transfer reassembly,
snapshot deltas and nonce pings. Browser WebSocket events and an owned native
Tokio/tokio-tungstenite thread feed it; no JavaScript transport bridge remains in
the native path. Rustls validates remote TLS certificates. Reliable messages are
bounded, snapshots coalesce after decoding, and queued sends retain byte credits
until sent or dropped. Dropping the native connection cancels and joins its owner
without waiting indefinitely for a stalled handshake or DNS worker.

Input sequencing commits the held state, sequence and pending tick only after the
transport admits the message. A rejected release remains eligible to send again;
preparing a message cannot mark it as sent.

Each transport sends its admission message as soon as the socket opens, before
publishing the Opened event. Admission must progress while a slow frame occupies
the renderer; the socket's independent Welcome deadline cannot depend on frame
polling to begin the exchange. The Game still validates and accepts Welcome before
acknowledging it, preserving the durable reconnect credential contract.

The native desktop authority embeds the established standalone supervisor on an
owned thread, with an ephemeral loopback listener and private generated credential.
Its accepted Origin remains `solomon-darker://app` for compatibility with the
existing authenticated peer contract. HTTP asset/API location is independently
configured from the local authority URL. The OS application must complete the
existing save-before-leave coordinator before releasing Game, its tasks, authority
and GPU context, in that order. A failed save keeps the running game and offers
Keep playing or Close without saving.

Hardware-accelerated windows use V-sync to pace presentation. An unaccelerated
graphics context leaves a 16 ms event-dispatch interval after presentation before
requesting another draw. Unconditionally requesting redraw before `WaitUntil`
prevented the Mac software renderer from yielding and delayed accessibility
requests by tens of seconds. The window supplies its next redraw deadline, which
the application respects while continuing to process native events and client work.

A close deadline cancels the pending leave request and its disconnect receipt;
a late successful checkpoint may persist while the player continues. Retiring a
native session revokes any write still waiting for the document lock and waits for
an atomic replacement already in progress. An abandoned session cannot commit
after its replacement starts. Single-instance endpoint metadata is separate from
the lifetime lock because Windows exclusive locks also deny reads from other handles.

## Native encrypted peers and macOS Quit

Desktop peer invitations preserve the existing v1 capability grammar and HKDF-SHA256
key derivation. Tokio owns cancellation and byte forwarding; vendored OpenSSL owns
TLS 1.3 PSK authentication with TLS_AES_128_GCM_SHA256, matching the qualified Node
transport. Native/Rust and Node/Rust exchanges are checked in both directions. The
host requests no X.509 client certificate; the invite key authenticates the peer.
Each service cancels its sockets and joins its worker before its session retires.

Winit retains its macOS NSApplication delegate. Its absent optional
applicationShouldTerminate selector is supplied through objc2 with the documented
NSUInteger ABI. The callback only requests deferred termination; the ordinary Rust
event loop runs the shared save coordinator and replies to Cocoa. Keep playing
cancels the pending Cocoa termination. GPU owners and cancelled tasks retire with
their window context current.

The macOS setup removes Winit's unused Services submenu by its registered menu
object, preserving About, Hide and Quit. The game exposes no Cocoa Services
integration. An [Intel main-thread sample](https://github.com/JarrettOneSource/solomon-darker-rust/actions/runs/40637948757)
captured Services population waiting synchronously for Shortcuts over XPC during
a movement-key hold; the queued press and release then reached gameplay together.
Unregistering and detaching that submenu keeps ordinary input independent of the
external service. Gameplay sampling and the save-before-Quit path retain their
existing behavior.

## Native Opus decoding

Browser audio accepts Ogg Opus in custom content. Rodio 0.22.2's installed
Symphonia decoder rejects the real Opus fixture, so the native audio edge adds
`ogg` 0.9.2 for container packets and `opus-codec` 0.2.0 for bundled static libopus.
Neither changes the WASM dependencies. CMake and libclang are build dependencies;
the application needs no system codec, JavaScript runtime or codec download.

The adapter implements the Ogg Opus timing contract in RFC 7845: header gain,
pre-skip across packets/pages, initial granule offsets and final-page trimming.
Only one encoded page and one decoded packet are buffered in addition to the
resident compressed song. Looping reopens the encoded stream and resets decoder
state. Chained streams may change channel count; the output stays stereo through
the Xiph libopusfile downmix table, whose license is retained under `third-party`.
Midstream decoding errors reach the existing audio error owner. Sound decoding
checks the same error before accepting the collected PCM.

Synthesized mono, stereo, 5.1 and multi-page fixtures exercise these contracts,
including malformed input and errors after playback starts. The surround fixture
also compares sampled frames against an independent FFmpeg/libopus decode.

## Automatic equipment removal uses the open inventory root

Double activation of removable equipment returns it to the first available slot
of the currently open Sack. At the top-level backpack it retains ordinary
top-level insertion. This explicitly requested convenience lets a player organize
gear within a Sack without navigating back from the root after each removal.
It is distinct from the recovered pointed-holder drag contract, which continues
to preserve the exact selected destination.

`PlayerPanels` supplies the current Sack ID and an absent slot through the
existing `Unequip` command. The canonical economy retains ownership of first-hole
insertion, item admission, capacity, feedback and persistence. Required Hat/Robe
notices, the third Ring gate, transition input locks and all explicit drag
destinations retain their existing rules. No protocol or save-schema change is
needed. The real panel attach/update/action regression applies emitted commands
through the canonical economy for all removable sinks, three inventory depths
and Inventory plus the four trader companion panels.

## Player Card lookup feedback belongs to its session

Hosted Player Card lookup can legitimately return no profile after a chat author
disconnects. The maintained Website contract in social ledger 136 treats this
as dismissible feedback for five application seconds. Retail has no Website
Player Card service, so this is a product UI contract rather than a recovered
native lifetime. The lookup itself has a ten-second application deadline.

One shared Rust owner keeps the latest request, its deadline and its feedback.
Older replies cannot replace a newer pending request or result. Session replacement
and transport retirement cancel the lookup without changing the retained world,
and request IDs remain distinct across those boundaries. Feedback never enters a
save. UI application time continues while gameplay ticks are paused.

The real Dismiss control seals gameplay shortcuts only while focused; dismissal,
expiry and a covering surface release that focus. Loading, leaving, routing and
serious runtime errors retain rendering priority. College invitation failures keep
their separate notice owner, and card expiry or dismissal cannot clear them.
Controller and composed egui regressions protect the deadline, stale-reply,
keyboard, pointer and focus lifetimes; actual built-game acceptance is separate.
