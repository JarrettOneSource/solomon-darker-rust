import { loadKernel, writeFixture } from './oracle.mjs'

const vfx = await loadKernel('element-vfx-native.ts')
const cases = []
for (const element of ['air', 'earth', 'ether', 'fire', 'water']) {
  for (const phase of [-2, 0, 0.25, 1, 6, 27, 35.75, 67, 145]) {
    for (const scale of [1, 2, 6]) {
      cases.push({ element, phase, scale, plan: vfx.nativeElementVfxPlanAtPhase(element, phase, scale) })
    }
  }
}
const selected = []
for (const skillId of [8, 16, 24, 32, 40, ...Array.from({ length: 15 }, (_, i) => 1000 + i), 999]) {
  for (const scale of [1, 2]) {
    selected.push({ skillId, tick: 67, scale, plan: vfx.nativeSelectedPrimaryElementVfxPlan(skillId, 67, scale) })
  }
}
writeFixture('element-vfx-goldens.json', 'tools/website-oracle/element-vfx-goldens.mjs',
  ['element-vfx-native.ts'], { records: vfx.NATIVE_ELEMENT_VFX_RECORDS, cases, selected })
