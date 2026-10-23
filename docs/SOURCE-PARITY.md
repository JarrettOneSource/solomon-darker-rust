# Current Website behavior and ownership

The acceptance reference is the Website at
`fa08ae249ff5563570bdb2a653e2ef9358ce7b83`. The Rust implementation owns gameplay,
the native authority, browser presentation, save tooling and desktop delivery. The Website
retains accounts, cloud storage, content publication and its editors. Its `/game` document
loads the separately built Rust artifact.

This inventory maps the enabled source behavior to its canonical implementation and
verification. [PORT-COMPLETION.md](PORT-COMPLETION.md) records current results and remaining
release checks. A recording retains its actual source revision: unchanged historical
fixtures are not relabeled as current-source recordings.

## Simulation and persistent state

Paths in this table are relative to `crates/sim` unless a crate is named explicitly.

| Behavior | Canonical owner | Verification |
| --- | --- | --- |
| Native random streams, single-precision boundaries and V8 numeric operations | `src/rng.rs`, `src/random_domain.rs`, `src/float.rs` | Native recordings, Website differential fixtures and native/WASM numeric probes; exact scalar comparisons at observed rounding boundaries. |
| College admission, loadout, four rooms, NPCs, conversations and services | `src/hub/`, `src/profile/`, `src/session/` | College/room/economy fixtures and ordinary admission, room, purchase, storage and dialogue journeys. |
| Elemental secondary roots, effective equipment ranks, permanent advanced unlocks and retirement | `src/profile/create.rs`, `src/profile/progression.rs`, `src/profile/runtime.rs` | `create_loadout_goldens`, `progression_goldens`, `player_skill_runtime_goldens`, current saves and actual retirement-to-Create checks. |
| Three eight-slot banks, binding, cycling and item identity | `src/belt.rs`, Session inventory commands | `belt_goldens`, `skill_quickbar_goldens`, current save migration and actual pointer/key/touch bank use. |
| Skill books, Dowsing, addressed equipment, Sack children and persistent painting | `src/economy/`, `src/session/dye.rs`, `src/player_panels/` | Current inventory/Dowsing, skill-book outcome, Sack and dye fixtures; ordinary nested-item, rename and same-session painting journeys. |
| All stock scenes, collision, navigation, director, waves and ordered loot | `src/boneyard/`, `src/loot/`, `solomon-assets` scene compilation | Scene/director/loot/current-ground fixtures and full Tutorial, actual shared runs and authored-scene play. |
| Enemy construction, movement, contacts, attacks, status and retirement | `src/enemies/` | The current enemy program has 132 independent scenarios, 105,000 ticks and 5,511 retained states. Separate presenter, animation, membership and fractional-attack recordings cover display endpoints. |
| Coffin/Portal roots, Wraith flight, fleeing cadence, Mage Dampen and Spider/Cocoon restraint | The existing enemy brains and movement/contact owners | Current enemy and quality fixtures, complete enemy-program continuations and explicit absent/null contact decoding. |
| Elemental and welded primary casts, mana, contact order and retirement | `src/spells/`, `src/session/` | Primary/Weld world programs and contact fixtures; retained clocks, pools, painter identities and random streams remain independently evolving. |
| Secondary casts, summons, growing waves, Frost contacts, Golem, Ether Drain and death effects | `src/secondary/`, `src/session/`, existing world owners | Complete secondary lifetime, contact, membership, quality and presentation fixtures; actual world/Session continuation and restored-save checks. |
| Rescue, harmful contact, poison, direct authored damage and Reverie | `src/player_rescue.rs`, `src/session/rescue.rs`, player combat/runtime owners | Current harmful-contact, player-damage, rescue and actual Session control/contact fixtures. |
| Enhanced Effects, ordered shared flashes, precise headings and terminal clocks | `src/session.rs`, `src/session/`, canonical world state | Current quality/heading/flash/world programs and checkpoint/snapshot continuation. |
| Flame births, channel/fade state and sparse presentation history | `src/spells/weld/flame_*.rs`, protocol history and web display timeline | Current Flame state/history/geometry fixtures, sparse-frame checks and actual retained-history browser rendering. |
| Source schema51, earlier supported versions, native files and portable ZIP | `solomon-assets::website_save`, save codecs, `solomon-save-tools` | 217 current continuation/migration/rejection cases, native bridge fixtures, the real Account WASM ABI and ordinary import/export/cloud-save journeys. |

## Presentation and enabled user flows

| Behavior | Owner | Verification |
| --- | --- | --- |
| World/actor painter order, lights, Terrain, shadows, weather and native pixel centers | `crates/web/src/render.rs`, world renderers and pure simulation presentation plans | Source geometry/material fixtures, real GPU captures and bounded same-state Source/Rust pixel comparisons. |
| Fractional display clocks and retained actor/effect history | `crates/web/src/presentation.rs`, protocol and existing view owners | Snapshot/interpolation fixtures and GPU submissions between accepted host snapshots. Display copies never advance gameplay or save state. |
| Title/Create/Pause/settings, Inventory/SkillBook, native text/art and overlays | `crates/web/src/menu/`, `player_panels/`, `hud/`, `ui/` | Source layout/text/input fixtures and ordinary desktop/phone/control journeys, including modal and hidden-layout lifetimes. |
| Keyboard, mouse, controller and touch ownership | `crates/web/src/input.rs`, menu navigation and mobile controls | Input fixtures and actual trusted keyboard/pointer/touch events; controller samples use native Gamepad-shaped observations. |
| Sound/music, authored audio and source preferences/Hall/playtime | Browser audio/settings/storage owners | Complete audio catalog, event fixtures, actual sound playback, Source storage import and durable reload checks. |
| Learned bots | Existing model, observer and controller | Exact inference and observation/action programs plus a real admitted bot moving, casting and spending resources. |
| Lua definitions, shops, items, skills, actors, scenes, overlays and Console | `solomon-modding`, `sim::modding`, prepared Session extensions and web projections | Source preparation/runtime fixtures, admitted supervisor programs, actual authored shop/skill/scene/save journeys and CLI tests. |

The rank-book confirmation follows `skill-book-feedback.ts`,
`use-skill-book-feedback.ts`, `SkillBookFeedback.tsx` and `MainMenuScene.tsx` at
`fa08ae24`. Their retained native evidence is `0x0056D570..0x0056D7A0`: the
screen-centered MsgBox has a Menu “Skill improved” line, gap 10, a Medium native
skill name followed by “ +1”, both tinted `0xb2b2ff`, and one OKAY button. Every
catalog row 8 through 81 uses the same recovered message-layout kernel.

Source-produced book import fixtures also exercise a freshly reconstructed Hub:
`hub-skorcha.ts::createHubSkorchaAtVariant` starts its gesture timer at zero, and
`game-save-document.ts::decodeHubSkorchaState` accepts the complete interval 0..29.
The file-selection preview recreates a current save, which must restore again
before that first actor tick. Rejecting the constructor's zero timer breaks this
ordinary import/Continue boundary; negative values and 30 remain invalid.

Custom Sack names retain `renderer/hub-inventory/sack-name.ts` and its Pixi
wrapping contract. A detached native font canvas measures the authored `|ÉqÅM`
Arial probe, complete token advances and ink overhang, then shapes each complete
font run into an egui-managed GPU texture. This preserves real baseline/height,
kerning, Arabic joining and emoji clusters. The texture cache belongs to the
current egui context and retires old runs through the same managed GPU lifetime;
panel geometry, clipping and input remain in Rust.

Dialogue navigation follows the actual control owner in `HubDialogueActions.tsx`:
speech, keyed question/command choices, and each selector branch replace their
available controls. `gamepad-menu-navigation.ts::confirm` resolves the current
focusable controls when the prior element is gone. A presentation timestamp is
not that identity: speech/choices/selector transitions can share one fixed input
dispatch time. The Rust navigation scope therefore includes content kind and
speech key or selector kind, preserving real Enter routing at the same time.
An invisible initial Area sizing pass cannot consume that default-focus request;
the request remains pending until the real enabled control is registered.

Native HUD, books, pickers and notices use design coordinates inside a transformed
egui layer. The browser viewport is therefore inverse-transformed before clipping
that Area. The actual `58df78d` landscape selector witness at 844×390 retained only
controls inside the unscaled CSS rectangle: the primary selector near native x974,
the books and the belt disappeared before their layer transform. The current
`hud-browser-58df78d/selector-witness` receipt and screenshot establish that clip
ownership defect; stock geometry, scale and interaction behavior remain unchanged.

The accepted economy receipt owns the result. Independent cursor consumers seed
the current sequence on connection and advance over every newer receipt; restored,
stale, duplicate, rejected and unrelated actions never reopen it. An accepted choice
book or a rank outcome without an eligible skill plays the book stream without a
confirmation. A rank award closes Inventory first and keeps its pause/input hold
until OKAY or Escape dismisses the result. Changing session/world, missing the local
player, entering Game Over or returning to loadout clears the result and stops the
book stream. The modal and the separate floating world text consume the same
accepted outcome independently. These are recovered source contracts; GPU modal
rendering, lifecycle tests and an ordinary earned-book journey are the validation
boundary for the Rust consumer.

## Runtime, Website and delivery

| Boundary | Owner and contract | Verification |
| --- | --- | --- |
| Authoritative shared/private runs | `solomon-launcher`, one canonical Session per world | Real HTTP/WebSocket admission, participant/input ordering, readiness, pause, detach/rejoin and terminal-save integration tests. |
| Large welcomes/saves, recovery and replacement | Existing protocol transfer owners and browser save coordinator | Interrupted/duplicate transfer cases, real held-save and failed-save journeys, matching-revision replacement and same-wizard Continue. |
| Party, Global chat, invitations, cards and consent | Native supervisor plus Website broker | Actual registered two-account social journeys and Source broker/party fixtures. Direct standalone play has the source's own capability boundary. |
| Scores, Memorial and run archives | Native runtime publisher and independent Website APIs/storage | Signed real HTTP publication, ordinary Global Hall views, Memorial codec/file tests and actual archive shutdown tests. |
| Account portability | Website UI and independent Rust save-tools WASM | Real preview/import/CAS/export without a game canvas or socket. |
| Editor and Boneyard library | Website editor; one-use handoff and disposable Rust test authority | Actual local/cloud draft Save/Import/Edit/Delete, Test/Return, cancellation and hidden-frame resume; the editor stays mounted. |
| Website artifact consumption | `scripts/package-hosted-game.py` and Website artifact adapter | Full file membership/SHA256 validation; separate game revision/assets and Website API/storage mounts. No TypeScript gameplay or Node authority is required. |
| Desktop offline, peer play and updates | `desktop/`, native child, `tools/package-desktop.mjs` | Retained shell tests, actual Electron play/save/reopen/peer/close-failure checks, plus the four-platform publication workflow. |
| Reproducible build inputs | Tracked stock files, lockfiles and pinned toolchains | `scripts/verify-client-assets.py`, complete compressed-fixture replay and clean-checkout packaging. A Website checkout is needed only to record new source fixtures. |

Native and browser renderers can differ in final raster rounding. The numeric and pixel
receipts retain their measured limits; a successful game journey is not a claim that every
future simulation state or every GPU pixel has been exhaustively enumerated.
