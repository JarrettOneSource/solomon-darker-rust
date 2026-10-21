import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { playerStateSnapshot as snapshot } from './player-state-snapshot.mjs'

const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const nativeRng = await loadKernel('core-kernels/native-rng.ts')
const playerId = 'local'


const cases = []
for (const [seed, element, discipline] of [[0, 'ether', 'arcane'], [17, 'fire', 'body'], [91, 'air', 'mind']]) {
  const config = { displayName: 'Oracle', element, discipline }
  let state = game.createGameSimulation({ [playerId]: config }, { gameRngSeed: seed })
  const steps = [{ action: { kind: 'initial' }, expected: snapshot(state) }]
  const inventory = action => {
    const result = game.applyGameSimulationHubAction(state, playerId, action)
    state = result.state
    steps.push({ action: { kind: 'inventory', command: action }, accepted: result.accepted, expected: snapshot(state) })
  }
  const progression = { ...state.playerEntities.progressions[0], currentHealth: 17.25, currentMana: 11.5 }
  const economy = { ...state.playerEntities.economies[0], gold: 1000000 }
  state = { ...state, playerEntities: { ...state.playerEntities, progressions: [progression], economies: [economy] } }
  steps.push({ action: { kind: 'resource-input', health: 17.25, mana: 11.5, gold: 1000000 }, expected: snapshot(state) })
  for (const subtype of [0, 1]) {
    const item = state.playerEntities.economies[0].backpack.find(item => item.nativeTypeId === 7001 && item.nativeSubtype === subtype)
    inventory({ type: 'consume', itemId: item.id })
  }
  inventory({ type: 'buy-hagatha', selector: 6 })
  inventory({ type: 'buy-hagatha', selector: 14 })
  inventory({ type: 'buy-hagatha', selector: 21 })
  const hat = state.playerEntities.economies[0].equipment.hat
  inventory({ type: 'unequip', slot: 'hat' })
  inventory({ type: 'equip', itemId: hat.id, slot: 'hat' })
  const granted = entities.grantPlayerEntityExperience(state.playerEntities, playerId, 95, state.gameRng)
  state = { ...state, playerEntities: granted.store, gameRng: granted.rng }
  steps.push({ action: { kind: 'experience', amount: 95 }, expected: snapshot(state) })
  const offer = state.playerEntities.progressions[0].pendingOffer
  const choice = { choiceIndex: 0, offerSequence: offer.sequence, skillId: offer.options[0].skillId }
  const chosen = entities.applyPlayerEntitySkillChoice(state.playerEntities, playerId, choice, state.gameRng)
  if (!chosen) throw new Error('Source rejected its current skill offer')
  state = { ...state, playerEntities: chosen.store, gameRng: chosen.rng }
  steps.push({ action: { kind: 'choice', ...choice }, expected: snapshot(state) })
  inventory({ type: 'buy-hagatha', selector: 17 })
  const grant = amount => {
    const result = entities.grantPlayerEntityExperience(state.playerEntities, playerId, amount, state.gameRng)
    state = { ...state, playerEntities: result.store, gameRng: result.rng }
    steps.push({ action: { kind: 'experience', amount }, expected: snapshot(state) })
  }
  const special = (kind, sequence) => {
    // The Game host publishes this RNG draw only when the entity accepts the command.
    const draw = nativeRng.drawNativeInteger(state.gameRng, 1_000_000)
    const result = kind === 'reroll'
      ? entities.rerollPlayerEntitySkillOffer(state.playerEntities, playerId, sequence, draw.value, draw.state)
      : entities.deferPlayerEntitySkillChoice(state.playerEntities, playerId, sequence, state.gameRng)
    if (result) state = { ...state, playerEntities: result.store, gameRng: result.rng }
    steps.push({ action: { kind, offerSequence: sequence }, accepted: result !== null, expected: snapshot(state) })
  }
  grant(500)
  special('reroll', state.playerEntities.progressions[0].pendingOffer.sequence + 1)
  special('reroll', state.playerEntities.progressions[0].pendingOffer.sequence)
  special('defer', state.playerEntities.progressions[0].pendingOffer.sequence)
  grant(1000)
  special('defer', state.playerEntities.progressions[0].pendingOffer.sequence + 1)
  special('defer', state.playerEntities.progressions[0].pendingOffer.sequence)
  cases.push({ seed, config, steps })
}
writeFixture('player-state-goldens.json', 'tools/website-oracle/player-state-goldens.mjs',
  'Website/frontend/src/game/core-server/player-entity-store.ts and game-simulation.ts', { sourceRevision, cases })
