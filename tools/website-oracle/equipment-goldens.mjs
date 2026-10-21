import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { gameDir, loadKernel, writeFixture } from './oracle.mjs'
const equipment = await loadKernel('core-kernels/native-equipment-effects.ts')
const profile = await loadKernel('core-kernels/player-progression.ts')
const catalog = JSON.parse(readFileSync(resolve(gameDir, 'core-kernels/native-equipment-effects-catalog.json')))
const fresh = profile.createPlayerSkillBook({ element: 'ether', discipline: 'arcane', displayName: 'oracle' }).permanentRanks
const all = profile.playerStatBook().entries.map((entry, index) => index < 8 ? 1 : Math.min(2, entry.maximumLevel))
const cases = []
function record(name, ranks, sources, owned = []) {
  cases.push({ name, ranks, sources, owned, result: equipment.resolveNativeEquipmentEffects(ranks, sources, owned) })
}
record('empty', fresh, [])
catalog.items.forEach((item, recipeIndex) => record(`recipe-${recipeIndex}`, fresh,
  [{ effects: item.effects, recipeIndex }], recipeIndex % 2 ? [6] : []))
catalog.sets.forEach((set, index) => record(`set-${index}`, all,
  set.memberRecipeIndices.map(recipeIndex => ({ effects: catalog.items[recipeIndex].effects, recipeIndex }))))
for (let kind = 1; kind <= 39; kind += 1) {
  for (let operator = 0; operator <= 2; operator += 1) {
    const target = [2, 6, 11, 13, 22].includes(kind) ? 2 : [4, 5, 7, 25].includes(kind) ? 8 : 0
    record(`effect-${kind}-${operator}`, all, [{ effects: [{ kind, operator, target, magnitude: operator === 1 ? 1.5 : 2 }], recipeIndex: null }], [6])
  }
}
record('deferred-grant-after-additive-sources', fresh, [
  { effects: [{ kind: 4, magnitude: 2, operator: 0, target: 16 }], recipeIndex: null },
  { effects: [{ kind: 5, magnitude: 2, operator: 0, target: 16 }], recipeIndex: null },
])
record('transforms-keep-native-store-order', all, [{ recipeIndex: null, effects: [
  { kind: 23, magnitude: 7, operator: 0, target: 0 },
  { kind: 23, magnitude: 17, operator: 2, target: 0 },
  { kind: 23, magnitude: 3, operator: 0, target: 0 },
  { kind: 25, magnitude: 7, operator: 0, target: 8 },
  { kind: 25, magnitude: 17, operator: 2, target: 8 },
] }])
record('temporary-acquisition-follows-skill-pass-order', fresh, [
  { effects: [{ kind: 4, magnitude: 2, operator: 0, target: 24 }], recipeIndex: null },
  { effects: [
    { kind: 7, magnitude: 1, operator: 0, target: 32 },
    { kind: 7, magnitude: 1, operator: 0, target: 16 },
  ], recipeIndex: null },
])
writeFixture('equipment-goldens.json', 'tools/website-oracle/equipment-goldens.mjs',
  'website-rust-current/frontend/src/game/core-kernels/native-equipment-effects.ts and native-equipment-effects-catalog.json',
  { sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83', catalog, cases })
