// Public membership, shared milestones and detached transactions; no copied host rules.
import { loadKernel, writeFixture } from './oracle.mjs'
import { multiplayerSessionSnapshot } from './session-tick-rows.mjs'
const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
function snapshot(state) {
  return { ...multiplayerSessionSnapshot(state), entityIds: state.playerEntities.entityIds,
    nextEntityId: state.playerEntities.nextEntityId }
}

const config = element => ({ displayName: 'Oracle', element, discipline: 'arcane' })
let state = game.createGameSimulation({ zeta: config('fire'), alpha: config('water') }, { gameRngSeed: 17, combatRngSeed: 91 })
let detached = null
const steps = []
function record(action, request = null, accepted = true) {
  const projected = detached !== null && entities.playerEntityIndex(state.playerEntities, detached.playerId) < 0
    ? snapshot(game.projectDetachedGameSimulationPlayer(state, detached)) : null
  const profiles = Object.fromEntries(state.playerEntities.identities.map(({playerId}, index) => {
    const current = state.playerEntities.economies[index]
    return [playerId, { nextItemId: current.nextItemId, storage: economy.projectInventoryItems(current.storage)
      .map(({depth, parentSackId, slot, item}) => [depth, parentSackId, slot, item.id, item.nativeTypeId, item.quantity, item.name]) }]
  }))
  const hasDetachedOwner = detached !== null && entities.playerEntityIndex(state.playerEntities, detached.playerId) < 0
  const detachedIds = hasDetachedOwner ? { id: detached.playerEntities.entityIds[0], next: detached.playerEntities.nextEntityId } : null
  steps.push({ action, request, accepted, expected: snapshot(state), projected, profiles, detachedIds })
}
record('create')
state = game.addPlayerCharacter(state, 'gamma', config('ether'))
record('add', { id: 'gamma', config: config('ether') })
state = game.addPlayerCharacter(state, 'gamma', config('air'))
record('duplicate-add', { id: 'gamma', config: config('air') }, false)
const gammaIndex = state.playerEntities.identities.findIndex(identity => identity.playerId === 'gamma')
const rich = { ...state.playerEntities.economies[gammaIndex], gold: 100_000 }
state.playerEntities = entities.replacePlayerEconomy(state.playerEntities, 'gamma', rich)
const charm = economy.hagathaOffers(rich).find(offer => offer.members.includes(17))
if (!charm) throw new Error('Native Hagatha must offer the Sorceror Charm')
const purchase = game.applyGameSimulationHubAction(state, 'gamma', { type: 'buy-hagatha', selector: charm.selector })
if (!purchase.accepted) throw new Error('Native charm purchase must be admitted')
state = purchase.state
record('buy-charm', { id: 'gamma', gold: 100_000, selector: charm.selector })
state = game.removePlayerCharacter(state, 'alpha')
record('remove', { id: 'alpha' })
const other = game.createGameSimulation({ omega: config('air') }, { gameRngSeed: 73 })
state = game.mergeGameSimulationPlayersIntoHub(state, other)
record('merge', { configurations: [['omega', config('air')]], seed: 73 })
state = game.enterBoneyardWorld(state, { ...NATIVE_GENERATED_BONEYARDS[0],
  choice: { id: 'default-random', source: 'default' }, runId: 'membership-run', seed: 'membership-run' })
record('enter')
state.playerEntities = entities.damagePlayerEntityWithResult(state.playerEntities, 'gamma', 57, 0, true, false, 1).store
record('damage-player', { id: 'gamma', amount: 57 })
detached = game.detachGameSimulationPlayer(state, 'gamma')
state = game.removePlayerCharacter(state, 'gamma')
record('detach-remove', { id: 'gamma' })
state = game.grantGameSimulationPlayerExperience(state, 'zeta', 95)
record('experience', { id: 'zeta', amount: 95 })
let result = game.synchronizeDetachedGameSimulationPlayer(state, detached, { crossedLevels: [2], experience: 95, level: 2 })
state = result.state; detached = result.detached
record('synchronize-detached', { crossedLevels: [2], experience: 95, level: 2 })
record('project')
let offer = detached.playerEntities.progressions[0].pendingOffer
let request = { choiceIndex: 0, offerSequence: offer.sequence + 1, skillId: offer.options[0].skillId }
result = game.selectDetachedGameSimulationPlayerSkill(state, detached, request)
if (result !== null) throw new Error('Stale detached choice must reject')
record('choice-detached', request, false)
request = { choiceIndex: 0, offerSequence: offer.sequence, skillId: offer.options[0].skillId }
result = game.selectDetachedGameSimulationPlayerSkill(state, detached, request)
if (result === null) throw new Error('Current detached choice must accept')
state = result.state; detached = result.detached
record('choice-detached', request)
state = game.grantGameSimulationPlayerExperience(state, 'zeta', 100)
record('experience', { id: 'zeta', amount: 100 })
result = game.synchronizeDetachedGameSimulationPlayer(state, detached, { crossedLevels: [3], experience: 195, level: 3 })
state = result.state; detached = result.detached
record('synchronize-detached', { crossedLevels: [3], experience: 195, level: 3 })
offer = detached.playerEntities.progressions[0].pendingOffer
result = game.rerollDetachedGameSimulationPlayerSkill(state, detached, offer.sequence + 1)
if (result !== null) throw new Error('Stale detached reroll must reject')
record('reroll-detached', { sequence: offer.sequence + 1 }, false)
result = game.rerollDetachedGameSimulationPlayerSkill(state, detached, offer.sequence)
if (result === null) throw new Error('Current detached reroll must accept')
state = result.state; detached = result.detached
record('reroll-detached', { sequence: offer.sequence })
offer = detached.playerEntities.progressions[0].pendingOffer
result = game.saveDetachedGameSimulationPlayerSkill(state, detached, offer.sequence)
// A reroll consumes this offer's native save admission. Preserve the real rejection.
if (result !== null) { state = result.state; detached = result.detached }
record('defer-detached', { sequence: offer.sequence }, result !== null)
record('project')
state = game.rejoinGameSimulationPlayer(state, detached, 'gamma', null)
record('rejoin', { id: 'gamma' })
while (state.levelUpBarrier !== null) {
  const id = state.levelUpBarrier.pendingPlayerIds[0]
  const index = entities.playerEntityIndex(state.playerEntities, id)
  const offer = state.playerEntities.progressions[index].pendingOffer
  const choice = { choiceIndex: 0, offerSequence: offer.sequence, skillId: offer.options[0].skillId }
  state = game.selectGameSimulationPlayerSkill(state, id, choice)
  if (state === null) throw new Error('Rejoined party choices must be admitted')
  record('choice', { id, choice })
}
for (let tick = 0; tick < 3; tick += 1) {
  state = game.stepGameSimulationTick(state, {})
  record('idle-tick')
}
state = game.returnGameSimulationToHub(state)
record('return')
for (const id of ['zeta', 'omega']) {
  state = game.removePlayerCharacter(state, id)
  record('remove', { id })
}
function resetWizard(element) {
  const id = 'gamma'
  const profile = game.gameSimulationRetiredWizardEconomy(state, id)
  const hagathaRuntime = game.getPlayerProgression(state, id).hagathaRuntime
  if (state.world.kind !== 'hub' || state.run.phase !== 'hub') state = game.returnGameSimulationToHub(state)
  state = game.removePlayerCharacter(state, id)
  state = game.addPlayerCharacter(state, id, config(element))
  let store = entities.replacePlayerEconomy(state.playerEntities, id, profile)
  store = entities.migratePlayerStarterEquipmentAppearance(store, id)
  const index = entities.playerEntityIndex(store, id)
  const progressions = store.progressions.slice()
  progressions[index] = { ...progressions[index], hagathaRuntime }
  state = { ...state, playerEntities: { ...store, progressions } }
  record('reset', { id, config: config(element) })
}
resetWizard('water')
state = game.enterBoneyardWorld(state, { ...NATIVE_GENERATED_BONEYARDS[0],
  choice: { id: 'default-random', source: 'default' }, runId: 'membership-reset-run', seed: 'membership-reset-run' })
record('enter', { runId: 'membership-reset-run' })
state.playerEntities = entities.damagePlayerEntityWithResult(state.playerEntities, 'gamma', 57, 0, true, false, 1).store
record('damage-player', { id: 'gamma', amount: 57 })
resetWizard('ether')
writeFixture('session-membership-goldens.json', 'tools/website-oracle/session-membership-goldens.mjs',
  'Website/frontend/src/game/core-server/game-simulation.ts public membership/shared XP/detached transactions', { steps })
