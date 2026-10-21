#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const deaths = await loadKernel('core-kernels/native-death-animations.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const rngValues = rng => [rng.indexA, rng.indexB, ...rng.words]
const motionValues = motion => [motion.bounceVelocity, motion.height, motion.position.x, motion.position.y,
  motion.rotationDegrees, motion.rotationStepDegrees, motion.velocity.x, motion.velocity.y, motion.verticalVelocity]

const golems = []
const checkpoints = new Set([0, 1, 2, 3, 25, 50, 100, 133, 134, 177, 300, 669, 710, 711, 720])
for (const seed of [17, 91]) {
  for (const enhanced of [false, true]) {
    const born = deaths.createNativeGolemDeathAnimation(random.createNativeRng(seed), { x: 321.25, y: 245.5 }, enhanced)
    let animation = born.animation
    let rng = born.rng
    const frames = []
    for (let tick = 0; tick <= 720; tick += 1) {
      if (tick > 0) {
        const stepped = deaths.stepNativeGolemDeathAnimation(animation, tick, rng)
        animation = stepped.animation
        rng = stepped.rng
      }
      if (checkpoints.has(tick)) frames.push({ tick, rng: rngValues(rng),
        star: [animation.starRotationDegrees, animation.starStepDegrees],
        fragments: animation.fragments.map(fragment => fragment === null ? null : [fragment.life, ...motionValues(fragment)]),
      })
    }
    golems.push({ seed, enhanced, frames })
  }
}

const weapons = []
const weaponCheckpoints = new Set([10, 11, 12, 13, 25, 50, 100, 150, 180])
for (const seed of [17, 91]) {
  for (const headingIndex of [0, 7, 23]) {
    const born = deaths.createNativeDeathWeaponActor({ deathEpoch: 4, headingIndex, id: 9, ownerId: 'hero',
      painterRegistration: { managerLane: 'transient', registrationOrdinal: 83 },
      position: { x: 321.25, y: 245.5 }, tick: 10, weapon: { kind: 'staff', selector: 2 },
    }, random.createNativeRng(seed))
    let actors = [born.actor]
    let rng = born.rng
    const frames = []
    for (let tick = 10; tick <= 180; tick += 1) {
      if (tick > 10) {
        const stepped = deaths.stepNativeDeathWeaponActors(actors, tick, rng)
        actors = stepped.actors
        rng = stepped.rng
      }
      if (weaponCheckpoints.has(tick)) frames.push({ tick, rng: rngValues(rng),
        actors: actors.map(actor => [actor.ageTicks, actor.life, ...motionValues(actor.motion)]),
      })
    }
    weapons.push({ seed, headingIndex, frames })
  }
}
writeFixture('current-death-animation-goldens.json', 'current-death-animation-goldens.mjs',
  'core-kernels/native-death-animations.ts and native-bouncer.ts', { sourceRevision, golems, weapons })
