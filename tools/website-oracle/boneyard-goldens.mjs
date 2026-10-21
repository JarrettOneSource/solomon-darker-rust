#!/usr/bin/env node
// Copies retained stock geometry and records the Website's all-pairs narrow phase.
import { mkdirSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, root, writeFixture } from './oracle.mjs'

const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { STOCK_TUTORIAL_BONEYARD } = await loadKernel('host/stock-tutorial-boneyard.ts')
const collision = await loadKernel('core-server/boneyard-collision.ts')
const gates = await loadKernel('core-kernels/boneyard-gate.ts')
const arena = await loadKernel('core-kernels/boneyard-arena-transition.ts')
const character = await loadKernel('core-kernels/player-character.ts')
const presentation = await loadKernel('renderer/boneyard-render-contract.ts')
const line = await loadKernel('core-kernels/line-obstruction.ts')

const assets = resolve(root, 'web/assets')
mkdirSync(assets, { recursive: true })
writeFileSync(resolve(assets, 'boneyards.json'), `${JSON.stringify({
  source: 'Website/frontend/src/game/host/native-generated-boneyards.ts and stock-tutorial-boneyard.ts',
  templates: NATIVE_GENERATED_BONEYARDS,
  tutorial: STOCK_TUTORIAL_BONEYARD,
})}\n`)

const scene = (objects = [], fences = []) => ({
  name: 'collision oracle', environmentMode: 0,
  bounds: { x: 0, y: 0, w: 1200, h: 1200 },
  spawn: { x: 150, y: 150, facingDeg: 180 },
  objects, fences, sprites: [], roads: [], terrain: [], solomonDig: null,
})
const object = (typeId, variant = 0, overlayVariant = 0) => ({
  eid: `${typeId}-${variant}-${overlayVariant}`, typeId, variant, overlayVariant,
  pos: { x: 500, y: 500 },
})
const shapes = [
  scene([object(2001)]), scene([object(2001, 1)]),
  ...Array.from({ length: 21 }, (_, variant) => scene([object(2009, variant)])),
  scene([object(2029)]), scene([object(2029, 1)]), scene([object(2029, 0, 8)]),
  ...Array.from({ length: 4 }, (_, variant) => scene([object(2040, variant)])),
  scene([object(2061)]),
  ...Array.from({ length: 4 }, (_, segmentCode) => scene([], [{
    eid: `fence-${segmentCode}`, typeId: 2030, segmentCode,
    points: [{ x: 400, y: 500 }, { x: 600, y: 500 }],
  }])),
]
const collisionCases = shapes.map(input => {
  const world = collision.createBoneyardCollisionWorldAllPairsOracle(input)
  const movements = []
  for (const start of [{ x: 450, y: 450 }, { x: 500, y: 450 }, { x: 500, y: 500 }]) {
    for (const delta of [{ x: 0, y: 65 }, { x: 70, y: 70 }, { x: -35, y: 5 }]) {
      const requested = { x: start.x + delta.x, y: start.y + delta.y }
      movements.push({ start, requested, radius: 25,
        position: collision.resolveBoneyardMovement(start, requested, input.bounds, world, 25) })
    }
  }
  const lines = []
  for (const nativeExclusionMask of [0, 0x100, 0x700]) {
    for (const [start, end] of [
      [{ x: 200, y: 500 }, { x: 800, y: 500 }],
      [{ x: 400, y: 350 }, { x: 550, y: 650 }],
      [{ x: 500, y: 500 }, { x: 1400, y: 500 }],
    ]) {
      const point = collision.firstBoneyardLineObstruction(start, end, input.bounds, world, undefined, nativeExclusionMask)
      lines.push({ start, end, nativeExclusionMask, point })
    }
  }
  const paths = []
  for (const ignoredSourceIds of [[], world.circles.flatMap(circle => circle.sourceId ? [circle.sourceId] : [])]) {
    const start = { x: 450, y: 450 }
    const end = { x: 600, y: 600 }
    paths.push({ start, end, ignoredSourceIds, progress: collision.firstBoneyardPathBlockProgress(
      start, end, input.bounds, world, 25, new Set(ignoredSourceIds),
    ) })
  }
  return { scene: input, world, movements, lines, paths }
})

const seed = '00112233445566778899aabbccddeeff'
let gateLeaves = gates.createBoneyardGateLeaves(shapes.at(-2).fences, seed)
const gateTicks = []
for (let tick = 0; tick < 160; tick++) {
  if (tick === 0 || tick === 70) {
    gateLeaves = gateLeaves.map(leaf => gates.applyBoneyardGateContact(leaf, { x: 1, y: tick ? -1 : 1 }))
  }
  gateLeaves = gateLeaves.map(gates.stepBoneyardGateLeaf)
  gateTicks.push(gateLeaves)
}

const walks = [0, 3].map(templateIndex => {
  const input = NATIVE_GENERATED_BONEYARDS[templateIndex].scene
  const world = collision.createBoneyardCollisionWorldAllPairsOracle(input)
  let walkGateLeaves = gates.createBoneyardGateLeaves(input.fences, seed)
  let wizard = character.createPlayerCharacter(
    { discipline: 'arcane', displayName: 'oracle', element: 'ether' }, input.spawn,
  )
  wizard.headingIndex = input.spawn.facingDeg === 0 ? 0 : 12
  const ticks = []
  const stages = [
    { movement: [0, templateIndex === 0 ? 1 : -1], ticks: 300 },
    { movement: [1, 1], ticks: 150 },
    { movement: [-1, 0], ticks: 150 },
    { movement: [0, 0], ticks: 50 },
  ]
  for (const stage of stages) {
    for (let tick = 0; tick < stage.ticks; tick++) {
      const plan = character.planPlayerCharacterTick(wizard, {
        movement: { x: stage.movement[0], y: stage.movement[1] },
      }, 1)
      const requested = { x: wizard.position.x + plan.delta.x, y: wizard.position.y + plan.delta.y }
      const contacts = collision.touchingBoneyardGateLeaves(requested, walkGateLeaves, 25)
      walkGateLeaves = walkGateLeaves.map((leaf, index) => contacts.includes(index)
        ? gates.applyBoneyardGateContact(leaf, plan.delta)
        : leaf)
      walkGateLeaves = walkGateLeaves.map(gates.stepBoneyardGateLeaf)
      const tickCollision = collision.withBoneyardGateCollision(world, walkGateLeaves)
      const position = collision.resolveBoneyardMovement(wizard.position, requested, input.bounds, tickCollision, 25)
      wizard = character.commitPlayerCharacterTick(wizard, plan, position)
      ticks.push([wizard.position.x, wizard.position.y, wizard.velocity.x, wizard.velocity.y,
        wizard.headingIndex, wizard.gaitDegrees, wizard.walkCyclePrimary])
    }
  }
  return { templateIndex, seed, stages, ticks }
})

const arenaCases = [0, 3].map(templateIndex => {
  const input = NATIVE_GENERATED_BONEYARDS[templateIndex].scene
  let state = arena.createBoneyardArenaTransition(input.bounds, input.spawn)
  const ticks = [state]
  state = arena.startBoneyardArenaTransition(state)
  ticks.push(state)
  for (let tick = 0; tick < 402; tick++) {
    state = arena.stepBoneyardArenaTransition(state)
    ticks.push(state)
  }
  return { bounds: input.bounds, spawn: input.spawn, ticks }
})

const cameraCases = []
for (const bounds of [
  { x: 42, y: 65, w: 400, h: 300 },
  NATIVE_GENERATED_BONEYARDS[0].scene.bounds,
]) {
  for (const position of [{ x: 0, y: 0 }, { x: 500, y: 800 }, { x: 5000, y: 5000 }]) {
    for (const scale of [1.2, 1.35]) {
      const view = { width: 1600, height: 900 }
      const camera = presentation.boneyardCamera(position, bounds, view, scale)
      cameraCases.push({ position, bounds, view, scale,
        origin: { x: camera.x - view.width / 2 / scale, y: camera.y - view.height / 2 / scale } })
    }
  }
}

const lineClipCases = [
  [{ x: -50, y: 100 }, { x: 800, y: 100 }],
  [{ x: 100, y: -50 }, { x: 100, y: 800 }],
  [{ x: -50, y: -50 }, { x: -10, y: 500 }],
  [{ x: 0, y: 0 }, { x: 0, y: 0 }],
].map(([start, end]) => {
  const bounds = { x: 0, y: 0, w: 500, h: 500 }
  return { start, end, bounds, clipped: line.clipLineToBounds(start, end, bounds) }
})

writeFixture('boneyard-goldens.json', 'tools/website-oracle/boneyard-goldens.mjs',
  'Website/frontend/src/game/core-server/boneyard-collision.ts; core-kernels/boneyard-gate.ts, boneyard-arena-transition.ts, player-character.ts',
  { collisionCases, seed, gateTicks, arenaCases, cameraCases, lineClipCases, walks })
console.log(`copied ${NATIVE_GENERATED_BONEYARDS.length} Boneyards and tutorial; recorded ${collisionCases.length} shapes and ${walks.reduce((n, walk) => n + walk.ticks.length, 0)} walk ticks`)
