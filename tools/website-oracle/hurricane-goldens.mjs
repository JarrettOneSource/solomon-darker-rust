import { loadKernel, writeFixture } from './oracle.mjs'
const hurricane = await loadKernel('core-kernels/native-hurricane.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const programs = []
for (const seed of [0, 1, 44, 77]) {
  let { program, rng } = hurricane.createNativeHurricanePresentation(random.createNativeRng(seed))
  const initial = { program, rng }
  const samples = []
  for (let tick = 1; tick <= 2000; tick++) {
    const charge = tick % 400 < 200 ? Math.fround(0.0015 + (tick % 200) / 200) : 1
    const result = hurricane.stepNativeHurricanePresentation(program, charge, rng)
    program = result.program
    rng = result.rng
    if (tick <= 5 || tick % 50 === 0) samples.push({ tick, program, rng })
  }
  programs.push({ seed, initial, samples })
}
const forces = []
const source = { x: 123.25, y: -54.5 }
for (const charge of [Math.fround(0.0015), 0.49, 0.5, 1]) for (const movementStep of [1, 10, 17]) {
  for (const dx of [0, 0.01, 30, 99.99, 100, 150, 250, 279.999, 280, 300]) for (const dy of [0, 0.01, 25, -100]) {
    const target = { x: source.x + dx, y: source.y + dy }
    forces.push({ charge, movementStep, source, target, force: hurricane.nativeHurricaneOrbitForce(source, target, charge, movementStep) })
  }
}
const damages = []
for (const seed of [0, 1, 44, 77]) {
  let rng = random.createNativeRng(seed)
  const samples = []
  for (const charge of [0.0015, 0.49, 0.5, 0.99, 1]) for (const [minimum, maximum] of [[0, 0], [2.5, 2.5], [1, 10], [0.25, 100.75]]) {
    const result = hurricane.drawNativeHurricaneDamage(rng, charge, minimum, maximum)
    rng = result.rng
    samples.push({ charge, minimum, maximum, result })
  }
  damages.push({ seed, samples })
}
const cadence = []
for (const step of [1, 3, 10, 17]) for (const serial of [0, 1, 9, 10, 100, 9007199254740991]) for (let tick = 0; tick < 20; tick++) {
  cadence.push({ serial, tick, step, due: hurricane.nativeHurricaneMovementDue(serial, tick, step) })
}
writeFixture('hurricane-goldens.json', 'tools/website-oracle/hurricane-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-hurricane.ts', { programs, forces, damages, cadence })
