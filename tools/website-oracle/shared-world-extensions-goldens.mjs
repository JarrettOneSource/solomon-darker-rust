// Actual independent party epochs use their own admitted extension owner.
import { loadKernel, writeFixture } from './oracle.mjs'

const shared = await loadKernel('host/shared-game-worlds.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const character = { displayName: 'Oracle', element: 'ether', discipline: 'arcane' }
const identity = id => ({ id: `party-${id}`, joinCode: `CODE-${id}`, listingId: `listing-${id}` })
const loaded = id => ({ ...NATIVE_GENERATED_BONEYARDS[0],
  choice: { id: 'default-random', name: 'Random Boneyard', source: 'default' },
  seed: '0123456789abcdef', runId: `run-${id}` })
const cases = []

for (const spec of [
  { name: 'separate-party-owners', paused: [], error: null },
  { name: 'one-party-held', paused: ['party-zeta'], error: null },
  { name: 'authored-error-is-fatal', paused: [], error: 'The actual party rule rejected its epoch.' },
]) {
  let worlds = shared.createSharedGameWorlds(0)
  for (const id of ['resident', 'zeta', 'alpha']) {
    worlds = shared.addSharedHubPlayer(worlds, id, character, identity(id))
  }
  for (const id of ['zeta', 'alpha']) {
    worlds = shared.startSharedPartyRun(worlds, id, loaded(id)).state
    const state = shared.sharedGameStateForPlayer(worlds, id)
    worlds = shared.replaceSharedGameStateForPlayer(worlds, id, { ...state,
      playerEntities: entities.setPlayerEntityMana(state.playerEntities, id, 10) })
  }
  const calls = []
  const extensions = new Map(['zeta', 'alpha'].map((id, index) => [`party-${id}`, {
    createLootItems: () => { throw new Error('The initial scene cannot award loot.') },
    filterDamage: input => {
      calls.push({ owner: id, kind: 'damage', player: input.targetPlayerId, value: input.amount })
      return input.amount
    },
    filterMana: input => {
      calls.push({ owner: id, kind: 'mana', player: input.playerId, value: input.delta })
      if (spec.error && index === 0) throw new Error(spec.error)
      return input.delta - (index + 1) / 8
    },
    hasConsumable: () => false,
  }]))
  let error = null
  try { worlds = shared.stepSharedGameWorlds(worlds, {}, new Set(spec.paused), new Map(), extensions) }
  catch (caught) { error = caught.message }
  const state = id => shared.sharedGameStateForPlayer(worlds, id)
  const players = error ? null : ['resident', 'zeta', 'alpha'].map(id => ({
    id, tick: state(id).tick,
    mana: state(id).playerEntities.progressions[0].currentMana,
    health: state(id).playerEntities.progressions[0].currentHealth,
  }))
  cases.push({ spec, calls, error, players })
}
writeFixture('shared-world-extensions-goldens.json', 'shared-world-extensions-goldens.mjs',
  'host/shared-game-worlds.ts::stepSharedGameWorlds with actual per-party extensions', { cases })
