// Source-produced import fixtures for the real File chooser/Inventory journey.
// These prove book use after import; they do not claim an ordinary loot drop.
import assert from 'node:assert/strict'
import { loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')
const saves = await loadKernel('save/game-save-document.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const cases = []
for (const world of ['hub', 'boneyard']) {
  let state, playerId, loadedBoneyard = null
  if (world === 'hub') {
    playerId = 'book-import-owner'
    state = game.createGameSimulation({ [playerId]: {
      displayName: 'Book Reader', element: 'ether', discipline: 'arcane',
    } }, { gameRngSeed: 17, combatRngSeed: 91 })
    state = game.declineGameSimulationTutorial(state, playerId)
  } else {
    playerId = 'active-owner'
    state = game.createGameSimulation({ [playerId]: {
      displayName: 'Actor Save Oracle', element: 'fire', discipline: 'mind',
    } }, { gameRngSeed: 17, combatRngSeed: 91 })
    state = game.declineGameSimulationTutorial(state, playerId)
    loadedBoneyard = { ...NATIVE_GENERATED_BONEYARDS[0], choice: {
      id: 'default-random', name: 'Random Boneyard', source: 'default',
    }, seed: '00000000000000000000000000000000', runId: 'book-import-stock' }
    state = game.enterBoneyardWorld(state, loadedBoneyard)
  }
  const bookIds = []
  for (const subtype of [3, 3, 3, 2]) {
    const prior = entities.playerEconomyAt(state.playerEntities, playerId)
    const definition = economy.NATIVE_SKILL_BOOK_DEFINITIONS.find(row => row.nativeSubtype === subtype)
    const inserted = economy.insertLootInventoryItem(prior,
      economy.createNativeSkillBookInventoryItem(definition, 0))
    assert.equal(inserted.accepted, true)
    const item = inserted.state.backpack.find(item => !prior.backpack.some(old => old.id === item.id))
    assert(item, 'source assigns the new book identity and addressed slot')
    bookIds.push(item.id)
    state = { ...state, playerEntities: entities.replacePlayerEconomy(state.playerEntities, playerId, inserted.state) }
  }
  // Keep a genuine accepted old rank receipt to prove admission does not replay it.
  const consumedId = bookIds.shift()
  const read = game.applyGameSimulationHubAction(state, playerId, { type: 'read-skill-book', itemId: consumedId })
  assert.equal(read.accepted, true)
  state = read.state
  const accepted = entities.playerEconomyAt(state.playerEntities, playerId).actionFeedback
  assert.equal(accepted.skillBookOutcome.kind, 'rank')
  assert.notEqual(accepted.skillBookOutcome.skillId, null)
  const document = JSON.parse(saves.createGameSaveDocument({
    state, playerId, loadedBoneyard, integrity: 'global-clean', mods: [], modState: {},
  }))
  // Run the real public import validation now, before the browser consumes it.
  saves.restoreGameSaveDocument(JSON.stringify(document))
  cases.push({ world, bookIds, retainedSequence: accepted.sequence, document })
}
writeFixture('skill-book-browser-saves.json.zlib',
  'tools/website-oracle/skill-book-browser-saves.mjs',
  'fa08 stock Boneyard entry, skill-book inventory insertion/read and public create/restore save APIs', {
    sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83',
    qualification: 'Source-produced inventory fixtures imported through the real File chooser; ordinary loot earning is separate.',
    cases,
  })
