#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { allLearnedBook, authority, context, factors } from './secondary-inputs.mjs'

const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const stoneskin = await loadKernel('core-kernels/native-stoneskin.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const rngValues = rng => [rng.indexA, rng.indexB, ...rng.words]
const gridCases = []
for (const seed of [17, 91]) {
  let rng = random.createNativeRng(seed)
  const grids = []
  for (let cast = 0; cast < 2; cast += 1) {
    const result = stoneskin.createNativeStoneskinWarp(rng)
    rng = result.rng
    grids.push({ positions: result.positions, rng: rngValues(rng) })
  }
  gridCases.push({ seed, grids })
}

const player = authority(allLearnedBook(), factors(false))
const waveCases = []
const actorValues = actor => [actor.ageTicks, actor.alpha, actor.phase, actor.radius, actor.scale,
  actor.position.x, actor.position.y, actor.velocity.x, actor.velocity.y]
for (const seed of [17, 91]) {
  for (const kind of ['shockwave', 'rescue-shockwave', 'mindblast-shockwave', 'freeze-wave']) {
    for (const gameOver of ['mindblast-shockwave', 'rescue-shockwave'].includes(kind) ? [false, true] : [false]) {
      let state = secondary.spawnNativePlayerRescueShockwave(secondary.createNativeSecondarySimulation(seed), {
        ownerId: 'player', position: { x: 21, y: 34 }, worldKey: player.worldKey,
        lightRegistration: { managerLane: 'actor', registrationOrdinal: 0 },
      })
      state = { ...state, actors: [{ ...state.actors[0], kind, phase: Math.fround(.03),
        velocity: { x: 13, y: -7 }, skillId: kind === 'shockwave' ? 21 : kind === 'freeze-wave' ? 35 : null }] }
      const frames = []
      for (let tick = 1; tick <= 5; tick += 1) {
        state = gameOver ? secondary.stepNativeMindblastPresentation(state, tick)
          : secondary.stepNativeSecondaryAbilities(state, context(player, tick)).state
        frames.push({ tick, actors: state.actors.map(actorValues), camera: state.cameraDisplacements, rng: rngValues(state.rng) })
      }
      waveCases.push({ seed, kind, gameOver, frames })
    }
  }
}

const proposals = [
  ['a', 3, 4, 1], ['b', 2, 0, 1], ['a', -3, -4, 2], ['a', 0, 4, 3],
  ['a', 0, 6, 4], ['c', 0, 0, 4], ['b', 6, 8, 5], ['c', 0, -2, 6],
]
let state = secondary.createNativeSecondarySimulation(17)
const proposalFrames = []
for (const [worldKey, x, y, tick] of proposals) {
  state = secondary.recordNativeSecondaryCameraDisplacement(state, worldKey, { x, y }, tick)
  proposalFrames.push(state.cameraDisplacements)
}
writeFixture('current-secondary-motion-goldens.json', 'current-secondary-motion-goldens.mjs',
  'core-kernels/native-secondary-abilities.ts, native-stoneskin.ts and native-expanding-wave.ts',
  { sourceRevision, player, gridCases, waveCases, proposals, proposalFrames })
