import { loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')
const profile = await loadKernel('core-kernels/player-progression.ts')
const owner = 'owner'
const cases = []
for (const [name, subtype, pending, capped] of [
  ['bonus-choice', 2, false, false],
  ['random-rank', 3, false, false],
  ['rank-refreshes-pending-offer', 3, true, false],
  ['no-eligible-rank', 3, false, true],
]) {
  const seed = 17
  let state = game.createGameSimulation({ [owner]: {
    displayName: 'Book Reader', element: 'fire', discipline: 'body',
  } }, { gameRngSeed: seed })
  if (pending) {
    const awarded = entities.grantPlayerEntityExperience(state.playerEntities, owner, 95, state.gameRng)
    state = { ...state, playerEntities: awarded.store, gameRng: awarded.rng }
  }
  if (capped) {
    let book = state.playerEntities.skillBooks[0]
    for (let id = 8; id <= 79; id += 1) {
      if (id === 52 || profile.playerStatBook().entries[id].maximumLevel < 1) continue
      book = profile.grantPlayerSkillRanks(book, id, 1000)
    }
    state = { ...state, playerEntities: { ...state.playerEntities, skillBooks: [book] } }
  }
  const definition = economy.NATIVE_SKILL_BOOK_DEFINITIONS.find(row => row.nativeSubtype === subtype)
  const item = economy.createNativeSkillBookInventoryItem(definition, 1000)
  state = { ...state, playerEntities: { ...state.playerEntities, economies: [{
    ...state.playerEntities.economies[0], backpack: [...state.playerEntities.economies[0].backpack, item],
  }] } }
  const records = []
  for (let read = 0; read < 2; read += 1) {
    const result = game.applyGameSimulationHubAction(state, owner, { type: 'read-skill-book', itemId: 1000 })
    state = result.state
    const store = state.playerEntities
    const progression = store.progressions[0]
    records.push({
      accepted: result.accepted,
      feedback: store.economies[0].actionFeedback,
      book: store.skillBooks[0],
      gameRng: state.gameRng,
      offerSeed: progression.offerSeed,
      pendingOffer: progression.pendingOffer,
      itemRemaining: economy.findInventoryItem(store.economies[0].backpack, 1000) !== null,
    })
  }
  cases.push({ name, seed, subtype, pending, capped, records })
}
writeFixture('skill-book-outcome-goldens.json', 'tools/website-oracle/skill-book-outcome-goldens.mjs',
  'website-rust-current/frontend/src/game/core-server/game-simulation.ts::applyGameSimulationHubAction', {
    sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83', cases,
  })
