// Public profile restoration and fresh-wizard hydration, including legacy migrations.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const saves = await loadKernel('save/game-save-document.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const content = await loadKernel('modding/content/mod-content-catalog.ts')
const { PreparedModAssetCatalog } = await loadKernel('modding/assets/mod-asset-compiler.ts')
const boast = await loadKernel('core-kernels/boast.ts')
const playerId = 'profile-owner'
const config = { displayName: 'Profile Oracle', element: 'fire', discipline: 'mind' }
const fresh = () => game.createGameSimulation({ [playerId]: config }, { gameRngSeed: 17, combatRngSeed: 91 })
const baseline = JSON.parse(saves.createGameProfileSaveDocument({
  state: fresh(), playerId, integrity: 'global-clean', mods: [], modState: {},
}))
const cases = []
function record(name, mutate) {
  const document = structuredClone(baseline)
  mutate(document)
  try {
    const profile = saves.restoreGameSaveProfile(JSON.stringify(document))
    const hydrated = saves.hydrateGameSaveProfile(fresh(), playerId, profile)
    cases.push({ name, document, accepted: true, economy: profile.economy,
      hagathaRuntime: profile.hagathaRuntime, players: hydrated.playerEntities })
  } catch (error) { cases.push({ name, document, accepted: false, error: error.message }) }
}
record('current', () => {})
for (const version of [5, 10, 11, 12, 13, 16, 17, 23, 24, 37]) {
  record(`schema-${version}`, document => {
    document.schemaVersion = version
    if (version < 17) delete document.nativeSource
    if (version < 13) delete document.profile.economy.collegeIntroPending
    if (version < 11) delete document.profile.economy.npc.helpFlags
  })
}
for (const [name, mutate] of [
  ['missing-intro', e => { delete e.collegeIntroPending }],
  ['missing-npc', e => { delete e.npc }],
  ['missing-help', e => { delete e.npc.helpFlags }],
  ['bad-boast', e => { e.npc.boast.failed = true }],
  ['missing-unforge', e => { delete e.unforgeBonuses }],
  ['missing-unforge-feedback', e => { if (e.actionFeedback) delete e.actionFeedback.unforgeOutcome }],
  ['tutorial-truthy', e => { e.tutorialPending = 1 }],
  ['missing-slots', e => { for (const item of e.backpack) delete item.inventorySlot }],
  ['duplicate-item', e => { e.backpack.push(structuredClone(e.backpack[0])) }],
  ['duplicate-perk', e => { e.ownedPerkSelectors = [7, 7] }],
  ['invalid-perk', e => { e.ownedPerkSelectors = [8] }],
  ['invalid-capacity', e => { e.charmCapacity = 4 }],
  ['owned-runtime', e => { e.ownedPerkSelectors = [7, 24, 25] }],
  ['old-vivid-appearance', e => {
    e.tutorialPending = false; e.collegeIntroPending = false
    e.equipment.hat.iconTints = [0xff1919, 0xffffff]
    e.equipment.robe.iconTints = [0xff1919, 0xffffff]
  }],
]) record(name, document => mutate(document.profile.economy))
for (const version of [16, 17, 23, 24, 37]) {
  record(`tonic-${version}`, document => {
    document.schemaVersion = version
    if (version < 17) delete document.nativeSource
    Object.assign(document.profile.economy, { ownedPerkSelectors: [0, 1, 2], tonicPurchases: 1, charmCapacity: 6 })
  })
  record(`overflow-${version}`, document => {
    document.schemaVersion = version
    if (version < 17) delete document.nativeSource
    Object.assign(document.profile.economy, { ownedPerkSelectors: [0, 1, 2, 3, 4, 5, 6, 27], tonicPurchases: 1, charmCapacity: 6 })
  })
  record(`unowned-runtime-${version}`, document => {
    document.schemaVersion = version
    if (version < 17) delete document.nativeSource
    document.profile.hagathaRuntime = { cheatDeathCharges: 1, reverieActive: true, serendipityActive: true }
  })
}
record('active-owned-runtime', document => {
  document.profile.economy.ownedPerkSelectors = [7, 24, 25]
  document.profile.hagathaRuntime = { cheatDeathCharges: 1, reverieActive: true, serendipityActive: true }
})
record('bad-runtime-charge', document => { document.profile.hagathaRuntime.cheatDeathCharges = 2 })
record('unknown-runtime-field', document => { document.profile.hagathaRuntime.unknown = 1 })

const definitions = JSON.parse(readFileSync(resolve(fixturesDir, 'mod-content-goldens.json'), 'utf8'))
  .cases.find(row => row.name === 'all-sixteen-families')
const catalog = content.compileModContentCatalog(definitions.mods, new PreparedModAssetCatalog(definitions.assets))
const byKey = key => catalog.all().find(row => row.key === key)
record('authored-items-affix-potion-and-boast', document => {
  let state = document.profile.economy
  for (const item of [content.modItemInventoryItem(byKey('robe').catalog),
    content.modItemInventoryItem(byKey('token').catalog, 6),
    content.modConsumableInventoryItem(byKey('tonic').catalog)]) {
    state = economy.insertLootInventoryItem(state, item).state
  }
  const robeId = state.backpack.find(item => item.modItemContent?.key === 'robe').id
  state = economy.reforgeModEquipment(state, robeId, [{
    contentId: byKey('lucky').contentId, modId: 'asset.test', name: 'Lucky',
    modifiers: [{ key: 'mana_spend', operation: 'multiply', value: .5 }],
  }]).state
  state = { ...state, npc: { ...state.npc,
    boast: boast.selectBoast(boast.createBoastState(), byKey('vow')) } }
  document.profile.economy = state
})

let feedbackState = fresh()
let feedbackEconomy = { ...game.getPlayerEconomy(feedbackState, playerId), gold: 1000000 }
feedbackEconomy = economy.insertLootInventoryItem(feedbackEconomy,
  economy.createEquipmentInventoryItem(economy.DOWSING_EQUIPMENT_RECIPES[0], 0)).state
feedbackState = { ...feedbackState, playerEntities: entities.replacePlayerEconomy(
  feedbackState.playerEntities, playerId, feedbackEconomy,
) }
const feedbackDocument = JSON.parse(saves.createGameSaveDocument({
  state: feedbackState, playerId, integrity: 'global-clean', mods: [], modState: {}, loadedBoneyard: null,
}))
feedbackState = saves.restoreGameSaveDocument(JSON.stringify(feedbackDocument)).state
const feedbackActions = []
const action = command => {
  const result = game.applyGameSimulationHubAction(feedbackState, playerId, command)
  feedbackState = result.state
  const state = game.getPlayerEconomy(feedbackState, playerId)
  feedbackActions.push({ command, accepted: result.accepted, feedback: state.actionFeedback,
    revision: state.revision, rng: state.rng })
}
action({ type: 'buy-fomentius', itemId: 999999 })
action({ type: 'close-hagatha' })
action({ type: 'dowse' })
action({ type: 'close-dowsing' })
const potionId = game.getPlayerEconomy(feedbackState, playerId).backpack[0].id
action({ type: 'transfer', itemId: potionId, direction: 'to-storage', gesture: 'drag' })
action({ type: 'transfer', itemId: potionId, direction: 'to-backpack', gesture: 'double-activation' })
action({ type: 'acknowledge-npc-hint', interactionId: 'teacher' })
action({ type: 'unequip', slot: 'hat' })
const ringId = game.getPlayerEconomy(feedbackState, playerId).backpack.find(item => item.recipeIndex === 0).id
action({ type: 'unforge', itemId: ringId })
const finalFeedback = game.getPlayerEconomy(feedbackState, playerId).actionFeedback
record('persisted-actual-action-feedback', document => {
  document.profile.economy.actionFeedback = finalFeedback
})
record('legacy-feedback-missing-unforge-outcome', document => {
  document.profile.economy.actionFeedback = { ...feedbackActions[0].feedback }
  delete document.profile.economy.actionFeedback.unforgeOutcome
})
const archived = economy.archiveCompletedRunEconomy(game.getPlayerEconomy(feedbackState, playerId), {
  displayName: 'Profile Oracle', groundGold: 0, groundItems: [], starterElement: 'fire', transferCarriedItems: false,
})
writeFixture('save-profile-goldens.json', 'tools/website-oracle/save-profile-goldens.mjs',
  'Website/frontend/src/game/save/game-save-document.ts restoreGameSaveProfile/hydrateGameSaveProfile',
  { playerId, config, cases, feedbackDocument, feedbackActions, archivedFeedback: archived.actionFeedback })
