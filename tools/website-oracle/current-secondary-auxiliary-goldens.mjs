#!/usr/bin/env node
// Real casts retain ordinary motes and distinguish a living blocked player from death.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { allLearnedBook, authority, context, factors } from './secondary-inputs.mjs'
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const belt = await loadKernel('core-kernels/native-belt.ts')
const painter = await loadKernel('core-kernels/native-world-manager-order.ts')
const presentation = await loadKernel('renderer/native-secondary-presentation.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const bits = value => {
  if (typeof value === 'number') {
    const buffer = Buffer.alloc(8); buffer.writeDoubleBE(value)
    return `f64:${buffer.toString('hex')}`
  }
  if (Array.isArray(value)) return value.map(bits)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, bits(item)]))
  return value
}
const actorFields = ['ageTicks', 'alpha', 'damage', 'enhanced', 'endpoint', 'frame', 'freezeTicks',
  'golem', 'hitTargetIds', 'id', 'kind', 'lifetimeTicks', 'lightRegistration', 'midpoint',
  'miscLightAppendOrdinal', 'ownerId', 'painterRegistrations', 'phase', 'position',
  'presentationRng', 'quantity', 'radius', 'rank', 'rotationRadians', 'scale', 'skillId',
  'slowFactor', 'targetId', 'variant', 'velocity', 'worldKey']
const drawFields = ['alpha', 'atlas', 'blend', 'entry', 'offset', 'role', 'rotationRadians', 'scaleX', 'scaleY', 'tint']
const project = (state, order) => ({
  actors: state.actors.map(actor => Object.fromEntries(actorFields.filter(key => actor[key] !== undefined).map(key => [key, actor[key]]))),
  plans: state.actors.slice(0, 2).map(actor => {
    const plan = presentation.nativeSecondaryPresentationPlan(actor)
    return { id: actor.id, root: plan.root, draws: plan.draws.map(draw => Object.fromEntries(drawFields.map(key => [key, draw[key]]))) }
  }),
  player: state.players.player, events: state.events, rng: state.rng,
  nextActorId: state.nextActorId, nextEventId: state.nextEventId, order: order.state(),
})
const cases = []
for (const skillId of [12, 79]) {
  for (const enhanced of [false, true]) {
    for (const mode of ['eligible', 'blocked-alive', 'dead']) {
      const player = authority(allLearnedBook(), factors(false))
      player.enhancedEffects = enhanced
      player.belt = belt.bindNativeBeltSkill(player.belt, player.skillBook, skillId, 0)
      const order = painter.createNativeWorldManagerOrder()
      let state = secondary.createNativeSecondarySimulation(91)
      const frames = []
      for (let tick = 1; tick <= 900; tick += 1) {
        const current = { ...player, alive: tick < 50 || mode !== 'dead',
          eligible: tick < 50 || mode === 'eligible',
          input: { ...player.input, cast: { primary: false, quickbar: tick === 1 ? 0 : null } } }
        state = secondary.stepNativeSecondaryAbilities(state, { ...context(current, tick),
          enhancedEffects: enhanced, registerWorldPainter: order.register }).state
        if ([1, 2, 3, 10, 49, 50, 51, 80, 100, 200, 400, 599, 600, 601, 700, 899, 900].includes(tick)) {
          frames.push({ tick, expected: bits(project(state, order)) })
        }
      }
      cases.push({ skillId, enhanced, mode, player, frames })
    }
  }
}
writeFixture('current-secondary-auxiliary-goldens.json', 'tools/website-oracle/current-secondary-auxiliary-goldens.mjs',
  'current native-secondary-abilities.ts Planewalker mote births/retirement and alive/eligible toggle authority',
  { sourceRevision, cases })
