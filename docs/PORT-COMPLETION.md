# Rust game parity and release

The complete Rust/WASM GPU game and desktop launcher are published at
`4aa3a7b733a46788e7cb27b601f6bceb5e4f08b9`; the game is installed in production. The
[release](https://github.com/JarrettOneSource/solomon-darker-rust/releases/tag/desktop-4aa3a7b733a46788e7cb27b601f6bceb5e4f08b9)
contains all 11 expected assets. Its
[main workflow](https://github.com/JarrettOneSource/solomon-darker-rust/actions/runs/40170772878)
passes every job: 701 native tests, formatting, strict native/main-WASM/loader checks,
45 browser-support tests, seven sdmod tests and all four complete packaged journeys.
Each platform passes 33 desktop contracts. Windows also passes 47 native launcher tests
and static CRT inspection. All 32 final screenshots, archive inspections and 28 safe child
stops are reviewed, with zero raw resource or media failures in these final packages.
Software-renderer qualification establishes functionality, not GPU performance.

[RUST-UI-MIGRATION.md](RUST-UI-MIGRATION.md) owns the complete surface matrix, platform
adapter boundary and earlier receipts. The launcher shares the Rust GPU runtime and
native text editor; its three HTML/CSS/JavaScript presentation files are removed.
All required game-browser families retain acceptance at their recorded revisions.
The final release qualification and anonymous asset verification are in
`main-release-4aa3a7b/qualification.json` and `public-verification.json` under the M2/M5
task `solomon-full-rust-ui-20261023T165939Z`.

Website `db9a9c98434aa5df56e3f9c402719209001f061b` consumes that exact Linux artifact,
SHA256 `1f193ff9fbe82d357dbb30583955b5d578ae876aa576b6e94a607226ba9f9a64`.
Its complete M5 gate and
[Linux CI](https://github.com/JarrettOneSource/solomon-dark-website/actions/runs/40176502878)
pass, covering all 78 backend/deployment contracts across both platforms and all 64
frontend tests on each. The normal installer admits the complete four-component worker
`7199590964dcb2f9f0a6467c0a6fea1d8cee8c28a8369cbfe2e908ce92559ff4`.
Its ordinary scheduled invocation deploys the release at 08:40 UTC on October 25,
draining one player with one acknowledged save and zero unacknowledged saves.
The installed Linux runtime passes every one of its 5,710 payload hashes and complete
membership checks. All 12 ordinary HTTPS identity/document/CSP/publication checks pass;
independent staging, notifier and Caddy retain their processes.

The live browser records the exact final PixiJS checkpoint, cloud persistence before its
matching readiness ACK, and automatic loading of the exact GPU client. A fresh account
read matches that entire checkpoint. Native Continue preserves Felix's loadout; ordinary
Leave persists, and a fresh document continues the same player and saves again. The final
four-group journey passes with no unexpected browser/API errors, reviewed title/College
images and graceful browser exit. Four raw music cancellations have complete ready-channel
suspension evidence recorded before Continue resumes playback.

The initial cutover observer remains failed: Playwright lost the final successful revision
poll body during navigation, after the exact-revision/title checks passed. It also records
five transient 502 polls during service exchange and two legacy media cancellations during
document retirement. The first continuation observer used the wrong save-response envelope;
the next observed audio too late, after playback resumed. All original receipts retain their
outcomes. `live-acceptance-review.json` records the independent save/reload witnesses,
these limits and the passing final `live-continuation-4aa-r3/` journey.

The scheduler is enabled, with no task operator hold or compute lease. Original pre-Rust
archives, later rollback generations and current persistent data are retained. The fresh
pre-deployment runtime is `/opt/solomon-dark-revived.rollback-pre-db9a9c98434a-20261025T084009Z`;
state/configuration backup is `/var/backups/solomon-dark-revived/pre-db9a9c98434a-20261025T084009Z/`.
`release-admission-4aa/` owns installation, deployment, backup and postflight receipts.
Task worktree/cache retirement is recorded in `final-cleanup-m2.json` and
`final-cleanup-m5.json`; durable artifacts, receipts and rollback material stay outside
removed source trees. The documentation-only completion commit retains this qualified
product revision and does not republish unchanged binaries.

The older release receipts below retain their original scope.

The earlier Website parity implementation passed its complete local gates.
The independent repository and Linux game artifact are public. All 44 public acceptance
groups pass across their recorded revisions, including the actual `/game-stg` replacement
from `ca728e90` to `d3903095`. Normal DNS access recovered after the separately recorded
authoritative serving failure; both M2 and NFO ordinary HTTPS checks pass. The desktop
packages pass final archive inspection and actual Windows, Linux, Intel Mac and Apple
silicon Mac journeys. The [standalone apps are public](https://github.com/JarrettOneSource/solomon-darker-rust/releases/tag/desktop-1a4f7063cd3f4b9cb23651bdbe7cfffa7620cf76),
and the Website extraction was subsequently merged. Current production acceptance is recorded above.
The target is the current Website's behavior and existing user data. The requested result is a separately published Rust game repository, removal of
the replaced game implementation from the Website repository, a WASM embed in that Website,
and a verified `/game-stg` deployment. The behavioral reference is
[Website `fa08ae24`](https://github.com/JarrettOneSource/solomon-dark-website/tree/fa08ae249ff5563570bdb2a653e2ef9358ce7b83).
Earlier fixture and standalone receipts retain their actual revisions and scope.

The implementation reuses the existing kernels, stock assets and dependencies. Canonical
Session state owns gameplay; presentation copies cannot advance authority, save state or
consume gameplay randomness. Source references and focused differential fixtures document
the translated behavior. Read [CLIENT-PRESENTATION.md](CLIENT-PRESENTATION.md) for the
accepted-state/display boundary.

## Acceptance boundaries

The current source inventory and removal dependencies are recorded in the task evidence
`current-main-delta-map.md` under `/Users/jarrett/codex-runs/solomon-rust-parity-20261021/`.

| Work | Owner and implementation boundary | Acceptance |
| --- | --- | --- |
| Current simulation and saves | Rooms, NPCs, visuals; root owns save adapters | Current enemy/effect lifetimes, protection and resource rules, permanent unlocks, creation choices, three hotbar banks, shared flashes, Flame history and supported earlier saves reproduce current Source state and continuation. |
| Complete interface and authored content | HUD, NPCs and visuals | Current title/Create/settings, social cards, inventory/book/painting, bank controls, authored shops/skills/audio, input and rendering pass focused source fixtures and ordinary browser journeys. |
| Hosted runtime | Runtime; root owns browser receive/save lifecycle | Global/private worlds, Party/social/bots, Memorial, run diagnostics, large welcome/save transfers, recovery and disposable editor tests preserve their actual ownership and failure behavior. |
| Website separation | Root | Account save tools, editor Test, authoring CLI, build/validation and deployment consume the Rust game artifact with no remaining TS game authority/client import closure. Website API, storage and game asset/revision mounts have explicit independent owners. |
| Repository publication | Root; Rooms owns lossless large-fixture compression | Publish the reviewed game tree with declared build inputs and reproducible artifacts; preserve local development history and evidence. Existing desktop delivery behavior remains covered by the artifact/shell integration. |
| Staging publication | Root | Isolated Linux services and `/game-stg` pass actual account, existing-save, party, authored-content, editor, score and deployment journeys without altering production state. |

Preserve valid earlier receipts for unchanged behavior. Changed families require focused
source fixtures and observable integration checks. Final acceptance requires the complete
repository gates, a reviewed feature inventory, current device documentation and post-push
cleanup of task-owned worktrees on every device. Missing enabled behavior cannot be
reclassified as a completed direct-host limitation.

## Retained parity verification

These local and development receipts retain their exact revisions. Current publication,
installed staging and remaining delivery checks are recorded below.

The current save corpus passes all **217** cases: current whole-state continuations,
supported historical migrations and invalid-state rejection. It includes both quality
settings, retained Flame births, shared flashes, rescue protection, precise headings,
three hotbar banks, death weapons, Golem fragments and Ether Drain references. Saved
wave tables survive restore without recompilation. Two positive current-source saves also
preserve decoded Terrain UID, profile samples and side sign through whole-state restore,
re-encode and retirement (`gates-20261021T181931Z/current-save-terrain.log`). The fresh wave compiler/director
fixtures, actual Session rescue/modal casts and Account portability cases also pass.
These results are in `hosted-browser/gates-20261021T154009Z/`; they qualify their recorded
behavior and revisions. The independent Account WASM facade passes all
12 real-browser ABI checks with no game canvas or socket. The actual mounted Website
Register/Account journey previews and imports an existing save, commits it to the cloud,
and exports ZIP through this facade. Evidence: `hosted-browser/save-tools-4935b027/` and
`hosted-browser/website-embed-41cae2b2/`.

The disposable editor Test iframe keeps one stable DOM parent while the React draft stays
mounted. Actual cancellation after HTTP admission claims and closes the authority, disposes
the hidden iframe, and preserves the cloud revision/document, resume state and editor draft.
The playable custom-scene journey reproduced a missing per-geometry render asset. The
replacement compiler now uses one Rust scene path for stock and authored maps, with
catalog-backed placement and native Canvas2D terrain/walls. Current scenery and lighting
fixtures pass; the actual compiled editor play/return and hidden-frame resume checks pass
in the complete nine-group `authoring-shelf-69d-04/` journey.
Flame client/history validation, display timeline and the shared camera/flash lane pass
the focused `gates-20261021T155931Z` checks. Current dialogue and native authoring fixtures
also pass; the extracted Website gates and mounted account/editor journeys pass.

As of 2026-10-21, the canonical V8 numeric corrections pass 7,697 atan2 cases,
20,100 native sine/cosine pairs, 40,200 WASM scalar results and 17,431 exact powers on
both native and WASM. The power correction is used at the measured Fire envelope
divergence; its retained C++ discriminator and WASM receipt are in `wasm-math/` and
`pow-{contract,no-contract}.json` under the parity evidence directory. Five independent
world journeys cover 5,200 ticks, including the original template-9 witness and
1,032 every-tick Solomon spatial/phase/overlap checks. Discrete state is exact;
retained general checkpoints have maximum position error `2.28e-13` and heading
error `1.54e-12`. The same unanchored trajectory tests also pass on Linux.

The actual supervisor passes 34 Source HTTP/WebSocket observations, warm recovered
Boneyard admission and four admitted-mod receipts through real Lua rules, shop actions
and terminal saves. The empty-private-world crash was reproduced by removing only its
teardown while an observer retained the authority; restoring the teardown passes.
Prepared Lua ownership also retires with the empty private run. The genuine browser ZIP
imports into its unchanged restored run and advances more than 100 later host ticks.
The nine Website save suites, native CFG/SAV/ZIP bridge, all ten prepared-save domains
and canonical mod-effect continuations retain their focused Source receipts.

The prepared host executes actor/control/rule epochs, authored services and casts,
canonical Session extensions and full engine-state saves. Source fixtures cover actual
Lua definition/callback/package preparation, status/affix/spells, skills/shops, actors,
scenes, portals, dynamic effects and projections. Console VM execution passes 53 cases
plus timeout and memory-failure recovery, and the actual supervisor passes ten Console/bot
lifecycle observations. The ordinary-browser bot combat journey passes as recorded below. The retained bot model passes exact inference, 26 complete observations,
7,693 action decisions and eight controller scenarios. Durable shared Memorial storage
is connected and its codec, persistence and shared-world checks pass. Score eligibility, once-only publication and timestamp
formatting pass the Source fixtures; actual browser score submission also passes as recorded below.

The unchanged C# Website backend served isolated local storage through the local TLS
`/game-stg` gateway using the intended public content policy and an external WASM
bootstrap. Matching client/host `5373106` passes actual mounted Website registration,
Rust account identity, sign-out and form sign-in; credentials remain scoped to this mount.
See `hosted-account/report.md`. The accepted two-account social journey includes Global
chat, cards, invitation decline, consent and a two-member private College. It holds the
actual save PUT to prove that transfer waits for persistence before the next authority
hello (`hosted-social/run-9-held-barrier-green/`, build `77ce4f8`).

Actual cheat consent passes Cancel and save-before-leave, then continues the same wizard
in a private College. Source-inherited prompt stacking and early Global SetCheats defects
were reproduced before repair. An explicit checkpoint-zero delivery test restores pause,
continues 25 accepted ticks on the same connection and succeeds on normal Leave retry.
These receipts are `hosted-browser/cheat-consent-5373106-green/` and
`hosted-browser/leave-failure-5373106-final/`. A naturally denied Inventory Equip with only
its optional feedback payload omitted froze the old client. Build `1d682fa` passes that
same boundary: normal rejection, accepted ticks continue, unchanged inventory and a normal
retry (`hosted-browser/inventory-rejection-1d682fa-green/`). This is a receiver failure test;
the separate native prepared-host test owns atomic authored-consume rollback evidence.

Local acceptance used the complete `ad1ba50b` native/WASM/save-tools artifact.
All 5,727 packaged files matched its SHA256 manifest; the immutable candidate owned served
assets as well as executables. Exact identities are in `hosted-browser/{host,browser}-build.json`. Signed score publication through the
unchanged Website API returns HTTP 201; a fresh sign-in sees the persisted row in all four
Global Hall boards with ordinary pointer controls and no errors. The report is
`hosted-browser/score-publication-report.md`. Authored shop purchases and Lua stock/state
changes pass. Gravity Lesson passes all eight groups on matching `4e5db0d4`, including
actual casts, binding and save/leave disposal. The Apothecary journey now also passes custom icons, rejected consume/drop constraints,
Sack movement, Reforge/equip, minimap/audio, Tonic consume/status and actual Save/Continue.
`authored-browser/accepted-phases.json` retains the exact revision and phase provenance.
The ordinary-account Console retake passes on `ad1ba50b`: real Lua values, retained state,
timers, sandbox restrictions, queued gold mutation and timeout recovery, with no errors.
`authored-browser/run-ad1ba50b-console-final/` owns that separate receipt.

Current effect verification passes actual Ether Drain world/Session, Coffin emergence,
Golem assembly/death, harmful-contact/Harden damage, Stoneskin lifetimes, dropped death
weapons, browser settings and Hall v3/v2 migration. The death-weapon replay retains actual
shared RNG state across Game Over and owner removal. Source Sack motion has 194 exact
height/frame cases. Focused evidence includes `gates-20261021T171610Z/`,
`gates-20261021T171728Z/` and `death-weapon-after-adapter.log` under `hosted-browser/`.
Complete current secondary lifetimes and dynamic authored-scene rendering now pass their
focused replays and the complete workspace gate.

Two ordinary repeat-run/reload failures are retained in
`visuals-current-browser/run-41cae2b2/`: readiness stayed cached across a new hosted world,
and a restored Boneyard welcome rejected numeric scenery-map keys. Readiness now follows
each accepted runtime snapshot, matching current Source. Both current restored-save wire
cases plus historical cases pass in `gates-20261021T171406Z/restored-wire-after.log`.
The hidden Cloud text zero-scale panic also has a Source-based correction. All three fixes
are included in `4159dc96`; compact Cloud browser checks now pass with no errors. The restored Boneyard retake passes on `69d28dc4` as recorded below. The separate mobile
frame double-scale correction is integrated and covered by the mounted mobile journeys.

Current Golem attack contacts, native Terrain/bridge surface queries and spectator selection
plans pass their Source replays. Cold Aura/Water/Hail/Earth values and electric status actor
births pass after genuine failing witnesses. Frost Missile direct/area/terrain contacts and
shared Bouncer/Growing effects pass their current recordings. These focused checks are in
`hosted-browser/gates-20261021T181227Z/`, `gates-20261021T181655Z/`,
`gates-20261021T181931Z/` and `gates-20261021T182516Z/`. The complete current fixture integration passes. Actual Flame and
Storm/spectator GPU journeys also have accepted receipts.

The ordinary `2073e7ca` Sack journey passes six groups: purchase, contained item movement,
all three hotbar banks, native open/close motion, Unicode rename and saved Leave, with no
runtime errors. `hud-sack-current/` owns the actual receipt. Its Canvas2D fallback now uses
104 Source layout cases. The actual `69d28dc4` retake passes all seven groups with empty
runtime errors and normal saved Leave: the Unicode caption and wrapped hover match the
Source Canvas metrics, and the Sack has one accessible rename label.
`hud-sack-canvas-current/` retains the receipt and inspected screenshots. A resume trace separately proves that Welcome precedes the
Boneyard metadata by 43 ms and produced a transient missing-scene error. The client now
retains the frame until matching metadata arrives; `restore-admission-before-2073/` records
the witness. The actual `69d28dc4` retake passes with the same run, Welcome arriving
28 ms before its matching scene, an active rendered world, empty runtime errors and normal
saved Leave. The screenshot and both positive receipts are in
`hosted-browser/restore-admission-after-69d28dc4/`; the browser is closed.

The same `69d28dc4` artifact passes ordinary repeated painting: one kit paints a nested
Hat and Robe through the same accepted session, then closes and saves normally
(`hud-dye-current/dye-receipt.json`). Authenticated mobile layout publication passes five
groups: real registration, editor drag/Save, HTTP201 publication of the exact saved layout,
trusted clipboard copy and reload persistence, with no errors
(`hud-layout-publish-current/receipt.json`).

The actual learned-bot journey passes five groups on `41cae2b2`: 362 accepted Boneyard
samples include movement, 50 bot-owned Water particles and actual mana use; normal saved
Leave returns the host to zero humans, bots, parties, runs and sessions. Runtime errors are
empty. `learned-bot-browser/report.md` and `41ca-attempt12/` retain the precise evidence.

The Rust repository owns the desktop Electron shell and native child authority. The WASM
client binds the installed preload endpoint, reads the existing Source IndexedDB slot,
uses Source settings/Hall keys and tracks page playtime, and waits for the canonical durable
save before close. The actual macOS ARM64 Electron package from `4159dc96` passes offline
play, saved close, exact-position Continue, settings persistence, two desktop peers entering
one Boneyard, update-notice dismissal and child-process teardown. The update notice uses
a declared release-response fixture; gameplay and persistence use the actual package.
A real IndexedDB revision conflict also passes: the committed document stays unchanged,
Keep playing retains the same host, participant and socket with accepted movement and no
Disconnect acknowledgement, and Close without saving exits and reaps the host. Only the
Electron warning response is automated. `desktop-4159dc96-failed-close-03/` retains the
complete repeated suite, both warning arguments and inspected screenshots. This is
functional software-renderer acceptance, not a graphics performance measurement.

The ordinary browser also accepts unchanged Source IndexedDB schema51 saves, camelCase
settings, Hall v3/v2 records and page playtime. It verifies initial reads without rewriting,
the real settings controls, periodic/pagehide playtime, actual native Hello, normal Continue,
two durable Leave cycles and reload. All eight groups pass with no errors in
`hosted-browser/source-storage-4159dc96-02/`.

The extracted Website keeps account/content/storage/editor ownership and consumes the
verified Rust artifact. All 63 backend/deployment contracts and backend formatting pass;
frontend tests, lint, production build and media policy pass. Existing React export warnings
remain. Evidence is `hosted-browser/website-backend-gates-1742.log` and
`hosted-browser/website-frontend-gates-1746.log`. The authoring handoff uses a one-use tab token and native
IndexedDB payload; its actual browser check preserves a 16 MiB document, rejects replay
and rolls back after a token quota failure. The actual `69d28dc4` shelf passes all nine groups with empty browser/API/runtime errors:
parent-keyboard routing, desktop/phone layout, New, cloud Save, exported Import, local Edit,
one-use replay rejection, Delete/Keep focus and both local/cloud Test. Hidden zero-size
frames resume accepted ticks when shown; Return preserves the editor, restores Test focus
and closes both admitted sockets. Cloud deletion returns HTTP204 and preserves the local
draft. `authoring-shelf-69d-04/` retains the complete receipt and inspected screenshots;
`authoring-shelf-2073-02/` retains the original hidden-layout failure.
Stock build inputs are Git-owned with complete SHA256 membership and all 233 native audio
entries checked. `scripts/package-hosted-game.py` successfully built the complete revisioned
native/client/save-tools artifact and archive. `hosted-browser/package-4159dc96/` owns the
build receipts. These are development-profile acceptance artifacts, not a final release.

The actual deployment journey passes all five groups on the `69d28dc4` to `07d65e33`
replacement, with empty browser/API/runtime errors. It holds the cloud-save PUT, proves
no premature ACK/close, then observes committed save, matching ACK, closed socket and
a persistent saved overlay. Incorrect publication metadata is ignored; the actual target
artifact reloads and Continue restores the same wizard. Screenshots were inspected.
The replacement host must be healthy before publishing its client revision. A coordinator
retake that exposed the client first correctly received HTTP503; no product change was
needed. The accepted receipt is `hosted-browser/deployment-69-to-07-ready/`.
The same candidate passes the actual saved-retirement-to-Create status regression;
`hud-earned-current/run-07d65e33-01/` retains both empty-status captures.

The complete Linux workspace passed at `433e9e25`: 383 tests, none ignored, formatting and
strict all-target/all-feature lint (`receipts/20261021T124927Z/` on NFO). Later targeted gates
pass the actual runtime-event HTTP publisher, native/WASM builds, 49 Memorial codec cases,
atomic file persistence and both shared-world suites (`receipts/20261021T132303Z/`). These
earlier receipts precede the complete 531-test qualification recorded below.

The repository now owns the existing four-platform desktop validation/publication workflow,
with pinned Rust/Node tools and a separately checksummed Linux Website artifact. Website
Downloads targets the new game repository. Published artifacts and platform qualification
are recorded below. The current complete enemy recording
retains 132 programs, 105,000 ticks and 5,511 states; all 214,267,806 decompressed bytes have
a recorded SHA256. The complete 25,643,928-byte secondary lifetime recording is also
losslessly compressed. The complete current audit in
`hosted-browser/gates-20261021T201705Z/` passes all **531 tests**, with none failed or
ignored. Formatting, strict native all-target/all-feature Clippy and strict WASM Clippy
also pass. This includes current Staff ordering, Weld continuation, ML mana costs,
player contacts, all current/historical saves, snapshot application and complete secondary
plans. All nine actual supervisor tests pass, including a retained idle College frame
that proves the archive test selects the actual run. No authority timing change was needed.

The complete enemy replay passes all twelve tests, including all 132 independent programs.
Fleeing movement uses the Source UID-phase cadence and immediate heading; the nine status
continuations pass. Four historical Spider view programs retain their ignored-key contract;
four added current-key programs and 29 Source presenter/animation endpoints cover the actual
view options. Existing expectations and tolerances remain intact. The focused receipt is
`hosted-browser/enemy-all-132-endpoints-after.log`.

The final Leviathan comparison at `2325c6ee` matches unchanged current Source pixels in all
nine normal/simple-hit/complex-hit cases at zoom 0.75, 1 and 1.5: **11,059,200 RGBA bytes,
zero differences**, with no tolerance or masking. Production-equivalent context attributes,
actual sample counts, inputs and source hashes are retained in
`leviathan-gpu/final-2325c6ee/`; runtime errors are empty and GL error is zero.
The normal release does not expose the optional browser fixture interface.

Ordinary progression earns the real level-up offer, chooses it and observes accepted rank1.
The paid Shlorio journey earns 655 gold through normal combat, uses Hat38 as its reference,
pays 650 and receives four offers. Source `hub-economy.ts` draws three or four ordinary
offers and adds two set offers only when the reference has a set; this starter Hat has none.
Continue preserves the resulting five gold and all four offers; actual Done clears the roll
and ordinary Leave saves. No developer currency grant or edited save supplies this result.
The HUD evidence keeps the paid action and resumed Done/save receipts separate.

A same-epoch ordinary-account witness proves accepted Enable Cheats could leave cached Pause
rows stale while Settings overlaid that screen. `ad1ba50b` refreshes those existing rows;
format, strict native/WASM checks and all web tests pass in
`hosted-browser/gates-20261021T203356Z/`. The actual before/after menu witness and ordinary-account Console retake both pass with
empty errors on the matching bundle. The moving Storm survives Enhanced Effects true,
false and true again with the same actor and birth state, no WebGL error and durable Leave
(`storm-quality-ad1ba50b-01/`). Actual Electric Arc/Flare acceptance remains
in `electric-gpu-9668b57a-03/`. Both require the normal Solomon entrance and
Source hotbar bindings. The actual Flame/spectator journey on `ad1ba50b` also passes held/released Ribbon and Fade
births, retained history/GPU batches, real death, spectator status/follow/target cycling,
no accidental casts, leader saved Leave and spectator reset. Its surviving peer correctly
returns to College after the leader leaves; a final cleanup helper wrongly expected a
Boneyard pause and is recorded separately from those passed assertions.

## Published repository and staging

[The independent repository](https://github.com/JarrettOneSource/solomon-darker-rust) starts
at `ca728e9088c09b829473bd2e0f3728ba69b35fd0`. All 7,624 initial files match the reviewed
integration snapshot `678b2404`; local development history remains in the legacy repository.
Follow-up `65117c8` preserves exact stock bytes during Windows Git checkout. The 21 changed
hashes were reproduced as LF-to-CRLF conversion and the stock-only attributes fix restores
all recorded asset hashes. It changes no gameplay code or source asset bytes.

The initial NFO `ca728e90` Linux qualification passed all ten gates: stock integrity, formatting, strict
native/WASM checks, 531 workspace tests without failures or ignored tests, authored-content
CLI checks and both optimized game mounts. Each artifact verifies 5,727 payload files.
The original `ca728e90` archive and its checksum remain recorded in
`release-final/game-release-publication.json`.

The current [Linux game release](https://github.com/JarrettOneSource/solomon-darker-rust/releases/tag/game-d3903095bdcafbf2fcf2fac9165bd69e9693e803)
is `d3903095bdcafbf2fcf2fac9165bd69e9693e803`. Its production-mount archive has SHA256
`3f12af6aa1aa9c5787379fb04fda1d81b08319560889e64751b805708d75d005`; all 5,727 payload
files, the public checksum and GitHub asset digest match. Website repository variables now
pin this artifact. `release-final/hosted-d3903095/` owns the archive, publication and pin receipts.
All ten Linux gates pass for this revision, including 534 workspace tests with no failures
or ignored tests. Its native transport retains partial nonblocking writes after the proved
Windows send-timeout failure. Three real TCP regressions cover ordered resumption,
heartbeat expiry and explicit-close deadlines. Game kernels, stock assets and WASM bytes
are unchanged from `ca728e90`; `release-final/linux-d3903095/` records this qualification.

The staging artifact at `/opt/solomon-rust-stg/current` serves
[the public game](https://solomondarker.com/game-stg/) through separate API 5229 and native 5230
services and isolated state. Its game is now `d3903095`; the unchanged Website API/static
source `fb042d14` retains its 25/563 verified files. Public acceptance passed 44 groups:

| Public journey | Groups | Receipt under `release-final/` |
| --- | --- | --- |
| Registration, native sign-out and form sign-in | 5 | `public-account-03/receipt.json` |
| Two-human social transfer and eligible score/persisted Hall | 17 | `public-runtime/acceptance-summary.json` |
| Account WASM import/export, editor lifecycle and authoring shelf | 14 | `public-editor/consolidated-acceptance.json` |
| Authored package, purchase, consume and Save/Continue | 4 | `public-authored/run-04/receipt.json` |
| Actual revision replacement and Continue | 4 | `public-replacement-02/receipt.json` |

The first 40 groups retain their accepted `ca728e90` identity. The final four prove the
actual `ca728e90` to `d3903095` replacement: hold the announced checkpoint's real save PUT,
observe its commit before the matching ACK and socket close, verify the replacement host
before publishing its client revision, then observe automatic reload and normal Continue
of the same wizard. Production and staging API PIDs and Caddy remain unchanged by that
replacement. All public acceptance browsers are closed.

After these journeys, Spaceship's delegated authoritative DNS service temporarily failed.
The October 22 17:52 UTC recheck finds current signed answers from both authorities,
Google DNSSEC validation, and ordinary HTTPS serving `d3903095` from M2 and NFO. All ten
normal-DNS HTTP/CSP checks pass. The earlier known-IP strict-TLS postflight retains its
separate scope. The observed KSK matches the parent DS, and no DNS, DNSSEC, delegation or
resolver settings were changed. `release-final/dns-postflight-20261022/` and
`public-http-d3903095.json` retain both the failure and recovery. These bounded checks do
not establish the provider-side cause or guarantee future availability.

An ordinary public cold start reached Title in 146.3 seconds with no errors after 438
resources, including all 233 audio files. The Source also preloads resident audio. Public
journey startup bounds allow 240 seconds; subsequent behavior retains its separate deadlines.
This observation is not a general startup-performance claim.

The Website extraction [PR8](https://github.com/JarrettOneSource/solomon-dark-website/pull/8),
source `2a7b40a0`, was merged as `7a787823`. Retained Linux CI receipts pass all 63 backend/deployment contracts and
frontend checks. Both `d3903095`-pinned reruns (`39789137765` and `39789127353`, attempt 2)
pass and their logs verify the actual artifact revision. Production migration requires manual admission of the accepted
Rust-aware M5 worker, its configured production artifact and a passing full gate; the
installed TypeScript worker cannot bootstrap this artifact-layout change. The procedure
is in Website `ops/local-ci/README.md` and remains separate from staging acceptance.
Website main `093aee99` only adds the tested staging Caddy route and
its operations note to the `fa08ae24` behavior. The normal M5 worker deployed it on
October 21, preserving staging through subsequent reconciliation. Directly editing the
managed Caddy file had been reverted by that worker; keeping the route in its source fixes
that ownership error.

Desktop acceptance uses actual child-process spawn observations, the native Windows
graphics backend, and fresh frames from both peer views inside the original shared
120-second deadline. The update notice includes its existing stylesheet and must fit
inside the viewport before capture. Earlier process-lookup, missing-style and build-lock
failures retain their red/green receipts. Desktop preparation removes Trunk build locks
after successful client validation. The final archive guard rejects stale Trunk locks,
unexpected stock/shell files and generated residue; all 27 desktop tests pass.

For source `5c094aaa`, run `39826304375` passes the game gate and the Linux and both macOS
packaged journeys. All four resulting archives pass full inspection. The retained Windows
pair independently passes the untouched driver and Playwright in run `39851732754`: all
51 original assertions, visible notice, exact schema-51 save/resume, both failed-save
choices, shared Boneyard readiness and native exit. Its earlier combined-job screenshot
timeouts remain recorded; the capture comparison does not establish a capture API defect.

That Windows executable depended on an unbundled `VCRUNTIME140.dll`. The static CRT
replacement at run `39854803735` passes complete import/archive inspection with all 5,723
client files unchanged, but its native tests expose two independent failures. The retained
unit diagnostic `39858878788` reproduces each original failure in six runs. Shutdown's
listener had already exited before its unconditional wake connection, which spent about
2.01 seconds returning connection refused; joins then completed immediately. The slow-reader
fixture had already handed its old snapshot to Tungstenite before trying to replace it,
even with constrained socket buffers. It had not established the intended unsent backlog.

The clean fixes at `41de8d5` retain the listening socket through wake and acceptor join,
then close it explicitly. The fixture now queues distinct reliable frames and proves the
snapshot is still unsent before replacement, retaining its ordering, resumed-input and
deadline assertions. Windows builds use the standard static CRT target setting, and CI
checks actual native imports and the affected launcher tests. Local macOS ARM64 validation
passes all 42 launcher tests, strict native Clippy, formatting and 19 desktop tests.
The subsequent `af80793` matrix passes the complete game gate, all four final-ZIP checks,
all 42 Windows native tests and its static CRT import check. Its ARM Mac journey passes;
Linux, Intel Mac and Windows reach the final renderer-error assertion and fail there.
The failed matrix remains recorded as failed. Earlier artifacts retain their own identity.

The desktop fix at `1a4f7063` retires the saved game document before stopping its services.
Repeated close/quit requests remain guarded through completed cleanup; interrupted
retirement destroys the surviving document before services stop. Keep playing retains
the existing document, native host and connection. An actual AF package fails the new
stop-order regression, while the Mac replacement passes all original 51 assertions,
repeated close/quit, Save and return to launcher and six observed service-stop requests
after document retirement. The AF local control proves the ordering gap but does not
reproduce the original CI image error. The strict error gate remains unchanged.

All four final platform jobs pass for `1a4f7063`: Linux, Windows and ARM Mac in
[run39879577620](https://github.com/JarrettOneSource/solomon-darker-rust/actions/runs/39879577620),
and Intel Mac in
[run39879923237](https://github.com/JarrettOneSource/solomon-darker-rust/actions/runs/39879923237).
The first Intel attempt stopped before packaging because its token could not read the
draft input asset; only Intel was rerun with the required access. The first workflow retains
its failed status from that access step; its other three jobs pass. Each final ZIP is
inspected before its actual save, peer, update, failed-close and shutdown journey.
Windows records no menu-image cancellations, and all seven screenshots are
reviewed. `release-final/desktop-lifecycle-qualification/` and
`lifecycle-1a4f7063-39879577620/` retain the actual checks and their boundaries.

The new shells use verified native/client components from clean `af80793` source after
proving their complete build inputs unchanged. The missing Intel native is built separately
from that same source with the pinned Rust toolchain; its build provenance is retained.
The reused client has 5,723 verified files. The complete AF Rust/WASM qualification remains
valid, and the new packages receive their own source identity, hashes and platform checks.
The Linux game archive accompanying desktop delivery retains its actual AF manifest.
The published release contains exactly 11 assets: four desktop ZIPs and their checksums,
the common desktop manifest, and the Linux game archive and checksum. The temporary Intel
build input is removed. Anonymous public checks verify every GitHub digest, all downloaded
checksums and the manifest, payload availability and the latest-release feed. The release
tag points to the accepted `1a4f7063` source. Prepared/publication evidence is in
`release-inputs-1a4f7063/` and `desktop-lifecycle-qualification/public-release-verification.json`.

The installed staging game and checksum-pinned `d3903095` Website artifact retain their
recorded public acceptance. These desktop packaging and listener-lifetime changes do not
change game kernels, stock assets or WASM gameplay behavior. Exact diagnosis and clean
local gate results are in `windows-release-5c094aaa-39826304375/` under the release evidence.
After desktop publication, the October 23 01:18 UTC postflight passes all ten ordinary-DNS
HTTP/CSP checks, Google DNSSEC validation and normal NFO HTTPS. All five production,
staging and Caddy service PIDs remain unchanged; staging is healthy, empty and accepting.
No DNS settings or live services changed during desktop delivery.

Exact manifests, logs, screenshots, raw failed attempts and successful receipts are under
`/Users/jarrett/codex-runs/solomon-rust-parity-20261021/release-final/` and NFO's matching
`receipts/final-release/` directory.

## Earlier standalone release

The macOS ARM64 release folder and ZIP are in:

```text
/Users/jarrett/Projects/Solomon Dark/Packages/solomon-darker-macos-arm64-20261020/
/Users/jarrett/Projects/Solomon Dark/Packages/solomon-darker-macos-arm64-20261020.zip
```

Double-click `Play Solomon Darker.command`, or run the packaged `solomon-darker` executable.
It serves the bundled game on loopback port 4188 and opens the system browser. Keep the
launcher running while playing. Its README explains direct multiplayer and durable saves.

Both native and WebAssembly release builds came from clean
`96ca53be8b3a41a2b9096963b0e813d9691b6546`. Subsequent receipt, browser-driver and documentation
edits do not change those game binaries. The archive contains 5,180 files and passes its CRC
check. Build metadata and SHA-256 digests are retained in `integration-final/package.json`.
The actual final browser runs independently recorded matching executable and WASM hashes.

## Earlier verified behavior

- New Game, the complete College admission, loadout selection, courtyard, all four private
  rooms, NPC/student behavior, conversations and services use canonical participant state.
- Boneyard gameplay composes all thirteen retained scenes, gates, scenery, navigation,
  enemies, Solomon, waves, all elemental/welded and secondary spells, loot, progression,
  death and return. Rendering includes lighting, shadows, rain, compact masks, death and
  level-up effects, with source painter ownership.
- Menus, inventory, skillbooks, settings, keyboard/controller/touch input and audio operate
  on the real session. Browser-only saves use IndexedDB; native sessions use the host's
  atomic durable file. Preferences and resume credentials remain browser-local.
- Direct native multiplayer supports ordered participants, input acknowledgements, purchases,
  Party/cards/whispers, accepted chat, shared holds, readiness, fullscreen, reconnect and
  save/restart. Public party codes/listings and Global chat require the Website's broker and
  are explicitly unavailable on a direct native host.
- The renderer samples the source eight-snapshot timeline for camera, actors, lights and
  painter order. Boneyard interpolation and bounded local College prediction preserve the
  accepted simulation, integer authoritative clocks and gameplay RNG.

## Verification

Evidence is retained under `/Users/jarrett/codex-runs/solomon-rust-fleet-20261020/`.

| Receipt | Result and scope |
| --- | --- |
| `integration-final/gates.json` | All 259 workspace tests passed with none ignored at `63c3976`. The first strict Clippy run identified six mechanical presentation-code issues. |
| `integration-final/after-clippy/gates.json` | At `96ca53b`, all 24 affected client tests passed, formatting passed, native all-target/all-feature Clippy passed with warnings denied, and WASM Clippy passed with warnings denied. |
| `integration-final/package.json` and `package-build.log` | Optimized native and WebAssembly builds, clean-source metadata, complete bundle and archive integrity passed. |
| `integration-final/native-browser-ready/` | The exact package passed fourteen actual two-browser checks on Metal, using an unrelated working directory, an empty command path and isolated native save. All 1,024 requests were bundled; no runtime errors occurred. |
| `visuals-presentation-final/` | Actual GPU submissions change at a median 16.7 ms while accepted snapshots remain approximately 20 Hz. The genuine Tutorial arrival closely matches the pinned JS capture. Zero browser errors or failed requests. |

The native browser journey covers two complete College admissions, shared movement and input
acknowledgements, real inventory transactions, chat focus above Inventory and Pause, Party
roster/card/whisper, old-shop cleanup at run entry, renderer readiness, named shared pause,
resume countdown, trusted fullscreen, save-before-leave, same-wizard reconnect with token
rotation, and native-file restart. Real movement then wakes Solomon and reaches combat death.
The actual Game Over title and Continue readiness are observed before disconnect; its overlay
leaves with the session. The deliberate final host shutdown causes three refused title-art
requests, recorded separately from runtime errors because the asset server has been stopped.

The cadence observation uses a 640×360 Metal canvas and real movement. Hub samples have
121/121 and 120/120 distinct submitted positions; Boneyard has 117/120, all near 60 FPS.
This verifies projection between messages; it is not a general graphics-speed benchmark.
The complete-scene pair uses 1600×900 and exactly the same Tutorial arrival position.
Scenery, player placement, compact ground stamps and rain agree closely; gate and effect
phases differ with capture time. See `reports/visuals-presentation-final.md`.

Earlier accepted receipts remain valid for their recorded revisions: all rooms and services,
complete College/Boneyard reload and stale-tab protection, actual audio playback, phone
inventory/dragging/equipment/services, and a 390×844 Acid Rain tap with no unintended movement.
The full genuine Tutorial journey completes every lesson, potion use and six-enemy survival,
ending with 69 kills. It includes restoration of the unchanged genuinely earned stage-13 save
after a QA helper left combat running; no gameplay values were edited. One unattributed 404
occurred at that neutral-document restore boundary, with no game-page errors or tracked
gameplay 4xx. A normal run also verifies six Fire kills, death, Continue and College return.
These receipts are in `integration-session/`, `integration-services/`, `integration-saves-fixed/`,
`integration-world-saves-fixed/`, `integration-audio-final/`, `integration-responsive/`,
`integration-combat/` and `npcs/fullcombat-browser/`.

Source replay additionally covers thirteen public-Game journeys (6,907 ticks, 386 continuation
checkpoints), five multiplayer journeys (805 inputs, 703 ticks, 127 continuations), and
36 detached-participant operations with twelve staged projections. Enemy display fixtures
cover 1,200 endpoints, 420 family brackets, fifteen membership brackets and ninety fractional
attack samples; secondary fractional fixtures cover seventeen plans and five light boundaries.
Platform trigonometry limits on long unanchored trajectories remain documented in
[DECISIONS.md](DECISIONS.md); whole-game bit-identical trajectories are not claimed.

## Source and cleanup

The permanent published checkout is
`/Users/jarrett/Projects/Solomon Dark/solomon-darker-rust`. The separate legacy `Rust Game`
checkout at `690730d` still owns the M2 launchd preview on port 4180.

Completion cleanup removes temporary task worktrees after their source and evidence are
preserved, including the Website reference/extraction, native fixes, archive checker and
desktop qualification trees. Published diagnostic branches remain available for their
retained evidence. The completed M2 acceptance services on ports 5229, 5230, 9449 and 5383
are stopped; the separate legacy 4180 preview is preserved. Exact paths, active-consumer
checks and removal results are in `release-final/worktree-cleanup.json` and the later
desktop cleanup receipts. Primary/shared checkouts and unrelated work are preserved.

The M2 task Cargo caches, completed desktop staging/dependency outputs and extracted
diagnostic app are removed. NFO's temporary source trees and Cargo/Python caches are also
removed. Installed immutable releases, state, configuration, rollback material, raw package
ZIPs and all acceptance evidence remain. `release-final/task-cache-cleanup.json`,
`nfo-task-cleanup.json` and
`windows-release-5c094aaa-39826304375/completed-windows-worktree-cache-cleanup.json` record
these checks. Future builds recreate their own clean source checkout and cache.

Jarrett explicitly accepted M2 validation on October 20. Each platform's receipt qualifies
that platform; publication requires the actual configured matrix checks to pass.
