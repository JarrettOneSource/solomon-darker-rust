#!/usr/bin/env node
// Records current Source modifier contacts, private FadeLightning births and lifetimes.
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { allLearnedBook, authority, context, factors } from './secondary-inputs.mjs'

const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const presentation = await loadKernel('renderer/native-secondary-presentation.ts')
const air = await loadKernel('core-kernels/native-air-presentation.ts')
const painter = await loadKernel('core-kernels/native-world-manager-order.ts')
const lights = await loadKernel('core-kernels/native-boneyard-light-model.ts')
const { NativeElectricBurnArcView } = await loadKernel('renderer/native-electric-burn-arc-view.ts')
const { Container, Texture } = await import(pathToFileURL(resolve(websiteDir, 'frontend/node_modules/pixi.js/lib/index.mjs')).href)
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const bits = value => {
  if (typeof value === 'number') {
    const bytes = Buffer.alloc(8); bytes.writeDoubleBE(value)
    return `f64:${bytes.toString('hex')}`
  }
  if (Array.isArray(value)) return value.map(bits)
  if (ArrayBuffer.isView(value)) return Array.from(value, bits)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, bits(item)]))
  return value
}
const targets = [
  [7, 123.25, -54.5], [90, 173.25, -54.5], [2, 73.25, -54.5],
  [8, 123.25, 45.5], [5, 123.25, -154.5], [60, 323.25, -54.5],
].map(([id, x, y], index) => ({ id, family: 'SKELETON', position: { x, y }, radius: 20, scale: 1,
  shieldHealth: 0, nativeFlags: 2, registrationOrder: 20 - index,
  lightRegistration: { managerLane: 'actor', registrationOrdinal: 100 + index } }))
const electric = (arcCount, ticks = 3) => ({ arcCount, damagePerTick: 1.25, ownerId: 'player',
  sourceActorId: 431, stunFactor: .625, ticks })
const steamed = (explodeRadius, ticks = 3) => ({ damagePerTick: 2.75, emberDamage: 4.25,
  emberFragments: 3, explodeDamage: 9.5, explodeRadius, ownerId: 'player', sourceActorId: 432, ticks })
const frost = { frostBurnDamagePerTick: .375, frostBurnOwnerId: 'player', frostBurnSkillId: 35,
  frostBurnSourceActorId: 433, frostBurnTicks: 3 }
const actor = () => ({ ageTicks: 0, alpha: 0, damage: .75, enhanced: false,
  endpoint: { x: 0, y: 0 }, frame: 0, freezeTicks: 0, golem: null, hitTargetIds: [],
  id: 31, kind: 'electric-burn', lifetimeTicks: 5, lightRegistration: targets[0].lightRegistration,
  midpoint: { x: 0, y: 0 }, miscLightAppendOrdinal: null, ownerId: 'player', painterRegistrations: [],
  phase: 0, position: targets[0].position, presentationRng: null, quantity: 0, radius: 0, rank: 1,
  rotationRadians: 0, scale: 1, skillId: 50, slowFactor: 0, targetId: 7, variant: 0,
  velocity: { x: 0, y: 0 }, worldKey: 'boneyard:oracle' })

const cases = []
for (const seed of [0, 1, 2, 3, 4, 17, 77, 319]) {
  for (const enhancedEffects of [false, true]) cases.push({
    name: `mixed-${seed}-${enhancedEffects}`, seed, enhancedEffects,
    patches: [{ id: 7, patch: { electricBurn: electric(2), steamed: steamed(80), ...frost } },
      { id: 90, patch: { prismaticTicks: 2 } }],
  })
}
cases.push(
  { name: 'no-arcs-no-steam-payload', seed: 77, patches: [{ id: 7, patch: { electricBurn: electric(0), steamed: steamed(0) } }] },
  { name: 'last-tick-prism-expires-before-hit', seed: 91, patches: [{ id: 7, patch: { electricBurn: electric(6, 1), prismaticTicks: 1 } }] },
  { name: 'reverse-equal-distance-registration', seed: 91, reverse: true, patches: [{ id: 7, patch: { electricBurn: electric(2) } }] },
  { name: 'absent-target-does-not-sample', seed: 91, absentFrom: 1, patches: [{ id: 7, patch: { electricBurn: electric(2), steamed: steamed(80), ...frost } }] },
  { name: 'target-retires-after-first-pulse', seed: 91, absentFrom: 2, patches: [{ id: 7, patch: { electricBurn: electric(2), steamed: steamed(80), ...frost } }] },
)
for (const seed of [0, 1, 2, 3, 4, 17, 77, 319]) cases.push({ name: `magic-trap-burn-${seed}`, seed,
  actor: actor(), patches: [{ id: 7, patch: { prismaticTicks: 3 } }] })

let actorSamples = 0
let contactSamples = 0
const presentations = []
const arcViews = []
function recordArcView(actor) {
  const root = new Container()
  // Texture pixels do not participate in the actual view's mesh/mask/painter construction.
  const view = new NativeElectricBurnArcView(actor, root,
    { ribbon: Texture.EMPTY, branches: [Texture.EMPTY, Texture.EMPTY] }, actor.enhanced)
  for (const enhancedEffects of [actor.enhanced, !actor.enhanced]) {
    view.update(actor, enhancedEffects)
    arcViews.push({ actor, enhancedEffects, expected: bits({
      painters: view.painterLayers(0), primitiveCount: view.primitiveCount,
    }) })
  }
  view.destroy()
  root.destroy()
}
for (const entry of cases) {
  let state = secondary.createNativeSecondarySimulation(entry.seed)
  if (entry.actor) state = { ...state, actors: [entry.actor], nextActorId: entry.actor.id + 1 }
  for (const row of entry.patches) state = secondary.applyNativeSecondaryTargetEffect(state, 'boneyard:oracle', row.id, row.patch)
  const player = authority(allLearnedBook(), factors(false))
  player.input = { ...player.input, cast: { ...player.input.cast, quickbar: null } }
  const selected = entry.reverse ? [...targets].reverse() : targets
  const order = painter.createNativeWorldManagerOrder()
  entry.frames = []
  for (let tick = 1; tick <= 8; tick += 1) {
    const live = selected.filter(target => !(target.id === 7 && tick >= (entry.absentFrom ?? Infinity)))
    const stepped = secondary.stepNativeSecondaryAbilities(state, { ...context(player, tick),
      ...(entry.enhancedEffects === undefined ? {} : { enhancedEffects: entry.enhancedEffects }),
      registerWorldPainter: order.register,
      targetExists: (key, id) => key === player.worldKey && live.some(target => target.id === id),
      target: (_key, id) => live.find(target => target.id === id) ?? null,
      targets: (_key, center, radius) => live.filter(target => Math.hypot(target.position.x - center.x, target.position.y - center.y) <= radius + target.radius),
    })
    state = stepped.state
    contactSamples += stepped.damage.length
    actorSamples += state.actors.length
    entry.frames.push({ tick, expected: bits({ actors: state.actors, damage: stepped.damage,
      effects: state.targetEffects, events: state.events, rng: state.rng,
      nextActorId: state.nextActorId, nextEventId: state.nextEventId }) })
    if (entry.name === 'mixed-77-true' || entry.name === 'mixed-77-false') {
      for (const row of state.actors.filter(row => ['electric-burn-flare', 'electric-burn-arc'].includes(row.kind))) {
        const plan = presentation.nativeSecondaryPresentationPlan(row, tick + .5, .75)
        const body = row.kind === 'electric-burn-arc' ? air.buildNativeAirLightningFactoryPlan({
          ageTicks: row.ageTicks, birthTick: row.phase, id: row.id, enhancedEffects: row.enhanced,
          endpoint: { x: row.endpoint.x - row.position.x, y: row.endpoint.y - row.position.y },
          midpoint: { x: row.midpoint.x - row.position.x, y: row.midpoint.y - row.position.y },
        }).body : null
        presentations.push({ actor: row, expected: bits({
          draws: plan.draws, queueFamily: plan.queueFamily, root: plan.root,
          sortBias: plan.sortBias, worldY: plan.worldY, body,
          lightDisposition: secondary.nativeSecondaryLightDisposition(row),
          painterLane: secondary.nativeSecondaryPainterManagerLane(row.kind),
          light: lights.nativeSecondaryProviderLightSource(row, tick, true, .75),
        }) })
        if (row.kind === 'electric-burn-arc') recordArcView(row)
      }
    }
  }
}
recordArcView({ ...presentations.find(row => row.actor.kind === 'electric-burn-arc').actor, ageTicks: 2 })
writeFixture('current-secondary-status-contacts-goldens.json',
  'tools/website-oracle/current-secondary-status-contacts-goldens.mjs',
  'current native-secondary-abilities.ts modifier pulses/Mod_ElectricBurn/FadeLightning + native-air-presentation.ts + native-secondary-presentation.ts',
  { sourceRevision, targets, cases, presentations, arcViews,
    counts: { cases: cases.length, actorSamples, contactSamples, presentations: presentations.length, arcViews: arcViews.length } })
