# Architecture

Solomon Darker shares one Rust simulation between its native authority and browser
client. The website supplies accounts, storage, content publication, and editing.

## Ownership

| Module | Responsibility |
| --- | --- |
| `solomon-sim` | Game rules, sessions, players, enemies, spells, inventory, and deterministic state |
| `solomon-assets` | Sprite bundles, scene decoding, navigation inputs, save formats, and validation |
| `solomon-graphics` | Shared WebGL2 rendering, textures, blend modes, and asset decoding |
| `solomon-web` | Browser input, GPU UI, world views, audio, and platform adapters |
| `solomon-loader` | Startup splash using the shared graphics code |
| `solomon-protocol` | Versioned messages, snapshots, commands, and acknowledgements |
| `solomon-launcher` | Native authority, HTTP/WebSocket transport, persistence, and service integration |
| `solomon-modding` | Definition compiler, native Lua execution, and authoring CLI |
| `solomon-save-tools` | Independent WebAssembly entry for Account save import/export |
| `desktop/` | Desktop shell, native process ownership, peer transport, and updates |

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

### Tick ordering

The [session tick](../crates/sim/src/session/tick.rs) owns the sequence across
systems. Participants have a map for lookup and an explicit `participant_order`
for ordered work. Map key order is not a substitute for admission order. Level-up
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

### Runtime entity storage

[`sim::ecs`](../crates/sim/src/ecs.rs) owns admission-ordered component queries.
Domain stores define the components and expose borrowed views to their callers.
Enemy views combine body and controller state; Maggots separate their body from
the controller that owns emergence, admission and navigation. Secondary views include the
ability-specific components present on that entity; loot views combine the body
and pickup reward. Enemy projectiles pair flight and contact state with their motion
program; boss spells pair identity and lifetime with their spell program. Small
particle and effect lifecycles use `ComponentEntities`.

Player-fired projectiles pair common flight state with an Earth, Ether, Fire or
Weld program. Motion updates those components directly. The contact transaction
reads the completed motion state, then commits retained projectile changes after
resolving contacts in game-ID order.

Primary transients use one concrete effect component per entity, so a short fade
does not reserve the space of a persistent spell. Their shared ordered store
preserves the mixed Staff, Weld, Harden and elemental queue. Lifecycle systems
update retained components and admit children at their original position in that
queue; renderers borrow the same components.

The store's private ECS handles identify live component allocations. Game IDs
and painter registrations remain the identities used by contacts, saves, and
network snapshots. Constructing or restoring an owned record enrolls its
components; serialization reads them in admission order. Enemy controller working
copies commit back to the same entity after each action. Projectile and effect
systems update their queried components in place.

The [component-storage decision](DECISIONS.md#component-storage-and-simulation-order)
explains the ordering and persistence constraints.

### Browser frame

For each browser frame:

1. Receive and validate host messages; retain their original receipt timestamps.
2. Apply completed authoritative state and sample input once.
3. Sample one display frame and project aim through its camera.
4. Commit admitted UI commands against accepted state.
5. Draw world geometry, actors, lights, and UI from their owning models.
6. Drain accepted audio events and retire unused GPU resources.

`crates/web/src/lib.rs` owns browser entry points.
`crates/web/src/game.rs` composes the running game.
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

`sim::audio` defines cues and event rules. Browser `GameAudio` owns decoded buffers,
loops, and music channels. Trusted user input resumes the AudioContext. Accepted
actor events supply sounds.
Narration duration remains simulation state even when sound is muted.

`GameAudio` owns one AudioContext, named loop sources, and six resident music
channels. A loop combines requests using the loudest gain. Gain changes preserve
its source, while rate changes recreate it. Accepted one-shot cursors prevent a
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
Browser/desktop player documents use the separate schema 51 codec with its supported
migrations. [Save conversion decisions](DECISIONS.md#save-conversion) explain the
retail container and portable-archive boundary.

Static asset transfer workers retain connection leases and check shutdown between
nonblocking writes. Slow readers cannot occupy the request-framing pool.
[HTTP media](NATIVE-HTTP-MEDIA.md) defines that contract.

## Website integration

A verified game artifact supplies the native host, mounted WebAssembly client,
editor entry, and independent save-tools module. The website validates the manifest,
file membership, and hashes before installation.

Website APIs own accounts, cloud documents, community content, party discovery,
and editing. Game revision/assets and website API/storage mounts are independent.
Direct standalone play retains local saves and peer transport without those services.

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
