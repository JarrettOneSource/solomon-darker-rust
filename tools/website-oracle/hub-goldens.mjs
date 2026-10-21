#!/usr/bin/env node
// Records differential goldens for the courtyard walk and starter-tint kernels.
//
// `crates/sim/tests/website_goldens.rs` replays these files through the Rust port and requires
// the same numbers bit for bit. `scripts/record-website-goldens.sh` runs every oracle script;
// `SOLOMON_WEBSITE_DIR` overrides the checkout path.
import { loadKernel, writeFixture } from './oracle.mjs'

const hubCollision = await loadKernel('core-kernels/hub-collision.ts')
const playerCharacter = await loadKernel('core-kernels/player-character.ts')
const starter = await loadKernel('core-kernels/native-starter-equipment.ts')
const nativeRng = await loadKernel('core-kernels/native-rng.ts')

const RADIUS = playerCharacter.PLAYER_CHARACTER_RADIUS
const COLLISION_SEED = 0x51a7c011
const HUB_SPAWN = { x: 950.64, y: 164.04 }

const E = [1, 0]
const W = [-1, 0]
const N = [0, -1]
const S = [0, 1]
const NE = [1, -1]
const NW = [-1, -1]
const SE = [1, 1]
const SW = [-1, 1]
const IDLE = [0, 0]
const stage = (movement, ticks) => ({ movement, ticks })

function randomStages(seedValue, totalTicks) {
  let seed = seedValue >>> 0
  const next = () => {
    seed = (Math.imul(seed, 1103515245) + 12345) >>> 0
    return seed >>> 8
  }
  const choices = [E, W, N, S, NE, NW, SE, SW, IDLE]
  const stages = []
  let remaining = totalTicks
  while (remaining > 0) {
    const ticks = Math.min(remaining, 5 + (next() % 36))
    stages.push(stage(choices[next() % choices.length], ticks))
    remaining -= ticks
  }
  return stages
}

function runWalk(name, start, stages) {
  let state = playerCharacter.createPlayerCharacter(
    { discipline: 'arcane', displayName: 'oracle', element: 'ether' },
    start,
  )
  if (!hubCollision.isHubTraversable(state.position, RADIUS)) {
    throw new Error(`${name}: start ${JSON.stringify(start)} is inside a wall`)
  }
  let rngState = COLLISION_SEED
  let fallbackSamples = 0
  const ticks = []
  for (const { movement, ticks: count } of stages) {
    const input = { movement: { x: movement[0], y: movement[1] } }
    for (let index = 0; index < count; index += 1) {
      const plan = playerCharacter.planPlayerCharacterTick(state, input, 1)
      const moved = hubCollision.moveWithHubSegmentsCollisionState(
        state.position,
        plan.delta,
        RADIUS,
        rngState,
        hubCollision.HUB_COURTYARD_SEGMENTS,
      )
      if (moved.rngState !== rngState) fallbackSamples += 1
      rngState = moved.rngState
      state = playerCharacter.commitPlayerCharacterTick(state, plan, moved.position)
      ticks.push([
        state.position.x,
        state.position.y,
        state.velocity.x,
        state.velocity.y,
        state.headingIndex,
        state.gaitDegrees,
        state.walkCyclePrimary,
        rngState,
      ])
    }
  }
  return { name, start: [start.x, start.y], stages, fallbackSamples, ticks }
}

const openStart = [
  { x: 1000, y: 500 },
  { x: 1000, y: 600 },
  { x: 700, y: 500 },
  { x: 1200, y: 700 },
].find((point) => hubCollision.isHubTraversable(point, RADIUS))
if (!openStart) throw new Error('no candidate start is clear of the courtyard segments')

const walks = [
  runWalk('arch-south-and-around', HUB_SPAWN, [
    stage(S, 120), stage(E, 150), stage(S, 150), stage(W, 300), stage(N, 200), stage(IDLE, 150),
  ]),
  runWalk('north-wall-slide', HUB_SPAWN, [
    stage(N, 300), stage(NE, 150), stage(NW, 150), stage(IDLE, 50),
  ]),
  runWalk('courtyard-loop', openStart, [
    stage(E, 100), stage(SE, 100), stage(S, 100), stage(SW, 100),
    stage(W, 100), stage(NW, 100), stage(N, 100), stage(NE, 100), stage(IDLE, 200),
  ]),
  runWalk('analog-input', openStart, [
    stage([0.3, 0.7], 80), stage([2, -1], 80), stage([0.05, 0], 80), stage([-0.5, -0.5], 80), stage(IDLE, 40),
  ]),
  runWalk('random-walk-from-spawn', HUB_SPAWN, randomStages(0x2545f491, 1500)),
  runWalk('random-walk-from-open-ground', openStart, randomStages(0x9e3779b9, 1500)),
]

const owners = ['air', 'earth', 'ether', 'fire', 'water', 'college']
const seeds = [0, 1, 2, 7, 12345, 0x5eed, 2147483647, -1, -12345]
const appearances = []
for (const owner of owners) {
  for (const seed of seeds) {
    const rolled = starter.rollNativeStarterEquipmentAppearance(nativeRng.createNativeRng(seed), owner)
    appearances.push({ owner, seed, primaryTint: rolled.primaryTint, secondaryTint: rolled.secondaryTint })
  }
}

const generator = 'tools/website-oracle/hub-goldens.mjs'
writeFixture(
  'hub-walk-goldens.json',
  generator,
  'Website/frontend/src/game/core-kernels: hub-collision.ts, player-character.ts',
  { scenarios: walks },
)
writeFixture(
  'starter-appearance-goldens.json',
  generator,
  'Website/frontend/src/game/core-kernels: native-starter-equipment.ts, native-rng.ts',
  { cases: appearances },
)
for (const walk of walks) {
  const last = walk.ticks.at(-1)
  console.log(`${walk.name}: ${walk.ticks.length} ticks, ${walk.fallbackSamples} fallback samples, end (${last[0].toFixed(2)}, ${last[1].toFixed(2)}) heading ${last[4]}`)
}
console.log(`open start: (${openStart.x}, ${openStart.y}); ${appearances.length} appearance cases`)
