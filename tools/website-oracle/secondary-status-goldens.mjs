import { loadKernel, writeFixture } from './oracle.mjs'
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const unexpected = () => { throw new Error('status-only replay reached world geometry') }
const context = tick => ({
  dampenCandidates: unexpected, golemMovement: unexpected, golemPlacement: unexpected,
  phasingDestination: unexpected, teleportDestination: unexpected,
  players: {}, target: () => null, targets: () => [], tick,
})
function journey(name, commands) {
  let state = secondary.createNativeSecondarySimulation(77)
  let tick = 0
  const records = []
  for (const action of commands) {
    if (action.type === 'apply') {
      state = secondary.applyNativeSecondaryTargetEffect(state, action.world ?? 'boneyard:oracle', action.target ?? 7, action.patch)
    } else if (action.type === 'dazzle') {
      state = secondary.applyNativeSecondaryDazzle(state, 'boneyard:oracle', 7, action.ticks)
    } else {
      for (let index = 0; index < action.ticks; index += 1) {
        tick += 1
        state = secondary.stepNativeSecondaryAbilities(state, context(tick)).state
      }
    }
    records.push({ action, effects: state.targetEffects })
  }
  return { name, records }
}
function permutations(values) {
  if (values.length === 0) return [[]]
  const results = []
  for (let index = 0; index < values.length; index += 1) {
    const rest = values.filter((_, other) => other !== index)
    for (const tail of permutations(rest)) results.push([values[index], ...tail])
  }
  return results
}
const movementPatches = [
  { coldSlowTicks: 7, coldSlowFactor: 0.61, coldSlowMaterial: true },
  { circleSlowTicks: 9, circleSlowFactor: 0.72 },
  { frozenTicks: 3, frozenTimeScale: 0.31 },
  { stunTicks: 5, stunFactor: 0.42 },
  { dazzleTicks: 4 },
]
const journeys = []
for (const order of permutations([0, 1, 2, 3, 4])) {
  journeys.push(journey(`movement-order-${order.join('-')}`, [
    ...order.map(index => ({ type: 'apply', patch: movementPatches[index] })),
    ...Array.from({ length: 12 }, () => ({ type: 'step', ticks: 1 })),
  ]))
}
journeys.push(journey('cold-material-and-duration-precedence', [
  { type: 'apply', patch: { coldSlowTicks: 20, coldSlowFactor: 0.8, coldSlowMaterial: false } },
  { type: 'apply', patch: { coldSlowTicks: 19, coldSlowFactor: 0.9, coldSlowMaterial: true } },
  { type: 'apply', patch: { coldSlowTicks: 20, coldSlowFactor: 0.7, coldSlowMaterial: true } },
  { type: 'apply', patch: { coldSlowTicks: 30, coldSlowFactor: 0.6, coldSlowMaterial: false } },
  { type: 'apply', patch: { coldSlowTicks: 31, coldSlowFactor: 0.9 } },
  { type: 'step', ticks: 32 },
  { type: 'dazzle', ticks: 0 },
]))
journeys.push(journey('frost-burn-owner-and-weaken-precedence', [
  { type: 'apply', patch: { frostBurnTicks: 30, frostBurnDamagePerTick: 10, frostBurnOwnerId: 'a', frostBurnSkillId: 35, frostBurnSourceActorId: 1, weakenFactor: 0.7 } },
  { type: 'apply', patch: { frostBurnTicks: 20, frostBurnDamagePerTick: 20, frostBurnOwnerId: 'b', frostBurnSkillId: 76, frostBurnSourceActorId: 2, weakenFactor: 0.2 } },
  { type: 'apply', patch: { frostBurnTicks: 40, frostBurnDamagePerTick: 5, frostBurnOwnerId: 'c', frostBurnSkillId: 76, frostBurnSourceActorId: 3 } },
  { type: 'step', ticks: 41 },
]))
const electric = (damage, ticks, ownerId, sourceActorId) => ({ arcCount: 2, damagePerTick: damage, ownerId, sourceActorId, stunFactor: 0.6, ticks })
const steamed = (damage, ticks, ownerId, sourceActorId) => ({ damagePerTick: damage, emberDamage: damage * 2,
  emberFragments: sourceActorId, explodeDamage: damage * 3, explodeRadius: damage * 4, ownerId, sourceActorId, ticks })
journeys.push(journey('electric-and-steamed-strongest-owner', [
  { type: 'apply', patch: { electricBurn: electric(2, 10, 'a', 1), steamed: steamed(2, 10, 'a', 1) } },
  { type: 'apply', patch: { electricBurn: electric(1, 20, 'b', 2), steamed: steamed(1, 20, 'b', 2) } },
  { type: 'apply', patch: { electricBurn: electric(2, 5, 'c', 3), steamed: steamed(2, 5, 'c', 3) } },
  { type: 'apply', patch: { electricBurn: electric(4, 5, 'd', 4), steamed: steamed(4, 5, 'd', 4) } },
  { type: 'step', ticks: 19 },
  { type: 'step', ticks: 1 },
  { type: 'apply', patch: { electricBurn: electric(1, 5, 'a', 1), steamed: steamed(1, 5, 'a', 1) } },
  { type: 'apply', patch: { electricBurn: null, steamed: null } },
  { type: 'step', ticks: 1 },
]))
journeys.push(journey('world-and-target-identities-thaw-and-dazzle', [
  { type: 'apply', patch: { frozenTicks: 205, frozenTimeScale: 0, disruptedTicks: 3, fleeTicks: 4, prismaticTicks: 5 } },
  { type: 'apply', world: 'boneyard:second', target: 7, patch: { stunTicks: 10, stunFactor: 0.25 } },
  { type: 'apply', target: 8, patch: { coldSlowTicks: 10, coldSlowFactor: 0.5 } },
  { type: 'dazzle', ticks: 6 },
  ...[1, 4, 1, 49, 149, 1, 1].map(ticks => ({ type: 'step', ticks })),
]))
const tints = []
for (const worldTint of [0, 0xffffff, 0x123456, 0xaabbcc]) {
  tints.push({ worldTint, patch: null, result: secondary.nativeSecondaryTargetMaterialTint(worldTint, null) })
  for (const coldSlowMaterial of [false, true]) {
    for (const frozenTimeScale of [0, 0.1, 0.5, 0.995, 1]) {
      const patch = { coldSlowTicks: 10, coldSlowFactor: 0.5, coldSlowMaterial, frozenTicks: 10, frozenTimeScale }
      const state = secondary.applyNativeSecondaryTargetEffect(secondary.createNativeSecondarySimulation(0), 'boneyard:oracle', 7, patch)
      tints.push({ worldTint, patch, result: secondary.nativeSecondaryTargetMaterialTint(worldTint, state.targetEffects[0]) })
    }
  }
}
writeFixture('secondary-status-goldens.json', 'tools/website-oracle/secondary-status-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-secondary-abilities.ts', { journeys, tints })
