// Stock standalone inventory and services, through the unmodified Website authority.
import { loadKernel, writeFixture } from './oracle.mjs'
const economy = await loadKernel('core-kernels/hub-economy.ts')
const rng = await loadKernel('core-kernels/native-rng.ts')

const item = value => ({
  id: value.id, kind: value.kind, name: value.name, nativeTypeId: value.nativeTypeId,
  nativeSubtype: value.nativeSubtype, quantity: value.quantity,
  inventorySlot: value.inventorySlot ?? null, equipmentType: value.equipmentType,
  recipeIndex: value.recipeIndex, generatedLevel: value.generatedLevel ?? null,
  iconRecords: value.iconRecords, iconTints: value.iconTints ?? null,
  nativeSelector: value.nativeSelector ?? null, nativeEffects: value.nativeEffects ?? null,
  rarity: value.rarity, contents: value.contents?.map(item) ?? null,
})
const state = value => ({
  backpack: value.backpack.map(item), storage: value.storage.map(item),
  equipment: {
    amulet: value.equipment.amulet && item(value.equipment.amulet),
    hat: value.equipment.hat && item(value.equipment.hat),
    robe: value.equipment.robe && item(value.equipment.robe),
    weapon: value.equipment.weapon && item(value.equipment.weapon),
    rings: value.equipment.rings.map(value => value && item(value)),
  },
  fomentiusStock: value.fomentiusStock.map(value => ({ item: item(value), price: value.price })),
  gold: value.gold, charmCapacity: value.charmCapacity, collegeIntroPending: value.collegeIntroPending,
  tutorialPending: value.tutorialPending, dowsingFee: value.dowsingFee,
  dowsingOffers: value.dowsingOffers, dowsingRolled: value.dowsingRolled,
  firstMixedSelectors: value.firstMixedSelectors,
  hagathaBundleSelectors: value.hagathaBundleSelectors, nextItemId: value.nextItemId,
  nextOfferId: value.nextOfferId, npc: value.npc, ownedPerkSelectors: value.ownedPerkSelectors,
  revision: value.revision, rng: value.rng, tonicPurchases: value.tonicPurchases,
  unforgeBonuses: value.unforgeBonuses,
})
const result = value => ({ accepted: value.accepted, reason: value.reason,
  dowsingPitch: value.dowsingPitch, dyeSession: value.session ?? null,
  unforgeOutcome: value.unforgeOutcome, state: state(value.state) })
const fresh = (seed = 1, element = 'ether') => economy.createHubEconomy(seed, { starterElement: element })
const rich = seed => ({ ...fresh(seed), gold: 1_000_000 })
const creations = [0, 1, 7, 12345, -1].flatMap(seed =>
  ['air', 'earth', 'ether', 'fire', 'water'].map(element => ({ seed, element, state: state(fresh(seed, element)) })))

const serviceCases = []
function service(name, source, operations) {
  let current = source
  const records = []
  for (const operation of operations) {
    const next = operation.run(current)
    current = next.state
    records.push({ operation: operation.input, result: result(next) })
  }
  serviceCases.push({ name, initial: state(source), records })
}
const op = (type, args, run) => ({ input: { type, ...args }, run })

for (const seed of [0, 1, 7, 12345]) {
  const source = rich(seed)
  service(`fomentius-${seed}`, source, [
    ...source.fomentiusStock.map(stock => op('buy-fomentius', { itemId: stock.id }, current => economy.buyFomentiusItem(current, stock.id))),
    op('buy-fomentius', { itemId: 999999 }, current => economy.buyFomentiusItem(current, 999999)),
  ])
}
service('fresh-gold-rejection', fresh(), [
  op('buy-hagatha', { selector: 0 }, current => economy.buyHagathaPerk(current, 0)),
  op('dowse', { playerLevel: 1 }, current => economy.dowse(current)),
])
service('hagatha-capacity-tonic-and-removal', rich(7), [
  ...[0, 1, 27, 19, 2, 3, 27, 4, 5, 6, 7, 8, 27].map(selector =>
    op('buy-hagatha', { selector }, current => economy.buyHagathaPerk(current, selector))),
  op('close-hagatha', {}, current => ({ accepted: true, reason: null, dowsingPitch: null, unforgeOutcome: null, state: economy.closeHagathaShop(current) })),
  op('remove-hagatha', { selector: 0 }, current => economy.removeHagathaPerk(current, 0)),
  op('remove-hagatha', { selector: 27 }, current => economy.removeHagathaPerk(current, 27)),
  op('buy-hagatha', { selector: 0 }, current => economy.buyHagathaPerk(current, 0)),
])
service('hagatha-bundle', { ...rich(2), hagathaBundleSelectors: [0, 27, 19, 1] }, [
  op('buy-hagatha', { selector: -1 }, current => economy.buyHagathaPerk(current, -1)),
])
for (const seed of [0, 1, 7, 99]) {
  for (const level of [1, 10, 75]) {
    let current = rich(seed)
    const first = economy.dowse(current)
    const offerId = first.state.dowsingOffers[0]?.id ?? 9999
    service(`shlorio-${seed}-${level}`, current, [
      // The sampled player levels are intentionally irrelevant to current Shlorio.
      op('dowse', { playerLevel: level }, value => economy.dowse(value)),
      op('dowse', { playerLevel: level }, value => economy.dowse(value)),
      op('buy-dowsing', { offerId }, value => economy.buyDowsingOffer(value, offerId)),
      op('buy-dowsing', { offerId }, value => economy.buyDowsingOffer(value, offerId)),
    ])
  }
}

const inventoryCases = []
function inventory(name, source, operations) {
  service(name, source, operations)
  inventoryCases.push(serviceCases.pop())
}
const equipment = (index, id) => economy.createEquipmentInventoryItem(economy.DOWSING_EQUIPMENT_RECIPES[index], id)
const potion = (index, id, quantity = 1) => economy.createFomentiusInventoryItem(economy.FOMENTIUS_STOCK_DEFINITIONS[index], id, quantity)
const sack = (id, contents = []) => ({ ...potion(5, id), contents })
const base = { ...rich(12345), backpack: [potion(0, 100, 2), potion(1, 101), sack(102, [equipment(29, 103), potion(0, 104), sack(105)]), equipment(39, 106)] }
inventory('nested-move-swap-stack-and-cycle', base, [
  op('move', { itemId: 101, destinationSackId: 102, destinationSlot: null }, value => economy.moveInventoryItem(value, 101, 102)),
  op('move', { itemId: 100, destinationSackId: 102, destinationSlot: null }, value => economy.moveInventoryItem(value, 100, 102)),
  op('move', { itemId: 102, destinationSackId: 105, destinationSlot: null }, value => economy.moveInventoryItem(value, 102, 105)),
  op('move', { itemId: 105, destinationSackId: 105, destinationSlot: null }, value => economy.moveInventoryItem(value, 105, 105)),
  op('move', { itemId: 103, destinationSackId: null, destinationSlot: 20 }, value => economy.moveInventoryItem(value, 103, null, 20)),
  op('move', { itemId: 106, destinationSackId: null, destinationSlot: 20 }, value => economy.moveInventoryItem(value, 106, null, 20)),
  op('transfer', { itemId: 102, direction: 'to-storage' }, value => economy.transferInventoryItem(value, 102, 'to-storage')),
  op('transfer', { itemId: 102, direction: 'to-backpack' }, value => economy.transferInventoryItem(value, 102, 'to-backpack')),
  op('consume', { itemId: 104 }, value => economy.consumeInventoryItem(value, 104)),
  op('consume', { itemId: 104 }, value => economy.consumeInventoryItem(value, 104)),
])
inventory('equipment-admission-and-swaps', base, [
  op('equip', { itemId: 103, slot: 'amulet', playerLevel: 1, creativityRank: 0 }, value => economy.equipInventoryItem(value, 103, 'amulet', { playerLevel: 1, creativityRank: 0 })),
  op('equip', { itemId: 103, slot: 'amulet', playerLevel: 6, creativityRank: 1 }, value => economy.equipInventoryItem(value, 103, 'amulet', { playerLevel: 6, creativityRank: 1 })),
  op('equip', { itemId: 106, slot: 'ring-2', playerLevel: 75, creativityRank: 0 }, value => economy.equipInventoryItem(value, 106, 'ring-2', { playerLevel: 75, creativityRank: 0 })),
  op('equip', { itemId: 106, slot: 'ring-0', playerLevel: 75, creativityRank: 0 }, value => economy.equipInventoryItem(value, 106, 'ring-0', { playerLevel: 75, creativityRank: 0 })),
  op('unequip', { slot: 'hat' }, value => economy.unequipInventorySlot(value, 'hat')),
  op('unequip', { slot: 'weapon' }, value => economy.unequipInventorySlot(value, 'weapon')),
  op('unequip', { slot: 'weapon' }, value => economy.unequipInventorySlot(value, 'weapon')),
  op('unequip', { slot: 'amulet' }, value => economy.unequipInventorySlot(value, 'amulet')),
])
const dyeSource = { ...rich(1), backpack: [sack(99, [potion(3, 100),
  { ...fresh(1).equipment.hat, id: 101 },
  equipment(economy.DOWSING_EQUIPMENT_RECIPES.find(recipe => recipe.type === 'robe').sourceIndex, 102),
])] }
let activeDyeSession = null
const openDye = sessionId => op('open-dye', { dyeItemId: 100, sessionId }, value => {
  const opened = economy.beginInventoryDyeSession(value, 100, sessionId)
  if (opened.accepted) activeDyeSession = opened.session
  return opened
})
const paint = (targetItemId, layer, swatchRows) => op('dye', {
  sessionId: 'retained-paint', targetItemId, layer, swatchRows,
}, value => {
  const painted = economy.dyeInventoryClothing(value, activeDyeSession, targetItemId, layer, swatchRows)
  if (painted.accepted) activeDyeSession = painted.session
  return painted
})
inventory('one-open-kit-paints-nested-layers-and-garments', dyeSource, [
  openDye(''), openDye('retained-paint'),
  paint(101, 'cloth', []), paint(101, 'cloth', [1, 5, 9]),
  openDye('missing-initiating-kit'),
  paint(101, 'trim', [0, 17]), paint(102, 'cloth', [5]), paint(99999, 'cloth', [1]),
])

const unforgeCases = []
for (let seed = 0; seed < 35; seed += 1) {
  let current = { ...rich(seed), rng: rng.createNativeRng(seed), backpack: [] }
  const records = []
  for (let index = 0; index < 12; index += 1) {
    const candidate = equipment(index % economy.DOWSING_EQUIPMENT_RECIPES.length, 1000 + index)
    current = { ...current, backpack: [candidate] }
    const vitals = { currentHealth: index % 2 ? 100 : 50, currentMana: 100, maximumHealth: 100, maximumMana: 100 }
    const next = economy.unforgeInventoryItem(current, candidate.id, vitals)
    records.push({ item: item(candidate), vitals, result: result(next) })
    current = next.state
  }
  unforgeCases.push({ seed, initial: state({ ...rich(seed), rng: rng.createNativeRng(seed), backpack: [] }), records })
}
const dyeCases = [[0], [1, 2], [1, 5, 9], [17, 0, 17], Array.from({ length: 256 }, (_, index) => index % 18)]
  .map(rows => ({ rows, mixed: economy.nativeDyeMixedTint(rows), committed: economy.nativeDyeCommittedTint(rows) }))
service('npc-books-boasts-and-teacher', rich(1), [
  op('select-boast', { boastId: 2 }, value => economy.selectHubBoast(value, 2)),
  op('select-boast', { boastId: 99 }, value => economy.selectHubBoast(value, 99)),
  op('read-librarian-book', { bookId: 0 }, value => economy.readLibrarianBook(value, 0)),
  op('read-librarian-book', { bookId: 25 }, value => economy.readLibrarianBook(value, 25)),
  op('read-librarian-book', { bookId: 25 }, value => economy.readLibrarianBook(value, 25)),
  op('buy-teacher-spell', { skillId: 72 }, value => economy.buyTeacherSpell(value, 72, Array(8).fill(false))),
  op('buy-teacher-spell', { skillId: 72, alreadyUnlocked: true }, value => economy.buyTeacherSpell(value, 72, Array(8).fill(true))),
])
const skillBookSource = { ...rich(1), backpack: economy.NATIVE_SKILL_BOOK_DEFINITIONS.map((definition, index) => economy.createNativeSkillBookInventoryItem(definition, 100 + index)) }
inventory('skill-books-remove-their-addressed-items', skillBookSource, [
  op('read-skill-book', { itemId: 100 }, value => economy.readInventorySkillBook(value, 100)),
  op('read-skill-book', { itemId: 101 }, value => economy.readInventorySkillBook(value, 101)),
])
const sackContents = [equipment(5, 301), equipment(2, 302), equipment(3, 303), equipment(0, 304), equipment(4, 305), equipment(10, 306)]
inventory('equip-direct-sack-contents-preserves-old-equipment', { ...rich(1), ownedPerkSelectors: [19], backpack: [sack(300, sackContents)] }, [
  op('equip-sack-contents', { itemId: 300, playerLevel: 75, creativityRank: 0 }, value => economy.equipEligibleInventorySackContents(value, 300, { playerLevel: 75, creativityRank: 0 })),
])
const archiveCases = []
for (const element of ['air', 'earth', 'ether', 'fire', 'water']) {
  for (const transferCarriedItems of [false, true]) {
    const initial = fresh(7, element)
    const source = {
      ...initial,
      collegeIntroPending: false,
      tutorialPending: false,
      nextItemId: 200,
      npc: {
        ...initial.npc,
        librarianLaceRead: true,
        helpFlags: [false, true, true, true, true, true, true, true, true, true],
        boast: { failed: false, failureSequence: 0, selected: 2, succeeded: true },
      },
      backpack: [...initial.backpack, sack(100, [potion(0, 101, 3), equipment(29, 102)])],
    }
    const archive = {
      displayName: 'Oracle',
      groundGold: 23,
      groundItems: [equipment(2, 999), sack(998, [potion(1, 997, 2)])],
      starterElement: element,
      transferCarriedItems,
    }
    archiveCases.push({
      name: `${element}-${transferCarriedItems}`,
      initial: state(source),
      archive: { ...archive, groundItems: archive.groundItems.map(item) },
      result: state(economy.archiveCompletedRunEconomy(source, archive)),
    })
  }
}
for (const boundary of ['full-storage', 'hidden-backpack']) {
  let source = { ...fresh(1), nextItemId: 50_000 }
  if (boundary === 'full-storage') {
    source = {
      ...source,
      storage: Array.from({ length: 28 }, (_, index) => ({
        ...sack(1000 + index), inventorySlot: index,
      })),
    }
  } else {
    source = {
      ...source,
      backpack: Array.from({ length: 2048 }, (_, index) => ({
        ...equipment(0, 1000 + index), inventorySlot: index,
      })),
    }
  }
  const archive = {
    displayName: 'Boundary',
    groundGold: 0,
    groundItems: Array.from({ length: 5 }, (_, index) => equipment(2, 9000 + index)),
    starterElement: 'ether',
    transferCarriedItems: true,
  }
  archiveCases.push({
    name: boundary,
    initial: state(source),
    archive: { ...archive, groundItems: archive.groundItems.map(item) },
    result: state(economy.archiveCompletedRunEconomy(source, archive)),
  })
}
const lastWordCases = []
for (const scenario of [
  { count: 0, fullStorage: false },
  { count: 3, fullStorage: false },
  { count: 2056, fullStorage: false },
  { count: 3, fullStorage: true },
]) {
  const source = { ...fresh(1), nextItemId: 50_000 }
  if (scenario.fullStorage) {
    source.storage = Array.from({ length: 28 }, (_, index) => ({
      ...sack(20_000 + index), inventorySlot: index,
    }))
  }
  const items = Array.from({ length: scenario.count }, (_, index) => equipment(0, 1000 + index))
  const result = economy.archiveHagathaLastWordItems(source, items, "Oracle's Loot")
  lastWordCases.push({
    initial: state(source),
    items: items.map(item),
    name: "Oracle's Loot",
    accepted: result.accepted,
    result: state(result.state),
  })
}
writeFixture('hub-economy-goldens.json', 'tools/website-oracle/hub-economy-goldens.mjs',
  'website-rust-current/frontend/src/game/core-kernels/hub-economy.ts', {
    sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83',
    catalog: {
      stock: economy.FOMENTIUS_STOCK_DEFINITIONS,
      recipes: economy.DOWSING_EQUIPMENT_RECIPES,
      perks: economy.HAGATHA_PERKS,
    },
    creations, serviceCases, inventoryCases, unforgeCases, dyeCases, archiveCases, lastWordCases,
  })
console.log(`${creations.length} fresh profiles, ${serviceCases.length} service journeys, ${inventoryCases.length} inventory journeys, ${unforgeCases.length * 12} Unforge outcomes`)
