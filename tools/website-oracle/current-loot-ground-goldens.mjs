#!/usr/bin/env node
// Actual store receivers must collect direct named items at each selection boundary.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const store = await loadKernel('core-server/boneyard-loot-store.ts')
const loot = await loadKernel('core-kernels/native-loot.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')
const painter = await loadKernel('core-kernels/native-world-manager-order.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const ground = economy.DOWSING_EQUIPMENT_RECIPES.filter(recipe => recipe.sourceIndex !== 39).map(recipe => recipe.sourceIndex)
const input = {
  actorSeed: 77, advancedUnlocks: new Array(8).fill(false), arena: { disableMask: 0, itemLevelMaximum: 100,
    itemLevelMinimum: 0, level: 31, mode: 0, specialSuppression: false }, inventoryHasHealthPotion: false,
  modifiers: loot.nativeLootModifiers([]), nearbyMaskTwoCount: 0, ownedRecipeIndexes: [], participantLevel: 31, participantSlot: 0,
  policies: { gold: 4, item: 3, orb: 4, potion: 4, powerup: 4, specificItem: 4 },
  position: { x: 100, y: 200 }, tick: 5, sceneForcesHealthPotion: false, worldBadguyCount: 100, worldHasHealthPotionSack: false,
}
const itemIdentity = item => item === null ? null : ({ id: item.id, nativeTypeId: item.nativeTypeId,
  recipeIndex: item.recipeIndex, contents: item.contents?.map(itemIdentity) ?? null })
const project = (state, order) => ({
  items: state.actors.map(actor => ({ id: actor.id, nativeTypeId: actor.nativeTypeId, item: itemIdentity(actor.item), itemId: actor.item?.id ?? null,
    recipeIndex: actor.item?.recipeIndex ?? null, amount: actor.amount, source: actor.source })),
  rng: state.sharedRng, nextActorId: state.nextActorId, nextItemId: state.nextItemId,
  lastSuccessfulItemLevel: state.lastSuccessfulItemLevel, nextKeyDropLevel: state.nextKeyDropLevel,
  order: order.state(),
})
const cases = []
for (const mode of ['enemy', 'goodies', 'miniboss']) for (const seed of ['ground-current-1', 'ground-current-2']) {
  const order = painter.createNativeWorldManagerOrder()
  const goodies = mode === 'goodies' ? ['first', 'second'].map(eid => ({ eid, position: { x: 100, y: 100 }, rewardSeed: 10 })) : []
  let state = store.createBoneyardLootStore(seed, goodies)
  state = store.spawnBoneyardCustomLootItems(state, ground.map(index => economy.createEquipmentInventoryItem(economy.DOWSING_EQUIPMENT_RECIPES[index], index + 1)), { x: 2000, y: 2000 }, 0, order.register).store
  const frames = []
  if (mode === 'goodies') {
    for (const goodie of goodies) state = store.activateBoneyardGoodie(state, goodie.eid, 'owner')
    for (let tick = 1; tick <= 250; tick += 1) {
      state = store.stepBoneyardLootStore(state, { tick, registerWorldPainter: order.register,
        participants: [{ playerId: 'owner', position: { x: -10000, y: -10000 }, alive: true, connected: true,
          advancedUnlocks: input.advancedUnlocks, headingIndex: 0, level: 31, modifiers: input.modifiers, ownedRecipeIndexes: [] }],
        placement: loot.NATIVE_LOOT_OPEN_PLACEMENT }).store
    }
    frames.push({ expected: project(state, order) })
  } else {
    for (let round = 0; round < 2; round += 1) {
      state = store.materializeBoneyardEnemyLoot(state, { ...input, tick: 5 + round,
        placement: loot.NATIVE_LOOT_OPEN_PLACEMENT, onDeathProgram: mode === 'miniboss' ? 'miniboss-die' : null }, order.register).store
      frames.push({ expected: project(state, order) })
    }
  }
  cases.push({ mode, seed, ground, frames })
}
writeFixture('current-loot-ground-goldens.json', 'tools/website-oracle/current-loot-ground-goldens.mjs',
  'current boneyard-loot-store.ts enemyLootSelectionInput/materializeGoodie direct ground recipes and sequential materialization',
  { sourceRevision, cases })
