#!/usr/bin/env node
// Source policy acceptance, exact retry draws, collision probes and stock-view rejection.
import { loadKernel, writeFixture } from './oracle.mjs'

const collision = await loadKernel('core-server/boneyard-collision.ts')
const { createNativeRng } = await loadKernel('core-kernels/native-rng.ts')
const bounds = { x: 0, y: 0, w: 2400, h: 1800 }
const object = (typeId, variant = 0) => ({ eid: `obstacle-${typeId}`, typeId, variant, pos: { x: 1200, y: 900 } })
const scene = (objects = [], fences = []) => ({
  name: 'spawn oracle', environmentMode: 0, bounds,
  spawn: { x: 200, y: 200, facingDeg: 180 }, objects, fences,
  sprites: [], roads: [], terrain: [], solomonDig: null,
})
const scenes = [
  scene(), scene([object(2001)]), scene([object(2009, 11)]),
  scene([object(2040, 2)]),
  scene([], [{ eid: 'fence', typeId: 2030, segmentCode: 3,
    points: [{ x: 1000, y: 900 }, { x: 1400, y: 900 }] }]),
]
const positions = [{ x: 1200, y: 900 }, { x: 10, y: -30 }, { x: 2100, y: 1600 }]
const policyBounds = { x: 300, y: 200, w: 1800, h: 1400 }
const retryBounds = { x: 900, y: 600, w: 600, h: 600 }
const focuses = [{ x: 1200, y: 900 }]
const contexts = ['ordinary', 'domain', 'retry', 'unlit', 'never']

function context(kind) {
  return {
    acceptsDomain: kind === 'domain' ? p => p.x >= 1230 : kind === 'never' ? () => false : undefined,
    isOffscreen: p => collision.boneyardSpawnPositionIsOffscreen(p, bounds, focuses),
    isOutsidePolicyBounds: p => p.x < policyBounds.x || p.x > policyBounds.x + policyBounds.w
      || p.y < policyBounds.y || p.y > policyBounds.y + policyBounds.h,
    lightAt: p => kind === 'unlit' ? 0 : p.x >= 1150 && p.x <= 1250 && p.y >= 750 && p.y <= 1050 ? 1 : 0,
    retryBounds: kind === 'retry' ? retryBounds : undefined,
  }
}

const cases = []
for (const [sceneIndex, input] of scenes.entries()) {
  const world = collision.createBoneyardCollisionWorldAllPairsOracle(input)
  for (const position of positions) {
    for (const radius of [8, 25, 65]) {
      const legacy = []
      for (const angle of [null, 17]) {
        try {
          legacy.push({ angle, position: collision.resolveBoneyardSpawnPosition(position, bounds, world, radius, angle ?? undefined) })
        } catch { legacy.push({ angle, position: null }) }
      }
      for (const policy of ['direct', 'dark', 'light', 'offscreen', 'edge']) {
        for (const kind of contexts) {
          // The return value exposes the exact source RNG state. Exhausted searches throw
          // before publishing their speculative immutable RNG state to the caller.
          const seed = 0x71a3 + sceneIndex * 17
          let expected = null
          try { expected = collision.resolveNativeBoneyardSpawnPosition(position, bounds, world, radius, policy, createNativeRng(seed), context(kind)) }
          catch { /* Failure is part of the recorded placement contract. */ }
          cases.push({ sceneIndex, position, radius, policy, context: kind, seed, expected, legacy })
        }
      }
    }
  }
}
const offscreen = []
for (const viewBounds of [bounds, { x: 50, y: 60, w: 300, h: 200 }]) {
  for (const cameraFocuses of [focuses, [{ x: 100, y: 100 }, { x: 2300, y: 1700 }]]) {
    for (const position of [...positions, { x: 1200 + 800 / 1.35, y: 900 }]) {
      offscreen.push({ bounds: viewBounds, focuses: cameraFocuses, position,
        expected: collision.boneyardSpawnPositionIsOffscreen(position, viewBounds, cameraFocuses) })
    }
  }
}
const samples = [8, 25, 65].flatMap(step => [1, 2, 3, 12].map(multiple => ({ step, radius: step * multiple,
  expected: collision.nativeSpawnRingSampleCount(step * multiple, step) })))
writeFixture('spawn-goldens.json', 'tools/website-oracle/spawn-goldens.mjs',
  'Website/frontend/src/game/core-server/boneyard-collision.ts::resolveNativeBoneyardSpawnPosition and resolveBoneyardSpawnPosition',
  { scenes, policyBounds, retryBounds, focuses, cases, offscreen, samples })
