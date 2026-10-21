// Frost birth, clipping/splay, phase changes and the two native draw/painter lanes.
import { loadKernel, point, writeFixture } from './oracle.mjs'
const water = await loadKernel('core-kernels/primary-spell-water.ts')
const header = plan => [plan.coreScale, plan.glintScale, plan.heading, Number(plan.kind === 'over'),
  plan.lifetime, ...point(plan.position), ...point(plan.velocity), plan.worldY]
const draws = plan => plan.draws.map(draw => [draw.alpha, Number(draw.blend === 'add'),
  draw.color.red, draw.color.green, draw.color.blue,
  ['additive-core', 'core', 'glint'].indexOf(draw.pass), ...point(draw.position), draw.rotation,
  draw.scale, Number(draw.sprite === 'glint'), water.packWaterFrostTint(draw.color), water.quantizeWaterFrostAlpha(draw.alpha)])
const emissions = []
for (const widen of [0, 5, 30]) {
  const count = water.waterFrostJetParticleCount(widen)
  const speed = water.waterFrostJetSpeed(widen)
  for (let ordinal = 0; ordinal < count; ordinal += 1) {
    const id = 10 + ordinal
    const emitter = { x: 110, y: 120 }
    const direction = { x: 0.6, y: 0.8 }
    const emission = water.waterFrostJetEmission(emitter, direction, 17, ordinal, id, count, speed, widen)
    emissions.push({ widen, count, speed, ordinal, id,
      direction: point(emission.direction), origin: point(emission.origin), jitter: emission.jitterRadius,
      obstruction: water.waterFrostJetObstruction(emission, { x: 100, y: 100 }, id,
        () => ({ x: 101, y: 101 })),
    })
  }
}
const plans = []
for (const id of [1, 2, 7, 51]) {
  for (const ageTicks of [0, 1, 8, 16, 32, 33]) {
    for (const underpowered of [false, true]) {
      const input = { id, ageTicks, underpowered, direction: { x: 0.6, y: 0.8 },
        origin: { x: 100.2, y: 100.7 }, speed: 4,
        obstructionDistance: id % 2 === 0 ? 15 : null,
        obstructionPoint: id % 2 === 0 ? { x: 110, y: 115 } : null,
      }
      const plan = water.waterFrostJetPlan(input)
      plans.push({ input, header: header(plan), draws: draws(plan),
        lifetimeTicks: water.waterFrostJetLifetimeTicks(id),
        lane: water.waterFrostJetPainterLane(plan.kind).lane,
      })
    }
  }
}
const chains = [0, 1, 8, 32].map(ageTicks => {
  const plan = water.waterFrostJetChainingPlan({ id: 7, ageTicks,
    direction: { x: 0.6, y: 0.8 }, origin: { x: 100.2, y: 100.7 } })
  return { ageTicks, header: header(plan), draws: draws(plan) }
})
writeFixture('water-spell-goldens.json', 'tools/website-oracle/water-spell-goldens.mjs',
  'Website/frontend/src/game/core-kernels/primary-spell-water.ts', { emissions, plans, chains })
