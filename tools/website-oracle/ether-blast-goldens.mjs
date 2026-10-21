import { loadKernel, writeFixture } from './oracle.mjs'
const ether = await loadKernel('core-kernels/native-ether-blast.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const chargeCases = []
for (const [start, capacity, fullMana, plane] of [[0, 3, true, false], [0.999, 3, true, false], [2.999, 3, true, false], [1.2, 0, true, false], [1.2, 3, false, false], [1.2, 3, true, true]]) {
  const step = ether.advanceNativeEtherBlastCharge(start, capacity, fullMana, plane)
  chargeCases.push({ start, capacity, fullMana, plane, ...step, released: ether.nativeEtherBlastReleaseCharges(step.charge) })
}
const damageCases = []
for (const charges of [1, 3, 6, 9]) for (const health of [0, 0.001, 20, 9999]) damageCases.push({ charges, health, damage: ether.nativeEtherBlastDamage(charges, health) })
const program = ether.createNativeEtherBlastParticleProgram(random.createNativeRng(7))
const particleRow = particle => [particle.alphaLoss, particle.blue, particle.damping,
  particle.green, particle.offset.x, particle.offset.y, particle.red,
  particle.rotationDegrees, particle.scale, particle.spriteRecord, particle.velocity.x, particle.velocity.y]
const frames = [0, 7.5, 100].map(age => ({ age, frames: program.particles.map(particle => {
  const frame = ether.nativeEtherBlastParticleFrame(particle, age)
  return [frame.alpha, frame.position.x, frame.position.y]
}) }))
writeFixture('ether-blast-goldens.json', 'tools/website-oracle/ether-blast-goldens.mjs', 'Website/frontend/src/game/core-kernels/native-ether-blast.ts', {
  chargeCases, damageCases, program: { particles: program.particles.map(particleRow), rng: program.rng }, frames,
  origin: ether.nativeEtherBlastPulseOrigin({ x: 100.1, y: -200.2 }, { x: 0.6, y: 0.8 }),
})
