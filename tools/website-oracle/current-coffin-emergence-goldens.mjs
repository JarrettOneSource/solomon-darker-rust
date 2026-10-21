#!/usr/bin/env node
// Current native factory, shared BoulderBit recurrence and actual hidden→rising world birth.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const emergence = await loadKernel('core-kernels/native-coffin-emergence.ts')
const debris = await loadKernel('core-kernels/native-weld-boulder-debris.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const store = await loadKernel('core-server/boneyard-enemy-store.ts')
const work = await loadKernel('core-server/enemies/work.ts')
const construction = await loadKernel('core-server/enemies/construction.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], {
  cwd: websiteDir, encoding: 'utf8',
}).trim()
const bits = value => {
  if (typeof value === 'number') {
    const buffer = Buffer.alloc(8)
    buffer.writeDoubleBE(value)
    return `f64:${buffer.toString('hex')}`
  }
  if (Array.isArray(value)) return value.map(bits)
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, bits(item)]))
  }
  return value
}
const kernels = []
for (const seed of [0, 91, 1073741823]) {
  for (const scale of [.125, 1, 2.75]) {
    for (const enhanced of [false, true]) {
      const position = { x: 137.125, y: -25.75 }
      const radius = 15.5
      const born = emergence.createNativeCoffinEmergence(
        random.createNativeRng(seed), position, scale, radius, enhanced,
      )
      let particles = born.particles
      let rng = born.rng
      const frames = [{ tick: 0, expected: bits({ particles, rng }) }]
      for (let tick = 1; tick <= 400; tick++) {
        particles = particles.flatMap(particle => {
          const stepped = debris.stepNativeBoulderDebrisMotion(particle, tick, rng)
          rng = stepped.rng
          return stepped.particle ? [stepped.particle] : []
        })
        if (tick <= 4 || tick % 20 === 0) frames.push({ tick, expected: bits({ particles, rng }) })
      }
      kernels.push({ seed, scale, position, radius, enhanced, frames })
    }
  }
}
const worlds = []
for (const enhanced of [false, true]) {
  const seed = `current-coffin-emergence:${enhanced}`
  const order = managers.createNativeWorldManagerOrder()
  const context = { tick: 0, players: {}, resolveSpawnIntents: () => [],
    enhancedEffects: enhanced, registerWorldPainter: order.register,
    resolveMovement: request => request.requestedPosition,
    projectileWorldBlocked: () => false,
  }
  const initial = work.createEnemyWork(store.createBoneyardEnemyStore(seed, 50), context, true)
  initial.actors.push(...construction.materializeSpawnIntents(initial, context, [{
    enemyToken: 'COFFIN', nativeTypeId: 1013, flags: [], id: 1,
    locationPolicy: 'anywhere', position: { x: 150, y: 250 }, spawnTick: 0, waveOrdinal: 31,
  }]))
  let world = work.finishEnemyStore(initial, 0)
  const frames = []
  let birthTick = null
  for (let tick = 1; tick <= 1000; tick++) {
    context.tick = tick
    world = store.stepBoneyardEnemyStore(world, context).store
    if (world.deathEffects.length > 0 && birthTick === null) birthTick = tick
    const actor = world.actors[0]
    frames.push({ tick, expected: bits({
      phase: actor.brain.phase, phaseTick: actor.brain.phaseTick,
      phaseTicksRemaining: actor.brain.phaseTicksRemaining,
      effects: world.deathEffects, nextDeathEffectId: world.nextDeathEffectId,
      rng: world.rngState, steering: world.steeringRngState, order: order.state(),
    }) })
    if (birthTick !== null && tick === birthTick + 5) break
  }
  if (birthTick === null) throw new Error('actual Coffin never emerged')
  worlds.push({ enhanced, seed, birthTick,
    frames: frames.filter(frame => frame.tick === 1 || frame.tick >= birthTick - 1) })
}
writeFixture('current-coffin-emergence-goldens.json',
  'tools/website-oracle/current-coffin-emergence-goldens.mjs',
  'native-coffin-emergence.ts + native-weld-boulder-debris.ts + actual stepBoneyardEnemyStore',
  { sourceRevision, kernels, worlds })
console.log(`recorded ${kernels.length} factories with 400 motion ticks and ${worlds.length} actual world births`)
