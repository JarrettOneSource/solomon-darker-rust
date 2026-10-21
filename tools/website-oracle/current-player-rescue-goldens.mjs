#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const rescue = await loadKernel('core-kernels/native-player-rescue.ts')
const sparkle = await loadKernel('core-kernels/native-sparkle.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const checkpoints = new Set([1, 2, 40, 100, 300, 399, 400, 401, 402, 450, 470])
const cases = []
for (const seed of [17, 91]) {
  for (const fraction of [0, .75, 1]) {
    const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 0, transient: 17 } })
    const worldKey = `boneyard:rescue:${seed}`
    let protection = { ...rescue.createNativePlayerRescueProtection(), fraction }
    let rng = random.createNativeRng(seed)
    const frames = []
    for (let tick = 1; tick <= 470; tick += 1) {
      const stepped = rescue.stepNativePlayerRescueProtection(protection, {
        position: { x: tick, y: 80 + tick * .125 }, register: order.register, worldKey,
      }, rng)
      protection = stepped.protection
      rng = stepped.rng
      if (!checkpoints.has(tick)) continue
      frames.push({ tick, fraction: protection.fraction, nextParticleId: protection.nextParticleId,
        nextOrdinal: order.state().nextRegistrationOrdinal.transient,
        rng: [rng.indexA, rng.indexB, ...rng.words],
        damage: [0, .125, 50, 100].map(amount => rescue.nativeRescueDamage(amount, protection)),
        particles: protection.particles.map(particle => ({
          values: [particle.alpha, particle.decay, particle.id, particle.position.x,
            particle.position.y, particle.painterRegistration.registrationOrdinal,
            particle.rotationDegrees, particle.timer, sparkle.nativeSparkleScale(particle.timer)],
          lane: particle.painterRegistration.managerLane, worldKey: particle.worldKey,
        })),
      })
    }
    cases.push({ seed, fraction, worldKey, frames })
  }
}
writeFixture('current-player-rescue-goldens.json', 'current-player-rescue-goldens.mjs',
  'core-kernels/native-player-rescue.ts and native-sparkle.ts', { sourceRevision, cases })
