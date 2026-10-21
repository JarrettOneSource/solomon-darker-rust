import { loadKernel, writeFixture } from './oracle.mjs'
const appearance = await loadKernel('core-kernels/player-equipment-appearance.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')
const row = p => [p.hat ? [p.hat.primaryTint, p.hat.secondaryTint, p.hat.selector] : null,
  p.robe ? [p.robe.primaryTint, p.robe.secondaryTint, p.robe.selector] : null,
  p.weapon ? [p.weapon.kind, p.weapon.selector] : null]
const cases = []
function record(input, equipment) {
  cases.push({ ...input, living: row(appearance.playerLivingNativeEquipmentAppearance(input.element, equipment)),
    death: row(appearance.playerDeathEquipmentAppearance(input.element, equipment)) })
}
for (const element of ['air', 'earth', 'ether', 'fire', 'water']) {
  record({ mode: 'fresh', element }, economy.createHubEconomy(7, { starterElement: element }).equipment)
  record({ mode: 'naked', element }, { hat: null, robe: null, weapon: null })
}
for (const recipe of economy.DOWSING_EQUIPMENT_RECIPES) {
  if (!['hat', 'robe', 'staff', 'wand'].includes(recipe.type)) continue
  const item = economy.createEquipmentInventoryItem(recipe, 100)
  const slot = ['staff', 'wand'].includes(recipe.type) ? 'weapon' : recipe.type
  const equipment = { hat: null, robe: null, weapon: null, [slot]: item }
  record({ mode: 'recipe', element: 'ether', recipe: recipe.sourceIndex }, equipment)
  record({ mode: 'dyed', element: 'ether', recipe: recipe.sourceIndex }, { ...equipment,
    [slot]: { ...item, iconTints: [0x123456, 0xabcdef] } })
}
for (const [type, count] of [['hat', 4], ['robe', 3], ['staff', 6], ['wand', 6]]) {
  for (let selector = 0; selector < count; selector += 1) {
    const item = { equipmentType: type, recipeIndex: null, nativeSelector: selector, iconTints: [0x123456, 0xabcdef] }
    const slot = ['staff', 'wand'].includes(type) ? 'weapon' : type
    record({ mode: 'generated', element: 'ether', type, selector }, { hat: null, robe: null, weapon: null, [slot]: item })
  }
}
const modCases = []
const renderableGarment = value => value === null ? null : 'content' in value
  ? ['mod', value.content.contentId, value.primaryTint, value.secondaryTint]
  : ['native', value.primaryTint, value.secondaryTint, value.selector]
const renderable = value => [renderableGarment(value.hat), renderableGarment(value.robe),
  value.weapon === null ? null : 'content' in value.weapon
    ? ['mod', value.weapon.kind, value.weapon.content.contentId]
    : ['native', value.weapon.kind, value.weapon.selector]]
for (const input of [
  { slot: 'hat', wearableSlot: 'hat', equipmentType: 'hat', deathShape: 3, tints: [0x123456, 0xabcdef] },
  { slot: 'robe', wearableSlot: 'robe', equipmentType: 'robe', deathShape: 2, tints: [0xffffff, 0] },
  { slot: 'weapon', wearableSlot: 'staff', equipmentType: 'staff', deathShape: 5, tints: null },
  { slot: 'hat', wearableSlot: 'robe', equipmentType: 'hat', deathShape: 0, tints: [1, 2] },
  { slot: 'robe', wearableSlot: 'robe', equipmentType: 'robe', deathShape: 0, tints: [null, 2] },
  { slot: 'weapon', wearableSlot: 'staff', equipmentType: 'wand', deathShape: 0, tints: null },
]) {
  const content = {
    contentId: `fixture:${input.wearableSlot}`, modId: 'fixture', key: input.wearableSlot,
    description: '', stackMaximum: 1,
    icon: { atlasId: 'fixture:icon', frameIndex: 0, imagePath: 'art/icon.png', frame: {
      x: 0, y: 0, width: 16, height: 16, logicalWidth: 16, logicalHeight: 16,
      centerOffsetX: 0, centerOffsetY: 0, contentWidth: 16, contentHeight: 16,
    } },
    wearable: { slot: input.wearableSlot, deathShape: input.deathShape,
      dyeable: false, wornImagePath: 'art/worn.png' },
  }
  const item = { equipmentType: input.equipmentType, recipeIndex: null,
    modItemContent: content, ...(input.tints === null ? {} : { iconTints: input.tints }) }
  const equipment = { hat: null, robe: null, weapon: null, [input.slot]: item }
  const outcome = action => {
    try { return { value: action() } } catch { return { error: true } }
  }
  modCases.push({ ...input, content,
    living: outcome(() => renderable(appearance.playerLivingEquipmentAppearance('ether', equipment))),
    native: outcome(() => row(appearance.playerLivingNativeEquipmentAppearance('ether', equipment))),
    death: outcome(() => row(appearance.playerDeathEquipmentAppearance('ether', equipment))),
  })
}
writeFixture('equipment-appearance-goldens.json', 'tools/website-oracle/equipment-appearance-goldens.mjs',
  'Website/frontend/src/game/core-kernels/player-equipment-appearance.ts', { cases, modCases })
