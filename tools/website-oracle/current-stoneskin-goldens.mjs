#!/usr/bin/env node
// Real Stoneskin casts, physical/magical interception, expiry and retained chips.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { allLearnedBook, authority, context, factors } from './secondary-inputs.mjs'

const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const belt = await loadKernel('core-kernels/native-belt.ts')
const painter = await loadKernel('core-kernels/native-world-manager-order.ts')
const presentation = await loadKernel('renderer/native-secondary-presentation.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], {
  cwd: websiteDir, encoding: 'utf8',
}).trim()
const bits = value => {
  if (typeof value === 'number') {
    const buffer = Buffer.alloc(8)
    buffer.writeDoubleBE(value)
    return `f64:${buffer.toString('hex')}`
  }
  if (Array.isArray(value)) return value.map(bits)
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, bits(item)]))
  }
  return value
}
const actorFields = ['ageTicks', 'alpha', 'damage', 'enhanced', 'endpoint', 'frame', 'freezeTicks',
  'golem', 'hitTargetIds', 'id', 'kind', 'lifetimeTicks', 'lightRegistration', 'midpoint',
  'miscLightAppendOrdinal', 'ownerId', 'painterRegistrations', 'phase', 'position',
  'presentationRng', 'quantity', 'radius', 'rank', 'rotationRadians', 'scale', 'skillId',
  'slowFactor', 'targetId', 'variant', 'velocity', 'worldKey']
const drawFields = ['alpha', 'atlas', 'blend', 'entry', 'offset', 'role', 'rotationRadians',
  'scaleX', 'scaleY', 'tint']
const project = (state, order, received) => ({
  actors: state.actors.map(actor => Object.fromEntries(
    actorFields.filter(key => actor[key] !== undefined).map(key => [key, actor[key]]),
  )),
  // The Source background member is distinct from the registered actor root.
  plans: state.actors.slice(0, 2).map(actor => {
    const plan = presentation.nativeSecondaryPresentationPlan(actor)
    const draws = values => values.map(draw => Object.fromEntries(drawFields.map(key => [key, draw[key]])))
    return { id: actor.id, root: plan.root, worldY: plan.worldY, queueFamily: plan.queueFamily,
      draws: draws(plan.draws), backgroundDraws: draws(plan.backgroundDraws) }
  }),
  events: state.events, rng: state.rng, nextActorId: state.nextActorId,
  nextEventId: state.nextEventId, order: order.state(), received,
  player: state.players.player,
  stoneskinWarp: state.stoneskinWarp,
})

const cases = []
for (const seed of [1, 91]) {
  for (const enhanced of [false, true]) {
    for (const physical of [false, true]) {
      const player = authority(allLearnedBook(), factors(false))
      player.enhancedEffects = enhanced
      player.belt = belt.bindNativeBeltSkill(player.belt, player.skillBook, 46, 0)
      const order = painter.createNativeWorldManagerOrder()
      let state = secondary.createNativeSecondarySimulation(seed)
      const frames = []
      for (let tick = 1; tick <= 1350; tick += 1) {
        const current = { ...player, input: { ...player.input,
          cast: { primary: false, quickbar: tick === 1 ? 0 : null } } }
        state = secondary.stepNativeSecondaryAbilities(state, { ...context(current, tick),
          enhancedEffects: enhanced, registerWorldPainter: order.register }).state
        let received = null
        if ([2, 3, 600, 601].includes(tick)) {
          const damage = secondary.applyNativeSecondaryPlayerDamage(state, 'player', 17.25,
            tick, player.character.position, player.worldKey, { physical, enhancedEffects: enhanced })
          state = secondary.enrollNativeSecondaryPainterOwners(damage.state, order.register)
          received = { absorbedDamage: damage.absorbedDamage, healthDamage: damage.healthDamage }
        }
        if (tick <= 5 || tick % 100 === 0 || [26, 51, 133, 134, 599, 600, 601, 602, 603, 734, 1269, 1270, 1350].includes(tick)) {
          frames.push({ tick, expected: bits(project(state, order, received)) })
        }
      }
      cases.push({ seed, enhanced, physical, player, frames })
    }
  }
}
writeFixture('current-stoneskin-goldens.json', 'tools/website-oracle/current-stoneskin-goldens.mjs',
  'current native-secondary-abilities.ts Stoneskin cast/damage/expiry and native-secondary-presentation.ts',
  { sourceRevision, cases })
console.log(`recorded ${cases.length} actual Stoneskin journeys / ${cases.length * 1350} epochs`)
