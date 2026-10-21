// Execute the unchanged TSX producer, not a handwritten expected-action table.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { gameDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const ts = (await import(pathToFileURL(resolve(websiteDir, 'frontend/node_modules/typescript/lib/typescript.js')).href)).default
const economy = await loadKernel('core-kernels/hub-economy.ts')
const belt = await loadKernel('core-kernels/native-belt.ts')
const presentation = await loadKernel('hub-inventory-presentation.ts')
const contract = await loadKernel('renderer/hub-inventory-render-contract.ts')
const quickbar = await loadKernel('skill-book-model.ts')
const equipment = await loadKernel('hub-inventory-equipment.ts')
const pointer = await loadKernel('hub-inventory-pointer.ts')
const notices = await loadKernel('hub-inventory-notices.ts')
const hud = await loadKernel('native-hud-layout.ts')
const ast = ts.createSourceFile('HubInventoryActions.tsx', readFileSync(resolve(gameDir, 'HubInventoryActions.tsx'), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX)
const functions = ['inventoryVisibleSlotAtPoint', 'dropInventorySource'].map(name => {
  const declaration = ast.statements.find(node => ts.isFunctionDeclaration(node) && node.name?.text === name)
  if (!declaration) throw new Error(`Missing actual Source function ${name}`)
  return declaration.getText(ast)
}).join('\n')
const bindings = {
  findInventoryItem: economy.findInventoryItem,
  inventoryItemsAtSackPath: economy.inventoryItemsAtSackPath,
  inventoryItemsShareStack: economy.inventoryItemsShareStack,
  nativeInventoryItemCanUnforge: economy.nativeInventoryItemCanUnforge,
  projectInventoryItems: economy.projectInventoryItems,
  projectInventoryRootSlots: economy.projectInventoryRootSlots,
  nativeInventoryItemCanBindToBelt: belt.nativeInventoryItemCanBindToBelt,
  equipmentSlotsForItem: presentation.equipmentSlotsForItem,
  nativeSkillQuickbarDropSlot: quickbar.nativeSkillQuickbarDropSlot,
  itemAtEquipmentSlot: equipment.itemAtEquipmentSlot,
  pointInRect: pointer.pointInRect,
  HUB_INVENTORY_GRID: contract.HUB_INVENTORY_GRID,
  HUB_UNFORGE_TARGET: contract.HUB_UNFORGE_TARGET,
  HUB_HAT_REMOVAL_MSGBOX: contract.HUB_HAT_REMOVAL_MSGBOX,
  HUB_ROBE_REMOVAL_MSGBOX: contract.HUB_ROBE_REMOVAL_MSGBOX,
  HUB_UNFORGE_CONFIRMATION_NOTICE: notices.HUB_UNFORGE_CONFIRMATION_NOTICE,
  hubInventoryEquipmentSlotRects: contract.hubInventoryEquipmentSlotRects,
  hubInventorySlotPosition: contract.hubInventorySlotPosition,
  hubInventoryVisibleSlot: contract.hubInventoryVisibleSlot,
  hubInventoryRootSlot: contract.hubInventoryRootSlot,
}
for (const [name, value] of Object.entries(bindings)) if (value === undefined) throw new Error(`Missing Source binding ${name}`)
const javascript = ts.transpileModule(functions, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None } }).outputText
const drop = new Function(...Object.keys(bindings), `${javascript}\nreturn dropInventorySource`)(...Object.values(bindings))
const recipe = type => {
  const index = economy.DOWSING_EQUIPMENT_RECIPES.findIndex(row => row.type === type)
  if (index < 0) throw new Error(`Missing actual equipment recipe for ${type}`)
  return index
}
const ring = (id, slot) => ({ id, slot, recipe: recipe('ring') })
const stock = (kind, id, slot, contents) => ({ kind, id, slot, ...(contents ? { contents } : {}) })
const health = (id, slot) => stock('health-potion', id, slot)
const sack = (id, slot, contents = []) => stock('sack', id, slot, contents)
const center = slot => { const point = contract.hubInventorySlotPosition(slot); return { x: point.x + 36, y: point.y + 36 } }
const rectCenter = rect => ({ x: rect[0] + rect[2] / 2, y: rect[1] + rect[3] / 2 })
const beltRects = hud.nativeHudModalSlideLayout(1600, 900, 1).belt
const grid = contract.HUB_SHOP_GRID
const storageRect = [grid.left, grid.top, (grid.columns - 1) * grid.pitchX + grid.cellSize, (grid.rows - 1) * grid.pitchY + grid.cellSize]
const referenceRect = contract.HUB_DOWSING_PREROLL.referenceDropRect
const cases = []
function add(name, values = {}) {
  cases.push({ name, backpack: [ring(1, 0), health(2, 1)], equipment: [], source: { owner: 'backpack', itemId: 1, equipmentSlot: null }, point: center(2), path: [], bankStart: 0, companion: false, storage: false, reference: false, unlocked: false, ...values })
}
add('empty destination dispatches immediately')
add('same slot restores', { point: center(0) })
add('outside grid restores', { point: { x: 1200, y: 440 } })
add('occupied destination makes two delayed swap lanes', { point: center(1) })
add('matching stack stays immediate', { backpack: [health(1, 0), health(2, 1)], point: center(1) })
add('empty backpack produces no action', { backpack: [] })
add('drop into a nested Sack', { backpack: [ring(1, 0), sack(2, 1)], point: center(1) })
add('nested source parent holder moves outward', { backpack: [sack(3, 0, [sack(4, 0, [ring(1, 0)])])], path: [3, 4], point: center(0) })
add('nested empty destination keeps address', { backpack: [sack(3, 0, [ring(1, 0)])], path: [3], point: center(2) })
add('nested swap keeps address', { backpack: [sack(3, 0, [ring(1, 0), health(2, 1)])], path: [3], point: center(2) })
add('storage drops carry Drag', { companion: true, storage: true, point: rectCenter(storageRect) })
add('Shlorio references borrowed backpack item then restores', { companion: true, reference: true, point: rectCenter(referenceRect) })
for (const slot of [0, 7]) {
  const rect = beltRects[slot]
  add(`belt ${slot} resolves bank3`, { bankStart: 16, point: { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 } })
}
for (const companion of [false, true]) for (const slot of ['ring-0', 'ring-2']) {
  add(`ring sink ${slot} companion=${companion}`, { companion, point: rectCenter(contract.hubInventoryEquipmentSlotRects(slot, companion)[0]) })
  add(`unlocked ${slot} companion=${companion}`, { companion, unlocked: true, point: rectCenter(contract.hubInventoryEquipmentSlotRects(slot, companion)[0]) })
}
add('equipment prompts before unforge', { point: rectCenter(contract.HUB_UNFORGE_TARGET.rect) })
add('potion cannot unforge and restores', { backpack: [health(1, 0)], point: rectCenter(contract.HUB_UNFORGE_TARGET.rect) })
add('empty Sack unforges directly', { backpack: [sack(1, 0)], point: rectCenter(contract.HUB_UNFORGE_TARGET.rect) })
add('filled Sack restores', { backpack: [sack(1, 0, [health(2, 0)])], point: rectCenter(contract.HUB_UNFORGE_TARGET.rect) })
for (const type of ['ring', 'hat', 'robe', 'staff', 'wand']) {
  const slot = type === 'ring' ? 'ring-0' : ['staff', 'wand'].includes(type) ? 'weapon' : type
  const equipped = { slot, item: { id: 10, recipe: recipe(type) } }
  const source = { owner: 'equipment', equipmentSlot: slot, itemId: 10 }
  add(`${type} to empty cell`, { equipment: [equipped], source })
  add(`${type} to occupied cell`, { equipment: [equipped], source, point: center(1) })
  add(`${type} to nested Sack`, { equipment: [equipped], source, backpack: [sack(3, 0)], point: center(0) })
  add(`${type} outside grid`, { equipment: [equipped], source, point: { x: 1200, y: 440 } })
  add(`${type} in borrowed Shlorio mirror`, { equipment: [equipped], source, companion: true, reference: true, point: rectCenter(referenceRect) })
}
function makeItem(spec) {
  const item = spec.recipe === undefined
    ? economy.createFomentiusInventoryItem(economy.FOMENTIUS_STOCK_DEFINITIONS.find(row => row.kind === spec.kind), spec.id, 1)
    : economy.createEquipmentInventoryItem(economy.DOWSING_EQUIPMENT_RECIPES[spec.recipe], spec.id)
  return { ...item, inventorySlot: spec.slot ?? null, ...(spec.contents ? { contents: spec.contents.map(makeItem) } : {}) }
}
function run(input) {
  const gear = { hat: null, robe: null, weapon: null, rings: [null, null, null], amulet: null }
  for (const { slot, item } of input.equipment) {
    if (slot.startsWith('ring-')) gear.rings[Number(slot.slice(5))] = makeItem(item)
    else gear[slot] = makeItem(item)
  }
  const owned = { backpack: input.backpack.map(makeItem), storage: [], equipment: gear, ownedPerkSelectors: input.unlocked ? [19] : [] }
  const effects = []
  drop(input.source, input.point, owned, input.unlocked,
    action => effects.push({ kind: 'action', action }),
    (itemId, slot) => effects.push({ kind: 'belt', itemId, slot: input.bankStart + slot }),
    request => effects.push({ kind: 'flyby', action: request.action, lanes: request.lanes.map(({ from, item, to }) => ({ from, to, itemId: item.id })) }),
    (cue, playbackRate) => effects.push({ kind: 'sound', cue, playbackRate }),
    notice => effects.push(notice === contract.HUB_HAT_REMOVAL_MSGBOX ? { kind: 'required-clothing', slot: 'hat' }
      : notice === contract.HUB_ROBE_REMOVAL_MSGBOX ? { kind: 'required-clothing', slot: 'robe' }
        : { kind: 'confirm-unforge', itemId: notice.unforgeItemId }),
    input.companion, input.path, beltRects, input.storage ? storageRect : null,
    input.reference ? { rect: referenceRect, select: itemId => effects.push({ kind: 'reference', itemId }) } : null)
  return effects
}
writeFixture('inventory-drop-goldens.json', 'inventory-drop-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 unchanged private HubInventoryActions.tsx dropInventorySource via TypeScript AST extraction',
  { beltRects, storageRect, referenceRect, cases: cases.map(input => ({ input, expected: run(input) })) })
