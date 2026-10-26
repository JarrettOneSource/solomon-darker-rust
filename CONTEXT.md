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
- **Native goldens**: language-neutral recordings from the retail executable in
  `crates/sim/tests/fixtures/native/`, with the recorded contract in each header.
- **Differential test**: a comparison of independently produced results for the
  same inputs, preserving the fixture's exactness or stated numeric bounds.
- **Smoke**: a browser journey through the served client using ordinary input.

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
- **Scripted speed**: a multiplier on the lane cap.
- **Heading**: one of 24 sprite facings, starting north and increasing clockwise.
- **Random domain**: the reduction that turns a semantic RNG word into a bounded
  integer or scaled float.
- **Gait** and **walk cycle**: pose phases advanced by distance travelled.

## Space and drawing

- **World units**: the coordinate system used by simulation and collision.
  The College courtyard is 2000 × 1024.
- **Segment**: a line obstacle tested against a swept circle.
- **Portal**: a doorway trigger with a destination and scripted transition.
- **Movement plan** (`HubWizardPlan`): a tick's requested delta, requested and
  retained velocities, and movement-active state before collision resolution.
- **Scripted movement**: a walk toward a target without input or retained velocity.
- **Transition**: a doorway's outgoing fade, destination placement and walk-in,
  then incoming fade. The College intro has its own fade rate.
- **Painter**: a world actor's entry in the ordered drawing queue.
- **Registration ordinal**: the actor's stable insertion index.
  The 48 fixed College actors reserve ordinals 0–47; the first player takes 48.
- **Painter row**: a two-unit row measured relative to the reference actor's feet.
- **Sort bias**: an offset applied before choosing a painter row.
- **Depth band**: a fixed drawing layer outside the actor queue.
- **Bundle record**: sprite metadata containing atlas bounds, logical size,
  registered origin, and collision points.
- **Atlas**: a texture containing sprites referenced by bundle records.
- **Registered origin**: the authored point used to position a sprite.
- **Logical view**: design coordinates mapped into the available canvas.
- **Camera origin**: the world-space corner of the visible gameplay region.
  Gameplay starts from a 1600 × 900 minimum logical view; wider or taller canvases
  can reveal more world before regional bounds and zoom are applied. Fixed UI
  stages have their own anchors.
- **Blend mode**: Normal, Multiply, or Add on premultiplied output.
- **Tint**: a quantized per-channel color multiplier, with separate alpha.
- **Starter tints**: hat and robe colors rolled from the owner's base color.
- **Transient**: an effect whose lifetime and painter registration are explicitly owned.

## College ambient life

- **Ambient state**: fountain spray, seal pulse, statue bob, and teacher clock,
  advanced once per tick with a separately owned random stream.
- **Drop**: a fountain particle with an ID, remaining lifetime, and scale.
- **Seal**: the additive floor emblem with thirteen glyph records and one core.
  Its colors follow three-color tracks selected by phase.
- **Statue bob**: the statue's vertical offset and aura drift from a phase in degrees.
- **Teacher cycle**: 847 ticks: cast at 0–267, release at 268–346, then idle.
  The burst contains core, flare, column, and frames; its release index counts cycles.

## Items, skills, and Tutorial

- **Ground loot actor**: an Orb, Gold, Sack, or Bonus on a Boneyard floor.
- **Sack**: either a ground item carrier or an inventory container with addressed children.
- **Goodie**: a locked scenery chest opened with a Wizard Key.
- **Loot stream**: the shared RNG used by loot construction, pickups, and Goodie effects.
  An enemy's private stream selects a candidate category; constructing the candidate
  advances the shared stream.
- **Weld**: a combination of two elemental primary spells.
- **Belt**: three banks of eight slots for skills and items.
- **Tutorial stage**: one of twenty progression states in the authored Tutorial.
- **Narration**: timed Tutorial voice cues; playback does not advance simulation time.
  Cue durations come from PCM lengths converted to 100 Hz ticks.
- **Camera lock**: the Tutorial transition from the full scene to the combat rectangle.
  Bodies and ground Sacks must be safe before its cleanup starts.

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
