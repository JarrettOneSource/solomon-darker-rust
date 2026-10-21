#!/usr/bin/env node
// Real cast→assembly→damage→retained death, including absence of the original owner.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { allLearnedBook, authority, context, factors } from './secondary-inputs.mjs'

const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const belt = await loadKernel('core-kernels/native-belt.ts')
const painter = await loadKernel('core-kernels/native-world-manager-order.ts')
const presentation = await loadKernel('renderer/native-secondary-golem-presentation.ts')
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
const fields = ['ageTicks', 'alpha', 'damage', 'enhanced', 'endpoint', 'frame', 'freezeTicks',
  'golem', 'golemDeath', 'hitTargetIds', 'id', 'kind', 'lifetimeTicks', 'lightRegistration',
  'midpoint', 'miscLightAppendOrdinal', 'ownerId', 'painterRegistrations', 'phase', 'position',
  'presentationRng', 'quantity', 'radius', 'rank', 'rotationRadians', 'scale', 'skillId',
  'slowFactor', 'targetId', 'variant', 'velocity', 'worldKey']
const planProjection = plan => ({
  root: plan.root, worldY: plan.worldY, queueFamily: plan.queueFamily,
  draws: plan.draws.map(draw => ({ alpha: draw.alpha, atlas: draw.atlas, blend: draw.blend,
    entry: draw.entry, offset: draw.offset, role: draw.role, rotationRadians: draw.rotationRadians,
    scaleX: draw.scaleX, scaleY: draw.scaleY, tint: draw.tint })),
})
const project = (state, order, received) => ({
  deathPlans: state.actors.filter(actor => actor.kind === 'golem-death').flatMap(actor => [
    { id: actor.id, member: 'all', plan: planProjection(presentation.nativeGolemDeathPresentationPlan(actor)) },
    { id: actor.id, member: 0, plan: planProjection(presentation.nativeGolemDeathPresentationPlan(actor, 0)) },
    { id: actor.id, member: 'star', plan: planProjection(presentation.nativeGolemDeathPresentationPlan(actor, 'star')) },
  ]),
  actors: state.actors.map(actor => Object.fromEntries(
    fields.filter(key => actor[key] !== undefined).map(key => [key, actor[key]]),
  )),
  events: state.events, rng: state.rng, nextActorId: state.nextActorId,
  nextEventId: state.nextEventId, order: order.state(), received,
})
const cases = []
for (const enhanced of [false, true]) {
  for (const iron of [false, true]) {
    const player = authority(allLearnedBook(), factors(false))
    player.golemIron = iron
    player.enhancedEffects = enhanced
    player.belt = belt.bindNativeBeltSkill(player.belt, player.skillBook, 45, 0)
    const order = painter.createNativeWorldManagerOrder()
    let state = secondary.createNativeSecondarySimulation(91)
    const frames = []
    for (let tick = 1; tick <= 1030; tick++) {
      if (!iron && tick === 311) state = secondary.removeNativeSecondaryOwner(state, 'player')
      const currentPlayer = { ...player, input: { ...player.input,
        aim: { x: 250, y: 250 }, cast: { primary: false, quickbar: tick === 1 ? 0 : null } } }
      const tickContext = { ...context(currentPlayer, tick), registerWorldPainter: order.register,
        enhancedEffects: enhanced }
      // Only one branch loses the owner; the retained corpse must still run.
      if (!iron && tick > 310) tickContext.players = {}
      state = secondary.stepNativeSecondaryAbilities(state, tickContext).state
      let received = null
      if (tick === 2 || tick === 310) {
        const golem = state.actors.find(actor => actor.kind === 'golem')
        if (!golem) throw new Error(`actual Golem absent before damage tick${tick}`)
        const result = secondary.applyNativeSecondaryGolemDamage(state, golem.id, {
          primaryDamage: 0, reflectablePhysicalSourceInRange: false, secondaryDamage: 100000,
        }, tick, enhanced, order.register)
        state = result.state
        received = { ignored: result.ignored, killed: result.killed,
          ownerId: result.ownerId, reflectedDamage: result.reflectedDamage }
      }
      if (tick <= 5 || tick % 25 === 0 || [26, 51, 101, 310, 311, 312, 313, 324, 325, 487, 980, 1020, 1030].includes(tick)) {
        frames.push({ tick, expected: bits(project(state, order, received)) })
      }
    }
    cases.push({ enhanced, iron, player, frames })
  }
}
writeFixture('current-golem-lifecycle-goldens.json',
  'tools/website-oracle/current-golem-lifecycle-goldens.mjs',
  'native-secondary-abilities.ts actual Golem cast/assembly/damage and retained death managers',
  { sourceRevision, cases })
console.log(`recorded ${cases.length} actual Golem cast/death journeys / 4120 epochs`)
