import { loadKernel, writeFixture } from './oracle.mjs'
const debris = await loadKernel('core-kernels/native-weld-boulder-debris.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const cases = []
for (const scale of [0, 0.18, 0.55, 1.2, Number.NaN]) {
  const born = debris.createNativeWeldBoulderContactDebrisProgram({ scale, rng: random.createNativeRng(7) })
  const lifecycles = []
  if (Number.isFinite(scale)) for (const enhanced of [false, true]) {
    let particle = debris.createNativeWeldBoulderDebrisParticle(born.debris[0], enhanced)
    let rng = born.rng
    const frames = [{ age: 0, particle, rng }]
    for (let age = 1; age <= 400 && particle; age += 1) {
      const step = debris.stepNativeWeldBoulderDebrisParticle(particle, 17 + age, rng)
      particle = step.particle; rng = step.rng
      if ([1, 2, 3, 20, 60, 120, 200].includes(age) || !particle) frames.push({ age, particle, rng })
    }
    lifecycles.push({ enhanced, frames })
  }
  cases.push({ scale, born, lifecycles })
}
writeFixture('boulder-contact-debris-goldens.json', 'tools/website-oracle/boulder-contact-debris-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-weld-boulder-debris.ts', { cases })
