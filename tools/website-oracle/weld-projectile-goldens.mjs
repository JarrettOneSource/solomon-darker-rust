import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'
import { addWeldVisualSamples } from './weld-visual-rows.mjs'

const weld = await loadKernel('core-kernels/native-weld-primary-runtime.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const groundSpark = await loadKernel('core-kernels/native-weld-ground-spark.ts')
const profiles = JSON.parse(readFileSync(resolve(fixturesDir, 'primary-profile-goldens.json'), 'utf8')).profiles
  .filter(sample => sample.profile.kind === 'weld' && sample.profile.castKind === 'one-shot')
const targets = [0, 1].map(index => ({ active: true, actorFlags: 2, attachment: { x: 0, y: 0 },
  bodyRadius: 15, cellBindingOrder: index, headingDeg: 0, id: `enemy:${index+1}`, kind: 'enemy',
  nativePriority: 0, pendingRemove: false, position: { x: 450 + index*250, y: 100 + index*200 },
  queryLane: 'grid', registrationOrder: index }))
const cases = []
for (const sample of profiles) {
  for (const underpowered of [false, true]) {
    const seed = 17 + cases.length
    const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 500, transient: 70 } })
    const input = { aimDirection: { x: 0.8, y: -0.6 }, firstId: 100,
      origin: { x: 100.25, y: 200.125 }, ownerId: 'hero', primarySkill: sample.profile,
      targets, underpowered, worldKey: 'boneyard:weld' }
    const born = weld.spawnNativeWeldOneShot({ ...input, rng: random.createNativeRng(seed), registerWorldPainter: order.register })
    let projectiles = born.projectiles
    let rng = born.rng
    const frames = [{ tick: 0, projectiles, rng }]
    for (let tick = 1; tick <= 120; tick += 1) {
      const liveTargets = targets.map(target => ({ ...target,
        active: target.id !== 'enemy:1' || tick < 30,
        position: { x: target.position.x + tick / 10, y: target.position.y - tick / 10 } }))
      projectiles = projectiles.map(projectile => {
        const advanced = weld.stepNativeWeldProjectile(projectile, tick < 60 ? liveTargets : liveTargets.slice(1))
        const presentation = weld.stepNativeWeldProjectilePresentation(advanced, rng)
        rng = presentation.rng
        return presentation.projectile
      })
      if ([1, 2, 19, 20, 29, 30, 59, 60, 79, 80, 120].includes(tick)) frames.push({ tick, projectiles, rng })
    }
    const groundSparkFades = []
    if (sample.profile.buildId === 1009) {
      for (const nativeAge of [0, 1, 7, 15, 29]) {
        const result = groundSpark.createNativeWeldGroundSparkFadeProgram({
          projectile: { ...projectiles[0], groundSparkNativeAgeTicks: nativeAge }, rng })
        rng = result.rng
        groundSparkFades.push({ nativeAge, ...result })
      }
    }
    cases.push({ name: `${sample.name}-${underpowered}`, seed, input, frames, groundSparkFades,
      order: order.state().nextRegistrationOrdinal })
  }
}
const privateWords = [0, 1, 0x80000000, 0xffffffff, 731, 1234567].map(value => [value, weld.nativeGroundSparkPrivateWord(value)])
for (const sample of cases) addWeldVisualSamples(sample, sample.frames, frame => frame.projectiles, [0, 20])
writeFixture('weld-projectile-goldens.json', 'tools/website-oracle/weld-projectile-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-weld-primary-runtime.ts', {
    cases, privateWords, audioPlans: Array.from({ length: 10 }, (_, index) => weld.nativeWeldAudioPlan(1000 + index)),
  })
