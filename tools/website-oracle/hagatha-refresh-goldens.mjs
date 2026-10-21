import { loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const character = await loadKernel('core-kernels/player-character.ts')
const profile = await loadKernel('core-kernels/player-progression.ts')
const playerId = 'owner'

function snapshot(state, grantedSkillId) {
  const store = state.playerEntities
  const progression = store.progressions[0]
  const runtime = store.skillRuntimes[0]
  return {
    book: store.skillBooks[0],
    belt: store.belts[0],
    concentrations: [runtime.concentrationSkillIdA, runtime.concentrationSkillIdB],
    gameRng: state.gameRng,
    grantedSkillId,
    hagathaRuntime: progression.hagathaRuntime,
    offerSeed: progression.offerSeed,
    pendingOffer: progression.pendingOffer,
    progressionRevision: progression.revision,
  }
}

const cases = []
for (const [name, owned, pending] of [
  ['revelation', [6], false],
  ['weird-caster', [14], false],
  ['combined-owned', [6, 14], false],
  ['charges-are-not-purchases', [6, 7, 14, 24, 25], false],
  ['pending-offer-refresh', [6, 14], true],
]) {
  const seed = 17
  const config = { displayName: 'Retained', element: 'fire', discipline: 'body' }
  let state = game.createGameSimulation({ [playerId]: config }, { gameRngSeed: seed })
  if (pending) {
    const granted = entities.grantPlayerEntityExperience(state.playerEntities, playerId, 95, state.gameRng)
    state = { ...state, playerEntities: granted.store, gameRng: granted.rng }
  }
  const initial = snapshot(state, null)
  state = { ...state, playerEntities: { ...state.playerEntities, economies: [
    { ...state.playerEntities.economies[0], ownedPerkSelectors: owned },
  ] } }
  const steps = []
  for (let refresh = 0; refresh < 2; refresh += 1) {
    const result = entities.refreshPlayerEntityHagathaSkillEffects(state.playerEntities, playerId, state.gameRng)
    state = { ...state, playerEntities: result.store, gameRng: result.rng }
    steps.push(snapshot(state, result.weirdCasterSkillId))
  }
  cases.push({ name, seed, config, owned, pending, initial, steps })
}

const loadouts = []
for (const owned of [[7, 24, 25], [6, 7, 14, 24, 25]]) {
  const seed = 17
  let state = game.createGameSimulation({ [playerId]: {
    displayName: 'Retained', element: 'fire', discipline: 'body',
  } }, { gameRngSeed: seed })
  let book = state.playerEntities.skillBooks[0]
  for (const skillId of [72, 77]) book = profile.unlockPlayerAdvancedSkill(book, skillId)
  state = { ...state, playerEntities: { ...state.playerEntities, skillBooks: [book], economies: [
    { ...state.playerEntities.economies[0], ownedPerkSelectors: owned },
  ] } }
  const initial = snapshot(state, null)
  const config = { displayName: 'Next', element: 'water', discipline: 'fire' }
  state = { ...state, playerEntities: entities.replacePlayerLoadout(state.playerEntities,
    playerId, character.createPlayerCharacter(config, { x: 0, y: 0 }), 12345) }
  const replaced = snapshot(state, null)
  const refreshed = entities.refreshPlayerEntityHagathaSkillEffects(state.playerEntities, playerId, state.gameRng)
  state = { ...state, playerEntities: refreshed.store, gameRng: refreshed.rng }
  loadouts.push({ seed, owned, config, offerSeed: 12345, initial, replaced,
    refreshed: snapshot(state, refreshed.weirdCasterSkillId) })
}

writeFixture('hagatha-refresh-goldens.json', 'tools/website-oracle/hagatha-refresh-goldens.mjs',
  'website-rust-current/frontend/src/game/core-server/player-entity-store.ts::refreshPlayerEntityHagathaSkillEffects', {
    sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83', cases, loadouts,
  })
