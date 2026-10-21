// Native offer replacement used by the actual prepared mod skill transaction.
import { loadKernel, writeFixture } from './oracle.mjs'
import { setup } from './player-entity-fixture.mjs'
import { playerStateSnapshot } from './player-state-snapshot.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const cases = []
for (const spec of [{}, { ranks: [[63, 2]] }, { perks: [17], ranks: [[63, 4]] }]) {
  let state = game.createGameSimulation({ local: {
    displayName: 'Oracle', element: 'ether', discipline: 'arcane',
  } }, { gameRngSeed: 17, combatRngSeed: 917 })
  state = { ...state, playerEntities: setup(spec) }
  state = game.grantGameSimulationPlayerExperience(state, 'local', 900)
  const samples = []
  function replace(playerId, offerSequence) {
    const replaced = game.replaceGameSimulationPlayerSkillWithMod(state, playerId, offerSequence)
    if (replaced !== null) state = replaced
    const progression = state.playerEntities.progressions[0]
    samples.push({ playerId, offerSequence, accepted: replaced !== null,
      player: playerStateSnapshot(state), barrier: state.levelUpBarrier,
      pendingLevels: progression.pendingLevels, forcedOfferSkillIds: progression.forcedOfferSkillIds,
      sorcerorsCharmAvailable: progression.sorcerorsCharmAvailable })
  }
  const first = state.playerEntities.progressions[0].pendingOffer.sequence
  replace('absent', first)
  replace('local', first + 1)
  replace('local', first)
  replace('local', first)
  while (state.playerEntities.progressions[0].pendingOffer !== null) {
    replace('local', state.playerEntities.progressions[0].pendingOffer.sequence)
  }
  replace('local', first)
  cases.push({ spec, samples })
}
writeFixture('session-mod-skill-goldens.json', 'tools/website-oracle/session-mod-skill-goldens.mjs',
  'core-server/game-simulation.ts::replaceGameSimulationPlayerSkillWithMod and player-entity-store.ts', { cases })
