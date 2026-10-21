#!/usr/bin/env node
// Real world callbacks: separate line, point, bounds and per-participant view semantics.
import { loadKernel, writeFixture } from './oracle.mjs'

const collision = await loadKernel('core-server/boneyard-collision.ts')
const projectile = await loadKernel('core-server/boneyard-projectile-world.ts')
const feedback = await loadKernel('core-kernels/native-enemy-world-feedback.ts')
const bounds = { x: 0, y: 0, w: 1200, h: 1200 }
const scene = { name: 'projectile world', environmentMode: 0, bounds,
  spawn: { x: 200, y: 200, facingDeg: 180 },
  objects: [{ eid: 'tree', typeId: 2001, variant: 1, pos: { x: 500, y: 500 } }],
  fences: [{ eid: 'fence', typeId: 2030, segmentCode: 0, points: [{ x: 650, y: 300 }, { x: 650, y: 750 }] }],
  roads: [], sprites: [], terrain: [], solomonDig: null }
const world = collision.createBoneyardCollisionWorldAllPairsOracle(scene)
const cases = []
for (const viewports of [[], [{ position: { x: 300, y: 300 }, viewportWidth: 1600, viewportHeight: 900, alternatePlayer: false }],
  [{ position: { x: 100, y: 100 }, viewportWidth: 800, viewportHeight: 600, alternatePlayer: true },
   { position: { x: 1100, y: 1100 }, viewportWidth: 1000, viewportHeight: 1200, alternatePlayer: false }]]) {
  const blocked = projectile.createBoneyardProjectileWorld(bounds, world, viewports)
  const queries = []
  const positions = [{ x: -1, y: 500 }, { x: 0, y: 0 }, { x: 1200, y: 1200 }, { x: 1200.01, y: 500 },
    { x: 500, y: 500 }, { x: 520, y: 500 }, { x: 650, y: 500 }, { x: 300, y: 300 }]
  for (const position of positions) {
    for (const margin of [-10, 0, 25]) {
      for (const kind of ['bounds', 'view']) {
        const query = { kind, position, margin, projectileId: 1 }
        queries.push({ query, expected: blocked(query) })
      }
    }
    for (const radius of [0, 8, 25]) {
      const query = { kind: 'point', position, radius, projectileId: 1 }
      queries.push({ query, expected: blocked(query) })
    }
  }
  for (const [start, end] of [[{ x: 200, y: 500 }, { x: 800, y: 500 }],
    [{ x: 500, y: 500 }, { x: 1300, y: 500 }], [{ x: 50, y: 50 }, { x: 300, y: 50 }]]) {
    for (const nativeExclusionMask of [0, 0x100, 0x700]) {
      const query = { kind: 'line', start, end, nativeExclusionMask, radius: 0, projectileId: 1 }
      queries.push({ query, expected: blocked(query) })
    }
  }
  const gains = viewports.flatMap(viewport => positions.map(position => ({ viewport, position,
    expected: projectile.boneyardProjectilePointGain(bounds, viewport, position, viewport.alternatePlayer) })))
  cases.push({ viewports, queries, gains })
}
const outputs = ['spider-collapse', 'discorporeal-banish', 'faculty-break', 'archer-shatter', 'coffin-break',
  'demon-split', 'heartmonger-shatter', 'imp-split', 'mage-shatter', 'portal-break', 'skeleton-shatter', 'wraith-fragments', 'zombie-collapse']
const impulses = outputs.flatMap(output => [null, 0, 3].map(count => ({ output, count,
  expected: feedback.nativeEnemyWorldFeedbackImpulses(output, count ?? undefined) })))
const transitions = []
for (const state of [{ accumulator: 0, magnitude: 0 }, { accumulator: 0.1, magnitude: 4 }, { accumulator: 3.5, magnitude: 0.001 }]) {
  for (const elapsed of [0, 1, 2, 17, 100, 500]) {
    transitions.push({ state, elapsed, expected: feedback.stepNativeEnemyWorldFeedback(state, elapsed) })
  }
  for (const intensity of [0, 0.05, 0.2, 4]) transitions.push({ state, intensity,
    expected: feedback.applyNativeEnemyWorldFeedback(state, intensity) })
}
writeFixture('projectile-world-goldens.json', 'tools/website-oracle/projectile-world-goldens.mjs',
  'Website/frontend/src/game/core-server/boneyard-projectile-world.ts and core-kernels/native-enemy-world-feedback.ts',
  { scene, cases, impulses, transitions })
