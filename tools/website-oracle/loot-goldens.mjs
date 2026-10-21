// Loot construction must retain the private selector and shared constructor streams.
import { loadKernel, writeFixture } from './oracle.mjs'

const loot = await loadKernel('core-kernels/native-loot.ts')
const items = await loadKernel('core-kernels/native-loot-items.ts')
const random = await loadKernel('core-kernels/native-random-equipment.ts')
const { createNativeRng } = await loadKernel('core-kernels/native-rng.ts')
const { selectNativeLootCandidate } = await loadKernel('core-kernels/native-loot-selection.ts')
const { DOWSING_EQUIPMENT_RECIPES } = await loadKernel('core-kernels/hub-economy.ts')
const { direBossItemRecipePool } = await loadKernel('core-kernels/dire-boss-loot.ts')
const { NATIVE_SURVIVAL_BOSS_SOURCES } = await loadKernel('core-kernels/native-survival-boss-catalog.ts')

const disabled = { gold: 4, item: 4, orb: 4, potion: 4, powerup: 4, specificItem: 0 }
const availability = (seed) => Array.from({ length: 8 }, (_, i) => seed % 3 === 1 || (seed % 3 === 2 && i % 2 === 0))
const equipment = []
for (const type of ['hat', 'robe', 'staff', 'wand', 'ring', 'amulet']) {
  for (const level of [-1, 1, 18, 19, 40, 80]) {
    for (let seed = 1; seed <= 64; seed++) {
      const advancedUnlocks = availability(seed)
      equipment.push({ type, level, seed, advancedUnlocks,
        expected: random.generateNativeRandomEquipmentEffects(createNativeRng(seed), type, level, { advancedUnlocks }) })
    }
  }
}

function input(seed, overrides = {}) {
  return {
    actorSeed: seed, arena: { disableMask: 0, itemLevelMaximum: 100, itemLevelMinimum: 0,
      lastSuccessfulItemLevel: -1, level: 10, mode: 0, specialSuppression: false },
    explicitGoldAmount: null, dropDelayContext: 0, inventoryHasHealthPotion: false,
    itemIds: items.createNativeLootItemIds(1), key: { current: 1, level: 0, remaining: 0 },
    nearbyMaskTwoCount: 0, participant: { advancedUnlocks: availability(seed), level: 12,
      modifiers: loot.nativeLootModifiers([]), ownedRecipeIndexes: [], slot: 0 },
    placement: loot.NATIVE_LOOT_OPEN_PLACEMENT, policies: { ...disabled },
    sceneForcesHealthPotion: false, sharedRng: createNativeRng(seed * 11 + 1),
    sourcePosition: { x: 100, y: 200 }, worldBadguyCount: 0, worldHasHealthPotionSack: false,
    ...overrides,
  }
}
function wireInput(source) {
  const { itemIds, placement, ...plain } = source
  return plain
}
function record(source, action = null, blocked = false) {
  source.placement = blocked ? { canPlace: ({ x, y }) => x < 70 || x > 125 || y < 165 || y > 230 } : loot.NATIVE_LOOT_OPEN_PLACEMENT
  const selected = selectNativeLootCandidate(source)
  const result = action ? loot.materializeNativeLootScriptAction(source, action) : loot.rollNativeEnemyLoot(source)
  const { itemIds, ...expected } = result
  return { input: wireInput(source), action, blocked, selected, expected, nextItemId: source.itemIds.peek() }
}
const rolls = []
for (const level of [1, 4, 5, 19, 20, 41]) {
  for (let policy = 0; policy <= 5; policy++) {
    for (let seed = 1; seed <= 20; seed++) {
      const source = input(seed)
      source.arena.level = level
      source.participant.level = level
      source.key = { current: 1, level, remaining: 2 }
      for (const category of ['gold', 'orb', 'item', 'potion', 'powerup']) source.policies[category] = policy
      rolls.push(record(source))
    }
  }
}
for (const category of ['gold', 'orb', 'item', 'potion', 'powerup']) {
  for (let seed = 1; seed <= 40; seed++) {
    const source = input(seed)
    source.policies[category] = 3
    source.participant.slot = seed % 5 === 0 ? 1 : 0
    source.participant.modifiers = loot.nativeLootModifiers([3, 4, 9, 23], { goldAmount: 1.5, orbPull: 2, pickupFactor: 1.75 })
    source.sceneForcesHealthPotion = seed % 4 === 0
    source.policies.specificItem = seed % 5
    source.arena.mode = seed % 2
    source.arena.lastSuccessfulItemLevel = seed % 2 === 0 ? source.arena.level : -1
    source.dropDelayContext = seed % 3 === 0 ? 40 : 0
    rolls.push(record(source, null, seed % 7 === 0))
  }
}
// Disabled/suppressed candidates and the emergency-potion branch retain different draws.
for (let seed = 1; seed <= 100; seed++) {
  const source = input(seed)
  source.policies = { gold: 0, item: 0, orb: 0, potion: 0, powerup: 0, specificItem: 0 }
  source.arena.disableMask = seed % 64
  source.arena.specialSuppression = seed % 3 === 0
  source.worldBadguyCount = 100
  source.nearbyMaskTwoCount = 60
  source.inventoryHasHealthPotion = seed % 4 === 0
  source.worldHasHealthPotionSack = seed % 7 === 0
  rolls.push(record(source))
}
const scripts = []
for (const current of [5, 20, 30, 45]) {
  for (let seed = 1; seed < 100_000; seed++) {
    const source = input(seed)
    source.key = { current, level: current, remaining: 2 }
    if (selectNativeLootCandidate(source).category === 'key') {
      rolls.push(record(source, null, true))
      break
    }
  }
}
const actions = [
  { kind: 'drop-gold', amount: 1 }, { kind: 'drop-gold', amount: 800 },
  { kind: 'drop-random-gold', minimum: 300, maximum: 600 },
  { kind: 'drop-random-gold', minimum: 5, maximum: 5 }, { kind: 'drop-key' },
  ...Array.from({ length: 6 }, (_, subtype) => ({ kind: 'drop-potion', subtype })),
  ...Array.from({ length: 5 }, (_, mode) => ({ kind: 'drop-random-item', mode })),
  ...DOWSING_EQUIPMENT_RECIPES.map(({ sourceIndex }) => ({ kind: 'drop-item', recipeIndex: sourceIndex })),
  { kind: 'drop-item', recipeIndex: 999 },
]
for (const action of actions) {
  for (const seed of [1, 3, 42]) scripts.push(record(input(seed), action, seed === 3))
}
const goodies = []
for (let selector = 0; selector < 18; selector++) {
  for (const seed of [1, 3, 42, 731]) {
    const itemIds = items.createNativeLootItemIds(17)
    const source = { advancedUnlocks: availability(seed), ownedRecipeIndexes: seed === 731 ? DOWSING_EQUIPMENT_RECIPES.map(({ sourceIndex }) => sourceIndex) : [],
      playerLevel: seed % 50 + 1, selector, sharedRng: createNativeRng(seed) }
    const { itemIds: ignored, ...expected } = items.resolveNativeGoodieContents({ ...source, itemIds })
    goodies.push({ input: source, expected, nextItemId: itemIds.peek() })
  }
}
const keys = []
for (const current of [0, 5, 12, 13, 25, 26, 40, 41, 99]) {
  for (const seed of [1, 42]) keys.push({ current, seed, expected: loot.advanceNativeKeyDropLevel(createNativeRng(seed), current) })
}
const allRecipes = DOWSING_EQUIPMENT_RECIPES.map(recipe => recipe.sourceIndex)
const currentGoodies = []
for (const [name, ownedRecipeIndexes, groundRecipeIndexes] of [
  ['only-missing-ground-recipe', [], allRecipes.filter(index => index !== 37)],
  ['all-recipes-on-ground', [], allRecipes],
  ['last-unowned-recipe-on-ground', allRecipes.filter(index => index !== 38), [38]],
]) {
  for (const seed of [1, 42]) {
    const itemIds = items.createNativeLootItemIds(17)
    const source = { advancedUnlocks: availability(seed), ownedRecipeIndexes, groundRecipeIndexes,
      playerLevel: 12, selector: 10, sharedRng: createNativeRng(seed) }
    const { itemIds: ignored, ...expected } = items.resolveNativeGoodieContents({ ...source, itemIds })
    currentGoodies.push({ name, input: source, expected, nextItemId: itemIds.peek() })
  }
}
const currentItems = []
for (const [name, owned, ground, itemRecipePool] of [
  ['only-missing-ground-recipe', [], allRecipes.filter(index => index !== 37), undefined],
  ['all-recipes-on-ground', [], allRecipes, undefined],
  ['owned-and-ground-exclusion', allRecipes.filter(index => ![20, 37].includes(index)), [20], undefined],
  ['faculty-pool-ignores-ground', [], [11, 12, 13, 14, 15], [11, 12, 13, 14, 15]],
  ['faculty-prefers-unowned', [20, 21], [3], [20, 21, 3]],
  ['faculty-owned-pool-fallback', [11, 12, 13, 14, 15], [], [11, 12, 13, 14, 15]],
  ['pool-retains-duplicate-weight', [], [], [11, 11, 12]],
]) {
  for (const seed of [1, 3, 42]) {
    const source = input(seed)
    source.arena.mode = 1
    source.policies.specificItem = 4
    source.participant.ownedRecipeIndexes = owned
    source.groundRecipeIndexes = ground
    if (itemRecipePool !== undefined) source.itemRecipePool = itemRecipePool
    const expected = items.selectEnemyItem(source.sharedRng, source, itemRecipePool)
    currentItems.push({ name, input: wireInput(source), expected, nextItemId: source.itemIds.peek() })
  }
}
const facultyPools = []
for (const source of NATIVE_SURVIVAL_BOSS_SOURCES) {
  for (const name of source.faculty.spawnOrder) {
    const recipeUid = source.recipeUids[name]
    facultyPools.push({ sourceSha256: source.sourceSha256, token: 'DIREFACULTY', recipeUid,
      expected: direBossItemRecipePool(source.sourceSha256, 'DIREFACULTY', recipeUid) ?? null })
  }
}
for (const [sourceSha256, token, recipeUid] of [
  [undefined, 'DIREFACULTY', 1], ['unknown', 'DIREFACULTY', 1],
  [NATIVE_SURVIVAL_BOSS_SOURCES[0].sourceSha256, 'SKELETON', 1],
  [NATIVE_SURVIVAL_BOSS_SOURCES[0].sourceSha256, 'DIREFACULTY', undefined],
]) facultyPools.push({ sourceSha256: sourceSha256 ?? null, token, recipeUid: recipeUid ?? null,
  expected: direBossItemRecipePool(sourceSha256, token, recipeUid) ?? null })
writeFixture('loot-goldens.json', 'tools/website-oracle/loot-goldens.mjs',
  'website-rust-current core-kernels/native-loot{,-selection,-items}.ts, native-random-equipment.ts and dire-boss-loot.ts',
  { sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83',
    equipment, rolls, scripts, goodies, keys, currentGoodies, currentItems, facultyPools })
console.log(JSON.stringify({ equipment: equipment.length, rolls: rolls.length, scripts: scripts.length, goodies: goodies.length, keys: keys.length }))
