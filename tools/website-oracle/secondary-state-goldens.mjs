import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const golem = await loadKernel('core-kernels/native-secondary-golem.ts')
const target = (id, x = 100, world = 'boneyard:oracle') => ({
  family: 'SKELETON', id, lightRegistration: { managerLane: 'actor', registrationOrdinal: 123 + id },
  nativeFlags: 2, position: { x, y: 100 }, radius: 15, scale: 1.2, shieldHealth: 0,
})
function journey(name, initial, actions) {
  let state = initial
  const records = []
  for (const action of actions) {
    let result = null
    switch (action.type) {
      case 'fire-burn': state = secondary.applyNativeSecondaryFireBurn(state, action.input); break
      case 'ether-burn': state = secondary.applyNativeSecondaryEtherBurn(state, action.input); break
      case 'player-damage': {
        const applied = secondary.applyNativeSecondaryPlayerDamage(state, action.playerId, action.amount, action.tick, action.position, 'boneyard:oracle')
        state = applied.state
        result = { absorbedDamage: applied.absorbedDamage, healthDamage: applied.healthDamage }
        break
      }
      case 'golem-damage': {
        const applied = secondary.applyNativeSecondaryGolemDamage(state, action.actorId, action.damage, action.tick)
        state = applied.state
        result = { ignored: applied.ignored, killed: applied.killed, ownerId: applied.ownerId, reflectedDamage: applied.reflectedDamage }
        break
      }
      case 'actor-age': state = { ...state, actors: state.actors.map(actor => actor.id === action.id ? { ...actor, ageTicks: action.age } : actor) }; break
      case 'player-patch': state = { ...state, players: { ...state.players, [action.id]: { ...state.players[action.id], ...action.patch } } }; break
      case 'target-effect': state = secondary.applyNativeSecondaryTargetEffect(state, action.world, action.target, action.patch); break
      case 'remove-owner': state = secondary.removeNativeSecondaryOwner(state, action.id); break
      case 'reset': state = secondary.resetNativeSecondaryWorld(state); break
      default: throw new Error(`unknown secondary state action ${action.type}`)
    }
    records.push({ action, result, state })
  }
  return { name, initial, records }
}
const journeys = []
const fire = (ownerId, damage, rank = 1, worldKey = 'boneyard:oracle') => ({
  damage, ownerId, rank, skillId: 22, target: target(7), worldKey,
})
const ether = (ownerId, rank = 1) => ({ ownerId, rank, target: target(7, 110), worldKey: 'boneyard:oracle' })
journeys.push(journey('burn-refresh-replacement-and-owner-cleanup', secondary.createNativeSecondarySimulation(77), [
  { type: 'fire-burn', input: fire('a', 0) },
  { type: 'fire-burn', input: fire('a', 20) },
  { type: 'actor-age', id: 1, age: 60 },
  { type: 'fire-burn', input: fire('b', 5, 2) },
  { type: 'fire-burn', input: fire('b', 40, 3) },
  { type: 'ether-burn', input: ether('a') },
  { type: 'actor-age', id: 2, age: 70 },
  { type: 'ether-burn', input: ether('b', 3) },
  { type: 'fire-burn', input: fire('a', 10, 1, 'boneyard:other') },
  { type: 'target-effect', world: 'boneyard:oracle', target: 7, patch: {
    coldSlowTicks: 10, coldSlowFactor: 0.5,
    frostBurnTicks: 100, frostBurnDamagePerTick: 0.01, frostBurnOwnerId: 'b', frostBurnSkillId: 35, frostBurnSourceActorId: 5,
    electricBurn: { arcCount: 2, damagePerTick: 1, ownerId: 'b', sourceActorId: 6, stunFactor: 0.8, ticks: 10 },
    steamed: { damagePerTick: 1, emberDamage: 2, emberFragments: 3, explodeDamage: 4, explodeRadius: 5, ownerId: 'b', sourceActorId: 7, ticks: 10 },
  } },
  { type: 'remove-owner', id: 'b' },
  { type: 'remove-owner', id: 'a' },
  { type: 'reset' },
]))
journeys.push(journey('missing-owner-keeps-unrelated-empty-status', secondary.createNativeSecondarySimulation(77), [
  { type: 'target-effect', world: 'boneyard:oracle', target: 8, patch: {} },
  { type: 'remove-owner', id: 'missing' },
]))
for (const seed of [1, 44]) {
  for (const explosion of [0, 25]) {
    const initial = { ...secondary.createNativeSecondarySimulation(seed), players: { player: {
      ...secondary.createNativeSecondaryPlayerState(), magicShieldAbsorb: 15,
      magicShieldMaximum: 15, magicShieldExplosionDamage: explosion,
    } } }
    const actions = [0, 5, 25, 100].map((amount, index) => ({
      type: 'player-damage', playerId: 'player', amount, tick: index + 1, position: { x: 100, y: 100 },
    }))
    actions.push(
      { type: 'player-patch', id: 'player', patch: { stoneskinTicksRemaining: 5 } },
      { type: 'player-damage', playerId: 'player', amount: 10, tick: 5, position: { x: 100, y: 100 } },
      { type: 'player-damage', playerId: 'missing', amount: 10, tick: 6, position: { x: 100, y: 100 } },
      { type: 'reset' },
    )
    journeys.push(journey(`shield-${seed}-${explosion}`, initial, actions))
  }
}
const position = { x: 100, y: 100 }
const rotation = Math.PI / 2
const golemActor = {
  ageTicks: 399, alpha: 1, damage: 10, enhanced: false, endpoint: { x: 0, y: 0 }, frame: 0, freezeTicks: 0,
  golem: { ...golem.nativeInitialGolemArticulation(position, rotation),
    actionDurationTicks: 0, actionTick: 0, circleSlowTicks: 0, currentHealth: 100, damageMaximum: 20,
    iron: true, maximumHealth: 100, orbitDirection: 0, orbitHeadingRadians: null, phase: 'active', poseVariant: 0,
    provokeRollBound: 0, reflectFactor: 1.5, targetPollTicksRemaining: 50 },
  hitTargetIds: [], id: 1, kind: 'golem', lifetimeTicks: Number.MAX_SAFE_INTEGER, lightRegistration: null,
  midpoint: { x: 0, y: 0 }, miscLightAppendOrdinal: null, ownerId: 'player', painterRegistrations: [],
  phase: 0, position, presentationRng: null, quantity: 100, radius: 30, rank: 1, rotationRadians: rotation,
  scale: 1, skillId: 45, slowFactor: 0, targetId: null, variant: 1, velocity: { x: 0, y: 0 }, worldKey: 'boneyard:oracle',
}
journeys.push(journey('golem-immunity-reflection-death', {
  ...secondary.createNativeSecondarySimulation(55), actors: [golemActor], nextActorId: 2,
}, [
  { type: 'golem-damage', actorId: 999, damage: { primaryDamage: 1, secondaryDamage: 0, reflectablePhysicalSourceInRange: false }, tick: 0 },
  { type: 'golem-damage', actorId: 1, damage: { primaryDamage: 1000, secondaryDamage: 0, reflectablePhysicalSourceInRange: true }, tick: 1 },
  { type: 'actor-age', id: 1, age: 400 },
  { type: 'golem-damage', actorId: 1, damage: { primaryDamage: 10, secondaryDamage: 20, reflectablePhysicalSourceInRange: true }, tick: 2 },
  { type: 'golem-damage', actorId: 1, damage: { primaryDamage: 70, secondaryDamage: 0, reflectablePhysicalSourceInRange: true }, tick: 3 },
  { type: 'remove-owner', id: 'player' },
  { type: 'reset' },
]))
writeFixture('secondary-state-goldens.json', 'tools/website-oracle/secondary-state-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-secondary-abilities.ts', { sourceRevision, journeys })
