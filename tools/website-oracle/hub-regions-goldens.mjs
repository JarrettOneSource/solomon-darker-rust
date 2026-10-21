#!/usr/bin/env node
// Records the Hub's region tables and doorway kernels, and the courtyard's fixed painter
// table with the north-arch sort bias.
import { loadKernel, point, writeFixture } from './oracle.mjs'

const regions = await loadKernel('core-kernels/hub-regions.ts')
const membership = await loadKernel('core-kernels/native-hub-world-membership.ts')

const generator = 'tools/website-oracle/hub-regions-goldens.mjs'
const segment = ({ x1, y1, x2, y2 }) => [x1, y1, x2, y2]

const regionTable = regions.HUB_REGION_IDS.map((id) => {
  const definition = regions.HUB_REGION_DEFINITIONS[id]
  return {
    id,
    nativeId: definition.nativeId,
    size: [definition.width, definition.height],
    segmentCount: definition.segments.length,
    incomingFadeRate: regions.HUB_INCOMING_FADE_RATES[id],
  }
})
const portals = regions.HUB_PORTALS.map((portal) => ({
  source: portal.source,
  destination: portal.destination,
  trigger: segment(portal.trigger),
  scriptedSpeed: portal.scriptedSpeed,
  scriptedTarget: point(portal.scriptedTarget),
  preserveContactX: portal.preserveContactX === true,
}))

// Every ordered pair of regions: whether it is a doorway edge, and where the arrival stands.
const edges = []
const placements = []
for (const source of regions.HUB_REGION_IDS) {
  for (const destination of regions.HUB_REGION_IDS) {
    const edge = regions.isHubTransitionEdge(source, destination)
    edges.push([source, destination, edge ? 1 : 0])
    if (!edge) continue
    const placement = regions.hubIncomingPlacement(source, destination)
    placements.push({
      source,
      destination,
      position: point(placement.position),
      scriptedSpeed: placement.scriptedSpeed,
      scriptedTarget: point(placement.scriptedTarget),
    })
  }
}

// A grid across (and just outside) each region: which doorway a player-sized wizard touches
// there, and the scripted target the transition it starts walks toward.
const GRID_STEP = 32
const portalHits = []
let portalMisses = 0
for (const [regionIndex, id] of regions.HUB_REGION_IDS.entries()) {
  const { width, height } = regions.HUB_REGION_DEFINITIONS[id]
  for (let y = -64; y <= height + 64; y += GRID_STEP) {
    for (let x = -64; x <= width + 64; x += GRID_STEP) {
      const position = { x: x + 0.5, y: y + 0.5 }
      const portal = regions.hubPortalAt(id, position)
      if (!portal) {
        portalMisses += 1
        continue
      }
      const participant = { ...regions.createHubParticipantState(), region: id }
      const { transition } = regions.beginHubTransition(participant, portal, position)
      portalHits.push([
        regionIndex,
        position.x,
        position.y,
        regions.HUB_PORTALS.indexOf(portal),
        transition.scriptedTarget.x,
        transition.scriptedTarget.y,
      ])
    }
  }
}

// Scripted walks toward a doorway target: the plan each tick, applied without collision.
const scriptedWalks = []
const WALKS = [
  ['mortuary-exit', [60.5, 460.5], [32, 363], 0.65],
  ['library-exit', [1950, 520], [2057.5, 460.5], 0.45],
  ['storeroom-exit', [627.5, 160], [627.5, -1000], 0.45],
  ['office-return', [512, 924], [512, 2024], 1],
  ['courtyard-reentry', [63, 413], [123, 488], 1],
  ['already-there', [100, 100], [100, 100], 1],
]
for (const [name, start, target, speed] of WALKS) {
  let position = { x: start[0], y: start[1] }
  const steps = []
  for (let tick = 0; tick < 200; tick += 1) {
    const plan = regions.planHubScriptedMovement({ position }, { x: target[0], y: target[1] }, speed)
    position = { x: position.x + plan.delta.x, y: position.y + plan.delta.y }
    steps.push([
      position.x,
      position.y,
      plan.requestedVelocity.x,
      plan.requestedVelocity.y,
      plan.movementActive ? 1 : 0,
    ])
    if (!plan.movementActive) break
  }
  scriptedWalks.push({ name, start, target, speed, steps })
}

// The courtyard's fixed painters and the north-arch sort bias around the rectangle's edges.
const obstacles = membership.NATIVE_HUB_COURTYARD_OBSTACLES.map((obstacle) => ({
  id: obstacle.id,
  position: point(obstacle.position),
  radius: obstacle.radius,
  sortBias: obstacle.sortBias,
}))
const statue = membership.NATIVE_HUB_COLLEGE_STATUE
const rect = membership.NATIVE_HUB_NORTH_ARCH_BIAS_RECT
const biasSamples = []
const xs = [850, 874, 874.5, 880, 900, 950, 1000, 1030, 1030.5, 1031, 1050]
const ys = [20, 34, 34.5, 50, 100, 150, 180, 180.5, 181, 200]
const headings = [-25, -1, 0, 1, 5, 11, 12, 13, 22, 23, 24, 35, 47, 48]
for (const x of xs) {
  for (const y of ys) {
    for (const heading of headings) {
      biasSamples.push([x, y, heading, membership.nativeCourtyardPlayerSortBias({ x, y }, heading)])
    }
  }
}

writeFixture(
  'hub-regions-goldens.json',
  generator,
  'Website/frontend/src/game/core-kernels: hub-regions.ts, hub-private-room-layout.ts',
  {
    collegeIntroFadeRate: regions.HUB_COLLEGE_INTRO_FADE_RATE,
    outgoingFadeRate: regions.HUB_OUTGOING_FADE_RATE,
    regions: regionTable,
    portals,
    edges,
    placements,
    gridStep: GRID_STEP,
    portalHits,
    portalMisses,
    scriptedWalks,
  },
)
writeFixture(
  'hub-painter-goldens.json',
  generator,
  'Website/frontend/src/game/core-kernels: native-hub-world-membership.ts',
  {
    fixedPainterIds: [...membership.NATIVE_HUB_FIXED_ACTOR_PAINTER_IDS],
    obstacles,
    statue: { id: statue.id, position: point(statue.position), radius: statue.radius, sortBias: statue.sortBias },
    northArchBiasRect: [rect.left, rect.top, rect.right, rect.bottom],
    biasSamples,
  },
)
console.log(`regions: ${portals.length} portals, ${placements.length} placements, ${portalHits.length} portal hits, ${portalMisses} misses, ${scriptedWalks.length} scripted walks`)
console.log(`painter: ${membership.NATIVE_HUB_FIXED_ACTOR_PAINTER_IDS.length} fixed ids, ${obstacles.length} obstacles, ${biasSamples.length} bias samples`)
