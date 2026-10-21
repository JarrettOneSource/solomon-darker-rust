#!/usr/bin/env node
// Current retained-target membership and actual rescue-wave cell/registration order.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { allLearnedBook, authority, context, factors } from './secondary-inputs.mjs'
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const world = await loadKernel('core-server/native-secondary-world.ts')
const painter = await loadKernel('core-kernels/native-world-manager-order.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const bits = value => {
  if (typeof value === 'number') {
    const bytes = Buffer.alloc(8); bytes.writeDoubleBE(value)
    return `f64:${bytes.toString('hex')}`
  }
  if (Array.isArray(value)) return value.map(bits)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, bits(item)]))
  return value
}
const membership = []
for (const kind of ['same-run', 'new-run', 'hub']) {
  let state = secondary.createNativeSecondarySimulation(77)
  const identities = [
    { worldKey: 'boneyard:oracle', targetId: 10 },
    { worldKey: 'boneyard:oracle', targetId: 20 },
    { worldKey: 'boneyard:oracle', targetId: 30 },
    { worldKey: 'boneyard:other', targetId: 10 },
  ]
  for (const row of identities) state = secondary.applyNativeSecondaryTargetEffect(state, row.worldKey, row.targetId,
    { dazzleTicks: 20, coldSlowFactor: .75, coldSlowMaterial: true, coldSlowTicks: 10 })
  const frames = []
  for (let tick = 1; tick <= 5; tick += 1) {
    // These are exactly the fields consumed by the Source membership factory. Dying,
    // inactive and emerging actors still own modifiers until their row is destroyed.
    const retained = kind === 'hub' ? { kind: 'hub' } : {
      kind: 'boneyard', runId: kind === 'new-run' ? 'other' : 'oracle',
      enemies: { actors: tick < 3 ? [{ id: 10, lifeState: 'dying' }, { id: 20, lifeState: 'alive', nativeFlags: 0 }] : [{ id: 20 }],
        maggots: tick < 4 ? [{ id: 30, combatActive: false }] : [] },
    }
    const targetExists = world.createNativeSecondaryTargetMembership(retained)
    const stepped = secondary.stepNativeSecondaryAbilities(state, {
      ...context(null, tick), players: {}, targetExists,
    })
    state = stepped.state
    frames.push({ tick, retained, expected: bits({ effects: state.targetEffects, rng: state.rng }) })
  }
  membership.push({ kind, identities, frames })
}
const targets = [
  { id: 3, x: 0, y: 0, order: 60 },
  { id: 90, x: -100, y: 0, order: 20 },
  { id: 2, x: 0, y: -100, order: 40 },
  { id: 8, x: 0, y: 50, order: 10 },
  { id: 5, x: 0, y: 50, order: 9 },
  { id: 60, x: 100, y: 0, order: 0 },
  { id: 11, x: 0, y: 200, order: 11 },
  { id: 12, x: 250, y: 0, order: 12 },
  { id: 13, x: 0, y: 0, order: 1, flags: 0 },
].map(row => ({ id: row.id, family: 'SKELETON', position: { x: row.x, y: row.y },
  radius: 20, scale: 1, shieldHealth: 0, nativeFlags: row.flags ?? 2, registrationOrder: row.order,
  lightRegistration: { managerLane: 'actor', registrationOrdinal: 100 + row.id } }))
const waves = []
for (const orderMode of ['forward', 'reverse', 'missing-registration']) {
  const selected = (orderMode === 'reverse' ? [...targets].reverse() : structuredClone(targets))
    .map(target => orderMode === 'missing-registration' ? Object.fromEntries(Object.entries(target).filter(([key]) => key !== 'registrationOrder')) : target)
  const player = authority(allLearnedBook(), factors(false))
  const order = painter.createNativeWorldManagerOrder()
  let state = secondary.spawnNativePlayerRescueShockwave(secondary.createNativeSecondarySimulation(91), {
    lightRegistration: { managerLane: 'actor', registrationOrdinal: 100 },
    ownerId: 'player', position: { x: 0, y: 0 }, worldKey: player.worldKey,
  }, order.register)
  const frames = []
  for (let tick = 1; tick <= 30; tick += 1) {
    const stepped = secondary.stepNativeSecondaryAbilities(state, { ...context(player, tick),
      registerWorldPainter: order.register,
      targetExists: (key, id) => key === player.worldKey && selected.some(target => target.id === id),
      target: (_key, id) => selected.find(target => target.id === id) ?? null,
      targets: (_key, center, radius) => selected.filter(target => Math.hypot(target.position.x - center.x, target.position.y - center.y) <= radius + target.radius),
    })
    state = stepped.state
    if (tick % 10 === 0) frames.push({ tick, expected: bits({
      actorKinds: state.actors.map(actor => actor.kind),
      hitTargetIds: state.actors[0].hitTargetIds,
      damagedTargetIds: stepped.damage.map(contact => contact.targetId),
      effects: state.targetEffects,
      rng: state.rng,
    }) })
  }
  waves.push({ orderMode, player, targets: selected, frames })
}
writeFixture('current-secondary-membership-goldens.json', 'tools/website-oracle/current-secondary-membership-goldens.mjs',
  'current native-secondary-world.ts retained identity + native-secondary-abilities.ts rescue-wave nativePrimaryCircleTargets',
  { sourceRevision, membership, waves })
