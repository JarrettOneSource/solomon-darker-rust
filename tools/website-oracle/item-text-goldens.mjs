import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { gameDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const text = await loadKernel('renderer/hub-inventory-item-text.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')
const ui = await loadKernel('native-ui/core.ts')
const tutorial = await loadKernel('core-kernels/native-tutorial.ts')
const contract = await loadKernel('renderer/hub-inventory-render-contract.ts')

const effects = []
for (let kind = 1; kind <= 39; kind += 1) {
  for (const operator of [0, 1, 2]) for (const magnitude of [-2.5, -0.5, 0, 0.5, 1, 2.55, 17, 1e20, 1e21, -1e22]) {
    const target = [2, 6, 11, 13, 22].includes(kind) ? 2 : [4, 5, 7, 25].includes(kind) ? 8 : 0
    const effect = { kind, magnitude, operator, target }
    effects.push({ effect, expected: text.hubNativeEquipmentEffectText(effect) })
  }
}
for (const kind of [4, 5, 7]) {
  const effect = { kind, magnitude: 2, operator: 0, target: 0 }
  effects.push({ effect, expected: text.hubNativeEquipmentEffectText(effect) })
}

const hagatha = []
for (let selector = -1; selector < 28; selector += 1) {
  for (const price of [null, 0, 1234]) for (const firstMixed of [false, true]) {
    const options = { selector, price, firstMixed, bundleSelectors: [0, 1, 19], cheatDeathCharges: selector === 7 ? price === null ? 0 : 3 : null }
    hagatha.push({ options, expected: text.hubHagathaTooltipLines(options) })
  }
}

let nextId = 1
const base = (kind, name, extra = {}) => ({
  id: nextId++, kind, name, nativeTypeId: 7001, nativeSubtype: 0, quantity: 1,
  inventorySlot: null, iconRecords: [], equipmentType: null, recipeIndex: null,
  ...extra,
})
const samples = []
for (const kind of ['health-potion', 'mana-potion', 'wizard-chug', 'antidote', 'mind-chug', 'rejuvenation-potion', 'dye', 'key']) samples.push(base(kind, kind))
for (const count of [0, 1, 3]) samples.push(base('sack', count === 3 ? '旅の袋' : 'Named Sack', { nativeTypeId: 7008, contents: Array.from({ length: count }, () => base('key', 'Key')) }))
for (const nativeSubtype of [0, 2]) samples.push(base('skill-book', 'Skill Book', { nativeSubtype }))
for (let recipeIndex = 0; recipeIndex < economy.DOWSING_EQUIPMENT_RECIPES.length; recipeIndex += 1) {
  const recipe = economy.DOWSING_EQUIPMENT_RECIPES[recipeIndex]
  samples.push(base('equipment', recipe.name, { equipmentType: recipe.equipmentType, nativeTypeId: recipe.nativeTypeId, recipeIndex, rarity: recipe.rarity }))
}
samples.push(base('equipment', 'Generated Hat', { equipmentType: 'hat', nativeTypeId: 7005, generatedLevel: 22, nativeEffects: [{ kind: 14, magnitude: -0.5, operator: 0, target: 0 }] }))
samples.push(base('equipment', 'Sorceror\'s Amulet', { equipmentType: 'amulet', nativeTypeId: 7007, nativeSubtype: 0, nativeSelector: 0, nativeEffects: [] }))
samples.push(tutorial.nativeTutorialAmuletItem())
const icon = { imagePath: 'icon.png', frame: { x: 0, y: 0, width: 8, height: 8 } }
const content = { contentId: 'example.item', description: 'Actual authored item description.', icon, key: 'item', modId: 'example', stackMaximum: 10 }
samples.push(base('mod-item', 'Moondust', { modItemContent: content }))
samples.push(base('mod-potion', 'Tonic', { modContent: { ...content, durationMs: 2000 } }))
samples.push(base('equipment', 'Authored Hat', { equipmentType: 'hat', nativeTypeId: 7005, modItemContent: content, modAffixes: [{ contentId: 'example.affix', modId: 'example', name: 'First Affix', modifiers: [] }, { contentId: 'example.second', modId: 'example', name: 'Second Affix', modifiers: [] }], nativeEffects: [] }))
const items = []
const allRecipes = economy.DOWSING_EQUIPMENT_RECIPES.map((_, index) => index)
for (const item of samples) {
  for (const options of [{}, { price: 0, playerLevel: 1 }, { price: 999, playerLevel: 3, creativityRank: 1, ownedRecipeIndexes: allRecipes }, { playerLevel: 100, ownedRecipeIndexes: [0, 1] }]) {
    items.push({ item, options, info: text.hubInventoryItemInfoText(item), expected: text.hubItemTooltipLines(item, options) })
  }
}

// Execute the unchanged private renderer against its real font helpers. Minimal
// scene objects retain submitted frame geometry; no expected layout is reimplemented.
const ts = (await import(pathToFileURL(resolve(websiteDir, 'frontend/node_modules/typescript/lib/typescript.js')).href)).default
const file = resolve(gameDir, 'renderer/hub-inventory/items.ts')
const ast = ts.createSourceFile(file, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TS)
let hoverFunction
for (const statement of ast.statements) {
  if (ts.isFunctionDeclaration(statement) && statement.name?.text === 'addNativeContextualHoverBox') hoverFunction = statement.getText(ast).replace(/^export /, '')
}
if (!hoverFunction) throw new Error('Missing actual addNativeContextualHoverBox')
const javascript = ts.transpileModule(hoverFunction, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None } }).outputText
class Container {
  children = []
  x = 0
  y = 0
  position = { set: (x, y) => { this.x = x; this.y = y } }
  addChild(child) { this.children.push(child); return child }
}
class Graphics {
  rect(x, y, width, height) { Object.assign(this, { x, y, width, height }); return this }
  fill() { return this }
  stroke() { return this }
}
const sample = new Function('nativeUiFont', 'wrapNativeUiText', 'measureNativeUiText', 'HUB_HOVER_BOX', 'HUB_NATIVE_UI_SIZE', 'Container', 'Graphics', 'addBitmapText', `${javascript}\nreturn addNativeContextualHoverBox`)(
  ui.nativeUiFont, ui.wrapNativeUiText, ui.measureNativeUiText, contract.HUB_HOVER_BOX,
  contract.HUB_NATIVE_UI_SIZE, Container, Graphics, () => {},
)

const layouts = []
for (const row of [...items.filter((_, index) => index % 4 === 0), ...hagatha.filter((_, index) => index % 6 === 0)]) {
  if (row.expected.some(line => line.sackNameBrowserFont)) continue
  const rendered = row.expected.map(line => {
    const sources = ui.wrapNativeUiText(line.text, line.font, 300)
    const lineHeight = ui.nativeUiFont(line.font).metrics[0]
    return { ...line, sources, width: Math.max(0, ...sources.map(source => ui.measureNativeUiText(source, line.font))), height: sources.length * lineHeight }
  })
  for (const source of [{ x: 20, y: 20 }, { x: 800, y: 450 }, { x: 1580, y: 880 }]) {
    const gap = 35
    const actual = sample({}, new Container(), row.expected, source.x, source.y, gap)
    const { width, height } = actual.children[0]
    layouts.push({ lines: row.expected, source, gap, expected: { x: actual.x, y: actual.y, width, height, rendered } })
  }
}
writeFixture('item-text-goldens.json', 'item-text-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 renderer/hub-inventory-item-text.ts, native-ui/core.ts and unchanged contextual hover geometry from renderer/hub-inventory/items.ts',
  { tooltipTable: text.HAGATHA_NATIVE_TOOLTIP_LINES, effects, hagatha, items, layouts })
