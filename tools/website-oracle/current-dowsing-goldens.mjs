import { loadKernel, writeFixture } from './oracle.mjs'

const economy = await loadKernel('core-kernels/hub-economy.ts')
const nativeRng = await loadKernel('core-kernels/native-rng.ts')
const recipes = economy.DOWSING_EQUIPMENT_RECIPES
const makeItem = index => economy.createEquipmentInventoryItem(recipes[index], 90000 + index)

function sourceFor(mode) {
  let state = { ...economy.createHubEconomy(1), gold: 20000 }
  if (mode !== 'ordinary') state = { ...state, rng: nativeRng.createNativeRng(mode === 'missing-37' ? 1394 : 1) }
  if (mode === 'hood') state = { ...state, backpack: [makeItem(16)] }
  if (mode === 'all' || mode === 'missing-37') state = {
    ...state, backpack: recipes.filter(recipe => mode !== 'missing-37' || recipe.sourceIndex !== 37)
      .map(recipe => makeItem(recipe.sourceIndex)),
  }
  if (mode === 'nested') {
    const sack = economy.createFomentiusInventoryItem(economy.FOMENTIUS_STOCK_DEFINITIONS[5], 99000)
    sack.contents = recipes.filter(recipe => ![5, 16, 17, 19].includes(recipe.sourceIndex))
      .map(recipe => makeItem(recipe.sourceIndex))
    state = { ...state, backpack: [makeItem(16), sack], storage: [makeItem(19)],
      equipment: { ...state.equipment, hat: makeItem(5) } }
  }
  return state
}

function snapshot(state) {
  return {
    gold: state.gold,
    dowsingFee: state.dowsingFee,
    dowsingOffers: state.dowsingOffers,
    dowsingRolled: state.dowsingRolled,
    nextItemId: state.nextItemId,
    nextOfferId: state.nextOfferId,
    revision: state.revision,
    rng: state.rng,
    backpack: economy.projectInventoryItems(state.backpack).map(({ item }) => [item.id, item.recipeIndex]),
    storage: economy.projectInventoryItems(state.storage).map(({ item }) => [item.id, item.recipeIndex]),
  }
}

const cases = []
function record(name, mode, reference, operations) {
  let state = sourceFor(mode)
  const initial = snapshot(state)
  const records = []
  for (const operation of operations) {
    const input = { type: operation }
    let result
    if (operation === 'dowse') {
      input.referenceItemId = reference === 'potion'
        ? state.backpack.find(item => item.kind === 'health-potion').id : reference
      result = economy.dowse(state, input.referenceItemId)
    } else if (operation === 'buy') {
      input.offerId = state.dowsingOffers[0]?.id ?? 99999
      result = economy.buyDowsingOffer(state, input.offerId)
    } else {
      result = { accepted: true, reason: null, dowsingPitch: null, state: economy.closeDowsingOffers(state) }
    }
    state = result.state
    records.push({ input, accepted: result.accepted, reason: result.reason,
      dowsingPitch: result.dowsingPitch, state: snapshot(state) })
  }
  cases.push({ name, mode, initial, records })
}
record('untargeted-native-selection-before-prices', 'ordinary', null, ['dowse', 'buy', 'dowse', 'close', 'dowse'])
record('reference-set-before-type-and-paid-lock', 'hood', 90016, ['dowse', 'buy', 'dowse', 'close', 'dowse'])
record('unowned-reference-is-atomic', 'ordinary', 999999, ['dowse'])
record('empty-reference-still-locks', 'potion', 'potion', ['dowse', 'dowse', 'close', 'dowse'])
record('exhausted-catalog-bounded-retries', 'all', null, ['dowse', 'dowse', 'close'])
record('hundredth-draw-is-rejected', 'missing-37', null, ['dowse'])
record('nested-storage-and-equipped-ownership', 'nested', 90016, ['dowse'])

writeFixture('current-dowsing-goldens.json', 'tools/website-oracle/current-dowsing-goldens.mjs',
  'website-rust-current/frontend/src/game/core-kernels/hub-economy.ts::dowse/buyDowsingOffer/closeDowsingOffers', {
    sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83', cases,
  })
