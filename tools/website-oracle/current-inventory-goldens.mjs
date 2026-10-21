import { loadKernel, writeFixture } from './oracle.mjs'

const economy = await loadKernel('core-kernels/hub-economy.ts')
const equipment = (type, id) => economy.createEquipmentInventoryItem(
  economy.DOWSING_EQUIPMENT_RECIPES.find(recipe => recipe.type === type), id)
const sack = (id, contents = []) => ({
  ...economy.createFomentiusInventoryItem(economy.FOMENTIUS_STOCK_DEFINITIONS[5], id), contents,
})
const potion = id => economy.createFomentiusInventoryItem(economy.FOMENTIUS_STOCK_DEFINITIONS[0], id)

const names = [
  '', '   ', '  My Sack  ', '\ufeffMy Sack\ufeff', 'a'.repeat(32), 'a'.repeat(33),
  '😀'.repeat(16), '😀'.repeat(17), 'bad\nname', 'bad\u0085name', '\u200b\u2066\u2069',
  '\u200bA', '\u2028A\u2029', 'A\u0000B',
].map(value => ({ value, result: economy.normalizeInventorySackName(value) }))
// Source uses Unicode Format, rather than ASCII whitespace, for the invisible-only gate.
// Retain every member from the same V8 Unicode table used by its unmodified function.
const formatNames = []
for (let point = 0; point <= 0x10ffff; point += 1) {
  const value = String.fromCodePoint(point)
  if (/\p{Cf}/u.test(value)) formatNames.push({ value, result: economy.normalizeInventorySackName(value) })
}

function snapshot(state) {
  const project = items => economy.projectInventoryItems(items).map(({ item, parentSackId, slot }) => ({
    id: item.id, name: item.name, parentSackId, slot, storedSlot: item.inventorySlot ?? null,
  }))
  return { backpack: project(state.backpack), storage: project(state.storage), revision: state.revision,
    equipment: ['hat', 'robe', 'weapon', 'amulet'].map(slot => state.equipment[slot]?.id ?? null),
    rings: state.equipment.rings.map(item => item?.id ?? null), rng: state.rng,
    valid: economy.hubEconomyInventoryIsValid(state) }
}

const renameInitial = { ...economy.createHubEconomy(1),
  backpack: [sack(100, [sack(101)]), potion(102)], storage: [sack(200, [sack(201)])] }
let state = renameInitial
const renames = []
for (const [itemId, name] of [[101, '  School Supplies  '], [201, '\ufeffStored Gear\ufeff'],
  [100, '😀'.repeat(16)], [100, '😀'.repeat(17)], [999999, '\u200b'],
  [102, 'Potion'], [999999, 'Missing']]) {
  const result = economy.renameInventorySack(state, itemId, name)
  state = result.state
  renames.push({ itemId, name, accepted: result.accepted, reason: result.reason, state: snapshot(state) })
}

const unequips = []
for (const [name, slot, destinationSackId, destinationSlot, mode] of [
  ['root-requested-cell', 'weapon', null, 12, 'normal'],
  ['root-occupied-cell-swaps', 'weapon', null, 0, 'normal'],
  ['nested-requested-cell', 'weapon', 100, 4, 'normal'],
  ['nested-occupied-cell-swaps', 'weapon', 100, 0, 'normal'],
  ['nested-visible-boundary', 'weapon', 100, 87, 'normal'],
  ['non-sack-target', 'weapon', 102, 1, 'normal'],
  ['required-robe', 'robe', 999999, 999999, 'normal'],
  ['locked-third-ring', 'ring-2', null, null, 'normal'],
  ['default-allows-hidden-sack-cell', 'weapon', 100, null, 'full-visible'],
  ['requested-cell-needs-visible-space', 'weapon', 100, 0, 'full-visible'],
]) {
  let source = { ...economy.createHubEconomy(1), backpack: [sack(100, [potion(103)]), potion(102)] }
  if (mode === 'full-visible') source = { ...source, backpack: [sack(100,
    Array.from({ length: 87 }, (_, index) => ({ ...equipment('ring', 1000 + index), inventorySlot: index })))] }
  const result = economy.unequipInventorySlot(source, slot, destinationSackId, destinationSlot)
  unequips.push({ name, slot, destinationSackId, destinationSlot, mode,
    initial: snapshot(source), accepted: result.accepted, reason: result.reason, state: snapshot(result.state) })
}

const equips = []
for (const [name, sourceSlot, destinationSlot, mode] of [
  ['equipped-rings-swap', 'ring-0', 'ring-1', 'ordinary'],
  ['equipped-ring-same-slot', 'ring-0', 'ring-0', 'ordinary'],
  ['equipped-ring-empty-slot', 'ring-0', 'ring-1', 'empty-ring'],
  ['equipped-third-ring-locked', 'ring-0', 'ring-2', 'ordinary'],
  ['equipped-third-ring-unlocked', 'ring-0', 'ring-2', 'third-ring'],
  ['equipped-hat-wrong-slot', 'hat', 'ring-0', 'ordinary'],
]) {
  const original = economy.createHubEconomy(1)
  const source = { ...original,
    ownedPerkSelectors: mode === 'third-ring' ? [19] : [],
    equipment: { ...original.equipment, rings: [equipment('ring', 900),
      mode === 'empty-ring' ? null : equipment('ring', 901),
      mode === 'third-ring' ? equipment('ring', 902) : null] },
  }
  const itemId = sourceSlot === 'hat' ? source.equipment.hat.id : source.equipment.rings[0].id
  const result = economy.equipInventoryItem(source, itemId, destinationSlot,
    { playerLevel: 1000, creativityRank: 0 })
  equips.push({ name, sourceSlot, destinationSlot, mode, itemId,
    initial: snapshot(source), accepted: result.accepted, reason: result.reason, state: snapshot(result.state) })
}

const bulk = []
for (const mode of ['displaced-wand', 'addressed-child-order']) {
  const original = economy.createHubEconomy(1)
  const contents = mode === 'displaced-wand' ? [equipment('staff', 101)] : [
    { ...equipment('staff', 101), inventorySlot: 7 },
    { ...equipment('staff', 102), inventorySlot: 2 },
    { ...equipment('robe', 103), inventorySlot: 5 },
    { ...equipment('ring', 104), inventorySlot: 9 },
    { ...equipment('ring', 105), inventorySlot: 1 },
  ]
  const source = { ...original, backpack: [sack(100, contents)],
    equipment: { ...original.equipment, weapon: equipment('wand', 900) } }
  const result = economy.equipEligibleInventorySackContents(source, 100,
    { playerLevel: 1000, creativityRank: 0 })
  bulk.push({ mode, initial: snapshot(source), accepted: result.accepted,
    reason: result.reason, state: snapshot(result.state) })
}

writeFixture('current-inventory-goldens.json', 'tools/website-oracle/current-inventory-goldens.mjs',
  'website-rust-current/frontend/src/game/core-kernels/hub-economy.ts::normalizeInventorySackName/renameInventorySack/unequipInventorySlot/equipInventoryItem/equipEligibleInventorySackContents', {
    sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83', unicode: process.versions.unicode,
    names, formatNames, renameInitial: snapshot(renameInitial), renames, unequips, equips, bulk,
  })
