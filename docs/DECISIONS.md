# Design decisions

These decisions preserve deterministic gameplay, save compatibility, and consistent
behavior across native and WebAssembly builds.

## Native desktop platform

Native client saves and authority checkpoints share `solomon-storage`'s existing
fsync/rename implementation. A separate persistent OS lock file covers every
revision check and replacement; renaming the data file cannot release the lock.
The client keeps complete schema51 documents and the established preference
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
retiring the graphics context. `Surface` represents a real canvas or native window;
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

## One simulation

Game rules live in `solomon-sim`. Browser, native, hosted, and modded play use the
same session types and update order. Contacts, actor admission, and random-number
consumption retain their defined order on both targets.

## Component storage and simulation order

`bevy_ecs` supplies component storage and typed queries. Domain stores own their
entity handles, admission order, and checkpoint conversion. Enemy bodies and
controller brains are separate components; ground loot separates its body from
the reward collected by a player. A small particle keeps its related fields together.

Removing an entity can move another entity within an ECS table. The ordered
handle index preserves survivor order, which determines contact selection, RNG
draws, and painter registration. Births enter at the owning system's boundary:
secondary effects wait for the next actor pass, while terminal enemy children
can advance later in the same pass. Boss spell children enter before their retained
parent and receive their first update on the next pass. Primary transient children
follow their retained parent, or occupy its position when it retires, and also
wait until the next pass. These rules keep Meteor debris, spent Embers and boss
spells from receiving an extra update on their birth tick.

Tick functions run in their declared sequence. Damage accepted by one phase can
retire a target or create an effect before the next phase runs, and several phases
consume the same random stream. Parallel work must preserve those dependencies
and demonstrate a benefit on an equivalent workload before changing that sequence.

Enemy controllers use a working record and commit its components together. Peer
queries during the action observe the previous committed record. Mutating the
live body throughout an action would change those queries. Read-only targeting
and rendering borrow components; they do not need working copies. Student route
plans similarly read the preceding population before any movement commits, so
crowd following and collision responses do not depend on table layout.

Save and snapshot fields retain their supported layout and game identities.
ECS handles stay local to the store. The flat deserialization records also retain
their declared struct names: supported save-import decoders use typed field and
enum visitors to translate values. `serde(flatten)` buffers untyped values and
would bypass those conversions.

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

Gameplay RNG, ambient xorshift state, collision fallback state, and presentation
randomness have separate owners. Changing their consumption order changes play.

`float::hypot` scales its operands before compensated summation. Replacing it with
`sqrt(x*x + y*y)` changes both rounding and overflow behavior. The trigonometric
owner also retains argument reduction for very large inputs. `sin_cos` returns
the two canonical scalar results rather than using a platform combined operation.
Recorded equal-input last-bit differences in sine and power are sufficient to
change saved positions and effect lifetimes after repeated steps.

Player-facing numeric text has its own rounding contract. Decimal labels preserve
the binary value and decimal tie handling used by `native_tooltip_fixed`; for example,
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
`NativePresentationTimeline` owns bounded display history and fractional clocks.
Boneyard frames interpolate without extrapolation; College prediction is limited
to one snapshot interval of local movement.

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

The patch retains upstream metadata and licenses. Egui upgrades must review
[the patch contract](../vendor/egui/SOLOMON-PATCH.md) and its focused regressions.

## Input and text

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
Reconnect starts with a full publication. ACKs, controls, Flame history, and large
Welcome/save transfers retain their separate sequencing contracts.

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

## Navigation and painter registration

Retained scene geometry and prepared meshes are checked by source and geometry
identity. Other navigation domains are built from the admitted scene. Route
selection depends on constraint sampling, triangulation, and neighbor order;
these operations are part of the deterministic simulation.

Prepared navigation belongs to retained geometry. Catalog clones share unchanged
preparations; replacing a catalog entry gives it a fresh identity even if its bytes
match. Other domains are prepared from the actual admitted geometry and gate state.
A direct route avoids unnecessary graph construction. Restored data gets its own
cache lifetime, so process-local cache identity never becomes a save contract.

Stock preparation warms hostile domains at 25/50 clearance and expanded escape
bounds when the Default scene owns Solomon. A missing domain is built on its
first blocked query using the then-current collision, including moving gates,
and retained thereafter. Rebuilding it on every query would change both cost and
route behavior. Graph, moving-gate, and catalog replacement tests cover that lifetime.

Painter registration belongs to the actor or transient owner. Fixed depth bands,
row sorting, and owner overlays are explicit data.

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
Ordinary party sessions use detach/remove/rejoin instead.

## Authored content and Lua

Compiled definitions, admitted asset descriptors, and checkpoints are plain
`sim::modding` values. Lua VMs and function handles live in native
`solomon-modding` and are reconstructed from the admitted package on reload.

`mlua` with vendored Lua 5.4 preserves the supported language without requiring a
system Lua installation. Execution hooks, memory limits, allowed globals, and
accepted includes bound the VM. Filesystem, process, and module loading are not
exposed. VM randomness does not consume gameplay RNG.

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
packaged browser artifact keeps its mounted WASM initializer and content policy.

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
