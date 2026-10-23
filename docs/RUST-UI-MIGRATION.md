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

## Acceptance matrix

`Pending` means implementation and/or current composed browser evidence has not been accepted.
Owner names match the parallel implementation lanes. Scripts are under `tools/browser-smoke/`.

| Surface and required variants | Owner | Implementation/reference and acceptance route | Current blocker/evidence |
| --- | --- | --- | --- |
| Process startup; cold, warm, held image/font/wasm; failed required asset; no partial Title or HUD | foundation + root; acceptance tests | Startup evidence above; `startup.cjs`; loader geometry, monotonic work, immediate readiness transition, screenshots | Baseline reproduces absent splash/premature Title; GPU repair and acceptance pending |
| GPU runtime; stock bundle crops/rotation/registered origins, glyph masks, browser fonts, dynamic images, blend/clips | foundation | `gpu_ui.rs`, existing `render.rs`, stock manifest; read-only `gpu_ui_state`, GPU meshes/textures and final screenshots | Pending foundation and composed GPU evidence |
| Title/Play; clouds, moon, graves, Solomon animation, logo/art, Continue policy, account/desktop links | menus | `menu/title.rs`, pinned Main Menu; `smoke.cjs`, `menus.cjs`, `menu-input.cjs` | Pending GPU port/current art |
| Tutorial prompt and saved-wizard retirement confirmation; Yes/No, reject/cancel and modal focus | menus | `menu/prompt.rs`; `title-prompts.cjs`, shared new-game journey | Pending GPU modal input |
| College introduction/admission then sole Create; native text validation/rejected draft, blank/random name, five elements/three disciplines, exact effects/clocks/randomization, locked Back and admission retry | menus + HUD | `menu/create.rs`, intro/dialogue; `menu-journey.cjs`, `menus.cjs`, `menu-input.cjs`, `solomon-entrance.cjs` | Pending GPU controls and ordinary admission path |
| Pause opening/closing, Resume/Settings/Cheats/Leave, shared pause behavior, held-input neutral handoff | menus + social | `menu.rs`, `shared_gameplay.rs`; `menus.cjs`, `menu-input.cjs`, `hosted-session.cjs` | Pending composed input ownership |
| Settings root/cloud/performance; sliders/toggles, permissions, saved preferences, nested account policies, mobile sharing | menus | `menu/settings/**`; `settings-parity.cjs`, `menus.cjs`, `fullscreen.cjs` | Pending real slider/key interaction |
| Control bindings; every action, duplicate swaps, Escape, modified-key rejection, allowed mouse bindings, reverse hotbar | menus + root input | `menu/settings.rs`, canonical `GameControls`; `menus.cjs`, `gameplay-input.cjs`, `hud-input.cjs` | Pending capture observations and real input |
| Save Transfer; multiple native/JSON/CFG/SAV/ZIP imports, inspected warnings, replace/cancel/busy/error, real export ZIP/download | menus | `menu/settings/transfer.rs`; `save-transfer.cjs`, `save-tools.cjs` | Pending GPU file/confirmation flow |
| Hall Local/Global × four boards; top100 and current row, portrait/orb, expand/scroll/pan, ranks/perks, loading/retry/stale result | menus | `menu/hall.rs`; `hall.cjs`, `hosted-score.cjs` | Pending GPU table and scroll |
| Join directory; normalized code, refresh, request/admission/cancel, private/mod/cheat/location policy, list scroll | menus | `menu/join.rs`; `join-party.cjs`, `hosted-social.cjs` | Pending actual host/directory journey |
| Routing consent; vanilla/private/mod/cheat, added/missing/changed save mods, downloads/busy/error/retry and close invalidation | menus + root | `menu/routing.rs`, host routing; `cheat-consent.cjs`, `cloud-subscription.cjs`, `hosted-session.cjs` | Pending GPU consent + native authority |
| Dark Cloud frame/tabs; Mods/Subscribed/Parties/Mobile UI/Boneyards, horizontal reveal, filters/sorts/search, empty/loading/error, account availability | menus; root Shelf | `menu/cloud/**`; `dark-cloud.cjs`, `cloud-subscription.cjs`, `authoring-shelf.cjs` | Pending all five GPU tabs |
| Cloud mods/detail; thumbnail lifecycle, subscription mutation/progress, screenshot contain/swipe/wrap, metadata/releases, comments/UTF16 limit/delete, preserved focus/scroll | menus | `menu/cloud/detail.rs`, real Website storage; `dark-cloud.cjs`, `cloud-subscription.cjs` | Pending real remote assets/actions |
| Boneyard Shelf; local/cloud content, search/sort/options, Test/Return/editor handoff and disposal | root | `game/hosted/boneyards.rs`; `authoring-shelf.cjs`, `authored-content.cjs` | Pending removal of visual iframe mount |
| HUD resources/XP/concentrations, native/mod belt quantity/cooldown, three banks, portraits/map, tutorial gates, custom mobile positions | HUD | `hud/**`, `hotbar.rs`; `hud-input.cjs`, `hud-selector.cjs`, `belt-pull-off-check.cjs` | Pending GPU HUD/input |
| Inventory; column-first cells, equipment/protected clothing/third ring, nested Sack pages/back/rename, animation lock, drag/drop/restoration flybys, stats/set/wizard preview | HUD | `player_panels/inventory*`, `sack_*`; `player-panels.cjs`, `responsive.cjs`, `save-resume.cjs` | Pending GPU pointer/touch and persisted state |
| Skills/quickbar; learned/ranked/weld pages, stock hover, every bank binding, skill/item drag and belt pull-off | HUD | `player_panels/skills.rs`, `skill_selector.rs`; `player-panels.cjs`, `hud-selector.cjs`, `mod-skills.cjs` | Pending GPU skill geometry |
| College services; all shops/perks/prices, insufficient gold, storage, paid Dowse/ref/roll/Done, Unforge confirmation/results, dye admission/swatches/trim/cloth | HUD | `player_panels/{services,crafting,sack_rename}.rs`; `hub-services.cjs`, `player-panels.cjs`, `responsive.cjs` | Pending ordinary earned economy and GPU actions |
| NPC chat/books/boasts/authored choices; history, progressive text/audio/acceleration, selector list drag/wheel/controller, notebox/error | HUD | `player_panels/{dialogue,notebox}.rs`; `hub-dialogue-rooms.mjs`, `menu-journey.cjs` | Pending GPU modal focus and dialogue |
| Level-up offer sequence; reveal/clocks, cards/insight/weld, responsive detail, choice/reroll/save and rejected-action recovery | HUD | `skill_picker.rs`; `hud-selector.cjs`, actual earned gameplay | Pending earned offer and real selection |
| Tutorial callouts/arrows; canonical text/narration and live GPU widget anchors; intro title/fade; Game Over riff/prompt/admission | HUD | `tutorial/**`, `college_intro.rs`, `game_over.rs`; `tutorial-text-bounds.cjs`, `solomon-entrance.cjs`, `boneyard.cjs` | Pending GPU anchor and terminal run evidence |
| Social Party; collapsed/coarse/expanded, Members/Settings/Mods, roster tags/scroll, privacy, copy/regenerate, Kick/Leave, request/invite Accept/Deny | social | `social/party/**`; `hosted-social.cjs`, `join-party.cjs` | Pending actual two-human host/input |
| Player cards/private College; Hub actor hit, Message/invite, resolve/decline/failure and owned roster | social | `social/{card,hub_players,hosted}.rs`; `hosted-social.cjs`, `hosted-session.cjs` | Pending private admission/teardown |
| Chat; Boneyard/Global/Party/Whisper, drafts/unread/fade/channel cycle, IME, author card and disconnect/reset | social | `social/chat.rs`; `hosted-social.cjs` | Pending real text composition and delivery |
| Shared Pause/resume waiting; other-owner activity, dim/reveal, countdown/progress/rejoin; spectator rail/target/respawn | social | `shared_gameplay.rs`, `spectator.rs`; `hosted-session.cjs`, `spectator.cjs` | Pending real lifecycle and GPU labels |
| Cheats/Console/developer observer; permissions, all resource/skill/world/dev controls, Lua submit/history/results/error/clear and bot targets | social | `cheats/**`, `observer.rs`; `authored-content.cjs`, `learned-bot.cjs`, `cheat-consent.cjs` | Pending authorized private-account UI journey |
| Mobile editor; all six anchors/elements, drag/resize/rotate/snap, zoom/pan/pinch, multi-touch/cancel, undo/redo/reset/pages/dock/presets/save | social | `mobile_editor/**`, `sim/mobile_ui/**`; `mobile-editor.cjs`, `responsive.cjs` | Pending real touch editing and persistence |
| Authored mod surfaces; Cast/Shop/Reforge/Portal, skills/subskills/offers/quickbar, minimap actions, mounted HUD text/image/bar/button, party attributes | social | `mod_panels.rs`, `mod_skills.rs`, `mod_skill_quickbar.rs`, `mod_minimap.rs`, `mod_world_overlays/**`; `authored-content.cjs`, `mod-skills.cjs` | Pending all authored widgets/GPU assets |
| Match loading; every recovered stage, progress/cancel/failure, host admission/world materialization | root | `match_loading.rs`; `hosted-session.cjs`, `deployment-restart.cjs` | Pending GPU stage visibility |
| Fullscreen/install guidance; request/exit/change/error, unsupported/standalone/per-screen placement | social | `fullscreen.rs`; `fullscreen.cjs`, real browser capability | Pending platform activation |
| Desktop notice/conflict/close lifecycle; offline/peer/update/Keep Playing/failure/save | root | `desktop.rs`, `desktop/`; `smoke-desktop-runtime.mjs`, desktop failure tests | Pending GPU notice and packaged journey |
| Deployment update/checkpoint/recovery; same revision identity and error ownership | root | `deployment.rs`, actual host lifecycle; `deployment-restart.cjs` | Pending GPU notice + actual update |
| Cross-cutting inputs; pointer/keyboard/touch/controller, focus/text/scroll/modal sealing, real screen-reader focus/value announcements, blur/cancel, resize/portrait/wide/fullscreen | all; acceptance common tests | `gpu-ui.cjs`, `menus.cjs`, `menu-input.cjs`, `responsive.cjs`; final current-art screenshots | Pending composed runtime |
| No DOM/CSS facade, iframe visual replacement, test-only state fabrication, unfinished required TODO or dropped feature | root + acceptance | Source scan, DOM paint audit, real GPU counters/read-only widget geometry, all rows above | Pending complete cutover |

## Test ownership and stopping checks

Acceptance owns `gpu-ui.cjs`, `gpu-ui-contract.test.cjs`, `menu-journey.cjs`, `smoke.cjs`,
`startup.cjs`, `menus.cjs`, `menu-input.cjs`, `responsive.cjs`, `audio.cjs`, `save-resume.cjs`,
`saved-game.cjs`, `college-clock.cjs`, `rooms.mjs`, `hud-input.cjs` and `tutorial-text-bounds.cjs`.
Menus owns detailed Hall/Settings/Cloud/Join/prompt
journeys. HUD and social own their family journeys. Root owns final joined execution and
packaged/hosted/authoring integration. Exact stable IDs and snapshot schema are in the task's
`gpu-ui-contract.md`; that is a development handoff, not an accepted product artifact.

The common scripts are migrated to read-only GPU geometry and actual browser input. Syntax
checks pass for all fifteen files. The architectural contract was run before the cutover:
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
