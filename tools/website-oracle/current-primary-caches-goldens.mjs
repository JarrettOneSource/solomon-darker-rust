// Current Cold Aura cache, constructor/native fade stores and Earth terminal pool cap.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const cold = await loadKernel('core-kernels/native-cold-aura.ts')
const weather = await loadKernel('core-kernels/air-water-spell-actors.ts')
const painter = await loadKernel('renderer/primary-spell-air-water-native.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const progression = await loadKernel('core-kernels/player-progression.ts')
const primary = await loadKernel('core-kernels/native-primary-skill-profile.ts')
const earth = await loadKernel('core-kernels/native-earth-boulder.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const profiles = []
for (const element of ['water', 'earth']) for (let rank = 0; rank <= 4; rank += 1) {
  const created = progression.createPlayerSkillBook({ element, discipline: 'arcane', displayName: 'Cache oracle' })
  const effectiveRanks = [...created.effectiveRanks]
  for (const id of element === 'water' ? [37, 38] : [47]) effectiveRanks[id] = rank
  const skillBook = { ...created, effectiveRanks }
  const profile = primary.nativePrimarySkillProfile(skillBook, progression.playerStatBook(), { damage: 1, manaCost: 1 })
  profiles.push({ element, rank, expected: element === 'water'
    ? [profile.auraRadiusScale, profile.hailThreshold] : [profile.maximumCharge] })
}
const auras = []
for (const feet of [.1, 1, 7, 14, 21, 35]) for (const seed of [0, 17, 91]) {
  const radiusScale = cold.nativeColdAuraRadiusScale(feet)
  const result = weather.createNativeWaterAuraActor(1, 'owner', 'boneyard:oracle', 0,
    { x: 10, y: 20 }, radiusScale, random.createNativeRng(seed))
  const actor = result.actor
  const ages = [...new Set([0, 1, 2, 10, 49.5, actor.durationTicks - 1, actor.durationTicks, actor.durationTicks + 10])]
  auras.push({ feet, seed, radiusScale, queryRadius: cold.nativeColdAuraQueryRadius(radiusScale),
    fields: [actor.alphaDecay, actor.durationTicks, actor.initialRotationDegrees, actor.rotationStepDegrees],
    rng: result.rng, frames: ages.map(ageTicks => ({ ageTicks,
      visual: painter.nativeWaterAuraVisualPlan({ ...actor, ageTicks }) })) })
}
const pools = []
for (const releaseCharge of [1.001, 1.1, 1.15, 1.2, 2.125]) {
  const input = { releaseBaseDamage: 100, releaseCharge, remainingPool: 100, targetHealth: 0, toughness: 1 }
  pools.push({ input, expected: earth.consumeNativeEarthBoulderContact(input) })
}
writeFixture('current-primary-caches-goldens.json', 'tools/website-oracle/current-primary-caches-goldens.mjs',
  'current native-cold-aura.ts, air-water-spell-actors.ts, native-primary-skill-profile.ts, native-earth-boulder.ts',
  { sourceRevision, profiles, auras, pools })
