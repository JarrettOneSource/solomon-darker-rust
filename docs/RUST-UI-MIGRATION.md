# Rust GPU UI migration

The application layer registry follows the actual c140 body append/context ownership,
with native inner geometry retained from fa08. `match-loading-screen.css` gives the
body-mounted transition z200000 and toggles child visibility directly after its owned
delay; it has no additional Area fade. The original `#status` in `web/index.html` has
auto z and is passive, below positive document contexts. Native Boneyard notices remain
inside the world below the HUD; Hub book notices are their separate z45 body stage.
Registered paint, pointer/modal comparison and controller scope must use the same native
layer comparator. A late painter callback must not override a higher Source scope.

Acceptance target: all visible game UI is authored by Rust and submitted as GPU geometry,
textures and text. The behavioral reference remains Website `fa08ae249ff5563570bdb2a653e2ef9358ce7b83`
and the recovered retail evidence it cites. Existing simulation fixtures keep their actual
provenance. Browser/platform adapters may own the canvas, composition/clipboard/file APIs,
native downloads, screen-reader announcements, audio, network and persistence; they do not
paint game controls or mirror them as hidden DOM. Accessibility uses native `ariaNotify`
where supported or one specifically marked, nonvisual, text-only live region.

The migration begins at Rust `c14068b3c95aa40f0b1c85d11f73fe2622349aae`. An implementation
or a passing kernel fixture does not establish composed visual/input acceptance. Every pending
row below remains required; there are no accepted placeholders, dropped variants or required
TODOs. Final integration, publication, installed/live checks and task cleanup belong to root.

## Startup evidence recorded before repair

The reported failure is absent intended splash and visibly incomplete menus for roughly
20 seconds. The current source establishes a readiness defect, not that exact duration:
`web/index.html` initially paints only `Loading stock assets…`; `lib.rs::run` awaits
`Game::load` before scheduling frames; visual HUD/menu constructors create DOM images without
awaiting their individual successful decode.

Fresh production navigation on October 23 confirmed absent splash. The first HUD appeared at
22.798 seconds and first Title at 25.316 seconds while the status still read `Loading stock
assets…` and three images were incomplete. At 74.712 seconds the status became empty and the
WASM diagnostic became available, with 49 menu images still incomplete in that sample. There
were no page errors or console errors. This establishes premature UI exposure, not a universal
20-second duration. The navigation-only browser closed without sign-in, admission or data
writes. Raw 145-sample receipt and reviewed 20-second/ready screenshots are retained in
`startup-baseline/` under the full-Rust-UI task evidence directory.

Pinned Website sources are `frontend/src/game/{NativeLoader.tsx,game-assets.ts,
game-asset-readiness.ts,native-loader.css}`, `renderer/{loader-render-contract.ts,
loader-renderer.ts,game-viewport.ts}` and `frontend/src/pages/Game.tsx`. Website ledger
`docs/Game Native Parity RE/041-2026-08-14-total-startup-progress-and-active-item-presentation.md`
records native `MyLoader::Render` at `0x005BCA40`, numerator `0x0081F6A8`, denominator
`0x0081F6AC` and completion gate `0x0081F6B0`, plus clean-stock delayed-work evidence.

- Loader artwork becomes resident before the general startup workload. Title is installed
  only after the required load/decode promises complete; no menu appears underneath a
  partially loaded presentation.
- Pinned `game-assets.ts::loadGameStartupAssets` awaits loader, title, match-loading and
  resident-audio groups in that order. `pages/Game.tsx` then admits `MainMenuScene` only when
  this whole promise completes and auth, save and subscribed-mod readiness are complete.
  `main.tsx` mounts the same native Loader during lazy game-code loading. The older ledger's
  635-item manifest is historical, not the current manifest count; future world assets are
  not part of that source startup promise merely because a later scene can use them.
- The background is `0x000054`. A centered 480 × 320 logical composition has logo bounds
  `(41,13,388,227)`, URL `(119,251,244,18)`, frame `(125,263,230,54)` and fill
  `(144,282,192,18)`. The extracted assets already contain the native 90-degree rotation.
  The fill is clipped to `192 * completed / total`; viewport fitting uses the fixed
  1600 × 900 stage and its display scale.
- Progress is de-duplicated completed work divided by total required work across sequential
  loader/resident phases. Concurrent work remains concurrent. It never regresses and never
  claims 100 percent before completion. There is no timer, easing or minimum duration.
- Pinned `game-asset-readiness.ts::loadAssetBatch` owns a pending Set and reports its first
  remaining source after every completion. The candidate's `runtime_ui::completed`
  initially forwarded the just-finished source instead; its label could describe an
  already decoded item while another resource was held. Font completions were also
  reported as a group after Promise.all. This is a recovered readiness-contract gap:
  canonical progress must identify real unfinished work and count individual completed
  resources. Foundation owns that repair; held Title/font/music/module cases must retain
  the held request's actual filename as currentItem before their splash is accepted.
- Resident music follows pinned `game-audio-browser.ts::loadMusicChannel`: its Promise
  executor creates the channel, installs loadeddata/error handlers and calls load in one
  JavaScript task. The Rust candidate initially constructed channels with src before an
  asynchronously polled readiness future called load again. The actual named smoke
  retained cancelled academy/combat/death MP3 requests before browser close. This is a
  producer-ordering defect, not an allowed navigation cancellation. Root's repair must
  start empty media, attach handlers, then set src/load together; loadeddata stays required
  and every unexpected request failure remains fatal in the next composed startup.
- Root's `media-{python,native}-58df78d-create.json` traces distinguish later browser
  suspension from that duplicate-load defect: one load per resident channel, successful
  loadeddata, native suspend/NETWORK_IDLE, no media abort/error, then cancelled pending
  HTTP transfers while the same channels remain resident. Academy subsequently plays.
  This also occurs on the canonical Range-capable AssetServer, so Range handling does not
  explain it. The [HTML media loading contract](https://html.spec.whatwg.org/multipage/media.html#concept-media-load-resource)
  permits suspension after buffering and requires NETWORK_IDLE plus a suspend event.
  Acceptance must retain raw network failures and await a shared, per-channel ownership
  classification. Missing readiness, media errors/abort, reload/source changes, removal
  and unrelated cancellations remain fatal; a URL or ERR_ABORTED match alone is insufficient.
- Pinned `MainMenuScene.tsx:730` installs pointerdown and keydown audio unlock on window
  capture. Rust installed them on document bubbling; the GPU canvas owns and stops its
  pointer event before it reaches that phase. Actual `audio.cjs` on 58df78d times out
  after the real Play click while waiting for AudioContext running
  (`common-browser/58df78d/audio-pointer-red/run.log`). Restore the Source capture owner;
  retain pointer-only activation and the real playback/mixer assertions.
- The same Source director's `primeMusicChannels`/`finishMusicPrime` owns one successful
  zero-volume play per unused stock channel during unlock. Current/outgoing, already
  primed and pending channels are excluded; deliberate music pause excludes priming.
  Completion resets unused channels, but must preserve a channel that became active in
  the meantime and honor a subsequent deliberate pause. Source test
  `game-audio-director.test.ts:538` proves later automatic Academy playback. Rust lacked
  this established behavior. Reuse its resident channels with one per-channel promise
  owner; the browser audio check must observe native prime calls before keyboard input.
- Pinned Website also shows truthful stage, whole percentage, pending filename and ready
  item count. Those are explicit browser enhancements; stock per-file order/weighting is
  unknown. The separate match-loading lifecycle is not the startup denominator.
- High confidence: source geometry, readiness ownership and native work-bound lifecycle.
  Unknown: exact reported 20-second sequence on the new build until measured under cold,
  warm and deliberately held asset requests.

The four inputs added by integration `5502339` were independently compared byte-for-byte
with the pinned Website files. All match, with dimensions logo 388 × 227, URL 244 × 18,
frame 230 × 54 and fill 192 × 18. The source comparison receipt is
`startup-baseline/loader-source-assets.json` in the task evidence directory. No composed
startup acceptance is implied by that input check.

The mandatory named smoke follows College admission, the Office doorway, loadout
confirmation and its incoming Courtyard walk. Pinned `hub-regions.ts` defines Office
reentry at `(952.5,67.5)` with exact scripted target `(952.5,157.5)`. Its participant
movement enters `college-loadout` at that placement; `hub-world.ts` confirmation changes
only that phase to `incoming`, and `game-simulation.ts` preserves the player position.
The initial `hub-math.ts::HUB_SPAWN` `(950.64,164.04)` belongs before that whole intro.
The former smoke compared the later arrival against this stale initial position.
`common-browser/source-college-settle.json` independently evaluates the unmodified fa08
function and matches the retained Hub-region fixture. Actual `58df78d` observes the exact
Office target with transition `-1`. Acceptance must assert that completed arrival exactly,
without broadening a tolerance or adding a fixed settling delay.

## GPU color-filter contract

Pinned Website `fa08ae24` uses image filters as interaction state, including
`main-menu.css` skull `brightness(1.28)` and name-die `brightness(1.25)`,
`hall-of-fame.css` row-toggle `brightness(1.35)` with 120ms ease-out, and
`native-ui/native-ui-settings.css` row hover/focus `brightness(1.16)` and disabled
`saturate(0.35) brightness(0.62)`. The pre-migration Rust CSS additionally retains
native-button focus `brightness(1.2)` and Create glyph hover/focus `brightness(1.3)`.
These values belong to their feature controllers; the shared material supplies the
filter operation. The existing byte vertex tint cannot express RGB gain above 1, and
the Arena's separate arithmetic-average saturation is a different operation.

The [Filter Effects 1 specification](https://drafts.csswg.org/filter-effects/)
defines these CSS functions in sRGB, on non-premultiplied color, with each result
clamped. The GPU material applies saturation using the specified .213/.715/.072
matrix, then brightness, before the renderer's blend premultiplication. Alpha is
preserved. Direct GPU text/outline colors use the same operation through
`UiFrame::filtered_color`; bitmap text and stock images inherit the scoped material.
Descriptors are shared through one UI frame's ordered passes and retired at its next
begin, so animated gains allocate no additional textures or persistent descriptors.

`tools/browser-smoke/gpu-color-filter.cjs` compiles the canonical Rust shader unchanged
and compares its pixels to native Canvas color filtering. Six cases pass, including
gain above 1 on translucent pixels, disabled-row saturation/brightness, CSS luminance,
black/identity alpha and restoration of the ordinary world shader path. The previous
shader fails the first case by 31 channel bytes. These focused shader results do not
establish feature-controller wiring or composed hover/focus appearance; those remain
part of the menu owner's actual browser acceptance.

The Create name die also retains Source `drop-shadow(0 1px 1px #000)` with outer
opacity 0.78, changing to `brightness(1.25)` and a gold `drop-shadow(0 0 3px .../0.55)`
on hover. Its offsets and Gaussian sigma are design-stage units, independent of the
art's 0.525 scale. `StockAtlas::paint_shadowed` submits one padded quad: the shader
convolves only the selected record's alpha, composites the filtered source over that
shadow, then applies group opacity. Sampling clamps to the record's texel centers and
returns transparent outside its bounds, preventing neighboring atlas records from
entering the blur. It creates no additional texture or persistent effect cache.
The canonical-shader probe now passes sixteen cases, including an asymmetric mirrored crop.
Native idle/hover shadow comparisons differ by at most 5/2 channel bytes; the exact
group-opacity case differs by at most 1 and the mirrored-offset case by at most 6.
These Gaussian cases allow the specification's finite-kernel implementation variance;
the original source without a shadow material fails the atlas-isolation case by 199.

The retained authored-enemy producer applies its filter and outer opacity to the whole
root: the background sprite, name span and health rail. Rust `c14068b3`
`mod_world_overlays.rs:183–188,205` and `mod-world.css` establish that boundary. Its
original sources are pinned `mod-ui/ModEnemies.tsx:58,73–74` and `mod-enemies.css`.
blur is `max(light_radius/10,4)`; the name also has two black text shadows. CSS filter
drop-shadow takes Gaussian sigma directly; text-shadow uses half its blur radius,
so the name's 2/3px text shadows use sigma1/1.5. A sprite-only filter does not reproduce
that source group.

`UiFrame::paint_group` captures the submitted GPU geometry, including managed font tiles,
into a bounded reusable target. Nested name shadows compose before the parent enemy
shadow and outer opacity. The depth-indexed pool reuses source/composite targets across
frames and destroys them with the painter. It clips after the filter while preserving
outside source pixels that contribute to the visible blur. Graphics uses standard
SourceOver alpha only within these captures, unpremultiplies target samples once, and
restores the enclosing framebuffer, projection and native canvas blend factors. No
HTML/control rasterization is involved. The small graphics `composition_probe` example
and `gpu-composition.cjs` exercise the actual renderer's nesting and hand-derived pixels;
the original composition probe passes all framebuffer/projection and pixel checks.
The authored producer proof remains pending its matched runtime artifact.

Enemy names and authored shop labels request Alegreya600. Pinned `main.tsx` imports
only its400/500/700 faces, so `Alegreya Semibold` keeps that native CSS600 request
against the same loaded faces. A browser font-only probe confirms its glyph pixels
and width exactly match700 and differ from500. No new face or startup resource is added.

Pinned `ModSpellEffects.tsx` and `mod-spell-effects.css` also distinguish rounded
border-box shadows from sprite alpha shadows. The area uses an opaque circular caster,
18px blur radius and #9760ee/0.65; the channel uses its rotated rounded capsule,
12px blur radius and #a670ff/0.75. Gaussian sigma is9/6 respectively. Outer box-shadow
clips its blurred caster from the original border-box interior, even when that box's
own gradient is transparent. The existing group capture can supply these opaque GPU
casters; its shadow-only output needs that interior knockout before the pulse's outer
opacity. Epaint's filled, linearly feathered `Shadow` does not implement this contract.
`UiFrame::paint_outer_shadow` captures that opaque GPU geometry in the same target pool
and emits only blurred alpha times inverse original alpha. The native circle/capsule
comparisons pass with maximum channel differences3/2 and exact zero interior alpha;
the prior shader paints unwanted interior alpha10 in the circle case. The actual Rust
renderer probe passes typed outer/inset modes and normal-draw restoration. Group markers
retain their opacity0 parent even when
egui would discard an ordinary shape at that opacity, preventing child geometry escape.

The observer's pinned `developer-observer.css` adds outer0/8/30px and inset0/0/24px
black box shadows. `paint_inner_shadow` uses the same opaque caster capture and Gaussian,
emitting caster alpha times inverse blurred caster alpha. Its padding-box caster and
clip exclude the1px border; outer shadows precede the background, inner shadows follow
it and precede border/content, as specified by [CSS Backgrounds3](https://www.w3.org/TR/css-backgrounds-3/#shadow-shape).
The offset outer rectangle and inset comparison differ by at most7/2 channel bytes,
with exact zero unwanted interior/exterior alpha. The prior shader differs by223 in
the inset case. There is one blur kernel, target pool and texture owner for these effects.

Pinned `c14068b3:web/shared-gameplay.css` adds positive spread to the pause and resume
shadows: `0 0 24px 8px #000` and `0 0 28px 10px #000`, beneath a separate
`0 0 0 2px #57451f` ring. Their backgrounds remain translucent. The expanded border-box
casts the blur, but the original unexpanded border-box clips its interior. A single
expanded caster used for both cannot preserve that contract. The shared GPU operation
captures expanded opaque geometry, reuses the existing Gaussian, then destination-out
erases the independent original opaque geometry before final group opacity. Native
caster/knockout layout and actual Renderer pixel/lifetime checks qualify this path;
the composed shared-pause/resume producer remains Social-owned.

The same pinned `web/mod-skill-quickbar.css` requires `image-rendering: pixelated` for
its contain-fitted icons. Verified authored world textures remain LINEAR and borrowed
by the UI, so changing those texture parameters would alter their world rendering.
A renderer-owned WebGL2 sampler supplies NEAREST for the individual UI draw and restores
the previous sampler afterward. The texture identity, upload count, owner and original
filter/wrap parameters stay unchanged. Material keys include the draw sampler choice.

A native long-edge probe additionally exposed under-sampling in the earlier19×19
sigma-spaced Gaussian: the pause-scale160px edge at sigma12 produced four-pixel
plateaus and a maximum19-byte alpha difference from native Canvas blur. The original
receipt is retained in `gpu-large-shadow-red/`. Large UI shadows now use horizontal and
vertical dense Gaussian alpha passes through the same canonical shader, then the same
Source/outer/inset/spread composition. The two additional targets are allocated lazily,
reused by group depth and destroyed with the existing pool. Large atlas shadows enter
that shared group path; small physical sigmas retain their qualified fast path.

The unchanged native-shader reference drops this long-edge difference to4 bytes, with
smooth single-pixel progression and glError0 (`gpu-large-shadow-shader-green/`). All16
prior shader/material/crop cases still pass. The actual Rust Renderer probe also checks
scale1/2, inverted target UVs and an oversized reused target containing opaque pixels
outside the admitted crop. That compiled probe passes on62f835b with maximum4-byte alpha
differences at sigma12/24. The same run proves per-draw NEAREST versus restored LINEAR,
unchanged world texture filter/wrap parameters and sampler deletion; spread interior
alpha0, retained halo, antialias premultiplication, nested capture/blend restoration and
visible underlying pixels are exact. `gpu-final-composition-native/receipt.json` records
these results with errors[]; its artifact record pins the compiled JS/WASM and source.
The owned probe browser/server are closed. This is renderer acceptance, separate from
the composed authored/session-reset journeys and final publication gates.

## Native pointer cancellation

The integration read-through found `BrowserInput` mapping native `pointercancel` to
`PointerButton { pressed:false }`. In egui0.37 that ordinary release creates a click
when the preceding press is short and stationary; the following PointerGone removes
hover but does not cancel that click. This can activate an actual GPU button on a
cancelled touch. [Pointer Events3 suppression](https://www.w3.org/TR/pointerevents3/#suppressing-a-pointer-event-stream)
ends the native pointer stream and capture; it is distinct from a successful release.

The production native boundary projection now emits explicit `Event::PointerCancelled`
and the real pointer disappearance. Egui clears its button state, potential click/drag
owners, coordinates and velocity without a release/click; raw TouchPhase::Cancel remains
available to actual touch owners. Secondary touches retain their existing primary-touch
admission. Native Context regressions cover a short cancellation, active drag retirement,
multi-button mouse cancellation and subsequent genuine tap/double-click behavior. The
root owns the original short-touch red and final canonical checks.

## World notifications recovered before repair

The visual-producer inventory found `crates/web/src/loot/messages.rs` still constructing
DOM, with no `LootMessages` owner or callers. The pure message clock existed, but the full
notification surface was absent. Earlier broad UI completion evidence cannot accept this
unwired system.

The complete boundary is the shared floating text owner, not loot alone. Pinned Website
`fa08ae249ff5563570bdb2a653e2ef9358ce7b83` sources are `NativeWorldNotifications.tsx`,
`NativeLootBitmapText.tsx`, `loot-message-presentation.{ts,css,test.ts}`,
`loot-event-audio.ts`, `skill-book-feedback.ts`, `use-skill-book-feedback.ts`,
`SkillBookFeedback.tsx`, and their `BoneyardScene.tsx`/`MainMenuScene.tsx` callers. Existing
Website ledger 100 records native insert/update/draw at `0x005CA7C0`/`0x005D7EF0`/
`0x005CF000`; ledger 229 confirms Tutorial Sack labels appear only after accepted pickup.

- Membership: local loot text (including active Gold merging and all tint branches),
  rank-book feedback in College and Boneyard, local same-run secondary `Overloaded Mana!`,
  and local combat `CHEAT DEATH!`. Rank books use `book:-sequence` and tint `0x8080ff`;
  loot, combat and secondary IDs may be numerically equal and remain distinct source keys.
  Other players' notices and non-rank/rejected receipts advance cursors without painting.
- Ownership: accepted local event lanes own births. Initial/restored scene snapshots seed
  cursors after retained receipts; repeated display frames never re-consume an event.
  Session/local-player/world changes, tick rewind, Game Over and loadout reset presentation
  and baselines. One presentation advances only on accepted integer ticks, never wall time.
- Clock: initial lifetime `f32(1.5)`, offset `f32(-18)`, one-unit rise while active offset
  is negative, ordinary lifetime loss `f32(0.005)`. Distinct insertion repeatedly shifts all
  rows by four until the active row reaches zero, reducing older rows by `f32(0.1)` each
  shift. The retained older-row pressure rule and float32 boundaries remain exact. Active
  Gold merges only above lifetime one, resets that lifetime and retains the original key.
- Current visual projection clamps alpha to lifetime in `[0,1]`; scale first narrows
  `1-max(offset,0)/250`, then rounds through Source `toFixed(2)` and narrows again.
  The established loot-only `sample()` API and untouched `ed0a2d59` fixture retain their
  historical unrounded projection. Production uses the current projection on the same
  state/clock; no duplicate algorithm or old visual fallback remains.
- Geometry: center the menu-font bitmap text in the full gameplay viewport. Clip from
  native Y 50 down; each baseline is `50 + 17 + Math.round(offset)`. Both UI scale and
  existing gameplay viewport display scale apply. Black shadow is two native units below
  the same text and shares alpha; message scale affects glyphs, not that shadow offset.
  These notices do not inherit a centered Inventory/modal-stage transform.
- Render order: pinned `loot-message-presentation.css` uses z-index 45; `hub.css`
  places the HUD at 10000 and Tutorial CSS uses 31000. Notices therefore sit above
  world effects and below the HUD. egui drains unregistered plain layers after its
  registered Areas, so a plain Middle notification painter cannot preserve this
  relation. Its Background layer must be explicitly ordered above the actual
  `mod/world` Background layer, while HUD remains Middle.
- Visibility: alive and lethal-pending may show; dying, spectating and absent participants
  cannot. Boneyard Game Over removes the surface and Tutorial combat-access false hides
  it. Cursors/clocks still follow accepted state while a visibility gate hides pixels.
  The source rank-book result modal was another missing neighboring surface, now owned
  by HUD; its confirmation does not replace this world text.

The current source clock is replayed by `world_notifications.rs` against the untouched
fa08 oracle: nine programs and 1,960 records cover all four source keys, Gold parsing and
merge boundaries, insertion pressure, complete fades and two-decimal scale rounding.
Ten visibility cases, 77 book-name/error cases and 12 receipt deliveries are also exact.
All eight focused notification/book/historical-loot tests pass locally. GPU composition
and accepted production-event delivery remain pending. `WorldNotifications` now replaces
the unused DOM type with one accepted-session consumer and the menu-font GPU painter.
Five focused controller/render-queue tests cover initial/restored receipts, all local lanes and remote
filters, repeated snapshots, world/player/missing-player/terminal/rewind resets, and hidden
life/Tutorial clocks, and actual world/notice/HUD paint order. They pass locally; final game-loop wiring and browser proof remain
owned by integration. Geometry diagnostics retain original glyph ink and the Y50 clip.

Confidence is high for this complete pinned-source contract and the cited recovered native
geometry/lifecycle. Unknown until measured: composed GPU glyph clipping/blending, actual
earned-event browser visibility and live hosted event delivery. The source fixture and
renderer/controller checks pass; integrated production-event browser proof remains required.

## Scaled GPU hit geometry

Pinned `GameHud.tsx:249–258` places the HUD under its viewport/UI scale;
`hub.css:1097–1114,1298` gives the phone Inventory and Skills controls their
130px local bounds and custom transforms. The actual pointer target follows
that painted transform. On24382f0, the ordinary844×390 touch journey completes
College and Create, then its first visible Inventory tap never opens the book.
`record_rotated_rect` records globally transformed rectangular bounds but retains
local center/size in the exact rotated shape. Touch ownership tests a global
point against that local shape and drops the event; mouse dispatch does not
have that ownership admission requirement. The shape must use the same layer
translation and uniform scale as its bounds, while preserving local rotation
and rejecting points outside the rotated rectangle. The native regression
covers the visible center, an interior edge and an exterior point under both
zero and45-degree rotation. Actual tap and responsive family proof remain
required; no larger rectangular hit fallback is permitted.

## HUD and Social layer lifetime

Established Rust `c14068b3` places `.game-hud` at z-index10 and `.game-social`
at60, with only their children accepting pointer input. On5f87eb2 a fresh
College Party click works, but opening Inventory, closing it with Escape and
waiting for the actual book surface to disappear permanently blocks that
same Party control. The native `native-party-book-only-5f` receipt isolates
the failure without Chat and without browser errors. Holding pointer down
across frames still fails. The initially suspected same-dispatch Social
mutation was not the demonstrated cause and its candidate change was removed.

HUD disappears while the book covers it; egui raises its full-viewport Area
when it reappears. Paint-call order alone therefore lets it cover Social.
The real HUD layer owner supplies its identity to Social, which explicitly
orders its ally, Party and collapsed-chat layers above it. The Context
regression repeats the actual hide/reappear and pointer sequence, preserving
the unlinked negative control. Browser acceptance must repeat the original
book/Chat/Party journey on the combined build.

## Acceptance matrix

`Pending` means current composed behavior has not been accepted. Owner names match the
implementation lanes; scripts are under `tools/browser-smoke/`. The GPU families, canonical
notification composition and old visual-factory retirement are integrated. The joined
visual-producer source contract passes 3/3; that source result does not establish runtime
visual fidelity.

Root `7884455` repairs startup future allocation after the first main-WASM trap. Foundation's
`loader-probe/ordinary/receipt.json` now proves ordinary startup to Title in 5.891 seconds,
no browser errors, native ariaNotify with no fallback node, loader/main ownership transfer,
and retired loader draws staying 537→537. The separate `loader-probe/receipt.json` retains
the earlier deliberately held-main exact GPU splash comparison: at most one channel byte
in one of 88,076 pixels, native navy, wide/mobile resize, and one fallback announcer across
two owners. That earlier run trapped before handoff; its partial proof is not a passed
startup matrix. Cold/warm/delayed-resource/failure cases remain assigned to foundation.

Composed source `58df78d66d0530841edcd1a083e7649bb2d25d3a` proves the formerly blocked
named admission: the required smoke accepts actual `Helvidius`, `Ether`, `Arcane` after the
ordinary College dialogue, Office exit and Create edit. GPU clicks wait for stable,
unobstructed hit geometry. Play click followed immediately by Escape returns Title;
Inventory click followed immediately by Escape closes the panel without opening Pause.
The final current-art Courtyard/HUD screenshot was reviewed. The retained raw receipt is
`common-browser/58df78d/smoke/smoke-receipt.json`; it still fails because two MP3 requests
abort and its old initial-spawn assertion was stale, as recovered above. The corrected
exact Office-arrival assertion has not yet been rerun.

`common-browser/58df78d/gameplay-input/run.log` completes real pointer aim/release,
keyboard movement, potion edge/GUI release, held-source precedence, trusted tab-blur,
controller aim/belt and panel neutral-handoff assertions. The unchanged exact polling
assertion now passes eight reads over eight animation frames, correcting the retained
7884455 red of 16/8. Its final errors gate still fails on three MP3 aborts. Both local
static hosts answer Range requests with a full 200 response; that is a diagnosed host
limitation, not yet the demonstrated owner of the cancellations. Root owns the media
trace. No broad cancellation allowance has been added.

The shared observer now has 30 focused positive/failure regressions. It captures the
actual request's owning frame, native media source/load/events and document baseURI, then
awaits final ownership/readiness before classifying suspension. HTTP/media errors, missing
readiness, reload, removal, duplicate owners and unrelated cancellations stay fatal.
`common-browser/58df78d/media-owned-input` passes the complete unchanged input journey,
with eight reads/eight RAF and a reviewed final GPU Pause screenshot. Its
`browser-errors-1.json` preserves all three raw failed transfers and both lifecycle
snapshots for each permitted native suspension; unexpected errors are empty. Earlier
failed family receipts remain unchanged and still need their own clean current reruns.

Required smoke on `88c3dfe5977d4ba96216afa25366895ab2aab542` now passes all 14 checks,
including named Helvidius/Ether/Arcane, exact Office arrival, both rapid-input sequences,
movement/students and genuine GPU-only presentation with no unexpected errors. Full
current-art Title and Courtyard/HUD screenshots were reviewed; evidence is
`common-browser/88c3dfe/smoke/`. The actual audio test on that artifact also passes pointer
capture unlock/Title playback, then fails the Source priming assertion with zero
Academy prime calls (`audio-prime-red/run.log`). The complete audio journey now passes on
`24382f0c2055763bf100775b72e9e78a596dc68f`: actual pointer activation resumes the context
and Title, all five inactive native music channels prime once silently and fulfill/reset
before any keyboard input, then ordinary College admission automatically plays Academy.
Choose/cast/drink effects and real music/sound sliders pass. The reviewed final GPU Settings
image, native playback receipts, zero raw network failures and graceful owned-browser exit
are retained in `common-browser/24382f0/audio/`.

All seven startup cases are now qualified on that immutable 88c3dfe artifact across the
retained `startup-88c3dfe-{warm-verified,held,music-verified,required-error}` receipts.
Cold/warm pass with real native announcements and exact pending-request/document-replacement
ownership for the three raw warm-reload cancellations. Held Title, font and main WASM prove
89, 8 and zero completed items respectively, the exact unfinished path and native splash
without partial menus. Held music proves 322/326 plus one fallback announcer across loader
and main, priority before handoff and actual changed-control Tab announcement afterward.
The required Title 503 remains an explicitly owned visible failure. The original combined
held receipt still records its later focus-observation race as failed; focused music/error
follow-ups qualify those cases without rewriting that raw result.

Settings' five behavior groups now pass cleanly in
`menu-runtime-88c3dfe/settings-observed`, including the real pointer slider, bindings,
preferences and reload. Root's `restore-88c3dfe/sack-ordinary` proves accepted Unicode
Sack names and exact header restoration after an ordinary full reload/Continue, with no
deserialization trap. That original Sack run still fails only on unobserved old-document
media cancellation; its owned-replacement rerun remains required before a clean gate claim.

`common-browser/58df78d/boneyard/receipt.json` uses ordinary named Felix admission and
HUD Map. Actual generated native world/source geometry, inward keyboard movement, Pause
freeze, Leave/Title and Continue at the same position all pass; full current-art entry
and walking screenshots were reviewed. The next Pause fails the retained exact-save
barrier: live tick 2923, completed stored tick 2910. `commit_ui` checks the menu-opening
save edge before `frame_gameplay` handles `GameplayAction::Pause`, so the next commit
already sees an open menu. Root owns this production save-edge repair; no test timer or
weaker checkpoint assertion substitutes for it. The actual save and GPU snapshot are
preserved, and the three independent media aborts remain fatal. A second seed was not
attempted after this bounded failure.

Family evidence is likewise qualified. `menu-runtime-58df78d/hall` passes its actual
journey with no browser errors; reviewed native/compact screenshots prompted a header
gradient/border source correction awaiting rebuild. Settings bindings pass, while real
pointer slider focus and Shelf Keep focus remain red pending the queued producer repair.
Create variants prove stock Agrippa, edited Felix/Ether/Arcane and blank Genericus/Earth/Mind,
but their final media gate fails. `hud-browser-58df78d/player-panels` completes its named
Felix/Ether/Arcane inventory/equipment/Unforge/Skills journey before four fatal media aborts.
Its `family-status.json` also retains ordinary paid Sack and accepted Unicode names/font
quads passing twice, followed by a genuine saved-player reload deserialization failure.
The original accepted save/browser storage are preserved for root's repair. College
purchase/storage/Hagatha/insufficient-Dowse/boasts/keyboard-scroll assertions pass before
three media aborts. Desktop selector passes; landscape 844 × 390 exposes transformed HUD
clipping and its queued producer correction still needs native/browser verification.
`hosted-social-gpu-58df78d` proves six private-College/chat/card/consent/held-save witnesses,
then finds a duplicate Party settings ID; its queued source repair and retained media aborts
both require a clean rerun. These partial proofs do not accept every variant in a row.

| Surface and required variants | Owner | Implementation/reference and acceptance route | Current blocker/evidence |
| --- | --- | --- | --- |
| Process startup; cold, warm, held image/font/wasm; failed required asset; no partial Title or HUD | foundation + root; acceptance tests | Startup evidence above; `startup.cjs`; loader geometry, monotonic work, immediate readiness transition, screenshots | All seven cases pass on 88c3dfe across retained receipts; exact held counts/path, splash, no partial menus, native/fallback announcements and failure ownership proved; unchanged-loader proof is retained. Current57626c4 held-music guard passes exact322/326, real heldTab/Enter and first8Title frames with clean lifecycle; see startup-held-input-diagnostic; current3660ab8 required named/rapid smoke14 also passes on M5 with strict errors[]/Chrome0 |
| GPU runtime; stock bundle crops/rotation/registered origins, glyph masks, browser fonts, dynamic images, blend/clips | foundation | `gpu_ui.rs`, shared `graphics/src/lib.rs`, stock manifest; read-only `gpu_ui_state`, GPU meshes/textures and final screenshots | Core probe15/15 and precise source audit4/4 pass; f41 real joystick group submission/reuse is proved by native GL observations with stable174textures/2FBO. Full composed surface/style proof remains pending |
| Title/Play; clouds, moon, graves, Solomon animation, logo/art, Continue policy, account/desktop links | menus | `menu/title.rs`, pinned Main Menu; `smoke.cjs`, `menus.cjs`, `menu-input.cjs` | 3660ab8 common menus passes12 groups/errors[]/Chrome89934 exit0 with reviewed full Title/Create/Ether/Pause/Settings art. A separate actual controller probe remains red: Up from Play stays Play instead of the c140 Hall wrap target; menus owns the shared navigation repair. Account/desktop variants stay in their family gates |
| Tutorial prompt and saved-wizard retirement confirmation; Yes/No, reject/cancel and modal focus | menus | `menu/prompt.rs`; `title-prompts.cjs`, shared new-game journey | 3660ab8 title-prompts passes four groups/errors[]/Chrome92498 exit0: real saved wizard, decline preserves exact state, compact stock confirmation and explicit replacement intent. Tutorial Yes/focus route remains in common input coverage |
| College introduction/admission then sole Create; native text validation/rejected draft, blank/random name, five elements/three disciplines, exact effects/clocks/randomization, locked Back and admission retry | menus + HUD | `menu/create.rs`, intro/dialogue; `menu-journey.cjs`, `menus.cjs`, `menu-input.cjs`, `solomon-entrance.cjs` | 3660ab8 named Helvidius/Ether/Arcane smoke14, Create variants3 and common menus12 are clean, including native width/character rejection and full settled five-element/Ether-three-discipline art. 243 audio priming/scene/mixer remains qualified separately. ceb3a17 menu-input passes10 groups including first neutral+B, generated Macro/Earth/Arcane and touchFelix/Water/Body admission; additional retry variants remain pending |
| Pause opening/closing, Resume/Settings/Cheats/Leave, shared pause behavior, held-input neutral handoff | menus + social | `menu.rs`, `shared_gameplay.rs`; `menus.cjs`, `menu-input.cjs`, `hosted-session.cjs` | 3660ab8 smoke proves immediate Inventory-click→Escape releases to gameplay without opening Pause; common menus proves College actors continue while movement is sealed, real Resume and same-world Continue. 88c3dfe two-seed Boneyard Pause/exact-save/Leave/Continue and ceb3a17 controller/touch neutral handoff remain qualified; complete shared variants stay in social acceptance |
| Settings root/cloud/performance; sliders/toggles, permissions, saved preferences, nested account policies, mobile sharing | menus | `menu/settings/**`; `settings-parity.cjs`, `menus.cjs`, `fullscreen.cjs` | 57626c4 settings-all18 passes seven groups/errors[]/Chrome0: all18 real bindings Tab/ShiftTab/controller focus with fullpaintRect containment, wheel-away reveal, rapid Escape/swap/F2/mouse, sliders/policies/performance, reload persistence and compact layout; account/mobile permission variants remain in family routes |
| Control bindings; every action, duplicate swaps, Escape, modified-key rejection, allowed mouse bindings, reverse hotbar | menus + root input | `menu/settings.rs`, canonical `GameControls`; `menus.cjs`, `gameplay-input.cjs`, `hud-input.cjs` | 58df78d gameplay-input retains exact8/8 reads; 57626c4 settings-all18 proves all binding focus/swap/rejection/persistence paths. 3660ab8 common menus and HUD10 pass real rebinding, controller stick/dpad/drift and touch cancellation. ceb3a17 first neutral+B close is accepted; the separate current Title Up-wrap defect remains owned by menus |
| Save Transfer; multiple native/JSON/CFG/SAV/ZIP imports, inspected warnings, replace/cancel/busy/error, real export ZIP/download | menus | `menu/settings/transfer.rs`; `save-transfer.cjs`, `save-tools.cjs` | 24382f0 save-transfer/native-save-transfer passes all five groups, including invalid chooser input, ZIP preview/Cancel/Replace, real download/re-import and full reload to the same credential-bound Felix Fire/Arcane run; errors[]/graceful exit0; additional busy/error variants remain pending |
| Hall Local/Global × four boards; top100 and current row, portrait/orb, expand/scroll/pan, ranks/perks, loading/retry/stale result | menus | `menu/hall.rs`; `hall.cjs`, `hosted-score.cjs` | 57626c4 local Hall passes two desktop/compact pan/Back groups/errors[]/Chrome46271 exit0; 243 earned-score-canonical passes8 genuine account/gate/Solomon/death/signed POST201-once/Continue/new-wizard/saved-Leave/pointer groups. earned-score-real-tab-order adds3 fresh-sign-in/actual Tab-Enter expand-collapse/all4 API200 Global board groups for the same genuinely earned run. Both errors[]/graceful exit0; final Source-style and error variants pending |
| Join directory; normalized code, refresh, request/admission/cancel, private/mod/cheat/location policy, list scroll | menus | `menu/join.rs`; `join-party.cjs`, `hosted-social.cjs` | 24382f0 directory-party-tabs passes seven groups: real Private/Public/InviteOnly, leader Deny, lowercase-code Enter/consent and admitted Visitor Ether/Arcane, shared Boneyard IN GAME guest policy and both saved Leave PUT200; all three clients errors[]/graceful exit0 |
| Routing consent; vanilla/private/mod/cheat, added/missing/changed save mods, downloads/busy/error/retry and close invalidation | menus + root | `menu/routing.rs`, host routing; `cheat-consent.cjs`, `cloud-subscription.cjs`, `hosted-session.cjs` | 24382f0 cheat-consent-title passes four groups/errors[]; directory and subscription consent/native-admission witnesses pass separately; additional save-mod mismatch/download/retry variants remain pending |
| Dark Cloud frame/tabs; Mods/Subscribed/Parties/Mobile UI/Boneyards, horizontal reveal, source-supported search/sort tools, empty/loading/error, account availability | menus; root Shelf | `menu/cloud/**`; `dark-cloud.cjs`, `cloud-subscription.cjs`, `authoring-shelf.cjs` | GPU source integrated; actual five-tab and portrait Cloud journey pending composed browser proof |
| Cloud mods/detail; thumbnail lifecycle, subscription mutation/progress, screenshot contain/swipe/wrap, metadata/releases, comments/UTF16 limit/delete, preserved focus/scroll | menus | `menu/cloud/detail.rs`, real Website storage; `dark-cloud.cjs`, `cloud-subscription.cjs` | 24382f0 subscription-native-heading passes four groups: real Subscribe201/Disable/Enable under held transport and enabled mod seen by ordinary new-game routing before catalog completion; errors[]/graceful exit0 and owned subscription cleaned. Protected detail images/comment limits have partial58df proof; complete newline/editor and rebuilt detail-art rerun pending |
| Boneyard Shelf; local/cloud content, New/Import/Edit/Test/Delete and Keep, Test/Return/editor handoff and disposal | root | `game/hosted/boneyards.rs`; `authoring-shelf.cjs`, `authored-content.cjs`; export belongs to the external editor | 24382f0 shelf-root-canonical passes all ten groups/errors[] with exact owned-frame retirement and graceful Chrome exit0: local/cloud New/Import/Edit/Test/Return/Delete/Keep, one-use handoff, full native/JSON round trips and independence; prior observer reds remain retained separately |
| HUD resources/XP/concentrations, native/mod belt quantity/cooldown, three banks, portraits/map, tutorial gates, custom mobile positions | HUD | `hud/**`, `hotbar.rs`; `hud-input.cjs`, `hud-selector.cjs`, `belt-pull-off-check.cjs` | 3660ab8 M5 HUD input passes10 checks/errors[]/Chrome0, including real touchcancel/tabblur/stick/dpad/drift, readable Source label/active appearance and actual64×16 GPU label-target reuse with stable174textures/2FBO. ceb3a17 responsive passes the full phone Inventory/drag/first Equip/shop/18-swatch Dye/touchscroll/portrait/desktop journey with actual opacity1/target phase and reviewed Source-shaped stock toolbar labels; no additional K ordering defect is established |
| Inventory and native rank-book confirmation; result/OKAY/Escape, retained pause/audio, column-first cells, equipment/protected clothing/third ring, nested Sack pages/back/rename, animation lock, drag/drop/restoration flybys, stats/set/wizard preview | HUD | `player_panels/{inventory*,skill_book_feedback}`, `sack_*`; `player-panels.cjs`, `skill-book-result.cjs`, `responsive.cjs`, `save-resume.cjs` | 58df panel/equip/Unforge/Skills witnesses retained; 88c3dfe ordinary Unicode Sack reload/Continue passes behavior. f41 first phone Equip after real potion drag passes/errors[]/Chrome0. skill-book-24382f0-entered-notices passes both worlds: two rank reads each, OKAY/Escape, independent movement/native pause seal, readable native +1, normal reload/consumed absence/no retained modal or notification replay and choice picker; imported Source inventory, not earned loot; errors[]/graceful exit0. Remaining touch variants pending |
| Skills/quickbar; learned/ranked/weld pages, stock hover, every bank binding, skill/item drag and belt pull-off | HUD | `player_panels/skills.rs`, `skill_selector.rs`; `player-panels.cjs`, `hud-selector.cjs`, `mod-skills.cjs` | GPU source integrated; real learned/drag/bank/pull-off journey pending composed browser proof |
| College services; all shops/perks/prices, insufficient gold, storage, paid Dowse/ref/roll/Done, Unforge confirmation/results, dye admission/swatches/trim/cloth | HUD | `player_panels/{services,crafting,sack_rename}.rs`; `hub-services.cjs`, `player-panels.cjs`, `responsive.cjs` | 58df real purchase/storage/27 Hagatha offers/insufficient Dowse/Unforge pass before fatal media errors; ceb3a17 responsive cleanly purchases a real Dye kit, mixes among18 swatches, observes opacity1/target and touch-scrolls DONE. Actual cloth/trim commits and paid Dowse variants remain pending |
| NPC chat/books/boasts/authored choices; history, progressive text/audio/acceleration, selector list drag/wheel/controller, notebox/error | HUD | `player_panels/{dialogue,notebox}.rs`; `hub-dialogue-rooms.mjs`, `menu-journey.cjs` | 58df78d College dialogue plus five boasts and keyboard scroll pass before media errors; complete books/authored/selector/error variants pending |
| Level-up offer sequence; reveal/clocks, cards/insight/weld, responsive detail, choice/reroll/save and rejected-action recovery | HUD | `skill_picker.rs`; `hud-selector.cjs`, actual earned gameplay | GPU source integrated; actual earned offer/card/selection journey pending composed browser proof |
| Tutorial callouts/arrows; canonical text/narration and live GPU widget anchors; intro title/fade; Game Over riff/prompt/admission | HUD | `tutorial/**`, `college_intro.rs`, `game_over.rs`; `tutorial-text-bounds.cjs`, `solomon-entrance.cjs`, `boneyard.cjs` | ceb3a17 tutorial-entry passes actual YES/Midnight/Sirmin/run, four desktop/touch lesson layouts, real movement acknowledgement and exact lesson/narration text through native ariaNotify and fallback live-region updates; unchanged loading owners each emit once, errors[]/Chrome0. Later combat/modal anchors/potion and terminal run remain pending in tutorial-progression.cjs |
| World notifications; local loot/Gold coalescing, Hub/Boneyard rank-book notices, Overloaded Mana and CHEAT DEATH; per-run cursors, native clip/rise/fade/shadow, life/Tutorial/Game Over gates and screen-reader text | acceptance; root wiring | Recovered notification contract above; `sim/boneyard/loot/messages.rs`, `web/loot/messages.rs`; current-source fixture and genuine earned browser events | Current-source clock/GPU controller and13 focused tests pass; skill-book-24382f0-entered-notices passes Hub and Boneyard with two actual accepted +1 notices each at offset0/baseline67/clipTop50/scale1, reviewed readable screenshots, retained receipt/no replay and errors[]/graceful exit0. Source inventory is imported through the real chooser; ordinary earned loot and actual combat/secondary variants remain pending |
| Social Party; collapsed/coarse/expanded, Members/Settings/Mods, roster tags/scroll, privacy, copy/regenerate, Kick/Leave, request/invite Accept/Deny | social | `social/party/**`; `hosted-social.cjs`, `join-party.cjs` | 58df78d private admission/decline/held-save witnesses pass; Party opening duplicate ID red, queued correction plus media gate and remaining variants pending |
| Player cards/private College; Hub actor hit, Message/invite, resolve/decline/failure and owned roster | social | `social/{card,hub_players,hosted}.rs`; `hosted-social.cjs`, `hosted-session.cjs` | 58df78d two private Colleges, Global author card, decline/consent and source-membership-before-save witnesses pass; complete journey/media gate pending |
| Chat; Boneyard/Global/Party/Whisper, drafts/unread/fade/channel cycle, IME, author card and disconnect/reset | social | `social/chat.rs`; `hosted-social.cjs` | 58df78d actual Global delivery/author card pass; complete channels/IME/reset and fatal media gate pending |
| Shared Pause/resume waiting; other-owner activity, dim/reveal, countdown/progress/rejoin; spectator rail/target/respawn | social | `shared_gameplay.rs`, `spectator.rs`; `hosted-session.cjs`, `spectator.cjs` | GPU source integrated; actual shared pause/respawn/target lifecycle pending composed browser proof |
| Cheats/Console/developer observer; permissions, all resource/skill/world/dev controls, Lua submit/history/results/error/clear and bot targets | social | `cheats/**`, `observer.rs`; `authored-content.cjs`, `learned-bot.cjs`, `cheat-consent.cjs` | GPU source integrated; actual authorized Console/observer/bot journey pending composed browser proof |
| Mobile editor; all six anchors/elements, drag/resize/rotate/snap, zoom/pan/pinch, multi-touch/cancel, undo/redo/reset/pages/dock/presets/save | social | `mobile_editor/**`, `sim/mobile_ui/**`; `mobile-editor.cjs`, `responsive.cjs` | 58df78d actual drag/Undo/Redo pass before the retained pointer-focus defect; its 24382f0 correction is integrated, full editor/resize/rotation/save/cancel rerun pending |
| Authored mod surfaces; Cast/Shop/Reforge/Portal, skills/subskills/offers/quickbar, minimap actions, mounted HUD text/image/bar/button, party attributes | social | `mod_panels.rs`, `mod_skills.rs`, `mod_skill_quickbar.rs`, `mod_minimap.rs`, `mod_world_overlays/**`; `authored-content.cjs`, `mod-skills.cjs` | 58df78d authored-r3 admits the actual package and purchases; five borrowed textures each upload once, then old scaled-HUD clipping blocks progress; 24382f0 contains that repair, remaining widgets/filtering/sampling/reset proof pending |
| Match loading; every recovered stage, progress/cancel/failure, host admission/world materialization | root | `match_loading.rs`; `boneyard.cjs`, `hosted-session.cjs`, `deployment-restart.cjs` | 88c3dfe complete two-seed ordinary Map/native-world/Leave/Continue/exact-save journey passes, including clean owned browser retirement; full stage/cancel/failure/host variants remain pending |
| Fullscreen/install guidance; request/exit/change/error, unsupported/standalone/per-screen placement | social | `fullscreen.rs`; `fullscreen.cjs`, real browser capability | GPU source integrated; real fullscreen/unsupported/standalone activation pending composed browser proof |
| Desktop notice/conflict/close lifecycle; offline/peer/update/Keep Playing/failure/save | root | `desktop.rs`, `desktop/`; `smoke-desktop-runtime.mjs`, desktop failure tests | GPU source integrated; actual update/close failure plus packaged journey pending composed browser proof |
| Deployment update/checkpoint/recovery; same revision identity and error ownership | root | `deployment.rs`, actual host lifecycle; `deployment-restart.cjs` | Three actual recovery branches pass/errors[]/graceful exit0: changed58→243 (deployment-changed-58-to-243-r2), same game243/Website6ad, and sleeping sequence4→5; checkpoint/ACK/retirement/negative publications/automatic replacement/Continue witnesses retained; packaged/update-notice variants remain separate pending gates |
| Cross-cutting inputs; pointer/keyboard/touch/controller, focus/text/scroll/modal sealing, real screen-reader focus/value announcements, blur/cancel, resize/portrait/wide/fullscreen | all; acceptance common tests | `gpu-ui.cjs`, `menus.cjs`, `menu-input.cjs`, `responsive.cjs`; final current-art screenshots | 3660ab8 smoke14, HUD10, menus12, prompts4 and native editor9 pass strict errors[]/Chrome0. Editor proves held-rAF public diagnostics/gamepad reads unchanged across Tab/ShiftTab and OS copy/paste, plus ArrowRight/bidi/Arabic/ZWJ/Undo/browser IME/UTF16 Enter-save; hardware candidate-window proof is excluded. 57626c4 settings-all18 and ceb3a17 native/fallback Tutorial entry/final-opacity responsive remain qualified. Current Title Up-wrap and remaining family journeys stay open |
| No DOM/CSS facade, iframe visual replacement, test-only state fabrication, unfinished required TODO or dropped feature | root + acceptance | Source scan, DOM paint audit, real GPU counters/read-only widget geometry, all rows above | Precise source producer contract4/4 passes, retaining only bounded native platform/font/a11y adapters; complete runtime surface/input proof remains pending |

## Saved-session startup allocation

Ordinary saved Sack and College reloads on58df78d trap first inside the generated
`PlayerState` JSON deserializer, before later loader diagnostics encounter the damaged stack.
The natural save, browser IndexedDB state and first stack are retained under
`reload-58df78d/` and `hud-browser-58df78d/sack-reload-witness/` in the task evidence.
The actual WASM prologues reserve525,760 bytes for `Game::load` and311,248 for `run`
before decoding; player deserialization adds66,992 bytes and its map visitor another100,368,
plus the containing save/participant frames. These overlap the unchanged1 MiB stack.

Resource construction and saved-session admission must be sequential lifecycle phases.
The constructor returns the permanent shared Game owner; saved-state parsing runs after
resource-constructor temporaries retire. Simulation/save structures, validation, defaults
and the WASM stack configuration retain their existing contracts. Real reload/Continue
with the naturally generated save is the regression witness; a compile alone is insufficient.

## Ordered UI dispatch

The actual rapid Cloud Sort→row double-click journey exposed a separate installed
egui0.37 count mismatch. Its click counter checks distance only to the latest click,
then classifies a triple from two prior timestamps even when the first click was far
away. The native Context regression confirms row counts1,3 instead of browser1,2.
The browser already supplies the correct click series in the canvas `click.detail`
event used by native editor selection. That receipt belongs to the preceding actual
pointer-release dispatch; preserving it beside that dispatch lets row double activation
retain the real Response hit/enabled result without accepting genuine triples as doubles.

The actual Settings binding journey reproduced a binding click followed by Escape being
interpreted in reverse order. The shared GPU batch previously handled every queued key before
resolving pointer clicks, and `Game::frame` consumed typed HUD/panel actions before the GPU
pass produced them. This also delayed opening Inventory until after a following Escape.
The Source's browser callbacks commit each event before dispatching the next event.

`GpuUi` now separates ordered input/layout commits from the final GPU submission. The Game
owner commits controller actions and refreshes accepted-state models between those batches;
transport polling, physical gameplay sampling and fixed simulation advancement retain one
owner per browser frame. World rendering precedes the final UI draw. Texture upload/retirement
is finalized even when a controller fails. Returning to the editor stops later queued input
and gameplay work in the same frame while still retiring pending textures. Observer and
deployment barriers retain their read-only/held semantics. The binding/text-focus batch regressions and actual rapid
Settings/HUD journeys qualify this seam; the latter remain pending the combined rebuild.

The ordinary58df78d Boneyard journey preserved world entry, movement, Pause/Leave and
Continue, then exposed a local durability regression on its second Escape: live tick2923
but saved tick2910. `MenuUi::paint` can open Pause before its typed-action commit, so sampling
the old blocking state inside that commit misses the edge. The Game owner now captures
that state before each input pass, preserving the existing local pause-save contract.
The exact completed-save assertion remains the acceptance check; no timing tolerance is added.

## Test ownership and stopping checks

Acceptance owns `gpu-ui.cjs`, `gpu-ui-contract.test.cjs`, `menu-journey.cjs`, `smoke.cjs`,
`startup.cjs`, `menus.cjs`, `menu-input.cjs`, `responsive.cjs`, `audio.cjs`, `save-resume.cjs`,
`saved-game.cjs`, `college-clock.cjs`, `rooms.mjs`, `hud-input.cjs`, `tutorial-text-bounds.cjs`,
`boneyard.cjs`, `gameplay-input.cjs` and `electric-burn.cjs`.
Menus owns detailed Hall/Settings/Cloud/Join/prompt
journeys. HUD and social own their family journeys. Root owns final joined execution and
packaged/hosted/authoring integration. Exact stable IDs and snapshot schema are in the task's
`gpu-ui-contract.md`; that is a development handoff, not an accepted product artifact.

The common scripts use read-only GPU geometry and actual browser input. Syntax checks pass.
The pre-cutover source audit retained two real visual-producer failures and one passing test in
`gpu-ui-contract-baseline.tap`; the joined producer cutover later passes all three original
checks. The precise typography-adapter extension passes all four source checks against the
HUD owner's implementation and still needs the joined CI gate. Actual passing and failing
journeys are recorded above; syntax or source checks alone never accept a visual surface. The independent
Save Tools module ABI journey remains a valid canonical-byte fixture; its real Files are
inputs to that independent module and do not fabricate gameplay state.

The screen-reader observer forwards actual native `ariaNotify` calls unchanged or observes
the one `data-gpu-announcer` live-region text node. The menu journey requires newly announced
slider label and accepted value after real keyboard edits. It does not create announcement
messages or mirror controls. Unmounted native download anchors and hidden audio/file/IME
adapters remain platform capabilities; mounted visual anchors and hidden gameplay controls
still fail the browser audit.

The native font-run geometry adapter is limited to
`gpu_ui/edit/font_run.rs`: an SVG root with one text child, marked `data-gpu-font-metrics`,
hidden and noninteractive, attaches only for synchronous typography measurement and is
removed after success or failure. It measures native joining, bidi caret and selection
geometry; an unattached canvas paints text-only GPU tiles. The source audit permits only
those exact SVG/text constructors in that module and checks the hidden/transient contract.
Other SVG producers and every HTML screenshot/control facade remain rejected. Runtime
GPU-only acceptance still requires the transient probe to be absent between operations.

Layout assertions use original `paintRect` glyph/card bounds. Visible clipped hit `rect`
remains the pointer target, so clipping cannot make an overflowing title, tooltip or Tutorial
instruction appear to pass. HUD blur uses an actual browser tab focus change; touch cancellation
uses the native touch-cancel input and verifies both the neutral painted joystick and accepted
zero movement. No synthetic browser blur event stands in for platform focus behavior.

Phone journeys preserve the actual portrait admission rule. Pinned Website
`MainMenuScene.tsx` and `main-menu.css` at its portrait media query, and Rust `c14068b3`
`web/menu.css`, place native stages behind rotate-to-landscape guidance on coarse portrait
devices; Dark Cloud alone remains portrait-admitted. The old phone scripts attempted native
gameplay at 390 × 844 despite that gate and are stale evidence. Current tests require the GPU
guidance, rotate to 844 × 390 for actual admission, compact controls, touch drag/shop/dye and
cancel checks, and verify guidance returns on portrait resize. Portrait Cloud acceptance
remains a separate required menu route. No media-query override or hidden activation bypasses
the source gate.

The foundation's retained `foundation-probe/result.json` and `extended-result.json` were
reviewed with `gpu-unicode-scroll.png`: all 15 checks pass for real GPU submission, loaded
static inputs, pointer button/checkbox, text editing, Unicode/IME commit, native clipboard
copy/cut/paste, wheel scrolling and absence of scrolled-out rows. The body contains only the
GPU canvas and marked text adapter in that probe, with no runtime errors. This is core runtime
evidence; it does not accept the game menus, mobile layout, startup or the later accessibility
follow-up by implication.

Checks must drive ordinary pointer, keyboard, touch or controller input against visible GPU
widgets. The read-only snapshot locates those widgets and reports accepted outcomes; it
cannot invoke commands, fabricate saves or mutate runtime state. Existing numeric/kernel
fixtures remain. Obsolete DOM assertions are replaced with equivalent visible behavior and
GPU evidence, not removed to make a gate pass. Actual browser errors and required-resource
failures remain fatal; an expected navigation/retired-resource cancellation needs an exact
owning transition and retained evidence.

Completion requires every surface row accepted, required Rust/native/WASM checks clean,
meaningful current-art browser smoke and affected full journeys clean, source/DOM review
clean, and packaged/live checks at their separately stated boundaries. Post-push cleanup of
task worktrees and owned processes on every device is part of root completion; retained paths
must carry a concrete reason. The unrelated launchd preview on port 4180/PID 96731 is preserved.

Browser teardown also has a concrete diagnosed boundary. Both 88c3dfe Boneyard runs pass
two ordinary seeded worlds, exact Pause saves and continuation. The first driver hangs
after Chrome exits and requires Node termination (143); the explicit-owner follow-up
retains graceful/forced close timeouts as fatal despite Chrome exit code zero. Native
`lsof` proves reparented Crashpad/GoogleUpdater helpers retain the owned child's stderr
socket. Installed Playwright `coreBundle.js:8799` waits for ChildProcess `close`, which
cannot arrive while inherited stdio stays open. The task's `boneyard-owned-close` receipt,
native sample and socket ownership evidence preserve that diagnosis. After confirmed
owned Chrome `exit`, release only its parent-side stdio so the real close/temporary-file
cleanup can finish. Other helper processes remain untouched; timeouts and nonzero exit
remain failed gates. A title-only two-context close probe is a narrower passing boundary,
not a clean full Boneyard execution claim. The later `boneyard-stdio-fixed` original-case
rerun qualifies the full process gate after the inherited-stdio repair, with Chrome exit
zero and no timeout or forced termination. Two real Node process regressions separately
prove the inherited-pipe failure, preserve live stderr/nonzero exit, and leave the holder
running when the parent's streams are released.

The first actual coarse-touch HUD journey on 88c3dfe exposes a stale common test contract,
after its desktop geometry/controller/blur checks pass. `reachCollegeLoadout` expected
`ARCH_DISMISS_0` after Q1 speech → question list → Done. Pinned fa08
`HubDialogueActions.tsx:249` calls onDone, and `HubInventorySurface.tsx:855` delegates to
`dismissOrCloseChat` at line 514, which closes the surface directly. Rust c14068b3 and
the GPU candidate match that behavior. The compiled dismissal row belongs to the
no-remaining-choices opening fallback in `hub-npc-dialogue.ts:98`, not this Done action.
Retain actual touch Skip/question/Done and visible-close assertions, remove the invented
extra dismissal/Skip step, and rerun the unchanged HUD movement/cancel requirements.
The original red is `common-browser/88c3dfe/hud-input/run.log`; cleanup exits zero.

The subsequent real-touch run isolates the production input defect:
`joystick-native-trace/receipt.json` records computed canvas touch-action auto, visual
scale one and native focus true. The real pointerdown acquires implicit capture; its
first move is followed immediately by a trusted pointercancel/lostpointercapture before
any test cancellation. Accepted movement stays zero. Dialogue is already closed and
the input scope is null, so delaying that transition is not the repair. Pinned fa08
`input/touch-joystick.css:17` and `hub.css:1029` disable native touch manipulation for
the joystick/scene. [Pointer Events section 8](https://www.w3.org/TR/pointerevents3/#declaring-direct-manipulation-behavior)
requires touch-action before the gesture; cancelling pointer events alone cannot
prevent browser pan/zoom, which cancels the pointer stream. Restore that nonvisual
platform input policy on the actual game canvas at adapter attachment. Keep genuine
pointer capture, explicit cancel/blur clearing, GPU joystick state and movement checks;
no keyboard, synthetic game command, delayed gesture or hidden control substitutes for it.

The original nine-part HUD input journey now passes on 24382f0, including genuine
coarse-pointer movement, native touch cancellation, zero accepted movement afterward,
neutral painted joystick and no joystick keyboard focus. The desktop geometry, wide layout,
stick/D-pad priority, drift and actual tab blur checks also pass. Evidence and reviewed
screenshots are in `common-browser/24382f0/hud-input-verified`; the process and owned Chrome
both exit zero with no unexpected errors. The earlier same-artifact run passed the behavior
but correctly failed its stale cleanup's TouchEnd after TouchCancel; that raw failure is
retained rather than rewritten.

That screenshot also exposes a remaining visual port gap: the MOVE label is about four
pixels high. Pinned fa08 `input/touch-joystick.css:49` uses Cinzel 700 via `--font-display`,
an 11px font divided by `--hud-display-scale`, unit line height, bottom 24px, 0.06em spacing
and active opacity 0.45. `HubScene.tsx:953` applies the same display scale to the native
frame, so the inverse font scaling preserves an 11px label at default UI/custom scale.
The current GPU painter uses Georgia Bold at 9px times the full joystick scale and a
different vertical anchor. The touch repair is accepted independently; label typography,
placement and active appearance still require the recovered Source styling and a new
current-art visual check.

The isolated `joystick-label-source/receipt.json` runs the unmodified pinned CSS and
viewport kernel through the native browser for five viewport/UI/custom-scale/rotation
cases. At 844 × 390 its neutral label line box is y=343.0307, height=11.0026 and bottom
354.0333 CSS pixels; active opacity is 0.45. It qualifies typography constants only,
not ordinary game admission or input. The correction reuses the installed Cinzel 700
face, a unit-height GPU galley and the shared GPU group compositor for its shadow and
opacity. [CSS Text Decoration](https://www.w3.org/TR/css-text-decor-3/#text-shadow-property)
uses box-shadow blur values; [CSS Backgrounds](https://www.w3.org/TR/css-backgrounds-3/#shadow-blur)
defines the corresponding Gaussian sigma as half the blur radius, so the Source 3px
text shadow uses sigma 1.5 before joystick transforms. Passive diagnostics record the
actual glyph mesh bounds and paint style; the original touch driver now requires
readable glyphs, exact default-size typography and neutral/active/cancelled appearance.
The composed build and actual corrected-art rerun are still pending.

The subsequent real 5f87eb2 phone journey opens Inventory and drags the actual Health
Potion into its addressed destination, then its first Equip tap leaves Backpack visible.
`responsive-5f87eb2-destination/responsive-failure.{json,png}` retains errors=[], actual
toolbar controls at y=0–40 and book/stats painted over that toolbar. The established
compact adaptation is explicit in c14068b3 `web/player-panels.css:89–96` and
`player_panels/viewport.rs::layout_compact_surface`: the scrolling body starts at 48px
and clips overflow, navigation stays above it at z-index 10, and modal overlays use 12.
This adaptation preserves the fa08 native Inventory/Skills/service designs at usable
phone sizes; the 48px toolbar is the existing Rust behavior, not a newly invented fa08
control. Installed egui 0.37 `containers/area.rs:610` raises an interacted Area, so giving
both Areas Foreground order and relying on paint-call order loses the toolbar after
body interaction. Raw panel input also ignores that body viewport and actual layer.
Use the same 48px-excluding viewport for rendering, clipped hit regions and raw pointer/
wheel admission, explicitly keep navigation above the body, and retain a drag's actual
owner through release/cancel outside its viewport. Preserve normal modal ownership.
The original failing browser path remains the acceptance case; no second tap, wait or
weaker equipment assertion substitutes for the first-tap transition.

The same recovery identifies the toolbar's visual producer as unfinished: its GPU code
uses default grey egui buttons. Established c14068b3 `ui.rs::native_button` instead uses
the stock main-menu plate/hover/corner/rail images and native Menu bitmap labels;
`web/player-panels.css:91–93` specifies solid #100d0a, 6px horizontal padding, 2px gaps,
40px controls centered in the 48px bar and horizontal overflow scrolling. Reuse the
existing Rust `menu::widgets::stock_button` to preserve that art, pressed/focus/disabled
appearance and actual semantic egui interaction IDs. Keep the fixed bar's scroll/input
owner separate from the scrolling body; no default-widget placeholder remains in it.

The hosted 24382f0 imported-book diagnostic retains the original normal File chooser →
reload → Continue → first KeyI sequence. At that trusted BODY dispatch, native focus is
true, no text adapter is active and default prevention is false. The actual GPU state
still owns `match-loading`, acceptedSelection is null and the default wizard tick is
zero. The real authority Welcome arrives 150ms later; the accepted Book Reader and
scope-null presentation arrive about 200ms after the rejected key. Full event, frame and
authority flags are retained in `skill-book-key-trace/24382f0/native-key-trace-1.json`,
with errors[] and graceful exit0. Enabled background widget records alone do not prove
visual exposure beneath the match-loading layer. Pinned fa08 `MainMenuScene.tsx`
`resumeLastGame`/`activateSession`/`finishLoading` and the loading-dependent
`sceneModalDisabled` seal input until the accepted session/renderer transition completes.
The book driver must observe its actual imported selection, ready authority flags and
released match-loading scope before sending the same immediate first KeyI. A timer,
repeat key or pointer substitution would not validate that admission boundary.

With that real admission boundary, hosted 24382f0 completes the Hub book journey:
two accepted rank reads, OKAY/Escape, independent movement seal, actual native +1
notifications, consumed-book persistence after normal reload and a genuine choice-book
offer, all with errors[]. The complete two-world receipt remains failed because the
Boneyard input restored `save-active-goldens.json`'s minimal Save Actor Arena. Its
unshipped 5849488c… navigation identity is explicitly rejected by the native content
loader. `skill-book-24382f0-admitted-verified/receipt.json` and screenshots preserve
both boundaries. Keep that historical actor/kernel fixture unchanged. Generate the
browser's Boneyard case through pinned Source `createGameSimulation`,
`enterBoneyardWorld` and stock `NATIVE_GENERATED_BONEYARDS`, then the same public book
insertion/read/create/restore APIs. The native import regression must also decode its
actual shipped navigation; no test-only navigation or fabricated runtime messages are
added to the product. This remains imported-inventory acceptance, not earned loot.

The f41dc57 original phone journey now passes Inventory admission, genuine potion drag
and the first Equip tap after that drag; `common-browser/f41dc57/responsive/` retains the
actual before/after GPU states and reviewed equipment image. Its next failure is the old
one-line hat-warning assertion. Pinned fa08 `hub-inventory-render-contract.test.ts:1282`
and `native-ui.test.ts:363` specify the painted wrap `A WIZARD WOULD NEVER\nREMOVE HIS HAT!`.
Require that exact painted text rather than flattening its layout or changing the producer.
The same toolbar image exposes one remaining local paint-order issue: later stock corner
art covers the prior label's final glyph. Established `web/menu.css:108–110` assigns all
native labels z-index 1, above the sibling button chrome. Preserve that by painting the
bar's stock art first, then its real native labels; keep the existing interaction owners.

The ordinary paid Sack journey in `native-editor-f41dc57-qualified-r2` passes on
f41dc57 with no unexpected browser errors and an owned Chrome exit of zero. The
reviewed GPU screenshots show joined Arabic, a ZWJ emoji and combining mark, two
disjoint mixed-direction selection rectangles, and the complete native IME preedit
underline while its caret is inside the preedit. Real pointer/keyboard input also
proves weak bidi caret placement, native arrow selection changes, synchronous OS
copy/cut/paste, separate undo transactions, the UTF-16 boundary and one Enter rename.
IME input uses Chromium's native Input protocol, rather than an application event;
an unchanged native textarea oracle produces the same untrusted composition-end
event for protocol commit. This proves browser IME integration, not an OS hardware
candidate window. The earlier failed trust assertion and stale-frame caret assertion
remain in their original receipts; the final driver waits for the actual painted
caret without repeating input or changing product behavior.

The following f41dc57 phone continuation passes the hat warning and Skills but misses
Fomentius with its ordinary world tap. `responsive-source-warning/phone-world-entry-failure.json`
retains wizard position (1358.58,728.51), requested canvas point (455.11,202.8), no dialogue,
errors[] and graceful Chrome exit0. Pinned fa08 `renderer/game-viewport.ts:110–141`,
`core-kernels/hub-math.ts:36–56` and `HubScene.tsx:864–886` expand the logical viewport
before clamping the camera and projecting the pointer; they do not center a 1600px world
stage. At 844×390/default FOV the logical view is 1947.69×900, so the courtyard camera's
right-edge origin is 376.923 and Fomentius' x1397 projects to x530.44. The test's fixed
1600px camera plus centered-stage offset projects x455.11 instead. Correct this test
projection using the same bounded viewport/camera contract, retain the real joystick
route and single native touch, and record its actual dialogue result. This evidence
establishes a driver projection error; it does not establish a game input defect.
The corrected single touch opens the actual Fomentius dialogue/shop in
`responsive-source-camera/phone-world-entry.json`. The next test-only failure assumes
one uniquely named Dye kit, while fa08 `core-kernels/hub-economy.ts:526` rolls two or
three identical 300-gold kits (`rollBound:2`, `quantityOffset:2`). The actual GPU shop
contains two distinct rows, IDs16/17, and errors[]/graceful exit0. Select its first
visible kit after requiring that source stock range; keep the real purchase and owned
inventory quantity assertions instead of weakening the common unique-label helper.
`responsive-source-dye` then purchases the real kit, opens Fabric Dye and selects
both swatches; its next failure targets DONE outside the scrolled viewport. The actual
clipped widget registry correctly excludes it. Established c14068b3
`web/player-panels.css:98–99` makes the compact dye sheet scroll, and
`crafting.rs:538,653–658` puts DONE below the item section. Preserve this layout and
reveal DONE with one genuine upward touch drag inside the actual dye viewport before
tapping it. `native_dye_modal_opacity` adds .01 per10ms tick: the opening reaches full
opacity at1000ms; the selected-swatch pulse is a separate200ms clock. The final art
screenshot must observe the full opening, without delaying or repeating activation.
HUD's owner agreed that the existing passive `panel:dye` value records JSON
`{phase,opacity}` from the same native clock used by its actual paint. It adds no action
or state setter; read-only consumers can wait for the submitted opacity1 frame.
The complete f41dc57 continuation `responsive-source-scroll` passes all original
touch/resize behavior, including first Equip after drag, exact hat warning, Skills,
single world touch to Fomentius, real Dye purchase/mixing, upward native touch scroll
to DONE, portrait guidance and desktop return; errors[]/Chrome71236 graceful0.
Reviewed final screenshots retain the queued sibling-chrome label correction and
the still-unobserved full Dye opacity. The rebuilt driver requires the actual JSON
opacity1/phase target frame before its final Dye screenshot. The controller College
helper likewise removes its stale second-B/ARCH_DISMISS expectation: pinned fa08
`HubInventorySurface.tsx:514–534,855` closes the Q1 dialogue on the first Back/Done,
matching the already-qualified touch source recovery; real B must prove that closure.
The first actual f41 controller run reaches Title/Settings/Play and Q1 through real
Gamepad input, then fails that unchanged first-B closure after a neutral release.
The retained job `job_20261024T000845Z_9eafc8d36d` reports the actual speech/Q1 scope,
Skip control and no focused widget; Chrome93904 exits zero gracefully, driver exits1.
Its old failure branch did not finalize the observer, so it has no clean-error claim.
The common driver now settles and records its actual pad/input/GPU state and screenshot
on failures. Root's new input/controller transaction requires the same first-B rerun;
no second press, pointer substitution or delay qualifies that behavior.

The Tutorial text-bounds helper currently has no executable caller, so its syntax check
does not qualify Tutorial layout. Pinned fa08 `core-kernels/native-tutorial.ts`
`nativeTutorialPresentation`/`nativeTutorialInstructionBaselines` owns the Movement
lesson's two-line desktop/mobile headings and subheading after the actual intro; the
first accepted requested movement acknowledges and hides that lesson. Exercise the
ordinary Title YES admission in fresh contexts, preserve the actual intro/save/run,
check the full painted glyph bounds at native and phone landscape sizes, then send real
keyboard/joystick movement and require the lesson to disappear with accepted displacement.
Later combat, Inventory/Skills and potion-stage callouts remain separate required gates.
The retained 58df teacher census failure is a driver ordering error: it stops at
Regenerate (79), the fifth Source row, and then requires eight observed rows.
Unchanged fa08 `native-hub-npc.ts::NATIVE_TEACHER_SPELLS` returns the authored order
72,73,74,75,79,78,77,76, matching the Rust catalog exactly. The driver now traverses
through Call Comet (76), the actual eighth row, before requiring all eight. It
keeps real Tab scrolling, full-row visibility, purchase rejection, reopening and
all 26 librarian rows/one-shot Lace removal. No product catalog or focus behavior
changes to accommodate the stale target; the original five-row failure remains.

The actual f41dc57 Tutorial entry passes both input contexts, retained intro/Sirmin/run,
four native/phone lesson layouts and real movement acknowledgement, with errors[] and
graceful Chrome exit0 (`common-browser/f41dc57/tutorial-entry`). Reviewed screenshots
show the native Midnight card and both lesson variants. Its unchanged native ariaNotify
trace also establishes two required accessibility corrections. First, fa08
`TutorialOverlay.tsx:157–163` exposes the newline-flattened heading plus subheading in
its polite live region; the GPU painter currently announces narration only. Second,
the shared announcement state stores only the last string from a frame's list, so an
unchanged pair of match-loading messages alternates and repeats every frame (about
50 pairs in this bounded entry). Preserve each current message, compare whole frame
membership rather than one global last string, and allow real later returns after a
message leaves the frame. Publish each newly changed batch together so the single
fallback live region also retains every message instead of overwriting earlier ones.
HUD handed the bounded Tutorial announcement edit to acceptance; foundation handed
the disjoint canonical accessibility batching seam to acceptance. ceb3a17 now proves actual lesson
announcements and unchanged-batch silence through both native and fallback delivery.

Announcement identity also belongs to its actual Source owner, rather than its text.
Pinned fa08 `NativeWorldNotifications.tsx:15–18` keeps each source-keyed message in
the live region, and `GameChat.tsx:474–499` keeps distinct accepted sequence rows;
two legitimate receipts can therefore have identical text. Text-only frame membership
would incorrectly suppress the second owner. Each GPU announcement must carry its
stable status/DOM owner or accepted receipt/event ID, preserve producer order, and
compare that owner's current text with the previous foreground frame. Repeated
same-owner/text is quiet; changed text, a different event owner or a real later return
is announced. No ever-seen text set may suppress later messages. Source confidence is
high; shared native regressions and actual native/fallback browser proof remain the
acceptance boundary. Social owns seeding/restoration and accepted chat-session epochs.

The remaining Tutorial GPU proof must progress through the real run. Pinned fa08
`core-kernels/native-tutorial.ts:581–705,744–773` makes primary casting clear the first
lesson, Acid Rain begin the item wave, actual loot pickup admit Inventory, its close
begin the club/level lesson, Skills close begin archers, and the authored Health Potion
drink begin survival. `tutorial-modal-callouts.ts`/`TutorialOverlay.tsx` own the Inventory
and Skills callouts. The browser may observe accepted native snapshots and use the
unchanged Source navigator/camera to choose genuine input, as the existing Solomon
entrance journey does; it may not install those planning values or bypass a stage.
Entry/layout proof alone does not accept these later surfaces or the terminal return.
Pinned `core-server/boneyard-loot-store.ts:778–781` publishes the actual captured
sack item's name with its accepted local pickup event. Tutorial combat is enabled
for the potion lesson (`BoneyardScene.tsx:1150–1151`), so the genuine authored potion
drop can also prove the earned local loot lane and its GPU/speech presentation.
That remains qualified as a scripted Tutorial drop, not an ordinary random Gold/book drop.

Fixed GPU application ordering follows actual c14068b3 append/context ownership;
pinned fa08 supplies native inner behavior/styles. `source-ui-stack.md` in the task
records the accepted registry. Tutorial's current bodyz40 root appends prelude,
instructions, callouts and pointers in that order (`tutorial.rs:112–118`), so its one
canonical painter/getter preserves that whole subtree. Do not import fa08's overlay
z31000 or sibling modal-callout z31001 as new global application roots. Newly wired
notifications do have distinct owners: fa08 `BoneyardScene.tsx:1139–1152` mounts its
z45 notice inside the transformed/isolated native frame, below that frame's HUD10000;
Hub `MainMenuScene.tsx:2357` / `SkillBookFeedback.tsx:25–30` mounts a separate
`.skill-book-hub-messages-stage` bodyz45 containing the notice z45. Root's accepted
registry places Boneyard notices below HUD and Hub notices45 after Tutorial40/before
menu50. Source numbers are in `loot-message-presentation.css:1–8`,
`skill-book-feedback.css:9–17` and `hub.css:30–32`. Distinct actual notice IDs/getters
preserve those owners. Remove the copied obsolete `mod/world` sublayer parent after
the four-root authored-overlay cutover; the fixed Context registry owns composition.
Generic egui Order categories do not define this order.

Final startup review finds `BrowserInput::attach` before the awaited resident audio
and saved/account admission in `Game::load_resources`. Its document handlers can queue
Tab/Enter while the separate loader still owns the screen, then replay those inputs
against the first main Title/prompt. Source startup renders its main scene only after
required readiness. Extend the existing held resident-theme case with those real
trusted keys, retain normal native AudioContext/Title-music unlock from the existing
post-handoff real Tab gesture, and observe
the first actual main frames: fresh Title and its untouched Tutorial offer must remain.
Root owns the shared main admission gate; this regression adds no state setter or
proxy controls and preserves the existing held-resource/fallback ownership checks.
The bounded f41 reproduction does **not** show hidden main actions:
`startup-f41dc57-boot-admission-verified/startup-receipt.json` passes the held322/326,
trusted Tab/Enter, all first8 untouched Title/prompt frames, normal post-ready focus/
audio unlock, exact splash/fallback handoff and errors[]/Chrome88742 graceful0.
Its initial AudioContext is suspended during the held load and runs after the ordinary
post-handoff Tab; only solomondarktheme then plays. The source review still justifies
an explicit gate for the new chronological input transaction, but f41 is a passing
baseline, not its demonstrated failing implementation. Earlier harness-development
receipts remain separately qualified and unchanged.

`skill-book-24382f0-entered-notices/receipt.json` now qualifies the full imported-book
journey in both worlds, including the shipped stock Boneyard fixture. Both OKAY and
Escape release the independent modal/pause, and each actual accepted +1 reaches native
offset0/baseline67/clipTop50/scale1. Reviewed notification screenshots contain readable
Call Leviathan and Ring of Fire text; normal reload preserves consumed-book absence and
seeds both the modal and notification cursors without replay. The genuine choice book
still opens its offer. Both contexts settle errors[] and Chrome exits zero gracefully.
This explicitly covers Source-produced inventory imported through the real File chooser,
not a naturally earned rank-book drop.
Book layer recovery also retains actual nesting. c140 `player-panels.css` places
belt effects4, tooltip8, flybys9 and dragger10 inside the transformed stage;
`viewport.rs:129–132` reparents compact tooltips to the book root, before fixed
navigation10. Active flight/drag suppresses tooltip, but trailing afterimages can
coexist with a tooltip or new drag. Separate actual passive GPU painters preserve
those overlaps: wide tooltip is below afterimages, while a compact portaled tooltip
is above the stage and below navigation. All remain in body book30. The native
regression drains the real graphics layers in both layouts, independently of paint
registration order; Source afterimage lifetime and interaction admission stay intact.

The retained f41 first-controller-B failure at `ARCH_Q1_0` loses focus but leaves
the dialogue open. Its producer projects browser key codes only; controller Back
is a genuine egui Escape without a browser KeyboardEvent. c140
`PlayerPanels::update:909–910` explicitly routes the old navigation back result
into `self.back`. The GPU adapter now consumes that genuine Escape through the
same typed panel-key owner once, deduplicates physical Escape and retains repeat,
suspension, modal and rename admission. QA keeps the original first-B journey;
no second press or timing replacement establishes acceptance.

Create rejection retains native value-restoration semantics as well as its text.
c140 `menu/create.rs:216` restores the accepted input value after a rejected edit.
The trusted native-input oracle in `native-name-rejection-oracle.json` inserts an
unsupported hyphen at UTF-16 position1 after accepted `HelvidiusX`: restoration
moves the caret to10, and subsequent native Undo/Redo retain that accepted value.
The Rust editor now restores end selection and retires editing/composition history
on the same rejecting validator boundary. Readonly and ordinary UTF-16 capacity
admission keep their distinct existing contracts. A focused model regression pins
accepted text, caret and undo/redo together; no renderer or validation policy changes.

Native input and drawing share the browser's default document timeline. The ceb3a17
paid-Sack, Settings and native-session journeys independently reproduced an egui
history panic when a native dispatch used `performance.now()` immediately before a
requestAnimationFrame callback with an earlier animation timestamp (17.7676 versus
17.7644 seconds in the Sack receipt). Their original errors and browser cleanup are
retained under `native-editor-ceb3a17`, `menu-runtime-ceb3a17/settings` and
`native-session-gpu-ceb3a17`. The [Web Animations timing model](https://www.w3.org/TR/web-animations-1/#document-timelines)
keeps the active document timeline monotonic, and its default timeline equals the rAF
callback timestamp. Native UI dispatch therefore reads `document.timeline.currentTime`;
an inactive document has no UI time to commit. Game simulation retains its existing
rAF clock. This repairs the shared clock source without clamping time or hiding failures.
The composed red journeys and held-rAF clipboard proof must pass on the rebuilt artifact.

The common ceb3a17 qualification index is
`common-browser/ceb3a17/qualification.json` in the task. It retains four clean reports:
menu-input10 groups (including original first-B), native Tutorial entry, forced-fallback
Tutorial entry and final-opacity responsive. Every owned Chrome exits zero gracefully,
Playwright disconnects and strict unexpected errors are empty. Native/fallback Tutorial
reports each include keyboard and real touch, all four Source lesson layouts, actual
movement acknowledgement and exact heading/subheading/narration announcements.

The first fallback report remains red and is qualified as an observer defect: its callback
read the final element textContent for each earlier mutation. Actual ChildList added text
retains `Loading Boneyard…` while callback currentText has already become `Play, button`.
The corrected read-only observer preserves both, and the unchanged single-announcement
assertion passes on the same immutable artifact. This did not justify a graphics repair
or establish hardware screen-reader audio. Native delivery is actual ariaNotify; fallback
delivery is the native live-region mutation stream.

Responsive now waits the actual Dye presentation `{opacity:1,phase:"target"}` before its
reviewed screenshot and retains the full18-swatch/mixing/touch-scroll journey. Toolbar
review against c140 `native_button` and `player-panels.css:89–93` finds the same equal
plate widths from zero-intrinsic-width bitmap containers. All glyphs paint after sibling
chrome, matching Source label z1; the final Backpack K lies over a gold ornament. No
additional ordering bug or speculative font/button-width change is claimed. These
individual passes do not accept the whole artifact: the separately reproduced shared
clock failure still requires the next compiled candidate, as does its held-startup-input
case. All common browsers are closed for that build.

Current57626c4 startup guard passes in
`common-browser/57626c4/startup-held-input-diagnostic/startup-receipt.json`: actual
322/326 completions retain the held theme's exact pending path, trusted Tab/Enter
remain under the loader, and the first eight main frames keep fresh Title/prompt with
no accepted selection. Exact loader art, fallback ownership/NO announcement, audio
activation and stopped loader draws also pass. Chrome31358 exits0 gracefully with no
unexpected errors. The earlier same-artifact exit2 report stays unqualified and intact;
a capacity-restored rerun does not establish its exact underlying process cause.
Menus' `menu-runtime-57626c4/settings-all18/receipt.json` independently passes seven
groups, all18 real binding rows, strict errors[] and graceful Chrome38791 exit0.

Current immutable3660 core proof on M5 is retained under
`common-browser/3660ab8/m5/{smoke,hud-input}` in the M2 task, with originals under
M5 `build-transfer/browser-proof/common/3660ab8/`. Required smoke passes all14 checks:
actual Helvidius/Ether/Arcane, settled Office, genuine two-axis movement and ambient
actors, Rust GPU Title/HUD, and immediate Play-click→Escape / Inventory-click→Escape
without inserted waits. HUD passes all10 checks, including trusted touchcancel, real
tab-blur release and neutral accepted movement after cancellation. Its64×16 label
target continues1466→1471→1515→1565 draws across neutral/idle/active/cancel, with
174 created textures and2 framebuffers unchanged. Reviewed full current-art screenshots,
strict errors[] and graceful Chrome85356/83196 exit0 accompany both accepted reports.
These bounded passes do not accept unexecuted hosted/Tutorial or other family variants.


Other current3660 reports remain independently qualified.
`menu-runtime-3660ab8/menus/receipt.json` passes12 groups with reviewed full Title,
settled five-element Create, selected Ether/three-discipline stones and effects, Pause
and Settings screenshots; Chrome89934 exits0. `title-prompts/receipt.json` passes
four real saved-wizard/decline/compact confirmation/replacement groups; Chrome92498
exits0. Both strict error lists are empty. `controller-wrap/receipt.json` deliberately
retains a separate red: actual Up from Play stays Play, while c140 requires Hall.
That required navigation repair is owned by menus; these passes do not hide it.

`native-editor-3660ab8/receipt.json` passes nine groups/errors[]/Chrome9760 exit0.
Real held-rAF Tab/ShiftTab and OS Copy/Paste leave the complete public GPU snapshot
and gamepad-read count1917 unchanged; actual ArrowRight/bidi weak caret, disjoint
selection, Arabic/ZWJ Undo, browser IME and UTF16 Enter-save pass. IME qualification
remains Chromium Input-domain emulation, without a hardware candidate-window claim.
The later Tutorial driver also awaits the canonical read-only route-helper migration
from legacy save reconstruction to the actual accepted world geometry; its genuine
pause/checkpoint persistence assertion remains independent of path planning.


The first later-Tutorial native run on3660 is retained as
`common-browser/3660ab8/m5/tutorial-native-r1/receipt.json`. It admits real Sirmin
Ether/Arcane with a genuine Welcome and shipped Tutorial SHA, but stops on a driver
assumption that Escape during Movement opens Pause. Pinned fa08
`BoneyardScene.tsx:308–314` and `native-tutorial.ts::nativeTutorialHudAccess` make
Pause unavailable until combat access (stage14 or later); current `Game::can_pause`
retains that rule. The revised driver checks that this real Escape leaves the world
uncovered and unpaused, then independently observes the actual durable native
checkpoint. It does not add a pause bypass or alter product timing. Original errors[]
and graceful Chrome22848/native22826 exits0 stay attached to the failed attempt.


Fresh native Tutorial r2 retains the actual schema2 checkpoint (tick258, matching
run/player/Source SHA) and correct early Escape sealing, then stops in the shared
route driver. Its2309 accepted samples oscillate between `solomon` and
`gate-crossing`: the missing arena transition defaulted to north-entry direction+1,
so near y1670 the helper steered south to1672.5. Pinned fa08
`stock-tutorial-boneyard.ts` places the immutable spawn at y2070.0703125, south of
the actual gate midpoint1612.5; `nativeTutorialForcedVelocity` likewise moves north.
The helper must derive entry side from this unchanged spawn/gate geometry when no
explicit arena transition exists, while retaining the Source transition side when
present. This corrects only ordinary input selection. The runtime accepted real W
movement and opened the gate; errors[] and graceful Chrome39258/native39250 exits0
remain in `common-browser/3660ab8/m5/tutorial-native-r2/`. No product defect is claimed.


Tutorial native r3 proves real gate crossing, Solomon's sequence and the ten-enemy
Magic Missile wave. It then retains a driver-only wrong-binding failure at Acid Rain: the
actual readable lesson says `Right Mouse`, while the script pressed Space. Pinned
fa08 `game-settings.ts:65–84` binds belt1 to Mouse2 and belt4 to Digit3;
`TutorialOverlay.tsx:94–95` uses those same slots for secondary/potion hints. The
driver now reads those unchanged Source defaults, compares their displayed labels
with the real lesson, and holds the corresponding real input only until the accepted
stage acknowledgement. It likewise replaces the ungrounded potion Q with the actual
source binding3. Errors[] and graceful Chrome50431/native50425 exits0 remain in
`common-browser/3660ab8/m5/tutorial-native-r3/`; later Tutorial UI is still unpassed.


Tutorial native r4 passes genuine Acid Rain, Inventory and Skills with exact Source
callouts/native pause and movement ownership, then the actual primary-selector
acknowledgement. It captures fully faded native Game Over and automatic College
return, but the complete journey remains red. Pinned fa08 native-tutorial stage14
starts the potion carrier wave at stage16; stage16 waits for a real ground Sack. The
driver had stopped attacking there and died before the drop. It now keeps ordinary
primary combat active through AwaitArcherWave/WaitForPotionDrop, without changing
world state, enemy health or progression. The original failed report is retained at
`common-browser/3660ab8/m5/tutorial-native-r4/`.

That report independently retains a fatal combat.mp3
`net::ERR_CONTENT_LENGTH_MISMATCH` and native browser console error. The strict
resident-media observer does not classify this as suspension. The channel had one
load, ready4 and a trusted suspend at88.7s; AssetServer's30s socket write timeout is
a transport hypothesis, not a proved cause. Root owns that source investigation.
Chrome63643 and native63633 exit0; no complete Tutorial/earned-potion claim is made.
