import { loadKernel, writeFixture } from './oracle.mjs'

const source = await loadKernel('hub-sack-rename.ts')
const layoutSource = await loadKernel('hub-sack-rename-dialog-layout.ts')
const core = await loadKernel('native-ui/core.ts')
const potion = { id: 1, name: 'Potion', kind: 'health-potion', nativeTypeId: 7001 }
const inner = { id: 2, name: 'Inner', kind: 'sack', nativeTypeId: 7008, contents: [potion] }
const outer = { ...inner, id: 3, name: 'Outer', contents: [inner] }
const stored = { ...inner, id: 4, name: 'Stored', contents: [] }
const economy = { backpack: [outer], storage: [stored] }
const targets = []
for (const selection of [null, { id: 1, owner: 'backpack' }, { id: 2, owner: 'backpack' },
  { id: 3, owner: 'backpack' }, { id: 4, owner: 'storage' }, { id: 3, owner: 'equipment' },
  { id: 3, owner: null }, { id: 99, owner: 'storage' }]) {
  for (const path of [[], [3], [3, 2], [2], [3, 99, 2]]) {
    targets.push({ selection, path, expected: source.inventorySackRenameTarget(economy, selection, path)?.id ?? null })
  }
}
const layouts = []
for (const width of [320, 390, 568, 844, 1280, 1600]) {
  for (const height of [320, 390, 720, 844, 900]) {
    const size = { width, height }
    const { artScale, body, buttonScale, cancel, field, save, text } = layoutSource.inventorySackRenameDialogLayout(size)
    layouts.push({ size, expected: { artScale, body, buttonScale, cancel, field, save,
      titleBaseline: text.nodes[0].text.y, limitBaseline: text.nodes[1].text.y } })
  }
}
const names = ["Wizard's Supplies", 'Sack', '', '火の袋', 'Potions火', 'Cafe\u0301', 'Family 👨‍👩‍👧'].map(name => ({
  name, expected: source.inventorySackNameNeedsBrowserFont(name),
}))
writeFixture('sack-rename-goldens.json', 'sack-rename-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 hub-sack-rename.ts and hub-sack-rename-dialog-layout.ts public functions',
  { headerWidth: core.nativeUiRecord('UI', 18).logicalSize[0], arrowHeight: core.nativeUiRecord('UI', 8).logicalSize[1],
    targets, layouts, names })
