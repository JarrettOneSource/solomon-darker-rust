import { loadKernel, writeFixture } from './oracle.mjs'
const secondary = await loadKernel('core-server/native-secondary-world.ts')
const collision = await loadKernel('core-server/boneyard-collision.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const rings = []
const requested = { x: 0.25, y: -0.5 }
for (const seed of [0, 1, 15, 44, 77, 121]) for (const radius of [25, 40]) {
  for (const blockedRadius of [0, 30, 100, 500]) for (const minimumX of [-200, 0.25, 100]) {
    const result = secondary.resolveNativeCollisionAdjustedPosition(random.createNativeRng(seed), requested, radius, position => {
      const dx = position.x - requested.x
      const dy = position.y - requested.y
      return position.x >= minimumX && dx * dx + dy * dy >= blockedRadius * blockedRadius
    })
    rings.push({ seed, radius, blockedRadius, minimumX, requested, result })
  }
}
const teleports = []
for (const seed of [0, 1, 15, 44, 77, 121]) {
  for (const bounds of [{ x: 0, y: 0, w: 1000, h: 800 }, { x: -500, y: -700, w: 1200, h: 1000 }, { x: 0, y: 0, w: 200, h: 800 }]) {
    const bodySets = bounds.w <= 200 ? [[]] : [[],
      [{ position: { x: bounds.x + 500, y: bounds.y + 400 }, radius: 25 }],
      [{ position: { x: bounds.x + 100, y: bounds.y + 100 }, radius: 80 }, { position: { x: bounds.x + bounds.w - 100, y: bounds.y + bounds.h - 100 }, radius: 80 }],
    ]
    const scene = { name: 'Placement oracle', environmentMode: 0, bounds, spawn: { position: { x: 0, y: 0 }, facingDegrees: 0 }, objects: [], sprites: [], roads: [], fences: [], terrain: [], solomonDig: null }
    for (const bodies of bodySets) {
      const result = secondary.resolveBoneyardNativeTeleport(random.createNativeRng(seed), { bodies, bounds, collision: collision.createBoneyardCollisionWorldAllPairsOracle(scene) })
      teleports.push({ seed, bounds, bodies, result })
    }
  }
}
writeFixture('secondary-geometry-goldens.json', 'tools/website-oracle/secondary-geometry-goldens.mjs',
  'Website/frontend/src/game/core-server/native-secondary-world.ts', { rings, teleports })
