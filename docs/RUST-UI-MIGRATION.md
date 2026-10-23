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
earned-event browser visibility and live hosted event delivery. All members remain
`recovered-pending-port` until the current fixture/regression, renderer/controller checks
and genuine browser journey establish their supported final disposition.

## Acceptance matrix

`Pending` means current composed behavior has not been accepted. Owner names match the
implementation lanes; scripts are under `tools/browser-smoke/`. The composed `ec344fe`
candidate includes the GPU surface families and notification wiring. Its first actual
browser run is blocked by main-WASM `memory access out of bounds` during startup
(`runtime_ui::loader_active` in the retained trace); menus, HUD and foundation independently
reproduced it. No family route downstream of Title has passed this candidate.

Before that trap, foundation's `loader-probe/receipt.json` proves a deliberately held main
WASM still paints the real tiny Rust GPU splash: exact source logo differs by at most one
channel byte in one of 88,076 pixels, navy background and real GPU submissions, wide/mobile
resize, and one fallback announcer across two loader/main owners. Only loader progress was
announced while it owned the foreground. The trap prevents Title/focus handoff acceptance.
The final joined visual-producer source contract passes 3/3 after root's old factory removal;
this source result does not establish runtime visual fidelity.

| Surface and required variants | Owner | Implementation/reference and acceptance route | Current blocker/evidence |
| --- | --- | --- | --- |
| Process startup; cold, warm, held image/font/wasm; failed required asset; no partial Title or HUD | foundation + root; acceptance tests | Startup evidence above; `startup.cjs`; loader geometry, monotonic work, immediate readiness transition, screenshots | Tiny GPU loader pixels/resize/overlap proved; ordinary/cold/warm/held-asset handoff blocked by main startup trap |
| GPU runtime; stock bundle crops/rotation/registered origins, glyph masks, browser fonts, dynamic images, blend/clips | foundation | `gpu_ui.rs`, shared `graphics/src/lib.rs`, stock manifest; read-only `gpu_ui_state`, GPU meshes/textures and final screenshots | Core probe 15/15; joined source audit 3/3; composed game/art blocked by main startup trap |
| Title/Play; clouds, moon, graves, Solomon animation, logo/art, Continue policy, account/desktop links | menus | `menu/title.rs`, pinned Main Menu; `smoke.cjs`, `menus.cjs`, `menu-input.cjs` | GPU source integrated; actual Title art/admission blocked by main startup trap |
| Tutorial prompt and saved-wizard retirement confirmation; Yes/No, reject/cancel and modal focus | menus | `menu/prompt.rs`; `title-prompts.cjs`, shared new-game journey | GPU source integrated; real prompt cancel/focus journey blocked by main startup trap |
| College introduction/admission then sole Create; native text validation/rejected draft, blank/random name, five elements/three disciplines, exact effects/clocks/randomization, locked Back and admission retry | menus + HUD | `menu/create.rs`, intro/dialogue; `menu-journey.cjs`, `menus.cjs`, `menu-input.cjs`, `solomon-entrance.cjs` | GPU source integrated; ordinary College/Create admission blocked by main startup trap |
| Pause opening/closing, Resume/Settings/Cheats/Leave, shared pause behavior, held-input neutral handoff | menus + social | `menu.rs`, `shared_gameplay.rs`; `menus.cjs`, `menu-input.cjs`, `hosted-session.cjs` | GPU source integrated; actual pause/neutral handoff blocked by main startup trap |
| Settings root/cloud/performance; sliders/toggles, permissions, saved preferences, nested account policies, mobile sharing | menus | `menu/settings/**`; `settings-parity.cjs`, `menus.cjs`, `fullscreen.cjs` | GPU source integrated; real slider/key and saved preference journey pending startup fix |
| Control bindings; every action, duplicate swaps, Escape, modified-key rejection, allowed mouse bindings, reverse hotbar | menus + root input | `menu/settings.rs`, canonical `GameControls`; `menus.cjs`, `gameplay-input.cjs`, `hud-input.cjs` | GPU source integrated; real binding/capture/belt input journey pending startup fix |
| Save Transfer; multiple native/JSON/CFG/SAV/ZIP imports, inspected warnings, replace/cancel/busy/error, real export ZIP/download | menus | `menu/settings/transfer.rs`; `save-transfer.cjs`, `save-tools.cjs` | GPU source integrated; native file/confirmation/export journey pending startup fix |
| Hall Local/Global × four boards; top100 and current row, portrait/orb, expand/scroll/pan, ranks/perks, loading/retry/stale result | menus | `menu/hall.rs`; `hall.cjs`, `hosted-score.cjs` | GPU source integrated; all board/scroll/retry variants pending startup fix |
| Join directory; normalized code, refresh, request/admission/cancel, private/mod/cheat/location policy, list scroll | menus | `menu/join.rs`; `join-party.cjs`, `hosted-social.cjs` | GPU source integrated; actual host/directory admission pending startup fix |
| Routing consent; vanilla/private/mod/cheat, added/missing/changed save mods, downloads/busy/error/retry and close invalidation | menus + root | `menu/routing.rs`, host routing; `cheat-consent.cjs`, `cloud-subscription.cjs`, `hosted-session.cjs` | GPU source integrated; consent/retry plus native authority proof pending startup fix |
| Dark Cloud frame/tabs; Mods/Subscribed/Parties/Mobile UI/Boneyards, horizontal reveal, source-supported search/sort tools, empty/loading/error, account availability | menus; root Shelf | `menu/cloud/**`; `dark-cloud.cjs`, `cloud-subscription.cjs`, `authoring-shelf.cjs` | GPU source integrated; actual five-tab and portrait Cloud journey pending startup fix |
| Cloud mods/detail; thumbnail lifecycle, subscription mutation/progress, screenshot contain/swipe/wrap, metadata/releases, comments/UTF16 limit/delete, preserved focus/scroll | menus | `menu/cloud/detail.rs`, real Website storage; `dark-cloud.cjs`, `cloud-subscription.cjs` | GPU source integrated; actual remote assets/subscriptions/comments pending startup fix |
| Boneyard Shelf; local/cloud content, New/Import/Edit/Test/Delete and Keep, Test/Return/editor handoff and disposal | root | `game/hosted/boneyards.rs`; `authoring-shelf.cjs`, `authored-content.cjs`; export belongs to the external editor | Visual iframe retired in source; actual New/Import/Edit/Test/Delete/Keep journey pending startup fix |
| HUD resources/XP/concentrations, native/mod belt quantity/cooldown, three banks, portraits/map, tutorial gates, custom mobile positions | HUD | `hud/**`, `hotbar.rs`; `hud-input.cjs`, `hud-selector.cjs`, `belt-pull-off-check.cjs` | GPU source integrated; HUD geometry/input plus actual cancellation journey pending startup fix |
| Inventory and native rank-book confirmation; result/OKAY/Escape, retained pause/audio, column-first cells, equipment/protected clothing/third ring, nested Sack pages/back/rename, animation lock, drag/drop/restoration flybys, stats/set/wizard preview | HUD | `player_panels/{inventory*,skill_book_feedback}`, `sack_*`; `player-panels.cjs`, `skill-book-result.cjs`, `responsive.cjs`, `save-resume.cjs` | GPU source integrated; actual pointer/touch/Sack/save journey pending startup fix |

| Skills/quickbar; learned/ranked/weld pages, stock hover, every bank binding, skill/item drag and belt pull-off | HUD | `player_panels/skills.rs`, `skill_selector.rs`; `player-panels.cjs`, `hud-selector.cjs`, `mod-skills.cjs` | GPU source integrated; real learned/drag/bank/pull-off journey pending startup fix |
| College services; all shops/perks/prices, insufficient gold, storage, paid Dowse/ref/roll/Done, Unforge confirmation/results, dye admission/swatches/trim/cloth | HUD | `player_panels/{services,crafting,sack_rename}.rs`; `hub-services.cjs`, `player-panels.cjs`, `responsive.cjs` | GPU source integrated; actual paid/rejected shop, Dowse, dye and Unforge journey pending startup fix |
| NPC chat/books/boasts/authored choices; history, progressive text/audio/acceleration, selector list drag/wheel/controller, notebox/error | HUD | `player_panels/{dialogue,notebox}.rs`; `hub-dialogue-rooms.mjs`, `menu-journey.cjs` | GPU source integrated; actual dialogue/selector/notebox focus and text journey pending startup fix |
| Level-up offer sequence; reveal/clocks, cards/insight/weld, responsive detail, choice/reroll/save and rejected-action recovery | HUD | `skill_picker.rs`; `hud-selector.cjs`, actual earned gameplay | GPU source integrated; actual earned offer/card/selection journey pending startup fix |
| Tutorial callouts/arrows; canonical text/narration and live GPU widget anchors; intro title/fade; Game Over riff/prompt/admission | HUD | `tutorial/**`, `college_intro.rs`, `game_over.rs`; `tutorial-text-bounds.cjs`, `solomon-entrance.cjs`, `boneyard.cjs` | GPU source integrated; actual Tutorial anchors/text and terminal run journey pending startup fix |
| World notifications; local loot/Gold coalescing, Hub/Boneyard rank-book notices, Overloaded Mana and CHEAT DEATH; per-run cursors, native clip/rise/fade/shadow, life/Tutorial/Game Over gates and screen-reader text | acceptance; root wiring | Recovered notification contract above; `sim/boneyard/loot/messages.rs`, `web/loot/messages.rs`; current-source fixture and genuine earned browser events | Current-source clock and GPU controller implemented; 13 focused tests pass; integrated browser event/render proof pending |
| Social Party; collapsed/coarse/expanded, Members/Settings/Mods, roster tags/scroll, privacy, copy/regenerate, Kick/Leave, request/invite Accept/Deny | social | `social/party/**`; `hosted-social.cjs`, `join-party.cjs` | GPU source integrated; actual two-human roster/privacy/request journey pending startup fix |
| Player cards/private College; Hub actor hit, Message/invite, resolve/decline/failure and owned roster | social | `social/{card,hub_players,hosted}.rs`; `hosted-social.cjs`, `hosted-session.cjs` | GPU source integrated; actual actor card/private admission/teardown pending startup fix |
| Chat; Boneyard/Global/Party/Whisper, drafts/unread/fade/channel cycle, IME, author card and disconnect/reset | social | `social/chat.rs`; `hosted-social.cjs` | GPU source integrated; actual IME/channel/whisper delivery pending startup fix |
| Shared Pause/resume waiting; other-owner activity, dim/reveal, countdown/progress/rejoin; spectator rail/target/respawn | social | `shared_gameplay.rs`, `spectator.rs`; `hosted-session.cjs`, `spectator.cjs` | GPU source integrated; actual shared pause/respawn/target lifecycle pending startup fix |
| Cheats/Console/developer observer; permissions, all resource/skill/world/dev controls, Lua submit/history/results/error/clear and bot targets | social | `cheats/**`, `observer.rs`; `authored-content.cjs`, `learned-bot.cjs`, `cheat-consent.cjs` | GPU source integrated; actual authorized Console/observer/bot journey pending startup fix |
| Mobile editor; all six anchors/elements, drag/resize/rotate/snap, zoom/pan/pinch, multi-touch/cancel, undo/redo/reset/pages/dock/presets/save | social | `mobile_editor/**`, `sim/mobile_ui/**`; `mobile-editor.cjs`, `responsive.cjs` | GPU source integrated; actual parent-layer touch/edit/save journey pending startup fix |
| Authored mod surfaces; Cast/Shop/Reforge/Portal, skills/subskills/offers/quickbar, minimap actions, mounted HUD text/image/bar/button, party attributes | social | `mod_panels.rs`, `mod_skills.rs`, `mod_skill_quickbar.rs`, `mod_minimap.rs`, `mod_world_overlays/**`; `authored-content.cjs`, `mod-skills.cjs` | GPU source integrated; actual admitted authored widgets/assets/retirement pending startup fix |
| Match loading; every recovered stage, progress/cancel/failure, host admission/world materialization | root | `match_loading.rs`; `hosted-session.cjs`, `deployment-restart.cjs` | GPU source integrated; actual host/world loading stages and failures pending startup fix |
| Fullscreen/install guidance; request/exit/change/error, unsupported/standalone/per-screen placement | social | `fullscreen.rs`; `fullscreen.cjs`, real browser capability | GPU source integrated; real fullscreen/unsupported/standalone activation pending startup fix |
| Desktop notice/conflict/close lifecycle; offline/peer/update/Keep Playing/failure/save | root | `desktop.rs`, `desktop/`; `smoke-desktop-runtime.mjs`, desktop failure tests | GPU source integrated; actual update/close failure plus packaged journey pending startup fix |
| Deployment update/checkpoint/recovery; same revision identity and error ownership | root | `deployment.rs`, actual host lifecycle; `deployment-restart.cjs` | GPU source integrated; native recovery regressions retained; actual update journey pending startup fix |
| Cross-cutting inputs; pointer/keyboard/touch/controller, focus/text/scroll/modal sealing, real screen-reader focus/value announcements, blur/cancel, resize/portrait/wide/fullscreen | all; acceptance common tests | `gpu-ui.cjs`, `menus.cjs`, `menu-input.cjs`, `responsive.cjs`; final current-art screenshots | Real input drivers migrated; composed runtime blocked by main startup trap |
| No DOM/CSS facade, iframe visual replacement, test-only state fabrication, unfinished required TODO or dropped feature | root + acceptance | Source scan, DOM paint audit, real GPU counters/read-only widget geometry, all rows above | Joined source producer contract 3/3; complete runtime surface/input proof pending startup fix |

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
