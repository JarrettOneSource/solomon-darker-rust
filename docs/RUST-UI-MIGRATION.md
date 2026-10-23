# Rust GPU UI migration

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

The repaired exact-save edge and browser close owner are now verified together by the
complete `common-browser/88c3dfe/boneyard-stdio-fixed` rerun: two independently generated
named worlds, native source/geometry, textured scenery, inward movement, Pause freeze,
Leave/Continue, exact subsequent paused checkpoint and the same wizard/run/seed all pass.
Both current-art world/HUD screenshots were reviewed. The process gate also passes:
Playwright disconnects, owned Chrome exits zero gracefully, cleanup errors are empty and
the driver exits zero without forced termination. Earlier 58df save failure and both 88
teardown failures remain unchanged as the retained red evidence.

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
| Process startup; cold, warm, held image/font/wasm; failed required asset; no partial Title or HUD | foundation + root; acceptance tests | Startup evidence above; `startup.cjs`; loader geometry, monotonic work, immediate readiness transition, screenshots | All seven cases pass on 88c3dfe across retained receipts; exact held counts/path, splash, no partial menus, native/fallback announcements and failure ownership proved |
| GPU runtime; stock bundle crops/rotation/registered origins, glyph masks, browser fonts, dynamic images, blend/clips | foundation | `gpu_ui.rs`, shared `graphics/src/lib.rs`, stock manifest; read-only `gpu_ui_state`, GPU meshes/textures and final screenshots | Core probe 15/15; joined source audit 3/3; composed game/art pending complete composed browser proof |
| Title/Play; clouds, moon, graves, Solomon animation, logo/art, Continue policy, account/desktop links | menus | `menu/title.rs`, pinned Main Menu; `smoke.cjs`, `menus.cjs`, `menu-input.cjs` | Ordinary Title/Tutorial prompt screenshot reviewed; complete Title variants pending |
| Tutorial prompt and saved-wizard retirement confirmation; Yes/No, reject/cancel and modal focus | menus | `menu/prompt.rs`; `title-prompts.cjs`, shared new-game journey | 88c3dfe prompts-observed clean: real saved wizard, Escape decline preserves exact state, narrow viewport confirmation and explicit replacement; Tutorial Yes/focus route remains in common input coverage |
| College introduction/admission then sole Create; native text validation/rejected draft, blank/random name, five elements/three disciplines, exact effects/clocks/randomization, locked Back and admission retry | menus + HUD | `menu/create.rs`, intro/dialogue; `menu-journey.cjs`, `menus.cjs`, `menu-input.cjs`, `solomon-entrance.cjs` | 88c3dfe required named Helvidius/Ether/Arcane smoke and default/random/blank variants clean; 24382f0 full audio activation/priming/scene/mixer journey clean; complete element/discipline/admission variants pending |
| Pause opening/closing, Resume/Settings/Cheats/Leave, shared pause behavior, held-input neutral handoff | menus + social | `menu.rs`, `shared_gameplay.rs`; `menus.cjs`, `menu-input.cjs`, `hosted-session.cjs` | 88c3dfe rapid inputs and complete two-seed Boneyard Pause/exact-save/Leave/Continue pass cleanly; common input neutral handoff passes; complete shared variants remain pending |
| Settings root/cloud/performance; sliders/toggles, permissions, saved preferences, nested account policies, mobile sharing | menus | `menu/settings/**`; `settings-parity.cjs`, `menus.cjs`, `fullscreen.cjs` | 88c3dfe Settings five-group driver passes cleanly, including pointer slider and real reload; account/mobile permission variants remain owned by their family routes |
| Control bindings; every action, duplicate swaps, Escape, modified-key rejection, allowed mouse bindings, reverse hotbar | menus + root input | `menu/settings.rs`, canonical `GameControls`; `menus.cjs`, `gameplay-input.cjs`, `hud-input.cjs` | 58df78d common gameplay-input fully passes with exact reads 8/8, raw native suspensions preserved; binding witnesses retained, complete current HUD/controller variants pending |
| Save Transfer; multiple native/JSON/CFG/SAV/ZIP imports, inspected warnings, replace/cancel/busy/error, real export ZIP/download | menus | `menu/settings/transfer.rs`; `save-transfer.cjs`, `save-tools.cjs` | GPU source integrated; native file/confirmation/export journey pending composed browser proof |
| Hall Local/Global × four boards; top100 and current row, portrait/orb, expand/scroll/pan, ranks/perks, loading/retry/stale result | menus | `menu/hall.rs`; `hall.cjs`, `hosted-score.cjs` | 58df78d Hall driver passes/errors empty; native/compact screenshots reviewed; queued gradient/border correction and hosted variants pending |
| Join directory; normalized code, refresh, request/admission/cancel, private/mod/cheat/location policy, list scroll | menus | `menu/join.rs`; `join-party.cjs`, `hosted-social.cjs` | GPU source integrated; actual host/directory admission pending composed browser proof |
| Routing consent; vanilla/private/mod/cheat, added/missing/changed save mods, downloads/busy/error/retry and close invalidation | menus + root | `menu/routing.rs`, host routing; `cheat-consent.cjs`, `cloud-subscription.cjs`, `hosted-session.cjs` | GPU source integrated; consent/retry plus native authority proof pending composed browser proof |
| Dark Cloud frame/tabs; Mods/Subscribed/Parties/Mobile UI/Boneyards, horizontal reveal, source-supported search/sort tools, empty/loading/error, account availability | menus; root Shelf | `menu/cloud/**`; `dark-cloud.cjs`, `cloud-subscription.cjs`, `authoring-shelf.cjs` | GPU source integrated; actual five-tab and portrait Cloud journey pending composed browser proof |
| Cloud mods/detail; thumbnail lifecycle, subscription mutation/progress, screenshot contain/swipe/wrap, metadata/releases, comments/UTF16 limit/delete, preserved focus/scroll | menus | `menu/cloud/detail.rs`, real Website storage; `dark-cloud.cjs`, `cloud-subscription.cjs` | GPU source integrated; actual remote assets/subscriptions/comments pending composed browser proof |
| Boneyard Shelf; local/cloud content, New/Import/Edit/Test/Delete and Keep, Test/Return/editor handoff and disposal | root | `game/hosted/boneyards.rs`; `authoring-shelf.cjs`, `authored-content.cjs`; export belongs to the external editor | 58df78d New/editor/cloud save/JSON/native imports pass; pointer Keep focus red; queued repair and Test/Return/Delete variants pending |
| HUD resources/XP/concentrations, native/mod belt quantity/cooldown, three banks, portraits/map, tutorial gates, custom mobile positions | HUD | `hud/**`, `hotbar.rs`; `hud-input.cjs`, `hud-selector.cjs`, `belt-pull-off-check.cjs` | 58df78d desktop selector passes; 844 × 390 central/bottom HUD clipped, queued transform correction; touch/cancel and remaining variants pending |
| Inventory and native rank-book confirmation; result/OKAY/Escape, retained pause/audio, column-first cells, equipment/protected clothing/third ring, nested Sack pages/back/rename, animation lock, drag/drop/restoration flybys, stats/set/wizard preview | HUD | `player_panels/{inventory*,skill_book_feedback}`, `sack_*`; `player-panels.cjs`, `skill-book-result.cjs`, `responsive.cjs`, `save-resume.cjs` | 58df panel/equip/Unforge/Skills and Sack/font-quads retained; 88c3dfe ordinary Unicode Sack reload/Continue now passes behavior without trap, clean replacement-observed and book/touch gates pending |
| Skills/quickbar; learned/ranked/weld pages, stock hover, every bank binding, skill/item drag and belt pull-off | HUD | `player_panels/skills.rs`, `skill_selector.rs`; `player-panels.cjs`, `hud-selector.cjs`, `mod-skills.cjs` | GPU source integrated; real learned/drag/bank/pull-off journey pending composed browser proof |
| College services; all shops/perks/prices, insufficient gold, storage, paid Dowse/ref/roll/Done, Unforge confirmation/results, dye admission/swatches/trim/cloth | HUD | `player_panels/{services,crafting,sack_rename}.rs`; `hub-services.cjs`, `player-panels.cjs`, `responsive.cjs` | 58df78d real purchase/storage/27 Hagatha offers/insufficient Dowse/Unforge pass before fatal media errors; paid Dowse/dye variants pending |
| NPC chat/books/boasts/authored choices; history, progressive text/audio/acceleration, selector list drag/wheel/controller, notebox/error | HUD | `player_panels/{dialogue,notebox}.rs`; `hub-dialogue-rooms.mjs`, `menu-journey.cjs` | 58df78d College dialogue plus five boasts and keyboard scroll pass before media errors; complete books/authored/selector/error variants pending |
| Level-up offer sequence; reveal/clocks, cards/insight/weld, responsive detail, choice/reroll/save and rejected-action recovery | HUD | `skill_picker.rs`; `hud-selector.cjs`, actual earned gameplay | GPU source integrated; actual earned offer/card/selection journey pending composed browser proof |
| Tutorial callouts/arrows; canonical text/narration and live GPU widget anchors; intro title/fade; Game Over riff/prompt/admission | HUD | `tutorial/**`, `college_intro.rs`, `game_over.rs`; `tutorial-text-bounds.cjs`, `solomon-entrance.cjs`, `boneyard.cjs` | GPU source integrated; actual Tutorial anchors/text and terminal run journey pending composed browser proof |
| World notifications; local loot/Gold coalescing, Hub/Boneyard rank-book notices, Overloaded Mana and CHEAT DEATH; per-run cursors, native clip/rise/fade/shadow, life/Tutorial/Game Over gates and screen-reader text | acceptance; root wiring | Recovered notification contract above; `sim/boneyard/loot/messages.rs`, `web/loot/messages.rs`; current-source fixture and genuine earned browser events | Current-source clock and GPU controller implemented; 13 focused tests pass; integrated browser event/render proof pending |
| Social Party; collapsed/coarse/expanded, Members/Settings/Mods, roster tags/scroll, privacy, copy/regenerate, Kick/Leave, request/invite Accept/Deny | social | `social/party/**`; `hosted-social.cjs`, `join-party.cjs` | 58df78d private admission/decline/held-save witnesses pass; Party opening duplicate ID red, queued correction plus media gate and remaining variants pending |
| Player cards/private College; Hub actor hit, Message/invite, resolve/decline/failure and owned roster | social | `social/{card,hub_players,hosted}.rs`; `hosted-social.cjs`, `hosted-session.cjs` | 58df78d two private Colleges, Global author card, decline/consent and source-membership-before-save witnesses pass; complete journey/media gate pending |
| Chat; Boneyard/Global/Party/Whisper, drafts/unread/fade/channel cycle, IME, author card and disconnect/reset | social | `social/chat.rs`; `hosted-social.cjs` | 58df78d actual Global delivery/author card pass; complete channels/IME/reset and fatal media gate pending |
| Shared Pause/resume waiting; other-owner activity, dim/reveal, countdown/progress/rejoin; spectator rail/target/respawn | social | `shared_gameplay.rs`, `spectator.rs`; `hosted-session.cjs`, `spectator.cjs` | GPU source integrated; actual shared pause/respawn/target lifecycle pending composed browser proof |
| Cheats/Console/developer observer; permissions, all resource/skill/world/dev controls, Lua submit/history/results/error/clear and bot targets | social | `cheats/**`, `observer.rs`; `authored-content.cjs`, `learned-bot.cjs`, `cheat-consent.cjs` | GPU source integrated; actual authorized Console/observer/bot journey pending composed browser proof |
| Mobile editor; all six anchors/elements, drag/resize/rotate/snap, zoom/pan/pinch, multi-touch/cancel, undo/redo/reset/pages/dock/presets/save | social | `mobile_editor/**`, `sim/mobile_ui/**`; `mobile-editor.cjs`, `responsive.cjs` | GPU source integrated; actual parent-layer touch/edit/save journey pending composed browser proof |
| Authored mod surfaces; Cast/Shop/Reforge/Portal, skills/subskills/offers/quickbar, minimap actions, mounted HUD text/image/bar/button, party attributes | social | `mod_panels.rs`, `mod_skills.rs`, `mod_skill_quickbar.rs`, `mod_minimap.rs`, `mod_world_overlays/**`; `authored-content.cjs`, `mod-skills.cjs` | GPU source integrated; actual admitted authored widgets/assets/retirement pending composed browser proof |
| Match loading; every recovered stage, progress/cancel/failure, host admission/world materialization | root | `match_loading.rs`; `boneyard.cjs`, `hosted-session.cjs`, `deployment-restart.cjs` | 88c3dfe complete two-seed ordinary Map/native-world/Leave/Continue/exact-save journey passes, including clean owned browser retirement; full stage/cancel/failure/host variants remain pending |
| Fullscreen/install guidance; request/exit/change/error, unsupported/standalone/per-screen placement | social | `fullscreen.rs`; `fullscreen.cjs`, real browser capability | GPU source integrated; real fullscreen/unsupported/standalone activation pending composed browser proof |
| Desktop notice/conflict/close lifecycle; offline/peer/update/Keep Playing/failure/save | root | `desktop.rs`, `desktop/`; `smoke-desktop-runtime.mjs`, desktop failure tests | GPU source integrated; actual update/close failure plus packaged journey pending composed browser proof |
| Deployment update/checkpoint/recovery; same revision identity and error ownership | root | `deployment.rs`, actual host lifecycle; `deployment-restart.cjs` | GPU source integrated; native recovery regressions retained; actual update journey pending composed browser proof |
| Cross-cutting inputs; pointer/keyboard/touch/controller, focus/text/scroll/modal sealing, real screen-reader focus/value announcements, blur/cancel, resize/portrait/wide/fullscreen | all; acceptance common tests | `gpu-ui.cjs`, `menus.cjs`, `menu-input.cjs`, `responsive.cjs`; final current-art screenshots | 88c3dfe mandatory named/rapid smoke clean; 58df common input fully clean/exact8/8 with strict raw media evidence; touch/resize/accessibility and focused family reruns pending |
| No DOM/CSS facade, iframe visual replacement, test-only state fabrication, unfinished required TODO or dropped feature | root + acceptance | Source scan, DOM paint audit, real GPU counters/read-only widget geometry, all rows above | Joined source producer contract 3/3; complete runtime surface/input proof remains pending |

## Test ownership and stopping checks

Acceptance owns `gpu-ui.cjs`, `gpu-ui-contract.test.cjs`, `menu-journey.cjs`, `smoke.cjs`,
`startup.cjs`, `menus.cjs`, `menu-input.cjs`, `responsive.cjs`, `audio.cjs`, `save-resume.cjs`,
`saved-game.cjs`, `college-clock.cjs`, `rooms.mjs`, `hud-input.cjs`, `tutorial-text-bounds.cjs`,
`boneyard.cjs`, `gameplay-input.cjs` and `electric-burn.cjs`.
Menus owns detailed Hall/Settings/Cloud/Join/prompt
journeys. HUD and social own their family journeys. Root owns final joined execution and
packaged/hosted/authoring integration. Exact stable IDs and snapshot schema are in the task's
`gpu-ui-contract.md`; that is a development handoff, not an accepted product artifact.

The common scripts are migrated to read-only GPU geometry and actual browser input. Syntax
checks pass for the migrated common files. The architectural contract was run before the cutover:
two tests fail for actual existing visual DOM/CSS producers and one passes; its TAP output is
retained as `gpu-ui-contract-baseline.tap`. Current composed browser acceptance is pending,
so no matrix surface is marked accepted from syntax/source checks alone. The independent
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
