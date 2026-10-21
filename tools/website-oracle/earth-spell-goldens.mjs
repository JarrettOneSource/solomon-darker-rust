import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], {cwd:websiteDir,encoding:'utf8'}).trim()
const boulder = await loadKernel('core-kernels/native-earth-boulder.ts')
const orientation = await loadKernel('core-kernels/primary-spell-earth-orientation.ts')
const earth = await loadKernel('core-kernels/primary-spell-earth.ts')
const growth = [[0.18, 1, 1, 1], [0.99, 2, 1, 2], [1, 1, 1, 1], [0.3, 1.5, 3, 0]]
  .map(input => [...input, boulder.advanceNativeEarthBoulderCharge(...input)])
const release = [[100, 0.18, 1], [100, 0.3, 1], [100, 1, 1], [100, 2.2, 0.5], [0, 0, 1]]
  .map(input => [...input, boulder.nativeEarthBoulderReleasedDamage(...input)])
const contacts = [[100, 1, 100, 25, 1], [100, 2, 100, 200, 1], [100, 1, 0.0005, 10, 3], [0, 1, 0, 10, 1]]
  .map(([releaseBaseDamage, releaseCharge, remainingPool, targetHealth, toughness]) => {
    const input = { releaseBaseDamage, releaseCharge, remainingPool, targetHealth, toughness }
    const result = boulder.consumeNativeEarthBoulderContact(input)
    // JSON null retains the source's NaN at a zero release pool for an explicit Rust check.
    return { input, ...result }
  })
const orientations = []
for (const direction of [{ x: 0, y: -1 }, { x: 0.6, y: 0.8 }, { x: 1, y: 0 }, { x: 0, y: 1 }]) {
  let value = orientation.EARTH_BOULDER_IDENTITY_ORIENTATION
  for (let step = 1; step <= 10; step += 1) {
    value = step <= 5 ? orientation.earthBoulderHeldOrientationStep(value, direction)
      : orientation.earthBoulderFlightOrientationStep(value, direction, { x: 1.8, y: 2.4 }, 1)
    if (![1, 5, 10].includes(step)) continue
    orientations.push({ direction: [direction.x, direction.y], step, value,
      point: orientation.earthBoulderTransformPoint({ x: 0.3, y: 0.4, z: 0.5 }, value) })
  }
}
const fragment = value => [value.alpha, value.height, value.index, value.position.x, value.position.y,
  value.record, value.rotation, value.scale, value.velocity.x, value.velocity.y, value.verticalVelocity]
const fragments = []
for (const base of [{ charge: 0.18, id: 1, birthTick: 0 }, { charge: 0.3, id: 7, birthTick: 17 },
  { charge: 1, id: 19, birthTick: 21 }, { charge: 2.25, id: 31, birthTick: 22 }]) {
 for (const enhancedEffects of [undefined, false, true]) {
  const seed = {...base,...(enhancedEffects === undefined ? {} : {enhancedEffects})}
  fragments.push({ seed, count: earth.earthImpactFragmentCount(seed.charge),
    lifetime: earth.earthImpactLifetimeTicks(seed),
    frames: [0, 3, 60.5, 400].map(age => ({ age, fragments: earth.earthImpactFragmentsAtAge(seed, age).map(fragment) })),
  })
 }
}
writeFixture('earth-spell-goldens.json', 'tools/website-oracle/earth-spell-goldens.mjs',
  'Website/frontend/src/game/core-kernels: native-earth-boulder.ts, primary-spell-earth-orientation.ts, primary-spell-earth.ts',
  { sourceRevision, growth, release, contacts, orientations, fragments })
