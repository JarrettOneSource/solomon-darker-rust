#!/usr/bin/env node
// Exercise the policy's actual geometry sampler, including coincident and tied features.
import { loadKernel, writeFixture } from './oracle.mjs'

const { observeMlBotPolicyGeometry } = await loadKernel('core-server/ml-bot-policy/geometry.ts')
const { withBoneyardGateCollision } = await loadKernel('core-server/boneyard-collision.ts')
const empty = { circles: [], segments: [], polygons: [] }
const mixed = {
  circles: [
    { center: { x: 400, y: 400 }, radius: 30, sourceId: 'grave' },
    { center: { x: 480, y: 400 }, radius: 10 },
  ],
  segments: [
    { start: { x: 100, y: 250 }, end: { x: 700, y: 250 }, radius: 0 },
    { start: { x: 600, y: 350 }, end: { x: 600, y: 650 }, radius: 15, sourceId: 'grave' },
    { start: { x: 400, y: 400 }, end: { x: 400, y: 400 }, radius: 0 },
  ],
  polygons: [
    { points: [{ x: 200, y: 600 }, { x: 360, y: 600 }, { x: 360, y: 800 }, { x: 200, y: 800 }], sourceId: 'grave' },
    { points: [{ x: 650, y: 650 }, { x: 800, y: 650 }, { x: 725, y: 800 }] },
    { points: [] },
    { points: [{ x: 400, y: 400 }] },
  ],
}
const positions = [
  { x: 400, y: 400 }, { x: 400 + 1e-10, y: 400 }, { x: 400 + 2e-9, y: 400 },
  { x: 440, y: 400 }, { x: 280, y: 700 }, { x: 750, y: 700 },
  { x: 25, y: 25 }, { x: 1000, y: 1000 }, { x: -20, y: 600 },
]
const cases = []
for (const [name, collision, gateLeaves] of [
  ['empty', empty, []],
  ['all shape lanes', mixed, []],
  ['moving gate overlay', mixed, [
    { hinge: { x: 700, y: 100 }, tip: { x: 600, y: 300 } },
    { hinge: { x: 900, y: 100 }, tip: { x: 800, y: 280 } },
  ]],
]) {
  for (const position of positions) {
    const world = {
      bounds: { x: 0, y: 0, w: 1100, h: 1100 }, collision, gateLeaves,
      scenerySpellTargets: [{ id: 'grave' }],
    }
    const output = observeMlBotPolicyGeometry(world, position)
    cases.push({ name, bounds: world.bounds, position,
      collision: withBoneyardGateCollision(collision, gateLeaves),
      destructibleIds: ['grave'],
      output: { obstacles: Array.from(output.obstacles), patchAndRays: Array.from(output.patchAndRays) },
    })
  }
}
writeFixture('ml-bot-geometry-goldens.json', 'tools/website-oracle/ml-bot-geometry-goldens.mjs',
  'Website core-server/ml-bot-policy/geometry.ts', { cases })
