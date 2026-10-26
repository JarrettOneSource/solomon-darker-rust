# Game glossary

## Game and state

- **Retail**: the original C++ Solomon Dark 0.72.5 Windows game.
- **College** or **Hub**: the courtyard and its four private rooms: mortuary,
  library, storeroom, and office.
- **Boneyard**: a combat scene with geometry, enemies, waves, loot, and a run identity.
- **Session**: the canonical `GameSession`, including participants, world state,
  inventories, random streams, and run lifecycle.
- **Participant**: one player's identity, wizard, profile, equipment, and combat state.
- **Accepted state**: a completed session state received from the authoritative host.
- **Checkpoint**: the state needed to restore and continue a session.
- **Kernel**: a game rule implemented as a pure function or explicitly stepped state.
- **Golden fixture**: recorded input and expected output replayed by a regression test.
  [Reference data](docs/REFERENCE-DATA.md) describes its provenance.

## Time and numbers

- **Tick**: one 10 ms simulation step; the simulation runs at 100 Hz.
- **Frame**: one display update, independent of the simulation tick.
- **Display frame**: a presentation copy sampled from accepted snapshots.
- **Presentation tick**: the fractional time of a display frame. It cannot advance
  gameplay or be saved as authoritative state.
- **Hub prediction**: bounded local movement between snapshots, reconciled on receipt.
- **Single-precision boundary**: a deliberate narrowing through `float::fround`.
- **Native RNG**: the 55-word lagged-Fibonacci generator, with 30-bit words and
  float divisor 100000.
- **Semantic word**: an RNG word reduced to a bounded integer or scaled float.
- **Collision fallback sampler**: the separate xorshift state consumed by unresolved
  courtyard collision.
- **Lane cap**: 118.75 world units per second per axis.
- **Heading**: one of 24 sprite facings, starting north and increasing clockwise.

## Space and drawing

- **World units**: the coordinate system used by simulation and collision.
  The College courtyard is 2000 × 1024.
- **Segment**: a line obstacle tested against a swept circle.
- **Portal**: a doorway trigger with a destination and scripted transition.
- **Painter**: a world actor's entry in the ordered drawing queue.
- **Registration ordinal**: the actor's stable insertion index.
- **Painter row**: a two-unit row measured relative to the reference actor's feet.
- **Sort bias**: an offset applied before choosing a painter row.
- **Depth band**: a fixed drawing layer outside the actor queue.
- **Bundle record**: sprite metadata containing atlas bounds, logical size,
  registered origin, and collision points.
- **Atlas**: a texture containing sprites referenced by bundle records.
- **Registered origin**: the authored point used to position a sprite.
- **Logical view**: design coordinates mapped into the available canvas.
- **Tint**: a quantized per-channel color multiplier, with separate alpha.
- **Transient**: an effect whose lifetime and painter registration are explicitly owned.

## Items, skills, and Tutorial

- **Ground loot actor**: an Orb, Gold, Sack, or Bonus on a Boneyard floor.
- **Sack**: either a ground item carrier or an inventory container with addressed children.
- **Goodie**: a locked scenery chest opened with a Wizard Key.
- **Loot stream**: the shared RNG used by loot construction, pickups, and Goodie effects.
- **Weld**: a combination of two elemental primary spells.
- **Belt**: three banks of eight slots for skills and items.
- **Tutorial stage**: one of twenty progression states in the authored Tutorial.
- **Narration**: timed Tutorial voice cues; playback does not advance simulation time.
- **Camera lock**: the Tutorial transition from the full scene to the combat rectangle.

## Runtime and services

- **Native authority**: the Rust host that accepts commands and advances the session.
- **Standalone session**: desktop or direct-host play with local persistence.
- **Hosted session**: a session integrated with website accounts and services.
- **Social snapshot**: host-authored membership, chat, and profile information.
- **Directory capability**: access to party listings and invitations through the
  website broker. Direct peer play has no global directory.
- **Game artifact**: a packaged native host, WebAssembly client, tools, assets, and
  manifest consumed by the website.
- **Publication**: the website's advertised deployment identity and readiness.
- **Trunk**: the WebAssembly bundler and development server rooted at `web/`.
