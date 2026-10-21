import {loadKernel, writeFixture} from './oracle.mjs'

const mobile = await loadKernel('mobile-quickbar-layout.ts')
const cases = []
for (const width of [1200, 1280, 1500, 1600, 896 / .46, 2560]) {
  for (const scale of [.75, 1, 1.25, 1.5]) {
    const bank = mobile.mobileQuickbarBankLayout(width, scale)
    const slots = []
    for (let slot = 0; slot < 8; slot += 1) {
      slots.push(mobile.mobileQuickbarSlotPlacement(slot, bank))
    }
    cases.push({width, scale, bank, slots})
  }
}
writeFixture(
  'mobile-quickbar-goldens.json',
  'tools/website-oracle/mobile-quickbar-goldens.mjs',
  ['mobile-quickbar-layout.ts'],
  {cases},
)
