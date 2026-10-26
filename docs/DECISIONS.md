# Design decisions

These choices explain constraints that are easy to miss when changing the game.
Keep each entry current; record execution history in commits and test artifacts.

## One simulation

Game rules live in `solomon-sim`. Browser, native, hosted, and modded play use the
same session types and update order. Presentation and transport do not maintain
another gameplay model.

Typed contiguous collections preserve actor order. A general engine or ECS would
add lifecycle and ordering changes without a demonstrated benefit.

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
trajectory replays protect the contract. Preserve recorded tolerance bounds
without substituting expected positions into an independently running replay.

Gameplay RNG, ambient xorshift state, collision fallback state, and presentation
randomness have separate owners. Changing their consumption order changes play.

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
testing, modal scope, and controller ownership. Resetting Area state to repair
order would discard real focus and geometry.

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

## Persistent state

Supported player saves are a compatibility contract independent of implementation
history. Native save containers retain unknown bytes; known fields are patched in
the retained tree. ZIP, XOR/LZ, profile, and document codecs live in `solomon-assets`
and work on native and WebAssembly targets.

Large save/checkpoint records are heap-owned at deserialization boundaries.
Resource construction completes before saved-session admission so their large
futures and parsing frames do not overlap the WebAssembly stack.

Persistence uses revision checks and atomic synced replacement. Encoding and disk
I/O run outside the tick deadline. Welcome waits for a reconnect credential to be
durable; SaveCommitted follows actual file completion.

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

## Deployment identity and recovery

The game recovery revision and website publication identity have separate roles.
The deployment publication names website and game revisions, a unique deployment
ID, a monotonic sequence, and readiness.

Automatic reload requires the final durable save receipt, retirement of the old
transport, and a healthy publication at the announced sequence or later with the
signed target game revision. A new ID alone does not establish ordering.

## Navigation and painter registration

Retained scene geometry and prepared meshes are checked by source and geometry
identity. Other navigation domains are built from the admitted scene. The ordered
constraint sampling and triangulation remain in Rust because replacing them can
change route and neighbor order.

Painter registration belongs to the actor or transient owner. Fixed depth bands,
row sorting, and owner overlays are explicit data. A render cache cannot assign
new gameplay identities or reorder random consumption.

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

Package graph digests are a data compatibility contract: en-US collation,
ECMAScript number spelling, array-index ordering, and SHA256 must stay stable.
ICU collation and `ryu-js` provide those representations. Compiler tests protect
the digest when dependencies change.

## Native I/O

The native transport uses synchronous Tungstenite and standard TCP with owned
connection lifetimes. Asset transfers leave the framing pool after header parsing
and use shutdown-aware nonblocking writes. Slow media readers can pause without
truncating the declared response or preventing new request framing.

Event publishing and diagnostic serialization stay off the authority thread.
The UTC formatter at the launcher edge emits signed score timestamps with three
millisecond digits; the simulation itself does not read wall-clock time.

## Desktop packaging

The released shell owns the window, native menus, child processes, peer transport,
and optional updates. The Rust authority owns gameplay. Desktop startup serves
root-mounted WebAssembly with an external initializer and a narrow CSP that
permits WASM compilation.

A package records its release repository and immutable revision. Release archives
are inspected before the packaged journey. Unit tests alone cannot qualify a
desktop archive. Optional update failures leave offline play available.

Media checks distinguish trusted suspension on an already-ready channel from
truncation or an unexpected failure. Error reports copy their observations so
later page failures cannot mutate an earlier result.

## Test and documentation data

Committed fixtures keep their exact expected values, provenance, and compression
checksums. They run without a reference server. New fixtures need independent
evidence; expected output generated by the code under test does not establish
correctness.
