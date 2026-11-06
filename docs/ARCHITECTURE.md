# Architecture

Solomon Darker shares one Rust simulation, game client and GPU interface across
its native application and browser WASM embedding. The website supplies accounts, storage, content publication, and editing.

## Ownership

| Module | Responsibility |
| --- | --- |
| `solomon-sim` | Game rules, sessions, players, enemies, spells, inventory, and deterministic state |
| `solomon-assets` | Sprite bundles, scene decoding, navigation inputs, save formats, and validation |
| `solomon-graphics` | Shared OpenGL/WebGL2 rendering, textures, blend modes, image decoding and text rasterization |
| `solomon-client` | Shared Game, GPU UI, world presentation, audio director and native/browser platform adapters |
| `solomon-loader` | Browser startup splash before the main WASM module loads |
| `solomon-protocol` | Versioned messages, snapshots, commands, and acknowledgements |
| `solomon-launcher` | Native authority, HTTP/WebSocket transport, persistence, and service integration |
| `solomon-modding` | Definition compiler, native Lua execution, and authoring CLI |
| `solomon-save-tools` | Independent WebAssembly entry for Account save import/export |
| `solomon-desktop` | OS window, native session lifetime, encrypted peers, single-instance focus and updates |
| `solomon-storage` | Locked atomic native documents for client saves/preferences and authority checkpoints |

A rule that changes what the game does belongs in `sim`. Asset or save decoding
belongs in `assets`. Drawing and platform I/O belong at the edge. Browser and
native hosts share the same session and scene types.

### Choosing an owner

| Change | Place it with |
| --- | --- |
| A movement, collision, damage, or progression rule | The corresponding simulation kernel and state owner |
| A fixed obstacle, portal, or painter table | Simulation data in authored order |
| A sprite record, texture, or sound file | Asset decoding and the view that consumes it |
| A panel's selection, layout, or focus | Its UI controller, borrowing accepted game state |
| Save parsing or conversion | The existing asset codec and validation boundary |
| A socket, file, browser callback, or process lifetime | The platform adapter that owns the resource |

Choose the smallest interface that owns the whole operation: a pure function for
a value calculation, a stepped state for a lifecycle, a constant table for fixed
data, or a time-sampling function for presentation. For example,
`HUB_COURTYARD_OBSTACLES` supplies simulation positions/radii, while the client's
`OBSTACLE_RECORDS` supplies matching art by index. Neither needs to own the other.

## Session and frame flow

`GameSession` owns participants, world state, inventory, run lifecycle, and random
streams. Its ordered tick composes movement, contacts, casts, world effects, loot,
waves, and rewards. Actor insertion and random consumption are part of that contract.

The native authority runs at 100 Hz and publishes snapshots at up to 20 Hz.
Static browser mode advances the same rules locally with a 250 ms catch-up cap.

Hosted transport recovery belongs to the existing authority membership and
supervisor admission owners (`host/hosted/reconnect.rs`, `host/supervisor/http.rs`
and `host/supervisor/socket.rs`). They keep one live participant under a bounded
lease and mint a fresh purpose-bound ticket; they do not deserialize another
saved world. The client hosted reconnect owner replaces only its native
connection and preserves the shared save/presentation owners. See
[retained hosted authority](DECISIONS.md#retained-hosted-authority-after-transport-interruption)
for deadlines, account binding, readiness, cleanup and deployment exclusions.

### Tick ordering

The [session tick](../crates/sim/src/session/tick.rs) owns the sequence across
systems. Participants are keyed by player ID, with an explicit `participant_order`
for ordered work. Key order is not a substitute for admission order. Level-up
cohorts and secondary-spell player order have their own sorting rules.

Existing actors advance once per global tick, with participant phases between
those advances. Contacts retain producer order; regrouping damage by recipient
can change deaths, rewards, and RNG consumption. Primary spells use
`SpellState::step_primary_players`: advance existing transients/projectiles once,
apply player casts in order, then finish the shared spell phase. The context
resolves each actor's owner, including effects whose caster has left.

Enemy projectile painters created during the world update are committed after
primary casting and before late contacts. Moving registration to a generic
end-of-frame pass changes draw order even if the final actor set is identical.

`actor_physics` owns ordered circle movement and recursive pushes. World queries
borrow the actual participant bodies so a second position store cannot diverge
from canonical players. Checkpoints retain the loaded world's identity so restore
can rebuild immutable geometry and navigation inputs consistently.

Spell-driven body placement uses the same active arena bounds as ordinary
movement. Teleport, Phase, Golem placement, Golem movement and its foot targets
use the full entrance while Open and the combat domain while Locking or Sealed.
Using the full scene after closure can leave a wizard outside the domain required
by enemy routes, making every subsequent enemy birth unreachable. The existing
placement algorithms and strict rejection of impossible births remain unchanged.

### Runtime entity storage

Each population is a plain collection owned by its domain store: a `Vec` in
admission order, an `Option` for a single optional actor, or a `BTreeMap` keyed by
player ID. Removing from the middle of a `Vec` keeps the survivors in order, and
painter registration, contact selection and random-number draws depend on that
order. Callers borrow records as `&T` or `&mut T`.

Enemies are `EnemyActor` records, each holding its family's controller state in
`brain`. Maggots have their own `Vec<EnemyMaggotActor>` beside them; a Maggot's
record carries its emergence, admission and navigation state. An enemy projectile
keeps flight, contact and motion state in one record, and a boss spell pairs its
owner with its spell kind. Ground loot keeps the body and its pickup reward
together. Small particle and effect lists, including authored consumable effects,
are `Vec`s. Spider web modifiers are a `BTreeMap` keyed by the affected player's ID.

Player-fired projectiles are `PrimarySpellProjectile` values: an Earth, Ether, Fire
or Weld record that holds the shared flight state in `common` beside its family's
fields. Motion updates the records in place. The contact transaction reads the
completed motion state, then commits retained projectile changes after resolving
contacts in game-ID order.

Primary transients share one ordered `PrimaryTransients` list, so Staff, Weld,
Harden and elemental effects keep a single update and painter order. A lifecycle
pass steps each effect once; the children an effect creates enter just behind it
and take their first step on the next pass. Renderers borrow the same records.

Secondary effect actors are flat records whose Golem, Golem death and Ether Drain
parts are optional fields. Actors born during the actor pass become live together
after it. Ability clocks and mana reserves are a `BTreeMap` keyed by player ID, so
iteration follows sorted player IDs. Target modifiers keep their application
order. A tick reads the previous modifier values before applying expiry and
damage, and commits player ability transactions to the same records.

Participants are `SessionParticipant` records in a `BTreeMap` keyed by player ID,
with `participant_order` holding admission order. A participant groups identity,
character state, movement and aim, Hub state, lighting, run score and screen
feedback. `PlayerState` keeps inventory, skill choices, derived combat values and
their transaction rules together. The same record carries saves, detach
transactions and private prediction.

Courtyard students are `HubStudentState` records in admission order. All routes
read the preceding population, then collision resolution commits the planned
bodies to the same records. Skorcha is an `Option` inside its appearance schedule.
Fixed scenery animation clocks are plain world state.

Game IDs and painter registrations identify actors in contacts, saves and network
snapshots. Serialization writes each collection in its stored order. Enemy
controllers update a working copy of their record and write it back in one
assignment; projectile and effect passes update their records in place.

The [storage decision](DECISIONS.md#entity-storage-and-simulation-order) explains
the ordering and persistence constraints.

### Client frame

For each client frame:

1. Receive and validate host messages; retain their original receipt timestamps.
2. Apply completed authoritative state and sample input once.
3. Sample one display frame and project aim through its camera.
4. Commit admitted UI commands against accepted state.
5. Draw world geometry, actors, lights, and UI from their owning models.
6. Drain accepted audio events and retire unused GPU resources.

`crates/client/src/game.rs` composes the running game. The native event loop in
`crates/desktop/src/app.rs` owns windows and session retirement;
`crates/client/src/bootstrap.rs` owns browser callbacks and embedding.
Target-selected platform modules provide input, text, clipboard, files, storage,
networking and clocks without a JavaScript runtime in desktop builds.
[Client presentation](CLIENT-PRESENTATION.md) defines interpolation and prediction.

## Rendering and UI

World actors have a stable registration ordinal. The painter planner orders actors
by row, manager lane, and ordinal; fixed depth bands place scenery and effects around
that queue. Art records supply the sprite; simulation tables supply ordering rules.

The College row calculation truncates toward zero at each indicated point:

```text
trunc((trunc(world_y) + trunc(sort_bias) - trunc(reference_y)) / 2)
```

The 48 fixed College actors reserve ordinals before player admission. The north
arch gives the player a ±20 sort bias. Teacher release effects retain their two
transient registrations for the release's lifetime. An actor's draw registration
belongs to its lifecycle, rather than being recreated on each render.

GPU batching preserves that submission order. A target, projection, texture,
blend, color material, sampler, or clip change can require a flush. Nested group
rendering restores the enclosing target and state; retained vertex/index storage
does not permit reordering across those boundaries.

Courtyard art uses source-rectangle clamping so linear filtering cannot read an
adjacent atlas image. Its full source geometry is preserved, as if each record
were an isolated cropped texture. The southern wall and platform bank follows
the separate 1.25 camera scope after courtyard actors and foreground art.

Game menus, HUD, overlays, and the desktop launcher are Rust GPU UI. Egui handles
interaction/layout and tessellation. The shared graphics code submits stock sprites,
bitmap text, shaped font runs, and meshes.

One application layer order controls painting, hit testing, modal ownership, and
controller scope. UI controllers borrow inventory, social, and session models and
emit typed actions for their owners to apply.
[GPU UI](GPU-UI.md) describes the interaction and platform boundaries.

`PlayerPanelSnapshot` borrows the local wizard, inventory, profile, and accepted
feedback. The panel owns selection and focus, then emits typed commands; it does
not maintain an independently editable inventory. Skill-picker state belongs to
the current offer and sequence, so stale responses cannot accept a replaced offer.
Social UI follows host capabilities and membership. Opening chat seals local
gameplay input without implicitly pausing a shared world.

Immutable scenes, collision primitives, skill definitions, and unchanged scenery
target tables share `Arc` storage. Dynamic gate segments and mutable display actors
remain private.

## Assets and audio

Tracked assets include stock atlases, bundle records, scenes, fonts, and audio.
`scripts/verify-client-assets.py` validates complete membership and SHA256 values.
[Reference data](REFERENCE-DATA.md) owns input provenance.

`sim::audio` defines cues and event rules. Shared `GameAudio` owns cues, delayed
sounds, loops and music channels. Browser playback uses WebAudio and media elements;
native playback uses Rodio/CPAL and streaming compressed music. Trusted browser
input resumes the AudioContext. Accepted actor events supply sounds.
Narration duration remains simulation state even when sound is muted.
If the OS reports no output device at startup, native play uses an elapsed-time
audio clock and inactive playback controls. It creates no mixer or audio worker.
Device initialization and decoding failures still propagate; an audio device
connected later is picked up on the next game launch.
CPAL buffer underruns are warnings because the backend recovers them. Device loss,
stream invalidation and failed recovery still stop playback and report the failure.

The browser audio backend owns one AudioContext. The shared director owns named
loop sources and six resident music channels. A loop combines requests using the
loudest gain. Gain changes preserve its source, while rate changes recreate it.
Accepted one-shot cursors prevent a
repeated snapshot from replaying a sound; display interpolation cannot create a
new audio event.

## Transport and persistence

`solomon-protocol` carries canonical values between matched host/client builds.
Snapshot publications are replaceable; commands, accepted audio, save receipts,
and acknowledgements have their own reliable ordering.

Snapshot compaction runs after each connection selects its next publication.
The first packet is complete; later packets may contain lossless changes against
the last document actually written. The receiver reconstructs every packet before
coalescing display state. Unsent replacements cannot become baselines.

The host owns connection generations, admission credentials, input edges,
readiness, pause/resume holds, and durable save receipts. The session owns rules.
A checkpoint writer performs encoding and atomic synced replacement away from
the tick deadline. Welcome and SaveCommitted wait for their required disk writes.

The client Welcome receive deadline ends when the full message has been decoded.
Local validation and durable reconnect-credential storage still precede game
acceptance and the final chunk acknowledgement. A delayed local operation cannot
expire a completed network transfer. Connection failures remain available after
event polling, so rejected commands report the original failure.

`GameSnapshot` borrows state for serialization; `GameCheckpoint` owns state for
restoration. They retain the same serialized shape. Snapshot application validates
the complete candidate before mutating the session and reuses immutable data when
the world identity is unchanged. Detached-player saves project one participant
and the small records that differ, borrowing the shared world instead of cloning
an entire session per peer. Starting a new player cannot reset other participants.

The host prepares one canonical publication for peers sharing a world. Each
connection then chooses and encodes its own next snapshot. A reliable result names
the publication it belongs to. Checkpoint writes keep one owned pending document
and coalesce later revisions, preserving the revision needed by a waiting receipt.

Native server files use `assets::save_document` schema 2; schema 1 is rejected.
Browser/desktop player documents use the separate schema 52 codec with its supported
migrations. [Save conversion decisions](DECISIONS.md#save-conversion) explain the
retail container and portable-archive boundary.

Static asset transfer workers retain connection leases and check shutdown between
nonblocking writes. Slow readers cannot occupy the request-framing pool.
[HTTP media](NATIVE-HTTP-MEDIA.md) defines that contract.

## Website integration

A verified game artifact supplies the native host, WebAssembly client, editor entry,
and independent save-tools module. The website validates the manifest, file
membership, and hashes before installation. The client's URLs are relative, so one
artifact serves at any mount; page metadata names the game mount and, separately,
the Website API and storage mount ([decision](DECISIONS.md#mount-independent-browser-build)).

Website APIs own accounts, cloud documents, community content, party discovery,
and editing. Direct standalone play retains local saves and peer transport without
those services.

The website editor hands Test a disposable authority and restores the editor on
Return. The Account initializes save tools without starting a game, renderer, or
socket. See [Implementation](IMPLEMENTATION.md) for the feature map.

## Authored content and diagnostics

Compiled definitions, admitted asset descriptors, and checkpoints are plain values
in `sim::modding`. Native Lua VMs and executable handles belong to
`solomon-modding`; they are reconstructed from admitted content on restore.

Run archives are private operator diagnostics, separate from player saves.
They serialize complete retained states on an owned worker.
[Run archives](RUN-ARCHIVES.md) defines their format, permissions, and retention.

## Verification

Unit tests exercise rules and failure paths. Regression replays use tracked
recordings, and browser/package journeys check composed behavior.
[AGENTS.md](../AGENTS.md#checks) lists the game gates;
[Release](RELEASE.md) separates source validation, artifacts, installation,
and live acceptance.
